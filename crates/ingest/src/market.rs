//! Stores what a market adapter returned.
//!
//! For every listing the adapter saw:
//! - `market_listings` gets an upsert, so editors can see and map everything;
//! - when the listing is mapped to a product, the offering of that product at
//!   the adapter's organization gets a new row if the price changed, or its
//!   `confirmed_at` moved forward if it did not. Price history stays append-only.
//!
//! Listings the adapter no longer returns are marked delisted, and their
//! offerings ended, so comparisons stop showing a price nobody sells at.
//! A run that returns far fewer listings than the last one is refused: that is
//! far more often a broken API or page than a provider dropping most of its catalog.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::adapters::{AdapterError, GpuListing, MarketAdapter, ModelListing};

/// A run must return at least this share of the listings the last good run saw.
const MIN_SHARE_OF_PREVIOUS: f64 = 0.5;

/// Plausible ranges. Generous on purpose: they catch unit mistakes (cents
/// read as dollars, per-token read as per-million), not unusual prices.
const GPU_HOUR_RANGE: (f64, f64) = (0.01, 100.0);
const MAX_INPUT_PER_MTOK: f64 = 500.0;
const MAX_OUTPUT_PER_MTOK: f64 = 2_000.0;
/// A price more than this factor away from the stored one is held.
const MAX_JUMP: f64 = 5.0;

/// Why a price is held, or `None` when it may be stored.
fn check_price(
    row: &Row,
    previous: Option<&PreviousPrice>,
    volatile: bool,
) -> Option<(&'static str, String)> {
    if let (Some(per_hour), Some(count)) = (row.per_hour, row.gpu_count) {
        let per_gpu = per_hour / f64::from(count.max(1));
        if !(GPU_HOUR_RANGE.0..=GPU_HOUR_RANGE.1).contains(&per_gpu) {
            return Some((
                "out_of_band",
                format!(
                    "${per_gpu:.4} per GPU-hour is outside ${}–${}",
                    GPU_HOUR_RANGE.0, GPU_HOUR_RANGE.1
                ),
            ));
        }
    }
    if let (Some(price), Some(max)) = (row.per_unit, row.max_per_unit)
        && (!price.is_finite() || price < 0.0 || price > max)
    {
        return Some((
            "out_of_band",
            format!(
                "${price} per {} is outside $0–${max}",
                row.unit.unwrap_or("unit")
            ),
        ));
    }
    if row.input.is_some_and(|p| p > MAX_INPUT_PER_MTOK)
        || row.output.is_some_and(|p| p > MAX_OUTPUT_PER_MTOK)
    {
        return Some((
            "out_of_band",
            format!(
                "${} in / ${} out per million tokens is above ${MAX_INPUT_PER_MTOK} / ${MAX_OUTPUT_PER_MTOK}",
                row.input.unwrap_or_default(),
                row.output.unwrap_or_default()
            ),
        ));
    }
    if volatile {
        return None;
    }
    let previous = previous?;
    let jumped = |new: Option<f64>, old: Option<f64>| match (new, old) {
        // Zero is a free tier: going to or from free is a real change, not a parse error.
        (Some(n), Some(o)) if n > 0.0 && o > 0.0 => n / o > MAX_JUMP || o / n > MAX_JUMP,
        _ => false,
    };
    for (label, new, old) in [
        ("per unit", row.per_unit, previous.per_unit),
        ("per hour", row.per_hour, previous.per_hour),
        ("input", row.input, previous.input),
        ("output", row.output, previous.output),
    ] {
        if jumped(new, old) {
            return Some((
                "jump",
                format!(
                    "{label} moved from {} to {}, more than {MAX_JUMP}×",
                    old.unwrap_or_default(),
                    new.unwrap_or_default()
                ),
            ));
        }
    }
    None
}

