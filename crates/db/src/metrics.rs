//! Reads of the metric series: rankings, totals and the history of the leaders.

use chrono::NaiveDate;
use ih_core::{
    METRICS, MetricBoard, MetricCatalog, MetricDef, MetricLine, MetricPoint, MetricRow,
    MetricSourceStatus, MetricSummary, OrganizationRef,
};
use sqlx::PgPool;

/// A series whose newest point is older than this, in days, behind its metric's newest is dropped
/// from rankings: the subject left the source's list.
const STALE_DAYS: i32 = 3;

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    subject_key: String,
    subject_name: String,
    value: f64,
    organization_slug: Option<String>,
    organization_name: Option<String>,
    organization_logo: Option<String>,
    change_7d: Option<f64>,
    change_30d: Option<f64>,
    change_90d: Option<f64>,
    spark: Option<Vec<f64>>,
}

struct Totals {
    as_of: Option<NaiveDate>,
    total: Option<f64>,
    change_30d: Option<f64>,
    history_days: i64,
    subjects: i64,
}

async fn totals(pool: &PgPool, def: &MetricDef) -> sqlx::Result<Totals> {
    let (as_of, total, subjects, first_day): (
        Option<NaiveDate>,
        Option<f64>,
        i64,
        Option<NaiveDate>,
    ) = sqlx::query_as(
        "WITH m AS (SELECT max(latest_day) AS d FROM metric_series WHERE metric = $1)
             SELECT (SELECT d FROM m), sum(s.latest_value), count(*),
                    (SELECT min(p.day) FROM metric_points p
                     JOIN metric_series x ON x.id = p.series_id WHERE x.metric = $1)
             FROM metric_series s WHERE s.metric = $1
               AND s.latest_day >= (SELECT d FROM m) - $2",
    )
    .bind(def.key)
    .bind(STALE_DAYS)
    .fetch_one(pool)
    .await?;
    let change_30d = match (def.additive, as_of) {
        (true, Some(day)) => {
            // Total of the series that have a point on both days, so a new entrant does not read as growth.
            let pair: Option<(Option<f64>, Option<f64>)> = sqlx::query_as(
                "SELECT sum(now.value), sum(then_.value)
                 FROM metric_series s
                 JOIN LATERAL (SELECT value FROM metric_points WHERE series_id = s.id
                               AND day <= $2 ORDER BY day DESC LIMIT 1) now ON true
                 JOIN LATERAL (SELECT value FROM metric_points WHERE series_id = s.id
                               AND day <= $2 - 30 AND day >= $2 - 37 ORDER BY day DESC LIMIT 1) then_ ON true
                 WHERE s.metric = $1 AND s.latest_day >= $2 - $3",
            )
            .bind(def.key)
            .bind(day)
            .bind(STALE_DAYS)
            .fetch_optional(pool)
            .await?;
            pair.and_then(|(n, old)| Some((n?, old?)))
                .filter(|(_, old)| *old > 0.0)
                .map(|(n, old)| n / old - 1.0)
        }
        _ => None,
    };
    Ok(Totals {
        as_of,
        total: def.additive.then_some(total).flatten(),
        change_30d,
        history_days: match (as_of, first_day) {
            (Some(a), Some(f)) => i64::from((a - f).num_days() as i32),
            _ => 0,
        },
        subjects,
    })
}

