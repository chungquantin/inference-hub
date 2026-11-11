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
    region: Option<String>,
    source_url: Option<String>,
    extra: serde_json::Value,
}

impl From<ModelListing> for Row {
    fn from(l: ModelListing) -> Self {
        Row {
            listing_key: match &l.variant {
                Some(v) => format!("{}|{v}", l.external_id),
                None => l.external_id.clone(),
            },
            external_id: l.external_id,
            kind: "model",
            name: l.name,
            maker: l.maker,
            context_window: l.context_window,
            input: Some(l.input_per_mtok),
            output: Some(l.output_per_mtok),
            gpu_type: None,
            gpu_count: None,
            per_hour: None,
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
    store(pool, info.id(), info.organization_slug(), rows, Utc::now()).await
}

async fn store(
    pool: &PgPool,
    adapter: &'static str,
    organization_slug: &'static str,
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
    if previous > 0 && (rows.len() as f64) < previous as f64 * MIN_SHARE_OF_PREVIOUS {
        return Err(MarketError::Shrunk {
            seen: rows.len(),
            previous,
        });
    }

    let mut tx = pool.begin().await?;
    let mut report = RunReport {
        listings: rows.len(),
        ..RunReport::default()
    };
    let mut keys = Vec::with_capacity(rows.len());

    for row in &rows {
        keys.push(row.listing_key.clone());
        upsert_listing(&mut tx, adapter, row, now).await?;

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
              region, source_url, extra, first_seen_at, last_seen_at, delisted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $16, NULL)
         ON CONFLICT (adapter, listing_key) DO UPDATE SET
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