#[derive(sqlx::FromRow)]
struct PreviousPrice {
    listing_key: String,
    input: Option<f64>,
    output: Option<f64>,
    per_hour: Option<f64>,
    per_unit: Option<f64>,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("organization `{0}` of the adapter is not in the directory")]
    UnknownOrganization(&'static str),
    #[error(
        "refusing a run that saw {seen} listings after {previous} last time; \
         the provider's API or page probably changed"
    )]
    Shrunk { seen: usize, previous: usize },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    pub listings: usize,
    /// Listings mapped to a product.
    pub mapped: usize,
    /// New offering rows: new products at this provider, or price changes.
    pub offerings_added: usize,
    /// Mapped offerings whose price was the same as last time.
    pub offerings_confirmed: usize,
    /// Listings gone since the last run.
    pub delisted: usize,
    /// Listings whose new price failed a sanity check and waits for review.
    pub held: usize,
}

#[derive(sqlx::FromRow)]
struct GoneListing {
    external_id: String,
    gpu_type: Option<String>,
    gpu_count: Option<i16>,
    region: Option<String>,
}

#[derive(sqlx::FromRow)]
struct LatestOffering {
    id: Uuid,
    input: Option<f64>,
    output: Option<f64>,
    per_hour: Option<f64>,
    ended: bool,
}

/// One listing in the shape the store needs, whatever its kind.
struct Row {
    listing_key: String,
    external_id: String,
    kind: &'static str,
    name: String,
    maker: Option<String>,
    context_window: Option<i32>,
    input: Option<f64>,
    output: Option<f64>,
    gpu_type: Option<String>,
    gpu_count: Option<i16>,
    per_hour: Option<f64>,
    /// Non-chat models: the unit and its price.
    unit: Option<&'static str>,
    per_unit: Option<f64>,
    /// Upper bound of `per_unit` for the plausibility check.
    max_per_unit: Option<f64>,
    region: Option<String>,
    source_url: Option<String>,
    extra: serde_json::Value,
    capabilities: Vec<String>,
}

impl From<ModelListing> for Row {
    fn from(l: ModelListing) -> Self {
        Row {
            listing_key: match &l.variant {
                Some(v) => format!("{}|{v}", l.external_id),
                None => l.external_id.clone(),
            },
            external_id: l.external_id,
            kind: l.media.map_or("model", |m| m.kind.as_str()),
            name: l.name,
            maker: l.maker,
            context_window: l.context_window,
            input: l.media.is_none().then_some(l.input_per_mtok),
            output: l.media.is_none().then_some(l.output_per_mtok),
            gpu_type: None,
            gpu_count: None,
            per_hour: None,
            capabilities: l.capabilities,
            unit: l.media.map(|m| m.kind.unit()),
            per_unit: l.media.map(|m| m.price),
            max_per_unit: l.media.map(|m| m.kind.max_price()),
            region: l.variant,
            source_url: l.source_url,
            extra: l.extra,
        }
    }
}

impl From<GpuListing> for Row {
    fn from(l: GpuListing) -> Self {
        Row {
            listing_key: format!(
                "{}|{}|{}",
                l.external_id,
                l.gpu_count,
                l.region.as_deref().unwrap_or("")
            ),
            external_id: l.external_id,
            kind: "gpu",
            name: l.name,
            maker: None,
            context_window: None,
            input: None,
            output: None,
            gpu_type: Some(l.gpu_type),
            gpu_count: Some(l.gpu_count),
            per_hour: Some(l.price_per_hour),
            unit: None,
            per_unit: None,
            max_per_unit: None,
            capabilities: Vec::new(),
            region: l.region,
            source_url: l.source_url,
            extra: l.extra,
        }
    }
}

