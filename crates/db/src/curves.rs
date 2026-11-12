//! Reads of the price curves: term structure and daily snapshots.

use ih_core::{
    OrganizationRef,
    curves::{
        CurveIndex, CurveItem, CurveNode, GpuCurve, LadderRung, ModelCurve, SnapshotPoint,
        TermPoint,
    },
};
use sqlx::PgPool;

/// GPU types with their tier counts, and the most widely sold models.
pub async fn index(pool: &PgPool) -> sqlx::Result<CurveIndex> {
    let gpus = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT g.gpu_type, g.providers::int8,
                (SELECT count(DISTINCT (t.tier, t.term_months)) FROM gpu_term_prices t
                 WHERE t.gpu_type = g.gpu_type)
         FROM market_gpus g
         ORDER BY 3 DESC, g.providers DESC, g.gpu_type",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(key, providers, tiers)| CurveItem {
        name: key.clone(),
        key,
        providers,
        tiers,
    })
    .collect();
    let models = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT model_key, name, providers::int8 FROM market_models
         WHERE kind = 'model' AND providers >= 2 AND min_blended_per_mtok > 0
         ORDER BY providers DESC, min_blended_per_mtok, model_key LIMIT 120",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(key, name, providers)| CurveItem {
        key,
        name,
        providers,
        tiers: 1,
    })
    .collect();
    Ok(CurveIndex { gpus, models })
}

#[derive(sqlx::FromRow)]
struct PointRow {
    tier: String,
    term_months: i32,
    per_gpu_hour: f64,
    source_url: Option<String>,
    organization_slug: String,
    organization_name: String,
    organization_logo: Option<String>,
}

async fn history(pool: &PgPool, kind: &str, key: &str) -> sqlx::Result<Vec<SnapshotPoint>> {
    Ok(
        sqlx::query_as::<_, (chrono::NaiveDate, i32, f64, f64, Option<f64>)>(
            "SELECT day, providers, min_price, median_price, max_price FROM price_snapshots
         WHERE kind = $1 AND item_key = $2 ORDER BY day",
        )
        .bind(kind)
        .bind(key)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(day, providers, min, median, max)| SnapshotPoint {
            day,
            providers: i64::from(providers),
            min,
            median,
            max,
        })
        .collect(),
    )
}

/// The term structure and the price history of one GPU type. `None` when the market has no such GPU.
pub async fn gpu(pool: &PgPool, gpu_type: &str) -> sqlx::Result<Option<GpuCurve>> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM market_gpus WHERE gpu_type = $1)")
            .bind(gpu_type)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Ok(None);
    }
    let rows: Vec<PointRow> = sqlx::query_as(
        "SELECT t.tier, t.term_months, t.per_gpu_hour, t.source_url,
                o.slug AS organization_slug, o.name AS organization_name,
                o.logo_url AS organization_logo
         FROM gpu_term_prices t JOIN organizations o ON o.id = t.organization_id
         WHERE t.gpu_type = $1
         ORDER BY t.term_months, t.tier, t.per_gpu_hour",
    )
    .bind(gpu_type)
    .fetch_all(pool)
    .await?;
    let points: Vec<TermPoint> = rows
        .into_iter()
        .map(|r| TermPoint {
            organization: OrganizationRef {
                slug: r.organization_slug,
                name: r.organization_name,
                logo_url: r.organization_logo,
            },
            tier: r.tier,
            term_months: r.term_months,
            per_gpu_hour: r.per_gpu_hour,
            source_url: r.source_url,
        })
        .collect();
    let nodes = sqlx::query_as::<_, (String, i32, i64, f64, f64, f64)>(
        "SELECT tier, term_months, count(*)::int8, min(per_gpu_hour),
                (percentile_cont(0.5) WITHIN GROUP (ORDER BY per_gpu_hour))::float8,
                max(per_gpu_hour)
         FROM gpu_term_prices WHERE gpu_type = $1
         GROUP BY tier, term_months ORDER BY term_months, tier",
    )
    .bind(gpu_type)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(
        |(tier, term_months, providers, min, median, max)| CurveNode {
            tier,
            term_months,
            providers,
            min,
            median,
            max,
        },
    )
    .collect();
    Ok(Some(GpuCurve {
        gpu_type: gpu_type.to_owned(),
        nodes,
        points,
        history: history(pool, "gpu", gpu_type).await?,
    }))
}

/// The provider ladder and the price history of one model. `None` when the market has no such model.
pub async fn model(pool: &PgPool, model_key: &str) -> sqlx::Result<Option<ModelCurve>> {
    let name: Option<String> = sqlx::query_scalar(
        "SELECT name FROM market_models WHERE kind = 'model' AND model_key = $1",
    )
    .bind(model_key)
    .fetch_optional(pool)
    .await?;
    let Some(name) = name else {
        return Ok(None);
    };
    let ladder = sqlx::query_as::<_, (f64, String, String, Option<String>)>(
        "SELECT p.price, o.slug, o.name, o.logo_url
         FROM market_provider_offerings p JOIN organizations o ON o.id = p.organization_id
         WHERE p.kind = 'model' AND p.item_key = $1 AND p.price > 0 AND 'adapter' = ANY(p.sources)
         ORDER BY p.price, o.name",
    )
    .bind(model_key)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(price, slug, name, logo_url)| LadderRung {
        blended_per_mtok: price,
        organization: OrganizationRef {
            slug,
            name,
            logo_url,
        },
    })
    .collect();
    Ok(Some(ModelCurve {
        model_key: model_key.to_owned(),
        name,
        ladder,
        history: history(pool, "model", model_key).await?,
    }))
}
