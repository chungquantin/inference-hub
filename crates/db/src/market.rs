//! Market-wide reads.
//!
//! Index, overview and provider queries read the summaries the worker
//! precomputes (see [`crate::market_refresh`]): they page along indexes and do
//! not aggregate at request time. Detail queries (every price of one model or
//! GPU) read the live listings directly, through indexes.
//!
//! Prices are compared per unit: GPUs per GPU-hour, chat models on the 3:1
//! blend of input and output used by the model comparison, other models in
//! their own unit. Free listings (price 0) are left out of minimums and medians.

use ih_core::{
    GpuIndexRow, MarketDetail, MarketIndexQuery, MarketListing, MarketOverview, MarketProvider,
    MarketSource, MediaIndexQuery, MediaIndexRow, ModelIndexRow, Page,
};
use sqlx::PgPool;

use crate::compare::BLEND_INPUT_WEIGHT;

fn limit(q: Option<i64>) -> i64 {
    q.unwrap_or(100).clamp(1, 500)
}

fn offset(q: Option<i64>) -> i64 {
    q.unwrap_or(0).max(0)
}

/// Escapes `%`, `_` and `\` so user text matches literally in ILIKE.
fn like(q: &Option<String>) -> Option<String> {
    crate::sql::words_like(q.as_deref()?)
}

/// Capability filter: known names only, so the array never carries user junk.
fn caps(s: &Option<String>) -> Option<Vec<String>> {
    const KNOWN: [&str; 15] = [
        "in:text",
        "in:image",
        "in:audio",
        "in:video",
        "in:file",
        "out:text",
        "out:image",
        "out:audio",
        "out:video",
        "out:file",
        "tools",
        "json",
        "reasoning",
        "web",
        "caching",
    ];
    let list: Vec<String> = s
        .as_deref()?
        .split(',')
        .map(str::trim)
        .filter(|c| KNOWN.contains(c))
        .map(str::to_owned)
        .collect();
    (!list.is_empty()).then_some(list)
}

fn blank(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// The organization columns every index row carries, for `cheapest`.
const CHEAPEST: &str = org_ref!("o");

/// GPU types across clouds, the most widely sold first.
pub async fn gpu_index(pool: &PgPool, q: &MarketIndexQuery) -> sqlx::Result<Vec<GpuIndexRow>> {
    let (column, natural_desc) = match q.sort.as_deref().unwrap_or("providers") {
        "price" => ("g.min_per_gpu_hour", false),
        "median" => ("g.median_per_gpu_hour", false),
        "max" => ("g.max_per_gpu_hour", false),
        "listings" => ("g.listings", true),
        "name" => ("lower(g.gpu_type)", false),
        _ => ("g.providers", true),
    };
    let desc = match q.dir.as_deref() {
        Some("asc") => false,
        Some("desc") => true,
        _ => natural_desc,
    };
    let order = format!(
        "{column} {}, g.providers DESC, g.listings DESC, g.gpu_type",
        if desc { "DESC" } else { "ASC" }
    );
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT g.gpu_type, g.providers::int8 AS providers, g.listings::int8 AS listings,
                g.min_per_gpu_hour, g.median_per_gpu_hour, g.max_per_gpu_hour, {CHEAPEST}
         FROM market_gpus g
         JOIN organizations o ON o.id = g.cheapest_organization_id
         WHERE g.providers >= $2
           AND ($1::text IS NULL OR g.gpu_type ILIKE $1)
           AND ($5::text IS NULL OR g.organization_ids @> ARRAY[(SELECT id FROM organizations WHERE slug = $5)])
           AND ($6::float8 IS NULL OR g.min_per_gpu_hour <= $6)
           AND ($7::float8 IS NULL OR g.min_per_gpu_hour >= $7)
         ORDER BY {order}
         LIMIT $3 OFFSET $4"
    )))
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit(q.limit))
    .bind(offset(q.offset))
    .bind(blank(&q.provider))
    .bind(q.max_price)
    .bind(q.min_price)
    .fetch_all(pool)
    .await
}

#[derive(sqlx::FromRow)]
struct Counted<T> {
    #[sqlx(flatten)]
    row: T,
    total: i64,
}

fn page<T>(rows: Vec<Counted<T>>, limit: i64, offset: i64) -> Page<T> {
    Page {
        total: rows.first().map_or(0, |r| r.total),
        items: rows.into_iter().map(|r| r.row).collect(),
        limit,
        offset,
    }
}