async fn rows(
    pool: &PgPool,
    def: &MetricDef,
    limit: i64,
    total: Option<f64>,
) -> sqlx::Result<(Vec<MetricRow>, Vec<i64>)> {
    let order = if def.higher_is_better { "DESC" } else { "ASC" };
    let sql = format!(
        "WITH m AS (SELECT max(latest_day) AS d FROM metric_series WHERE metric = $1)
         SELECT s.id, s.subject_key, s.subject_name, s.latest_value AS value,
                o.slug AS organization_slug, o.name AS organization_name,
                o.logo_url AS organization_logo,
                (s.latest_value / nullif((SELECT value FROM metric_points WHERE series_id = s.id
                    AND day <= s.latest_day - 7 AND day >= s.latest_day - 10
                    ORDER BY day DESC LIMIT 1), 0) - 1)::float8 AS change_7d,
                (s.latest_value / nullif((SELECT value FROM metric_points WHERE series_id = s.id
                    AND day <= s.latest_day - 30 AND day >= s.latest_day - 37
                    ORDER BY day DESC LIMIT 1), 0) - 1)::float8 AS change_30d,
                (s.latest_value / nullif((SELECT value FROM metric_points WHERE series_id = s.id
                    AND day <= s.latest_day - 90 AND day >= s.latest_day - 100
                    ORDER BY day DESC LIMIT 1), 0) - 1)::float8 AS change_90d,
                (SELECT array_agg(value ORDER BY day) FROM
                    (SELECT day, value FROM metric_points WHERE series_id = s.id
                     ORDER BY day DESC LIMIT 60) x) AS spark
         FROM metric_series s
         LEFT JOIN organizations o ON o.id = s.organization_id
         WHERE s.metric = $1 AND s.latest_day >= (SELECT d FROM m) - $2
         ORDER BY s.latest_value {order}, s.subject_key
         LIMIT $3"
    );
    let found: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(def.key)
        .bind(STALE_DAYS)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    let ids = found.iter().map(|r| r.id).collect();
    let rows = found
        .into_iter()
        .enumerate()
        .map(|(i, r)| MetricRow {
            rank: i as i64 + 1,
            share: total.filter(|t| *t > 0.0).map(|t| r.value / t),
            organization: match (r.organization_slug, r.organization_name) {
                (Some(slug), Some(name)) => Some(OrganizationRef {
                    slug,
                    name,
                    logo_url: r.organization_logo,
                }),
                _ => None,
            },
            subject_key: r.subject_key,
            name: r.subject_name,
            value: r.value,
            change_7d: r.change_7d,
            change_30d: r.change_30d,
            change_90d: r.change_90d,
            spark: r.spark.unwrap_or_default(),
        })
        .collect();
    Ok((rows, ids))
}

pub async fn catalog(pool: &PgPool) -> sqlx::Result<MetricCatalog> {
    let mut metrics = Vec::new();
    for def in METRICS {
        let t = totals(pool, def).await?;
        let (leaders, _) = rows(pool, def, 5, t.total).await?;
        // A metric nothing has reported yet is not a card.
        if t.subjects == 0 {
            continue;
        }
        metrics.push(MetricSummary {
            def: *def,
            subjects: t.subjects,
            as_of: t.as_of,
            total: t.total,
            total_change_30d: t.change_30d,
            history_days: t.history_days,
            leaders,
        });
    }
    let sources = sqlx::query_as::<_, MetricSourceStatus>(
        "SELECT source, last_success_at, consecutive_failures, last_error, series
         FROM metric_source_state ORDER BY source",
    )
    .fetch_all(pool)
    .await?;
    Ok(MetricCatalog { metrics, sources })
}

/// The ranking of one metric, and the last year of its ten leaders.
pub async fn board(
    pool: &PgPool,
    def: &'static MetricDef,
    limit: i64,
) -> sqlx::Result<MetricBoard> {
    let t = totals(pool, def).await?;
    let (rows, ids) = rows(pool, def, limit.clamp(1, 500), t.total).await?;
    let top = &rows[..rows.len().min(10)];
    let mut lines = Vec::with_capacity(top.len());
    for (row, id) in top.iter().zip(&ids) {
        let points: Vec<MetricPoint> = sqlx::query_as(
            "SELECT day, value FROM metric_points
             WHERE series_id = $1 AND day >= (SELECT latest_day FROM metric_series WHERE id = $1) - 365
             ORDER BY day",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;
        lines.push(MetricLine {
            subject_key: row.subject_key.clone(),
            name: row.name.clone(),
            organization: row.organization.clone(),
            points,
        });
    }
    Ok(MetricBoard {
        def: *def,
        as_of: t.as_of,
        total: t.total,
        total_change_30d: t.change_30d,
        history_days: t.history_days,
        rows,
        lines,
    })
}