/// Records which organization each adapter reports for, for market views.
/// Cheap (one statement), so the scheduler runs it on every pass and picks up
/// organizations seeded after the worker started.
pub async fn register_adapters(pool: &PgPool, adapters: &[MarketAdapter]) -> sqlx::Result<()> {
    let ids: Vec<&str> = adapters.iter().map(|a| a.info().id()).collect();
    let orgs: Vec<&str> = adapters
        .iter()
        .map(|a| a.info().organization_slug())
        .collect();
    sqlx::query(
        "INSERT INTO adapter_state (adapter, organization_id)
         SELECT a.adapter, o.id
         FROM unnest($1::text[], $2::text[]) AS a (adapter, slug)
         JOIN organizations o ON o.slug = a.slug
         ON CONFLICT (adapter) DO UPDATE SET organization_id = EXCLUDED.organization_id
         WHERE adapter_state.organization_id IS DISTINCT FROM EXCLUDED.organization_id",
    )
    .bind(&ids)
    .bind(&orgs)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetches from one adapter and stores the result. Records success or failure
/// in `adapter_state` either way.
pub async fn run_adapter(
    pool: &PgPool,
    client: &reqwest::Client,
    adapter: &MarketAdapter,
) -> Result<RunReport, MarketError> {
    let id = adapter.info().id();
    let result = fetch_and_store(pool, client, adapter).await;

    // Prices announced for a later date are not a failure: note it, keep the schedule, store nothing.
    if let Err(MarketError::Adapter(note @ AdapterError::NotYet(..))) = &result {
        sqlx::query(
            "INSERT INTO adapter_state (adapter, last_run_at, last_error, consecutive_failures)
             VALUES ($1, now(), left($2, 2000), 0)
             ON CONFLICT (adapter) DO UPDATE SET
                 last_run_at = now(), last_error = EXCLUDED.last_error, consecutive_failures = 0",
        )
        .bind(id)
        .bind(note.to_string())
        .execute(pool)
        .await?;
        return Ok(RunReport::default());
    }

    match &result {
        Ok(report) => {
            sqlx::query(
                "INSERT INTO adapter_state (adapter, last_run_at, last_success_at, last_error,
                     consecutive_failures, listings_seen, offerings_added)
                 VALUES ($1, now(), now(), NULL, 0, $2, $3)
                 ON CONFLICT (adapter) DO UPDATE SET
                     last_run_at = now(), last_success_at = now(), last_error = NULL,
                     consecutive_failures = 0, listings_seen = EXCLUDED.listings_seen,
                     offerings_added = EXCLUDED.offerings_added",
            )
            .bind(id)
            .bind(report.listings as i32)
            .bind(report.offerings_added as i32)
            .execute(pool)
            .await?;
        }
        Err(err) => {
            sqlx::query(
                "INSERT INTO adapter_state (adapter, last_run_at, last_error, consecutive_failures)
                 VALUES ($1, now(), left($2, 2000), 1)
                 ON CONFLICT (adapter) DO UPDATE SET
                     last_run_at = now(), last_error = EXCLUDED.last_error,
                     consecutive_failures = adapter_state.consecutive_failures + 1",
            )
            .bind(id)
            .bind(err.to_string())
            .execute(pool)
            .await?;
        }
    }
    result
}

async fn fetch_and_store(
    pool: &PgPool,
    client: &reqwest::Client,
    adapter: &MarketAdapter,
) -> Result<RunReport, MarketError> {
    let info = adapter.info();
    let rows: Vec<Row> = match adapter {
        MarketAdapter::Gateway(a) => a
            .fetch_models(client)
            .await?
            .into_iter()
            .map(Row::from)
            .collect(),
        MarketAdapter::NeoCloud(a) => a
            .fetch_gpus(client)
            .await?
            .into_iter()
            .map(Row::from)
            .collect(),
    };
    store(
        pool,
        info.id(),
        info.organization_slug(),
        info.volatile(),
        info.inventory_varies(),
        rows,
        Utc::now(),
    )
    .await
}

