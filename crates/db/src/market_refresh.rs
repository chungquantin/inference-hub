//! Precomputing the market summaries from the raw listings.
//!
//! The worker runs [`refresh`] after adapters store new listings. It rebuilds
//! `market_models`, `market_gpus`, `market_providers` and `market_stats` in
//! one transaction, so readers see either the old summaries or the new ones,
//! never a half-built table.

use std::time::Instant;

use sqlx::PgPool;

use crate::compare::BLEND_INPUT_WEIGHT;

/// What a refresh produced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefreshReport {
    pub models: u64,
    pub gpus: u64,
    pub providers: u64,
    pub millis: u64,
}

/// Rebuilds every market summary from the live listings.
pub async fn refresh(pool: &PgPool) -> sqlx::Result<RefreshReport> {
    let started = Instant::now();
    let mut tx = pool.begin().await?;
    // One refresh at a time; a second waits instead of interleaving deletes.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('market_refresh'))")
        .execute(&mut *tx)
        .await?;

    // Live listings with their organization, computed once for this refresh.
    sqlx::query(
        "CREATE TEMP TABLE live ON COMMIT DROP AS
         SELECT l.kind, l.external_id, l.name, l.maker, l.context_window, l.gpu_type,
                l.gpu_count, l.unit, l.last_seen_at, l.source_url,
                a.organization_id AS org,
                o.name AS org_name,
                CASE WHEN l.kind = 'gpu' THEN l.gpu_type
                     ELSE market_model_key(l.external_id) END AS item,
                -- A router's listing names the provider behind it.
                coalesce(l.region, a.organization_id::text) AS seller,
                l.price_input_per_mtok::float8 AS input,
                l.price_output_per_mtok::float8 AS output,
                ((l.price_input_per_mtok * $1::numeric + l.price_output_per_mtok)
                    / ($1::numeric + 1))::float8 AS blended,
                (l.price_per_hour / nullif(l.gpu_count, 0))::float8 AS per_gpu,
                l.price_per_unit::float8 AS per_unit
         FROM market_listings l
         JOIN adapter_state a ON a.adapter = l.adapter
         JOIN organizations o ON o.id = a.organization_id
         WHERE l.delisted_at IS NULL",
    )
    .bind(BLEND_INPUT_WEIGHT)
    .execute(&mut *tx)
    .await?;

    for table in [
        "market_models",
        "market_gpus",
        "market_providers",
        "market_stats",
        "market_provider_offerings",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table}")))
            .execute(&mut *tx)
            .await?;
    }

    // Chat models. Free listings (price 0) are promotions; they count as
    // listings and sellers but do not set prices.
    let models = sqlx::query(
        "INSERT INTO market_models
             (kind, model_key, name, maker, providers, listings, organization_ids,
              cheapest_organization_id, min_input_per_mtok, min_output_per_mtok,
              min_blended_per_mtok, median_blended_per_mtok, max_context_window)
         SELECT 'model', item,
                mode() WITHIN GROUP (ORDER BY name),
                -- The maker as most providers name it; routing prefixes
                -- (bedrock, vertex, azure) are not makers.
                mode() WITHIN GROUP (ORDER BY maker) FILTER (WHERE maker NOT IN
                    ('bedrock', 'vertex', 'azure', 'aws', 'gcp', 'google-vertex', 'coding')),
                count(DISTINCT seller) FILTER (WHERE blended > 0),
                count(*),
                -- Organizations selling it at a price (a free tier alone is not selling).
                array_agg(DISTINCT org) FILTER (WHERE blended > 0),
                (array_agg(org ORDER BY blended, org_name) FILTER (WHERE blended > 0))[1],
                round(min(input) FILTER (WHERE blended > 0)::numeric, 6)::float8,
                round(min(output) FILTER (WHERE blended > 0)::numeric, 6)::float8,
                round(min(blended) FILTER (WHERE blended > 0)::numeric, 6)::float8,
                round((percentile_cont(0.5) WITHIN GROUP (ORDER BY blended)
                       FILTER (WHERE blended > 0))::numeric, 6)::float8,
                max(context_window)
         FROM live WHERE kind = 'model'
         GROUP BY item
         HAVING count(*) FILTER (WHERE blended > 0) > 0",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // Models priced per unit.
    let media = sqlx::query(
        "INSERT INTO market_models
             (kind, model_key, name, maker, providers, listings, organization_ids,
              cheapest_organization_id, unit, min_price, median_price)
         SELECT kind, item,
                mode() WITHIN GROUP (ORDER BY name),
                mode() WITHIN GROUP (ORDER BY maker),
                count(DISTINCT seller), count(*),
                array_agg(DISTINCT org),
                (array_agg(org ORDER BY per_unit, org_name))[1],
                mode() WITHIN GROUP (ORDER BY unit),
                min(per_unit),
                percentile_cont(0.5) WITHIN GROUP (ORDER BY per_unit)
         FROM live WHERE kind NOT IN ('model', 'gpu') AND per_unit > 0
         GROUP BY kind, item",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let gpus = sqlx::query(
        "INSERT INTO market_gpus
             (gpu_type, providers, listings, organization_ids, cheapest_organization_id,
              min_per_gpu_hour, median_per_gpu_hour, max_per_gpu_hour)
         SELECT item, count(DISTINCT org), count(*), array_agg(DISTINCT org),
                (array_agg(org ORDER BY per_gpu, org_name))[1],
                round(min(per_gpu)::numeric, 4)::float8,
                round((percentile_cont(0.5) WITHIN GROUP (ORDER BY per_gpu))::numeric, 4)::float8,
                round(max(per_gpu)::numeric, 4)::float8
         FROM live WHERE kind = 'gpu' AND per_gpu > 0
         GROUP BY item",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // Everything each provider sells, from its adapter and from the curated
    // data, one row per item. Adapter items first, compared with the market.
    sqlx::query(
        "INSERT INTO market_provider_offerings
             (organization_id, kind, item_key, name, maker, sources, variants, price,
              input_per_mtok, output_per_mtok, unit, context_window, source_url, last_seen_at)
         SELECT org, kind, item,
                mode() WITHIN GROUP (ORDER BY name),
                mode() WITHIN GROUP (ORDER BY maker) FILTER (WHERE maker NOT IN
                    ('bedrock', 'vertex', 'azure', 'aws', 'gcp', 'google-vertex', 'coding')),
                ARRAY['adapter'], count(*),
                min(CASE WHEN kind = 'gpu' THEN per_gpu
                         WHEN kind = 'model' THEN blended
                         ELSE per_unit END)
                    FILTER (WHERE coalesce(CASE WHEN kind = 'gpu' THEN per_gpu
                                                WHEN kind = 'model' THEN blended
                                                ELSE per_unit END, 0) > 0),
                (array_agg(input ORDER BY blended) FILTER (WHERE blended > 0))[1],
                (array_agg(output ORDER BY blended) FILTER (WHERE blended > 0))[1],
                mode() WITHIN GROUP (ORDER BY unit),
                max(context_window),
                (array_agg(source_url ORDER BY coalesce(blended, per_gpu, per_unit)))[1],
                max(last_seen_at)
         FROM live
         GROUP BY org, kind, item",
    )
    .execute(&mut *tx)
    .await?;

    // Curated offerings. A product mapped to adapter listings joins the item
    // those listings form; an unmapped one gets its own row, keyed by the
    // product (models) or its GPU type.
    sqlx::query(
        "WITH curated AS (
             SELECT f.provider_id AS org, p.id AS product_id, p.name, mk.name AS maker,
                    CASE WHEN f.gpu_type IS NOT NULL THEN 'gpu' ELSE 'model' END AS kind,
                    coalesce(
                        CASE WHEN f.gpu_type IS NOT NULL THEN f.gpu_type END,
                        (SELECT market_model_key(pl.external_id) FROM product_listing_ids pl
                         JOIN market_listings ml ON ml.adapter = pl.adapter
                                                AND ml.external_id = pl.external_id
                         WHERE pl.product_id = p.id AND ml.kind = 'model'
                         ORDER BY pl.adapter LIMIT 1),
                        market_model_key(p.slug)) AS item,
                    f.price_input_per_mtok::float8 AS input,
                    f.price_output_per_mtok::float8 AS output,
                    ((f.price_input_per_mtok * $1::numeric + f.price_output_per_mtok)
                        / ($1::numeric + 1))::float8 AS blended,
                    (f.price_per_hour / nullif(f.gpu_count, 0))::float8 AS per_gpu,
                    f.source_url, coalesce(f.confirmed_at, f.observed_at) AS seen
             FROM latest_offerings f
             JOIN products p ON p.id = f.product_id
             JOIN organizations mk ON mk.id = p.organization_id
         ),
         grouped AS (
             SELECT org, kind, item,
                    (array_agg(product_id))[1] AS product_id,
                    (array_agg(name))[1] AS name, (array_agg(maker))[1] AS maker,
                    count(*) AS variants,
                    min(CASE WHEN kind = 'gpu' THEN per_gpu ELSE blended END) AS price,
                    (array_agg(input ORDER BY blended))[1] AS input,
                    (array_agg(output ORDER BY blended))[1] AS output,
                    (array_agg(source_url ORDER BY coalesce(blended, per_gpu)))[1] AS source_url,
                    max(seen) AS seen
             FROM curated GROUP BY org, kind, item
         )
         INSERT INTO market_provider_offerings AS o
             (organization_id, kind, item_key, name, maker, product_id, sources, variants,
              price, input_per_mtok, output_per_mtok, source_url, last_seen_at)
         SELECT org, kind, item, name, maker, product_id, ARRAY['curated'], variants,
                price, input, output, source_url, seen
         FROM grouped
         ON CONFLICT (organization_id, kind, item_key) DO UPDATE SET
             product_id = EXCLUDED.product_id,
             -- The curated product's name reads better than a raw model id.
             name = EXCLUDED.name,
             maker = coalesce(EXCLUDED.maker, o.maker),
             sources = o.sources || EXCLUDED.sources",
    )
    .bind(BLEND_INPUT_WEIGHT)
    .execute(&mut *tx)
    .await?;

    // Compare each row with every seller of the same item: the market low,
    // median and count come from these rows, so a rank of #n of m always holds.
    // (The market indexes count a router's hosting providers as sellers; here
    // each organization counts once.)
    sqlx::query(
        "WITH stats AS (
             SELECT kind, item_key, count(*)::int AS n, min(price) AS min_price,
                    percentile_cont(0.5) WITHIN GROUP (ORDER BY price) AS median
             FROM market_provider_offerings WHERE price > 0
             GROUP BY kind, item_key
         ),
         ranks AS (
             SELECT organization_id, kind, item_key,
                    rank() OVER (PARTITION BY kind, item_key ORDER BY price)::int AS rank
             FROM market_provider_offerings WHERE price > 0
         )
         UPDATE market_provider_offerings o SET
             market_providers = s.n, market_min = s.min_price, market_median = s.median,
             rank = r.rank
         FROM stats s, ranks r
         WHERE s.kind = o.kind AND s.item_key = o.item_key
           AND r.organization_id = o.organization_id AND r.kind = o.kind
           AND r.item_key = o.item_key",
    )
    .execute(&mut *tx)
    .await?;

    // Providers, from the same rows their pages list: counts and lists agree.
    // Contested wins: items two or more providers sell where this one ranks first.
    let providers = sqlx::query(
        "INSERT INTO market_providers
             (organization_id, models, model_listings, media_models, gpu_configurations,
              cheapest_count, last_seen_at)
         SELECT organization_id,
                count(*) FILTER (WHERE kind = 'model' AND price > 0),
                coalesce(sum(variants) FILTER (WHERE kind = 'model'), 0),
                count(*) FILTER (WHERE kind NOT IN ('model', 'gpu') AND price > 0),
                coalesce(sum(variants) FILTER (WHERE kind = 'gpu'), 0),
                count(*) FILTER (WHERE rank = 1 AND market_providers >= 2),
                max(last_seen_at)
         FROM market_provider_offerings GROUP BY organization_id",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();

    sqlx::query(
        "INSERT INTO market_stats
             (providers, model_providers, gpu_providers, model_listings, distinct_models,
              multi_provider_models, gpu_listings, gpu_types, media_listings, media_models,
              updated_at, refreshed_at, refresh_ms)
         SELECT count(DISTINCT org),
                count(DISTINCT org) FILTER (WHERE kind = 'model'),
                count(DISTINCT org) FILTER (WHERE kind = 'gpu'),
                count(*) FILTER (WHERE kind = 'model'),
                (SELECT count(*) FROM market_models WHERE kind = 'model'),
                (SELECT count(*) FROM market_models WHERE kind = 'model' AND providers >= 2),
                count(*) FILTER (WHERE kind = 'gpu'),
                (SELECT count(*) FROM market_gpus),
                count(*) FILTER (WHERE kind NOT IN ('model', 'gpu')),
                (SELECT count(*) FROM market_models WHERE kind <> 'model'),
                max(last_seen_at), now(), $1
         FROM live",
    )
    .bind(started.elapsed().as_millis() as i32)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(RefreshReport {
        models: models + media,
        gpus,
        providers,
        millis: started.elapsed().as_millis() as u64,
    })
}
