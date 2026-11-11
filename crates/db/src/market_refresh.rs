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
                l.gpu_count, l.unit, l.last_seen_at,
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
                array_agg(DISTINCT org),
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

    // Providers: catalog size and contested wins (items two or more
    // organizations sell, where this one is cheapest).
    let providers = sqlx::query(
        "WITH priced AS (
             SELECT kind, item, org, org_name,
                    CASE WHEN kind = 'gpu' THEN per_gpu
                         WHEN kind = 'model' THEN blended
                         ELSE per_unit END AS unit_price
             FROM live
         ),
         contested AS (
             SELECT kind, item FROM priced WHERE unit_price > 0
             GROUP BY kind, item HAVING count(DISTINCT org) >= 2
         ),
         winners AS (
             SELECT DISTINCT ON (kind, item) org
             FROM priced JOIN contested USING (kind, item)
             WHERE unit_price > 0
             ORDER BY kind, item, unit_price, org_name
         )
         INSERT INTO market_providers
             (organization_id, models, model_listings, media_models, gpu_configurations,
              cheapest_count, last_seen_at)
         SELECT org,
                count(DISTINCT item) FILTER (WHERE kind = 'model'),
                count(*) FILTER (WHERE kind = 'model'),
                count(DISTINCT kind || ':' || item) FILTER (WHERE kind NOT IN ('model', 'gpu')),
                count(*) FILTER (WHERE kind = 'gpu'),
                (SELECT count(*) FROM winners w WHERE w.org = l.org),
                max(last_seen_at)
         FROM live l GROUP BY org",
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