async fn store(
    pool: &PgPool,
    adapter: &'static str,
    organization_slug: &'static str,
    volatile: bool,
    inventory_varies: bool,
    rows: Vec<Row>,
    now: DateTime<Utc>,
) -> Result<RunReport, MarketError> {
    let provider_id: Uuid = sqlx::query_scalar("SELECT id FROM organizations WHERE slug = $1")
        .bind(organization_slug)
        .fetch_optional(pool)
        .await?
        .ok_or(MarketError::UnknownOrganization(organization_slug))?;

    let previous: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM market_listings WHERE adapter = $1 AND delisted_at IS NULL",
    )
    .bind(adapter)
    .fetch_one(pool)
    .await?;
    let previous = previous as usize;
    // An empty run is always refused. A run under half of the last is too, except for sources
    // that list only what is in stock, whose lists legitimately swing.
    let too_small = rows.is_empty()
        || (!inventory_varies && (rows.len() as f64) < previous as f64 * MIN_SHARE_OF_PREVIOUS);
    if previous > 0 && too_small {
        return Err(MarketError::Shrunk {
            seen: rows.len(),
            previous,
        });
    }

    // The stored price of every live listing, and the editor's decisions on held ones.
    let previous: std::collections::HashMap<String, PreviousPrice> =
        sqlx::query_as::<_, PreviousPrice>(
            "SELECT listing_key, price_input_per_mtok::float8 AS input,
                price_output_per_mtok::float8 AS output, price_per_hour::float8 AS per_hour,
                price_per_unit::float8 AS per_unit
         FROM market_listings WHERE adapter = $1 AND delisted_at IS NULL",
        )
        .bind(adapter)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|p| (p.listing_key.clone(), p))
        .collect();
    let accepted: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT listing_key FROM price_holds WHERE adapter = $1 AND resolution = 'accepted'",
    )
    .bind(adapter)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let mut tx = pool.begin().await?;
    let mut report = RunReport {
        listings: rows.len(),
        ..RunReport::default()
    };
    let mut keys = Vec::with_capacity(rows.len());
    let mut stored = Vec::with_capacity(rows.len());

    for row in &rows {
        keys.push(row.listing_key.clone());
        let prev = previous.get(&row.listing_key);
        if !accepted.contains(&row.listing_key)
            && let Some((reason, detail)) = check_price(row, prev, volatile)
        {
            hold(&mut tx, adapter, row, prev, reason, &detail, now).await?;
            report.held += 1;
            continue;
        }
        upsert_listing(&mut tx, adapter, row, now).await?;
        stored.push(row.listing_key.clone());
        // Offerings hold token or hourly prices; media prices live in listings only.
        if row.unit.is_some() {
            continue;
        }

        let product: Option<Uuid> = sqlx::query_scalar(
            "SELECT product_id FROM product_listing_ids WHERE adapter = $1 AND external_id = $2",
        )
        .bind(adapter)
        .bind(&row.external_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(product) = product else { continue };
        report.mapped += 1;
        if record_offering(&mut tx, adapter, product, provider_id, row, now).await? {
            report.offerings_added += 1;
        } else {
            report.offerings_confirmed += 1;
        }
    }

    // Open or accepted holds whose listing was stored this run are settled: the
    // price came back within bounds, or the editor accepted it.
    sqlx::query(
        "DELETE FROM price_holds
         WHERE adapter = $1 AND listing_key = ANY ($2)
           AND (resolution IS NULL OR resolution = 'accepted')",
    )
    .bind(adapter)
    .bind(&stored)
    .execute(&mut *tx)
    .await?;

    // Everything active that this run did not see is gone.
    let gone: Vec<GoneListing> = sqlx::query_as(
        "UPDATE market_listings SET delisted_at = $3
         WHERE adapter = $1 AND delisted_at IS NULL AND listing_key <> ALL ($2)
         RETURNING external_id, gpu_type, gpu_count, region",
    )
    .bind(adapter)
    .bind(&keys)
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    report.delisted = gone.len();
    for g in gone {
        sqlx::query(
            "UPDATE offerings f SET ended_at = $7
             FROM product_listing_ids pl
             WHERE pl.adapter = $1 AND pl.external_id = $2 AND f.product_id = pl.product_id
               AND f.provider_id = $3 AND f.source = $1 AND f.ended_at IS NULL
               AND f.gpu_type IS NOT DISTINCT FROM $4 AND f.gpu_count IS NOT DISTINCT FROM $5
               AND f.region IS NOT DISTINCT FROM $6",
        )
        .bind(adapter)
        .bind(&g.external_id)
        .bind(provider_id)
        .bind(g.gpu_type)
        .bind(g.gpu_count)
        .bind(g.region)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(report)
}

/// Records or refreshes a held price. A listing whose price the editor
/// rejected stays held without reopening the review.
async fn hold(
    tx: &mut Transaction<'_, Postgres>,
    adapter: &str,
    row: &Row,
    previous: Option<&PreviousPrice>,
    reason: &str,
    detail: &str,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    tracing::warn!(adapter, listing = %row.listing_key, reason, detail, "holding a price for review");
    sqlx::query(
        "INSERT INTO price_holds
             (adapter, listing_key, external_id, kind, name, reason,
              price_input_per_mtok, price_output_per_mtok, price_per_hour,
              previous_input_per_mtok, previous_output_per_mtok, previous_per_hour,
              gpu_count, detail, held_at, last_seen_at, unit, price_per_unit, previous_per_unit)
         VALUES ($1, $2, $3, $4, $5, $6, $7::float8::numeric, $8::float8::numeric,
                 $9::float8::numeric, $10::float8::numeric, $11::float8::numeric,
                 $12::float8::numeric, $13, $14, $15, $15, $16, $17::float8::numeric,
                 $18::float8::numeric)
         ON CONFLICT (adapter, listing_key) DO UPDATE SET
             name = EXCLUDED.name, reason = EXCLUDED.reason,
             price_input_per_mtok = EXCLUDED.price_input_per_mtok,
             price_output_per_mtok = EXCLUDED.price_output_per_mtok,
             price_per_hour = EXCLUDED.price_per_hour, gpu_count = EXCLUDED.gpu_count,
             unit = EXCLUDED.unit, price_per_unit = EXCLUDED.price_per_unit,
             detail = EXCLUDED.detail, last_seen_at = EXCLUDED.last_seen_at,
             -- A different price than the one the editor rejected reopens the review.
             resolution = CASE
                 WHEN price_holds.resolution = 'rejected'
                  AND price_holds.price_input_per_mtok IS NOT DISTINCT FROM EXCLUDED.price_input_per_mtok
                  AND price_holds.price_output_per_mtok IS NOT DISTINCT FROM EXCLUDED.price_output_per_mtok
                  AND price_holds.price_per_hour IS NOT DISTINCT FROM EXCLUDED.price_per_hour
                  AND price_holds.price_per_unit IS NOT DISTINCT FROM EXCLUDED.price_per_unit
                 THEN 'rejected' END,
             resolved_at = CASE WHEN price_holds.resolution = 'rejected'
                  AND price_holds.price_input_per_mtok IS NOT DISTINCT FROM EXCLUDED.price_input_per_mtok
                  AND price_holds.price_output_per_mtok IS NOT DISTINCT FROM EXCLUDED.price_output_per_mtok
                  AND price_holds.price_per_hour IS NOT DISTINCT FROM EXCLUDED.price_per_hour
                  AND price_holds.price_per_unit IS NOT DISTINCT FROM EXCLUDED.price_per_unit
                 THEN price_holds.resolved_at END",
    )
    .bind(adapter)
    .bind(&row.listing_key)
    .bind(&row.external_id)
    .bind(row.kind)
    .bind(&row.name)
    .bind(reason)
    .bind(row.input)
    .bind(row.output)
    .bind(row.per_hour)
    .bind(previous.and_then(|p| p.input))
    .bind(previous.and_then(|p| p.output))
    .bind(previous.and_then(|p| p.per_hour))
    .bind(row.gpu_count)
    .bind(detail)
    .bind(now)
    .bind(row.unit)
    .bind(row.per_unit)
    .bind(previous.and_then(|p| p.per_unit))
    .execute(&mut **tx)
    .await?;
    // Keep the listing live at its old price, so it is not delisted or stale.
    sqlx::query(
        "UPDATE market_listings SET last_seen_at = $3
         WHERE adapter = $1 AND listing_key = $2",
    )
    .bind(adapter)
    .bind(&row.listing_key)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_listing(
    tx: &mut Transaction<'_, Postgres>,
    adapter: &str,
    row: &Row,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO market_listings
             (adapter, listing_key, external_id, kind, name, maker, context_window,
              price_input_per_mtok, price_output_per_mtok, gpu_type, gpu_count, price_per_hour,
              region, source_url, extra, first_seen_at, last_seen_at, delisted_at,
              unit, price_per_unit, capabilities)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $16, NULL,
                 $17, $18::float8::numeric, $19)
         ON CONFLICT (adapter, listing_key) DO UPDATE SET
             capabilities = EXCLUDED.capabilities,
             kind = EXCLUDED.kind, unit = EXCLUDED.unit, price_per_unit = EXCLUDED.price_per_unit,
             external_id = EXCLUDED.external_id, name = EXCLUDED.name, maker = EXCLUDED.maker,
             context_window = EXCLUDED.context_window,
             price_input_per_mtok = EXCLUDED.price_input_per_mtok,
             price_output_per_mtok = EXCLUDED.price_output_per_mtok,
             gpu_type = EXCLUDED.gpu_type, gpu_count = EXCLUDED.gpu_count,
             price_per_hour = EXCLUDED.price_per_hour, region = EXCLUDED.region,
             source_url = EXCLUDED.source_url, extra = EXCLUDED.extra,
             last_seen_at = EXCLUDED.last_seen_at, delisted_at = NULL",
    )
    .bind(adapter)
    .bind(&row.listing_key)
    .bind(&row.external_id)
    .bind(row.kind)
    .bind(&row.name)
    .bind(&row.maker)
    .bind(row.context_window)
    .bind(row.input)
    .bind(row.output)
    .bind(&row.gpu_type)
    .bind(row.gpu_count)
    .bind(row.per_hour)
    .bind(&row.region)
    .bind(&row.source_url)
    .bind(&row.extra)
    .bind(now)
    .bind(row.unit)
    .bind(row.per_unit)
    .bind(&row.capabilities)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Adds an offering row when the price is new; otherwise confirms the latest
/// one. Returns whether a row was added.
async fn record_offering(
    tx: &mut Transaction<'_, Postgres>,
    adapter: &str,
    product: Uuid,
    provider: Uuid,
    row: &Row,
    now: DateTime<Utc>,
) -> sqlx::Result<bool> {
    // The latest observation of this configuration from this adapter.
    let latest: Option<LatestOffering> = sqlx::query_as(
        "SELECT id, price_input_per_mtok::float8 AS input, price_output_per_mtok::float8 AS output,
                price_per_hour::float8 AS per_hour, ended_at IS NOT NULL AS ended
         FROM offerings
         WHERE product_id = $1 AND provider_id = $2 AND source = $3
           AND gpu_type IS NOT DISTINCT FROM $4 AND gpu_count IS NOT DISTINCT FROM $5
           AND region IS NOT DISTINCT FROM $6
         ORDER BY observed_at DESC, id DESC LIMIT 1",
    )
    .bind(product)
    .bind(provider)
    .bind(adapter)
    .bind(&row.gpu_type)
    .bind(row.gpu_count)
    .bind(&row.region)
    .fetch_optional(&mut **tx)
    .await?;

    let same = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-6,
        (None, None) => true,
        _ => false,
    };
    if let Some(latest) = latest
        && !latest.ended
        && same(latest.input, row.input)
        && same(latest.output, row.output)
        && same(latest.per_hour, row.per_hour)
    {
        sqlx::query("UPDATE offerings SET confirmed_at = $2, source_url = $3 WHERE id = $1")
            .bind(latest.id)
            .bind(now)
            .bind(&row.source_url)
            .execute(&mut **tx)
            .await?;
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO offerings
             (product_id, provider_id, price_input_per_mtok, price_output_per_mtok,
              gpu_type, gpu_count, price_per_hour, region, currency, source_url,
              observed_at, confirmed_at, source)
         VALUES ($1, $2, $3::float8::numeric, $4::float8::numeric, $5, $6,
                 $7::float8::numeric, $8, 'USD', $9, $10, $10, $11)",
    )
    .bind(product)
    .bind(provider)
    .bind(row.input)
    .bind(row.output)
    .bind(&row.gpu_type)
    .bind(row.gpu_count)
    .bind(row.per_hour)
    .bind(&row.region)
    .bind(&row.source_url)
    .bind(now)
    .bind(adapter)
    .execute(&mut **tx)
    .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu_row(per_hour: f64, count: i16) -> Row {
        Row::from(GpuListing {
            external_id: "H100".into(),
            name: "H100".into(),
            gpu_type: "H100 SXM".into(),
            gpu_count: count,
            price_per_hour: per_hour,
            region: None,
            source_url: None,
            extra: serde_json::Value::Null,
        })
    }

    fn model_row(input: f64, output: f64) -> Row {
        Row::from(ModelListing {
            external_id: "m".into(),
            name: "m".into(),
            maker: None,
            context_window: None,
            input_per_mtok: input,
            output_per_mtok: output,
            variant: None,
            media: None,
            capabilities: Vec::new(),
            source_url: None,
            extra: serde_json::Value::Null,
        })
    }

    fn prev(input: Option<f64>, output: Option<f64>, per_hour: Option<f64>) -> PreviousPrice {
        PreviousPrice {
            listing_key: "k".into(),
            input,
            output,
            per_hour,
            per_unit: None,
        }
    }

    #[test]
    fn plausible_range_is_per_gpu() {
        assert!(
            check_price(&gpu_row(24.0, 8), None, false).is_none(),
            "$3/GPU-hour"
        );
        assert!(
            check_price(&gpu_row(400.0, 8), None, false).is_none(),
            "$50/GPU-hour is high but plausible"
        );
        let (reason, _) = check_price(&gpu_row(2990.0, 1), None, false).unwrap();
        assert_eq!(reason, "out_of_band", "cents read as dollars");
        assert!(check_price(&gpu_row(0.001, 1), None, false).is_some());
        assert!(
            check_price(&model_row(2_500_000.0, 1.0), None, false).is_some(),
            "per token × 1e12"
        );
        assert!(
            check_price(&gpu_row(2990.0, 1), None, true).is_some(),
            "volatile sources keep the range check"
        );
    }

    #[test]
    fn large_jumps_are_held_except_to_or_from_free_and_on_volatile_sources() {
        let p = prev(Some(1.0), Some(2.0), None);
        assert!(check_price(&model_row(1.2, 2.4), Some(&p), false).is_none());
        assert_eq!(
            check_price(&model_row(10.0, 2.0), Some(&p), false)
                .unwrap()
                .0,
            "jump"
        );
        assert_eq!(
            check_price(&model_row(1.0, 0.3), Some(&p), false)
                .unwrap()
                .0,
            "jump",
            "down too"
        );
        assert!(
            check_price(&model_row(0.0, 0.0), Some(&p), false).is_none(),
            "now free"
        );
        let free = prev(Some(0.0), Some(0.0), None);
        assert!(
            check_price(&model_row(5.0, 10.0), Some(&free), false).is_none(),
            "no longer free"
        );
        assert!(
            check_price(&model_row(10.0, 2.0), Some(&p), true).is_none(),
            "order books move"
        );
        let gpu = prev(None, None, Some(3.99));
        assert_eq!(
            check_price(&gpu_row(39.9, 1), Some(&gpu), false).unwrap().0,
            "jump"
        );
    }
}
