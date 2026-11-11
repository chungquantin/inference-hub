//! Market-wide queries over `market_listings`: every live price any adapter
//! saw, mapped to a product or not.
//!
//! Prices are compared per unit: GPUs per GPU-hour, models on the 3:1 blend of
//! input and output used by the model comparison. Free listings (price 0) are
//! promotions, not market prices, and are left out of minimums and medians.

use ih_core::{
    GpuIndexRow, MarketDetail, MarketIndexQuery, MarketListing, MarketOverview, MarketProvider,
    MarketSource, ModelIndexRow,
};
use sqlx::PgPool;

use crate::compare::BLEND_INPUT_WEIGHT;

fn limit(q: &MarketIndexQuery) -> i64 {
    q.limit.unwrap_or(100).clamp(1, 500)
}

/// Escapes `%`, `_` and `\` so user text matches literally in ILIKE.
fn like(q: &Option<String>) -> Option<String> {
    q.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            format!(
                "%{}%",
                s.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        })
}

/// GPU types across clouds, the most widely sold first.
pub async fn gpu_index(pool: &PgPool, q: &MarketIndexQuery) -> sqlx::Result<Vec<GpuIndexRow>> {
    sqlx::query_as(
        "WITH live AS (
             SELECT l.gpu_type, o.slug, o.name, o.logo_url,
                    (l.price_per_hour / l.gpu_count)::float8 AS per_gpu
             FROM market_listings l
             JOIN adapter_state a ON a.adapter = l.adapter
             JOIN organizations o ON o.id = a.organization_id
             WHERE l.kind = 'gpu' AND l.delisted_at IS NULL AND l.gpu_count > 0
               AND l.price_per_hour > 0
               AND ($1::text IS NULL OR l.gpu_type ILIKE $1)
         ),
         stats AS (
             SELECT gpu_type, count(DISTINCT slug) AS providers, count(*) AS listings,
                    min(per_gpu) AS min_per_gpu_hour,
                    percentile_cont(0.5) WITHIN GROUP (ORDER BY per_gpu) AS median_per_gpu_hour,
                    max(per_gpu) AS max_per_gpu_hour
             FROM live GROUP BY gpu_type
         ),
         cheapest AS (
             SELECT DISTINCT ON (gpu_type) gpu_type, slug, name, logo_url
             FROM live ORDER BY gpu_type, per_gpu, name
         )
         SELECT s.gpu_type, s.providers, s.listings,
                round(s.min_per_gpu_hour::numeric, 4)::float8 AS min_per_gpu_hour,
                round(s.median_per_gpu_hour::numeric, 4)::float8 AS median_per_gpu_hour,
                round(s.max_per_gpu_hour::numeric, 4)::float8 AS max_per_gpu_hour,
                c.slug AS organization_slug, c.name AS organization_name,
                c.logo_url AS organization_logo
         FROM stats s JOIN cheapest c USING (gpu_type)
         WHERE s.providers >= $2
         ORDER BY s.providers DESC, s.listings DESC, s.gpu_type
         LIMIT $3",
    )
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit(q))
    .fetch_all(pool)
    .await
}