/// The ORDER BY of the model index for a `sort` and `dir`. Sort columns come
/// from this fixed list, never from user text.
fn model_order(sort: Option<&str>, dir: Option<&str>) -> String {
    let (column, natural_desc) = match sort.unwrap_or("providers") {
        "price" => ("m.min_blended_per_mtok", false),
        "median" => ("m.median_blended_per_mtok", false),
        "input" => ("m.min_input_per_mtok", false),
        "output" => ("m.min_output_per_mtok", false),
        "context" => ("coalesce(m.max_context_window, f.context_window)", true),
        "released" => ("f.released_on", true),
        "name" => ("lower(m.name)", false),
        _ => ("m.providers", true),
    };
    let desc = match dir {
        Some("asc") => false,
        Some("desc") => true,
        _ => natural_desc,
    };
    format!(
        "{column} {} NULLS LAST, m.providers DESC, m.min_blended_per_mtok, m.model_key",
        if desc { "DESC" } else { "ASC" }
    )
}

/// Chat models across providers, filtered and sorted; the most widely sold first by default.
pub async fn model_index(pool: &PgPool, q: &MarketIndexQuery) -> sqlx::Result<Page<ModelIndexRow>> {
    let (limit, offset) = (limit(q.limit), offset(q.offset));
    let order = model_order(q.sort.as_deref(), q.dir.as_deref());
    let rows: Vec<Counted<ModelIndexRow>> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) OVER () AS total, m.model_key, m.name, m.maker,
                m.providers::int8 AS providers,
                m.min_input_per_mtok, m.min_output_per_mtok, m.min_blended_per_mtok,
                m.median_blended_per_mtok,
                coalesce(m.max_context_window, f.context_window) AS max_context_window,
                f.released_on, f.open_weights, m.capabilities, {CHEAPEST}
         FROM market_models m
         JOIN organizations o ON o.id = m.cheapest_organization_id
         LEFT JOIN model_facts f ON f.model_key = m.model_key
         WHERE m.kind = 'model' AND m.providers >= $2
           AND ($1::text IS NULL OR m.model_key ILIKE $1 OR m.name ILIKE $1)
           AND ($5::text IS NULL OR m.organization_ids @> ARRAY[(SELECT id FROM organizations WHERE slug = $5)])
           AND ($6::text IS NULL OR lower(m.maker) = lower($6))
           AND ($7::float8 IS NULL OR m.min_blended_per_mtok <= $7)
           AND ($8::float8 IS NULL OR m.min_blended_per_mtok >= $8)
           AND ($9::int8 IS NULL OR coalesce(m.max_context_window, f.context_window) >= $9)
           AND ($10::text[] IS NULL OR m.capabilities @> $10)
         ORDER BY {order}
         LIMIT $3 OFFSET $4"
    )))
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit)
    .bind(offset)
    .bind(blank(&q.provider))
    .bind(blank(&q.maker))
    .bind(q.max_price)
    .bind(q.min_price)
    .bind(q.min_context)
    .bind(caps(&q.caps))
    .fetch_all(pool)
    .await?;
    Ok(page(rows, limit, offset))
}

/// Makers of models sold by at least `min_providers` providers, most models first.
pub async fn model_makers(
    pool: &PgPool,
    min_providers: i64,
) -> sqlx::Result<Vec<ih_core::MakerCount>> {
    sqlx::query_as(
        "SELECT maker, count(*) AS models FROM market_models
         WHERE kind = 'model' AND maker IS NOT NULL AND providers >= $1
         GROUP BY maker ORDER BY count(*) DESC, maker LIMIT 60",
    )
    .bind(min_providers)
    .fetch_all(pool)
    .await
}

/// Models priced per unit, the most widely sold first.
pub async fn media_index(pool: &PgPool, q: &MediaIndexQuery) -> sqlx::Result<Page<MediaIndexRow>> {
    let (limit, offset) = (limit(q.limit), offset(q.offset));
    let rows: Vec<Counted<MediaIndexRow>> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) OVER () AS total, m.model_key, m.name, m.maker, m.kind, m.unit,
                m.providers::int8 AS providers, m.min_price, m.median_price, m.capabilities, {CHEAPEST}
         FROM market_models m
         JOIN organizations o ON o.id = m.cheapest_organization_id
         WHERE m.kind <> 'model' AND m.providers >= $3
           AND ($1::text IS NULL OR m.kind = $1)
           AND ($2::text IS NULL OR m.model_key ILIKE $2 OR m.name ILIKE $2)
           AND ($6::text IS NULL OR m.organization_ids @> ARRAY[(SELECT id FROM organizations WHERE slug = $6)])
         ORDER BY m.providers DESC, m.kind, m.min_price, m.model_key
         LIMIT $4 OFFSET $5"
    )))
    .bind(blank(&q.kind))
    .bind(like(&q.q))
    .bind(q.min_providers.unwrap_or(1))
    .bind(limit)
    .bind(offset)
    .bind(blank(&q.provider))
    .fetch_all(pool)
    .await?;
    Ok(page(rows, limit, offset))
}

