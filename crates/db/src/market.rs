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
pub async fn model_index(pool: &PgPool, q: &MarketIndexQuery) -> sqlx::Result<Vec<ModelIndexRow>> {
    sqlx::query_as(
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
         SELECT s.model_key, s.name, s.maker, s.providers,
                round(s.min_input::numeric, 6)::float8 AS min_input_per_mtok,
                round(s.min_output::numeric, 6)::float8 AS min_output_per_mtok,
                round(s.min_blended::numeric, 6)::float8 AS min_blended_per_mtok,
                round(s.median_blended::numeric, 6)::float8 AS median_blended_per_mtok,
                s.max_context_window,
                c.slug AS organization_slug, c.org_name AS organization_name,
                c.logo_url AS organization_logo
         FROM stats s JOIN cheapest c USING (model_key)
         WHERE s.providers >= $2
         ORDER BY s.providers DESC, s.min_blended, s.model_key
         LIMIT $3",
    )
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit(q))
    .bind(BLEND_INPUT_WEIGHT)
    .fetch_all(pool)
    .await
}

const LISTING_COLUMNS: &str = "l.adapter, o.slug AS organization_slug, o.name AS organization_name,
    o.logo_url AS organization_logo, l.external_id, l.name, l.region AS variant, l.context_window,
    l.price_input_per_mtok::float8 AS price_input_per_mtok,
    l.price_output_per_mtok::float8 AS price_output_per_mtok,
    round((l.price_input_per_mtok * $2::numeric + l.price_output_per_mtok) / ($2::numeric + 1), 6)::float8
        AS blended_per_mtok,
    l.gpu_count, l.price_per_hour::float8 AS price_per_hour,
    round(l.price_per_hour / nullif(l.gpu_count, 0), 4)::float8 AS price_per_gpu_hour,
    l.source_url, l.last_seen_at";

/// Every live price of one model, cheapest first. `None` when no provider lists it.
pub async fn model_detail(pool: &PgPool, key: &str) -> sqlx::Result<Option<MarketDetail>> {
    let listings: Vec<MarketListing> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LISTING_COLUMNS}
         FROM market_listings l
         JOIN adapter_state a ON a.adapter = l.adapter
         JOIN organizations o ON o.id = a.organization_id
         WHERE l.kind = 'model' AND l.delisted_at IS NULL
           AND market_model_key(l.external_id) = $1
         ORDER BY blended_per_mtok, o.name, l.region"
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
                         ELSE (l.price_input_per_mtok * $1::numeric + l.price_output_per_mtok)
                              / ($1::numeric + 1) END AS unit_price,
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
                    count(*) FILTER (WHERE l.kind = 'gpu') AS gpu_configurations,
                    (SELECT count(*) FROM winners w WHERE w.id = l.id) AS cheapest_count,
                    max(l.last_seen_at) AS last_seen_at
             FROM live l
             GROUP BY l.id, l.slug, l.name, l.logo_url
         ) p
         ORDER BY models + gpu_configurations DESC, organization_name",
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
    gpu_listings: i64,
    gpu_types: i64,
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
                max(l.last_seen_at) AS updated_at
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
        gpu_listings: t.gpu_listings,
        gpu_types: t.gpu_types,
        updated_at: t.updated_at,
        gpu_index: gpu_index(pool, &top).await?,
        model_index: model_index(pool, &top).await?,
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