/// Models across providers, the most widely served first.
pub async fn model_index(
    pool: &PgPool,
    q: &MarketIndexQuery,
) -> sqlx::Result<ih_core::Page<ModelIndexRow>> {
    #[derive(sqlx::FromRow)]
    struct Counted {
        #[sqlx(flatten)]
        row: ModelIndexRow,
        total: i64,
    }
    let rows: Vec<Counted> = sqlx::query_as(
        "WITH live AS (
             SELECT market_model_key(l.external_id) AS model_key, l.name, l.maker,
                    l.context_window,
                    l.price_input_per_mtok::float8 AS input,
                    l.price_output_per_mtok::float8 AS output,
                    ((l.price_input_per_mtok * $4::numeric + l.price_output_per_mtok)
                        / ($4::numeric + 1))::float8 AS blended,
                    -- A router's listing names the provider behind it.
                    coalesce(l.region, o.slug) AS seller,
                    o.slug, o.name AS org_name, o.logo_url
             FROM market_listings l
             JOIN adapter_state a ON a.adapter = l.adapter
             JOIN organizations o ON o.id = a.organization_id
             WHERE l.kind = 'model' AND l.delisted_at IS NULL
               AND l.price_input_per_mtok + l.price_output_per_mtok > 0
               AND ($1::text IS NULL OR l.external_id ILIKE $1 OR l.name ILIKE $1)
         ),
         stats AS (
             SELECT model_key,
                    mode() WITHIN GROUP (ORDER BY name) AS name,
                    mode() WITHIN GROUP (ORDER BY maker) AS maker,
                    count(DISTINCT seller) AS providers,
                    min(input) AS min_input, min(output) AS min_output,
                    min(blended) AS min_blended,
                    percentile_cont(0.5) WITHIN GROUP (ORDER BY blended) AS median_blended,
                    max(context_window) AS max_context_window
             FROM live GROUP BY model_key
         ),
         cheapest AS (
             SELECT DISTINCT ON (model_key) model_key, slug, org_name, logo_url
             FROM live ORDER BY model_key, blended, org_name
         )
         SELECT count(*) OVER () AS total, s.model_key, s.name, s.maker, s.providers,
                round(s.min_input::numeric, 6)::float8 AS min_input_per_mtok,
                round(s.min_output::numeric, 6)::float8 AS min_output_per_mtok,
                round(s.min_blended::numeric, 6)::float8 AS min_blended_per_mtok,
                round(s.median_blended::numeric, 6)::float8 AS median_blended_per_mtok,
                s.max_context_window,
                c.slug AS organization_slug, c.org_name AS organization_name,
                c.logo_url AS organization_logo
         FROM stats s JOIN cheapest c USING (model_key)
         WHERE s.providers >= $2
           AND ($6::text IS NULL OR EXISTS (
                 SELECT 1 FROM live p WHERE p.model_key = s.model_key AND p.slug = $6))
         ORDER BY s.providers DESC, s.min_blended, s.model_key
         LIMIT $3 OFFSET $5",
    )
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit(q))
    .bind(BLEND_INPUT_WEIGHT)
    .bind(q.offset.unwrap_or(0).max(0))
    .bind(q.provider.as_deref().filter(|p| !p.is_empty()))
    .fetch_all(pool)
    .await?;
    // The window count; a page past the end has no rows to carry it.
    let total = rows.first().map_or(0, |r| r.total);
    Ok(ih_core::Page {
        items: rows.into_iter().map(|r| r.row).collect(),
        total,
        limit: limit(q),
        offset: q.offset.unwrap_or(0).max(0),
    })
}

const LISTING_COLUMNS: &str = "l.adapter, o.slug AS organization_slug, o.name AS organization_name,
    o.logo_url AS organization_logo, l.external_id, l.name, l.region AS variant, l.context_window,
    l.price_input_per_mtok::float8 AS price_input_per_mtok,
    l.price_output_per_mtok::float8 AS price_output_per_mtok,
    round((l.price_input_per_mtok * $2::numeric + l.price_output_per_mtok) / ($2::numeric + 1), 6)::float8
        AS blended_per_mtok,
    l.gpu_count, l.price_per_hour::float8 AS price_per_hour,
    round(l.price_per_hour / nullif(l.gpu_count, 0), 4)::float8 AS price_per_gpu_hour,
    l.unit, l.price_per_unit::float8 AS price_per_unit,
    l.source_url, l.last_seen_at";

/// Every live price of one model, cheapest first. `None` when no provider lists it.
pub async fn model_detail(pool: &PgPool, key: &str) -> sqlx::Result<Option<MarketDetail>> {
    let listings: Vec<MarketListing> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LISTING_COLUMNS}
         FROM market_listings l
         JOIN adapter_state a ON a.adapter = l.adapter
         JOIN organizations o ON o.id = a.organization_id
         WHERE l.kind NOT IN ('gpu') AND l.delisted_at IS NULL
           AND market_model_key(l.external_id) = $1
         ORDER BY l.kind, blended_per_mtok, price_per_unit, o.name, l.region"
    )))
    .bind(key)
    .bind(BLEND_INPUT_WEIGHT)
    .fetch_all(pool)
    .await?;
    Ok(detail(key, listings))
}

