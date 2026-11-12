//! Rankings and distributions over the market summaries.
//!
//! Reads `market_models`, `market_gpus` and `market_provider_offerings`, which the
//! worker rebuilds after each adapter run, so these queries touch only summary
//! rows (hundreds, not the 26,000 raw listings).

use ih_core::{
    MarketAnalytics,
    analytics::{GpuCell, GpuRank, MakerRank, ModelRank, PriceBucket, ProviderRank},
};
use sqlx::PgPool;

const CHEAPEST: &str = "o.slug AS organization_slug, o.name AS organization_name,
    o.logo_url AS organization_logo";

pub async fn analytics(pool: &PgPool) -> sqlx::Result<MarketAnalytics> {
    let refreshed_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT refreshed_at FROM market_stats")
            .fetch_optional(pool)
            .await?;

    // Each provider's lowest price per model is a row of market_provider_offerings. The
    // median and the maximum are taken over those, one price per provider, so a provider
    // with many variants does not outweigh the rest; the priciest is what a buyer pays
    // by picking the wrong provider.
    let models: Vec<ModelRank> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT m.model_key, m.name, m.maker, m.providers::int8 AS providers,
                m.min_blended_per_mtok, x.median_price AS median_blended_per_mtok,
                x.max_price AS max_blended_per_mtok,
                m.min_input_per_mtok, m.min_output_per_mtok,
                x.max_price / nullif(m.min_blended_per_mtok, 0) AS spread,
                m.max_context_window, m.capabilities, {CHEAPEST}
         FROM market_models m
         JOIN organizations o ON o.id = m.cheapest_organization_id
         JOIN (SELECT item_key, max(price) AS max_price,
                      percentile_cont(0.5) WITHIN GROUP (ORDER BY price)::float8 AS median_price
               FROM market_provider_offerings
               WHERE kind = 'model' AND price > 0 AND 'adapter' = ANY(sources)
               GROUP BY item_key) x ON x.item_key = m.model_key
         WHERE m.kind = 'model' AND m.providers >= 2 AND m.min_blended_per_mtok > 0
         ORDER BY m.providers DESC, m.min_blended_per_mtok, m.model_key
         LIMIT 2000"
    )))
    .fetch_all(pool)
    .await?;

    // Half-decade bands from $0.001 to $1,000 per million tokens, edges included.
    let price_histogram: Vec<PriceBucket> = sqlx::query_as(
        "WITH bands AS (
             SELECT g AS i, (10 ^ (g / 2.0))::float8 AS lo, (10 ^ ((g + 1) / 2.0))::float8 AS hi
             FROM generate_series(-6, 5) g
         )
         SELECT b.lo AS \"from\", b.hi AS \"to\",
                count(m.model_key) AS models,
                count(m.model_key) FILTER (WHERE m.providers >= 2) AS contested
         FROM bands b
         LEFT JOIN market_models m
                ON m.kind = 'model' AND m.min_blended_per_mtok >= b.lo
               AND (m.min_blended_per_mtok < b.hi OR (b.i = 5 AND m.min_blended_per_mtok >= b.hi))
         GROUP BY b.i, b.lo, b.hi
         ORDER BY b.i",
    )
    .fetch_all(pool)
    .await?;

    let makers: Vec<MakerRank> = sqlx::query_as(
        "SELECT maker, count(*) AS models,
                count(*) FILTER (WHERE providers >= 2) AS contested_models,
                avg(providers)::float8 AS avg_providers,
                (percentile_cont(0.5) WITHIN GROUP (ORDER BY min_blended_per_mtok))::float8
                    AS median_min_price,
                (count(*) FILTER (WHERE providers >= 2))::float8 / count(*) AS contested_share
         FROM market_models
         WHERE kind = 'model' AND maker IS NOT NULL AND min_blended_per_mtok > 0
         GROUP BY maker
         HAVING count(*) FILTER (WHERE providers >= 2) >= 1
         ORDER BY count(*) FILTER (WHERE providers >= 2) DESC, count(*) DESC, maker
         LIMIT 40",
    )
    .fetch_all(pool)
    .await?;

    let gpus: Vec<GpuRank> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT g.gpu_type, g.providers::int8 AS providers, g.listings::int8 AS listings,
                g.min_per_gpu_hour, g.median_per_gpu_hour, g.max_per_gpu_hour,
                g.max_per_gpu_hour / nullif(g.min_per_gpu_hour, 0) AS spread, {CHEAPEST}
         FROM market_gpus g
         JOIN organizations o ON o.id = g.cheapest_organization_id
         ORDER BY g.providers DESC, g.listings DESC, g.gpu_type"
    )))
    .fetch_all(pool)
    .await?;

    let gpu_prices: Vec<GpuCell> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT p.item_key AS gpu_type, p.price AS per_gpu_hour,
                coalesce(p.rank, 0) AS rank, p.price / p.market_median AS vs_median, {CHEAPEST}
         FROM market_provider_offerings p
         JOIN organizations o ON o.id = p.organization_id
         WHERE p.kind = 'gpu' AND p.price > 0 AND p.market_median > 0
           AND 'adapter' = ANY(p.sources) AND p.market_providers >= 2
         ORDER BY p.item_key, p.price"
    )))
    .fetch_all(pool)
    .await?;

    let providers: Vec<ProviderRank> = sqlx::query_as(
        "WITH shared AS (
             SELECT organization_id, kind, price / market_median AS ratio, rank
             FROM market_provider_offerings
             WHERE kind IN ('model', 'gpu') AND price > 0 AND market_median > 0
               AND market_providers >= 2 AND 'adapter' = ANY(sources)
         ), scored AS (
             SELECT organization_id,
                    count(*) FILTER (WHERE kind = 'model') AS contested_models,
                    count(*) FILTER (WHERE kind = 'gpu') AS contested_gpus,
                    count(*) FILTER (WHERE kind = 'model' AND rank = 1) AS model_wins,
                    count(*) FILTER (WHERE kind = 'gpu' AND rank = 1) AS gpu_wins,
                    exp(avg(ln(ratio)) FILTER (WHERE kind = 'model')) AS model_price_index,
                    exp(avg(ln(ratio)) FILTER (WHERE kind = 'gpu')) AS gpu_price_index
             FROM shared GROUP BY organization_id
         )
         SELECT o.slug AS organization_slug, o.name AS organization_name,
                o.logo_url AS organization_logo,
                mp.models::int8 AS models, mp.media_models::int8 AS media_models,
                mp.gpu_configurations::int8 AS gpu_configurations,
                coalesce(s.contested_models, 0) AS contested_models,
                coalesce(s.contested_gpus, 0) AS contested_gpus,
                coalesce(s.model_wins, 0) AS model_wins, coalesce(s.gpu_wins, 0) AS gpu_wins,
                s.model_price_index, s.gpu_price_index, mp.last_seen_at
         FROM market_providers mp
         JOIN organizations o ON o.id = mp.organization_id
         LEFT JOIN scored s ON s.organization_id = mp.organization_id
         ORDER BY coalesce(s.model_wins, 0) + coalesce(s.gpu_wins, 0) DESC, o.name",
    )
    .fetch_all(pool)
    .await?;

    Ok(MarketAnalytics {
        refreshed_at,
        models,
        price_histogram,
        makers,
        gpus,
        gpu_prices,
        providers,
    })
}