const LISTING_COLUMNS: &str = concat!(
    "l.adapter, ",
    org_ref!("o"),
    ", l.external_id, l.name, l.region AS variant, l.context_window,
    l.price_input_per_mtok::float8 AS price_input_per_mtok,
    l.price_output_per_mtok::float8 AS price_output_per_mtok,
    round(",
    blended!("l.price_input_per_mtok", "l.price_output_per_mtok", "$2"),
    ", 6)::float8
        AS blended_per_mtok,
    l.gpu_count, l.price_per_hour::float8 AS price_per_hour,
    round(",
    per_gpu_hour!("l.price_per_hour", "l.gpu_count"),
    ", 4)::float8 AS price_per_gpu_hour,
    l.unit, l.price_per_unit::float8 AS price_per_unit, l.capabilities,
    l.source_url, l.last_seen_at"
);

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
    // The maker the summaries settled on, routing prefixes already excluded.
    let maker: Option<String> = sqlx::query_scalar(
        "SELECT maker FROM market_models WHERE model_key = $1 AND maker IS NOT NULL LIMIT 1",
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(detail(key, listings).map(|d| MarketDetail { maker, ..d }))
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
        maker: None,
        listings,
    })
}

/// Providers by how much of the market they cover.
pub async fn providers(pool: &PgPool) -> sqlx::Result<Vec<MarketProvider>> {
    sqlx::query_as(concat!(
        "SELECT ",
        org_ref!("o"),
        ",
                p.models::int8 AS models, p.model_listings::int8 AS model_listings,
                p.media_models::int8 AS media_models,
                p.gpu_configurations::int8 AS gpu_configurations,
                p.cheapest_count::int8 AS cheapest_count, p.last_seen_at
         FROM market_providers p JOIN organizations o ON o.id = p.organization_id
         ORDER BY p.models + p.media_models + p.gpu_configurations DESC, o.name"
    ))
    .fetch_all(pool)
    .await
}