/// Every live price of one GPU type, cheapest per GPU-hour first.
pub async fn gpu_detail(pool: &PgPool, gpu_type: &str) -> sqlx::Result<Option<MarketDetail>> {
    let listings: Vec<MarketListing> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LISTING_COLUMNS}
         FROM market_listings l
         JOIN adapter_state a ON a.adapter = l.adapter
         JOIN organizations o ON o.id = a.organization_id
         WHERE l.kind = 'gpu' AND l.delisted_at IS NULL AND l.gpu_count > 0
           AND lower(l.gpu_type) = lower($1)
         ORDER BY price_per_gpu_hour, o.name, l.gpu_count"
    )))
    .bind(gpu_type)
    .bind(BLEND_INPUT_WEIGHT)
    .fetch_all(pool)
    .await?;
    // Named by the GPU type, in the stored spelling.
    let name: Option<String> = sqlx::query_scalar(
        "SELECT gpu_type FROM market_listings
         WHERE kind = 'gpu' AND delisted_at IS NULL AND lower(gpu_type) = lower($1) LIMIT 1",
    )
    .bind(gpu_type)
    .fetch_optional(pool)
    .await?;
    Ok(detail(gpu_type, listings).map(|mut d| {
        if let Some(name) = name {
            d.key = name.clone();
            d.name = name;
        }
        d
    }))
}

fn detail(key: &str, listings: Vec<MarketListing>) -> Option<MarketDetail> {
    // The name most providers use, ties to the shortest.
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for l in &listings {
        *counts.entry(l.name.as_str()).or_default() += 1;
    }
    let name = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.len().cmp(&a.0.len())))?
        .0
        .to_owned();
    Some(MarketDetail {
        key: key.to_owned(),
        name,
        listings,
    })
}

/// Providers by how much of the market they cover.
pub async fn providers(pool: &PgPool) -> sqlx::Result<Vec<MarketProvider>> {
    sqlx::query_as(
        "WITH live AS (
             SELECT o.id, o.slug, o.name, o.logo_url, l.kind, l.external_id, l.gpu_type,
                    l.last_seen_at,
                    CASE WHEN l.kind = 'gpu' THEN l.price_per_hour / nullif(l.gpu_count, 0)
                         WHEN l.kind = 'model'
                         THEN (l.price_input_per_mtok * $1::numeric + l.price_output_per_mtok)
                              / ($1::numeric + 1)
                         ELSE l.price_per_unit END AS unit_price,
                    CASE WHEN l.kind = 'gpu' THEN l.gpu_type
                         ELSE market_model_key(l.external_id) END AS item
             FROM market_listings l
             JOIN adapter_state a ON a.adapter = l.adapter
             JOIN organizations o ON o.id = a.organization_id
             WHERE l.delisted_at IS NULL
         ),
         -- Only items at least two organizations sell: being the sole seller is not winning.
         contested AS (
             SELECT kind, item FROM live WHERE unit_price > 0
             GROUP BY kind, item HAVING count(DISTINCT id) >= 2
         ),
         winners AS (
             SELECT DISTINCT ON (kind, item) kind, item, id
             FROM live JOIN contested USING (kind, item)
             WHERE unit_price > 0
             ORDER BY kind, item, unit_price, name
         )
         SELECT * FROM (
             SELECT l.slug AS organization_slug, l.name AS organization_name,
                    l.logo_url AS organization_logo,
                    count(DISTINCT l.item) FILTER (WHERE l.kind = 'model') AS models,
                    count(*) FILTER (WHERE l.kind = 'model') AS model_listings,
                    count(DISTINCT l.item) FILTER (WHERE l.kind NOT IN ('model', 'gpu')) AS media_models,
                    count(*) FILTER (WHERE l.kind = 'gpu') AS gpu_configurations,
                    (SELECT count(*) FROM winners w WHERE w.id = l.id) AS cheapest_count,
                    max(l.last_seen_at) AS last_seen_at
             FROM live l
             GROUP BY l.id, l.slug, l.name, l.logo_url
         ) p
         ORDER BY models + media_models + gpu_configurations DESC, organization_name",
    )
    .bind(BLEND_INPUT_WEIGHT)
    .fetch_all(pool)
    .await
}