#[derive(sqlx::FromRow)]
struct Totals {
    providers: i32,
    model_providers: i32,
    gpu_providers: i32,
    model_listings: i32,
    distinct_models: i32,
    multi_provider_models: i32,
    gpu_listings: i32,
    gpu_types: i32,
    media_listings: i32,
    media_models: i32,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Headline numbers, the top of both indexes, and the health of every source.
pub async fn overview(pool: &PgPool) -> sqlx::Result<MarketOverview> {
    let t: Option<Totals> = sqlx::query_as(
        "SELECT providers, model_providers, gpu_providers, model_listings, distinct_models,
                multi_provider_models, gpu_listings, gpu_types, media_listings, media_models,
                updated_at
         FROM market_stats",
    )
    .fetch_optional(pool)
    .await?;
    // Before the first refresh everything is zero.
    let t = t.unwrap_or(Totals {
        providers: 0,
        model_providers: 0,
        gpu_providers: 0,
        model_listings: 0,
        distinct_models: 0,
        multi_provider_models: 0,
        gpu_listings: 0,
        gpu_types: 0,
        media_listings: 0,
        media_models: 0,
        updated_at: None,
    });
    let top = MarketIndexQuery {
        min_providers: Some(2),
        limit: Some(12),
        ..MarketIndexQuery::default()
    };
    Ok(MarketOverview {
        providers: t.providers.into(),
        model_providers: t.model_providers.into(),
        gpu_providers: t.gpu_providers.into(),
        model_listings: t.model_listings.into(),
        distinct_models: t.distinct_models.into(),
        multi_provider_models: t.multi_provider_models.into(),
        gpu_listings: t.gpu_listings.into(),
        gpu_types: t.gpu_types.into(),
        media_listings: t.media_listings.into(),
        media_models: t.media_models.into(),
        updated_at: t.updated_at,
        gpu_index: gpu_index(pool, &top).await?,
        model_index: model_index(pool, &top).await?.items,
        sources: sources(pool).await?,
    })
}

/// Every adapter with its organization and health.
pub async fn sources(pool: &PgPool) -> sqlx::Result<Vec<MarketSource>> {
    sqlx::query_as(concat!(
        "SELECT a.adapter, ",
        org_ref!("o"),
        ",
                (SELECT min(kind) FROM market_listings l WHERE l.adapter = a.adapter) AS kind,
                a.listings_seen::int8 AS listings,
                a.last_success_at,
                a.consecutive_failures = 0 AND a.last_success_at IS NOT NULL AS healthy
         FROM adapter_state a JOIN organizations o ON o.id = a.organization_id
         ORDER BY o.name"
    ))
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

/// One provider's catalog size, `None` when no adapter reports for it.
pub async fn provider(pool: &PgPool, slug: &str) -> sqlx::Result<Option<MarketProvider>> {
    sqlx::query_as(concat!(
        "SELECT ",
        org_ref!("o"),
        ",
                p.models::int8 AS models, p.model_listings::int8 AS model_listings,
                p.media_models::int8 AS media_models,
                p.gpu_configurations::int8 AS gpu_configurations,
                p.cheapest_count::int8 AS cheapest_count, p.last_seen_at
         FROM market_providers p JOIN organizations o ON o.id = p.organization_id
         WHERE o.slug = $1"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// A provider's offerings of one kind (or all), the most widely sold first,
/// with counts per kind. `None` when the provider sells nothing tracked.
pub async fn provider_offerings(
    pool: &PgPool,
    slug: &str,
    q: &ih_core::ProviderOfferingsQuery,
) -> sqlx::Result<Option<ih_core::ProviderOfferings>> {
    let org: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM organizations WHERE slug = $1")
            .bind(slug)
            .fetch_optional(pool)
            .await?;
    let Some(org) = org else { return Ok(None) };
    let kinds: Vec<ih_core::KindCount> = sqlx::query_as(
        "SELECT kind, count(*) AS count FROM market_provider_offerings
         WHERE organization_id = $1
         GROUP BY kind
         ORDER BY array_position(
             ARRAY['model', 'gpu', 'image', 'video', 'speech', 'transcription', 'embedding'], kind)",
    )
    .bind(org)
    .fetch_all(pool)
    .await?;
    if kinds.is_empty() {
        return Ok(None);
    }
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let offset = offset(q.offset);
    let (column, natural_desc) = match q.sort.as_deref().unwrap_or("providers") {
        "price" => ("f.price", false),
        "rank" => ("f.rank", false),
        "gap" => ("f.price / nullif(f.market_min, 0)", false),
        "name" => ("lower(f.name)", false),
        _ => ("f.market_providers", true),
    };
    let desc = match q.dir.as_deref() {
        Some("asc") => false,
        Some("desc") => true,
        _ => natural_desc,
    };
    let order = format!(
        "{column} {} NULLS LAST, f.market_providers DESC NULLS LAST, f.price NULLS LAST, f.item_key",
        if desc { "DESC" } else { "ASC" }
    );
    let rows: Vec<Counted<ih_core::ProviderOffering>> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) OVER () AS total, f.kind, f.item_key, f.name, f.maker,
                p.slug AS product_slug, f.sources, f.variants, f.price, f.input_per_mtok,
                f.output_per_mtok, f.unit, f.market_providers, f.market_min, f.market_median,
                f.rank, f.capabilities, f.context_window, f.source_url, f.last_seen_at
         FROM market_provider_offerings f
         LEFT JOIN products p ON p.id = f.product_id
         WHERE f.organization_id = $1
           AND ($2::text IS NULL OR f.kind = $2)
           AND ($3::text IS NULL OR f.name ILIKE $3 OR f.item_key ILIKE $3)
           AND (NOT $6 OR (f.rank = 1 AND f.market_providers >= 2))
         ORDER BY {order}
         LIMIT $4 OFFSET $5"
        )))
        .bind(org)
        .bind(blank(&q.kind))
        .bind(like(&q.q))
        .bind(limit)
        .bind(offset)
        .bind(q.cheapest.unwrap_or(false))
        .fetch_all(pool)
        .await?;
    let page = page(rows, limit, offset);
    Ok(Some(ih_core::ProviderOfferings {
        kinds,
        items: page.items,
        total: page.total,
        limit,
        offset,
    }))
}

/// What models.dev knows of a model, by its normalized key. `None` when it has no entry.
pub async fn model_facts(pool: &PgPool, key: &str) -> sqlx::Result<Option<ih_core::ModelFacts>> {
    sqlx::query_as("SELECT * FROM model_facts WHERE model_key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
}