#[derive(sqlx::FromRow)]
struct Totals {
    model_providers: i64,
    gpu_providers: i64,
    providers: i64,
    model_listings: i64,
    distinct_models: i64,
    multi_provider_models: i64,
    gpu_listings: i64,
    gpu_types: i64,
    media_listings: i64,
    media_models: i64,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Headline numbers, the top of both indexes, and the health of every source.
pub async fn overview(pool: &PgPool) -> sqlx::Result<MarketOverview> {
    let t: Totals = sqlx::query_as(
        "SELECT count(DISTINCT a.organization_id) FILTER (WHERE l.kind = 'model') AS model_providers,
                count(DISTINCT a.organization_id) FILTER (WHERE l.kind = 'gpu') AS gpu_providers,
                count(DISTINCT a.organization_id) AS providers,
                count(*) FILTER (WHERE l.kind = 'model') AS model_listings,
                count(DISTINCT market_model_key(l.external_id)) FILTER (WHERE l.kind = 'model')
                    AS distinct_models,
                count(*) FILTER (WHERE l.kind = 'gpu') AS gpu_listings,
                count(DISTINCT l.gpu_type) FILTER (WHERE l.kind = 'gpu') AS gpu_types,
                count(*) FILTER (WHERE l.kind NOT IN ('model', 'gpu')) AS media_listings,
                count(DISTINCT l.kind || ':' || market_model_key(l.external_id))
                    FILTER (WHERE l.kind NOT IN ('model', 'gpu')) AS media_models,
                max(l.last_seen_at) AS updated_at,
                (SELECT count(*) FROM (
                     SELECT 1 FROM market_listings m
                     JOIN adapter_state ma ON ma.adapter = m.adapter
                     WHERE m.kind = 'model' AND m.delisted_at IS NULL
                     GROUP BY market_model_key(m.external_id)
                     HAVING count(DISTINCT coalesce(m.region, ma.organization_id::text)) >= 2
                 ) multi) AS multi_provider_models
         FROM market_listings l
         JOIN adapter_state a ON a.adapter = l.adapter
         WHERE l.delisted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;
    let top = MarketIndexQuery {
        min_providers: Some(2),
        limit: Some(12),
        ..MarketIndexQuery::default()
    };
    Ok(MarketOverview {
        providers: t.providers,
        model_providers: t.model_providers,
        gpu_providers: t.gpu_providers,
        model_listings: t.model_listings,
        distinct_models: t.distinct_models,
        multi_provider_models: t.multi_provider_models,
        gpu_listings: t.gpu_listings,
        gpu_types: t.gpu_types,
        media_listings: t.media_listings,
        media_models: t.media_models,
        updated_at: t.updated_at,
        gpu_index: gpu_index(pool, &top).await?,
        model_index: model_index(pool, &top).await?.items,
        sources: sources(pool).await?,
    })
}

/// Every adapter with its organization and health.
pub async fn sources(pool: &PgPool) -> sqlx::Result<Vec<MarketSource>> {
    sqlx::query_as(
        "SELECT a.adapter, o.slug AS organization_slug, o.name AS organization_name,
                o.logo_url AS organization_logo,
                (SELECT min(kind) FROM market_listings l WHERE l.adapter = a.adapter) AS kind,
                (SELECT count(*) FROM market_listings l
                 WHERE l.adapter = a.adapter AND l.delisted_at IS NULL) AS listings,
                a.last_success_at,
                a.consecutive_failures = 0 AND a.last_success_at IS NOT NULL AS healthy
         FROM adapter_state a JOIN organizations o ON o.id = a.organization_id
         ORDER BY o.name",
    )
    .fetch_all(pool)
    .await
}

/// Held prices waiting for a decision, newest first.
pub async fn open_holds(pool: &PgPool) -> sqlx::Result<Vec<ih_core::PriceHold>> {
    sqlx::query_as(
        "SELECT id, adapter, external_id, kind, name, reason, detail,
                price_input_per_mtok::float8 AS price_input_per_mtok,
                price_output_per_mtok::float8 AS price_output_per_mtok,
                price_per_hour::float8 AS price_per_hour,
                previous_input_per_mtok::float8 AS previous_input_per_mtok,
                previous_output_per_mtok::float8 AS previous_output_per_mtok,
                previous_per_hour::float8 AS previous_per_hour,
                gpu_count, held_at, last_seen_at
         FROM price_holds WHERE resolution IS NULL
         ORDER BY held_at DESC, id
         LIMIT 500",
    )
    .fetch_all(pool)
    .await
}

/// Records an editor's decision. Returns whether an open hold was resolved.
pub async fn resolve_hold(
    pool: &PgPool,
    id: i64,
    resolution: ih_core::HoldResolution,
) -> sqlx::Result<bool> {
    let value = match resolution {
        ih_core::HoldResolution::Accepted => "accepted",
        ih_core::HoldResolution::Rejected => "rejected",
    };
    let result = sqlx::query(
        "UPDATE price_holds SET resolution = $2, resolved_at = now()
         WHERE id = $1 AND resolution IS NULL",
    )
    .bind(id)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Jobs that failed every attempt, newest first.
pub async fn failed_jobs(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<ih_core::FailedJob>> {
    sqlx::query_as(
        "SELECT id, job_id, kind, payload, attempts, last_error, failed_at
         FROM jobs_failed ORDER BY failed_at DESC, id DESC LIMIT $1",
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
}

/// Non-chat models across providers, by unit price, the most widely sold first.
pub async fn media_index(
    pool: &PgPool,
    q: &ih_core::MediaIndexQuery,
) -> sqlx::Result<ih_core::Page<ih_core::MediaIndexRow>> {
    #[derive(sqlx::FromRow)]
    struct Counted {
        #[sqlx(flatten)]
        row: ih_core::MediaIndexRow,
        total: i64,
    }
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    let rows: Vec<Counted> = sqlx::query_as(
        "WITH live AS (
             SELECT l.kind, l.unit, market_model_key(l.external_id) AS model_key, l.name, l.maker,
                    l.price_per_unit::float8 AS price,
                    coalesce(l.region, o.slug) AS seller,
                    o.slug, o.name AS org_name, o.logo_url
             FROM market_listings l
             JOIN adapter_state a ON a.adapter = l.adapter
             JOIN organizations o ON o.id = a.organization_id
             WHERE l.kind NOT IN ('model', 'gpu') AND l.delisted_at IS NULL
               AND l.price_per_unit > 0
               AND ($1::text IS NULL OR l.kind = $1)
               AND ($2::text IS NULL OR l.external_id ILIKE $2 OR l.name ILIKE $2)
         ),
         stats AS (
             SELECT kind, unit, model_key,
                    mode() WITHIN GROUP (ORDER BY name) AS name,
                    mode() WITHIN GROUP (ORDER BY maker) AS maker,
                    count(DISTINCT seller) AS providers,
                    min(price) AS min_price,
                    percentile_cont(0.5) WITHIN GROUP (ORDER BY price) AS median_price
             FROM live GROUP BY kind, unit, model_key
         ),
         cheapest AS (
             SELECT DISTINCT ON (kind, model_key) kind, model_key, slug, org_name, logo_url
             FROM live ORDER BY kind, model_key, price, org_name
         )
         SELECT count(*) OVER () AS total, s.model_key, s.name, s.maker, s.kind, s.unit,
                s.providers, s.min_price, s.median_price,
                c.slug AS organization_slug, c.org_name AS organization_name,
                c.logo_url AS organization_logo
         FROM stats s JOIN cheapest c USING (kind, model_key)
         WHERE s.providers >= $3
         ORDER BY s.providers DESC, s.kind, s.min_price, s.model_key
         LIMIT $4 OFFSET $5",
    )
    .bind(q.kind.as_deref().filter(|k| !k.is_empty()))
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(ih_core::Page {
        total: rows.first().map_or(0, |r| r.total),
        items: rows.into_iter().map(|r| r.row).collect(),
        limit,
        offset,
    })
}
