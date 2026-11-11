//! The worker's HTTP endpoints for the platform: liveness, readiness and
//! Prometheus metrics. No other routes; the worker has no public API.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

/// Counters the runners update as they work.
#[derive(Default)]
pub struct Metrics {
    jobs_ok: Mutex<BTreeMap<String, u64>>,
    jobs_failed: Mutex<BTreeMap<String, u64>>,
    /// Seconds since the Unix epoch of the last scheduler pass that reached the database.
    last_schedule_ok: AtomicU64,
}

impl Metrics {
    pub fn job_finished(&self, kind: &str, ok: bool) {
        let map = if ok { &self.jobs_ok } else { &self.jobs_failed };
        if let Ok(mut map) = map.lock() {
            *map.entry(kind.to_owned()).or_default() += 1;
        }
    }

    pub fn schedule_ok(&self) {
        self.last_schedule_ok.store(unix_now(), Ordering::Relaxed);
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    metrics: Arc<Metrics>,
    started: Instant,
}

pub fn router(pool: PgPool, metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics_text))
        .with_state(AppState {
            pool,
            metrics,
            started: Instant::now(),
        })
}

/// Serves the endpoints on `addr` until `cancel` fires.
pub async fn serve(
    addr: std::net::SocketAddr,
    pool: PgPool,
    metrics: Arc<Metrics>,
    cancel: CancellationToken,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "worker health server listening");
    axum::serve(listener, router(pool, metrics))
        .with_graceful_shutdown(async move { cancel.cancelled().await })
        .await
}

/// Ready when the database answers within two seconds.
async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let ping = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&state.pool),
    )
    .await;
    match ping {
        Ok(Ok(_)) => (StatusCode::OK, "ready"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "database unreachable"),
    }
}

#[derive(sqlx::FromRow)]
struct AdapterRow {
    adapter: String,
    last_success: Option<f64>,
    consecutive_failures: i32,
    listings_seen: i32,
}

/// Prometheus text exposition. Queue and adapter gauges are read from the
/// database, so they are correct across restarts and multiple workers.
async fn metrics_text(State(state): State<AppState>) -> impl IntoResponse {
    let mut out = String::new();
    let m = &state.metrics;

    let _ = writeln!(
        out,
        "# HELP ih_worker_uptime_seconds Seconds since the worker started."
    );
    let _ = writeln!(out, "# TYPE ih_worker_uptime_seconds gauge");
    let _ = writeln!(
        out,
        "ih_worker_uptime_seconds {}",
        state.started.elapsed().as_secs()
    );

    let _ = writeln!(
        out,
        "# HELP ih_worker_last_schedule_timestamp_seconds Last scheduler pass that reached the database."
    );
    let _ = writeln!(
        out,
        "# TYPE ih_worker_last_schedule_timestamp_seconds gauge"
    );
    let _ = writeln!(
        out,
        "ih_worker_last_schedule_timestamp_seconds {}",
        m.last_schedule_ok.load(Ordering::Relaxed)
    );

    let _ = writeln!(
        out,
        "# HELP ih_worker_jobs_total Jobs this process finished, by kind and outcome."
    );
    let _ = writeln!(out, "# TYPE ih_worker_jobs_total counter");
    for (outcome, map) in [("ok", &m.jobs_ok), ("failed", &m.jobs_failed)] {
        if let Ok(map) = map.lock() {
            for (kind, n) in map.iter() {
                let _ = writeln!(
                    out,
                    "ih_worker_jobs_total{{kind=\"{kind}\",outcome=\"{outcome}\"}} {n}"
                );
            }
        }
    }

    if let Ok(rows) = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, count(*) FROM jobs GROUP BY status",
    )
    .fetch_all(&state.pool)
    .await
    {
        let _ = writeln!(out, "# HELP ih_jobs Jobs in the queue, by status.");
        let _ = writeln!(out, "# TYPE ih_jobs gauge");
        for (status, n) in rows {
            let _ = writeln!(out, "ih_jobs{{status=\"{status}\"}} {n}");
        }
    }

    if let Ok(rows) = sqlx::query_as::<_, AdapterRow>(
        "SELECT adapter, extract(epoch FROM last_success_at)::float8 AS last_success,
                consecutive_failures, listings_seen
         FROM adapter_state ORDER BY adapter",
    )
    .fetch_all(&state.pool)
    .await
    {
        let _ = writeln!(
            out,
            "# HELP ih_adapter_last_success_timestamp_seconds Last successful run of each market adapter."
        );
        let _ = writeln!(
            out,
            "# TYPE ih_adapter_last_success_timestamp_seconds gauge"
        );
        for r in &rows {
            let _ = writeln!(
                out,
                "ih_adapter_last_success_timestamp_seconds{{adapter=\"{}\"}} {}",
                r.adapter,
                r.last_success.unwrap_or(0.0)
            );
        }
        let _ = writeln!(
            out,
            "# HELP ih_adapter_consecutive_failures Failed runs since the last success."
        );
        let _ = writeln!(out, "# TYPE ih_adapter_consecutive_failures gauge");
        for r in &rows {
            let _ = writeln!(
                out,
                "ih_adapter_consecutive_failures{{adapter=\"{}\"}} {}",
                r.adapter, r.consecutive_failures
            );
        }
        let _ = writeln!(
            out,
            "# HELP ih_adapter_listings Listings seen by the last successful run."
        );
        let _ = writeln!(out, "# TYPE ih_adapter_listings gauge");
        for r in &rows {
            let _ = writeln!(
                out,
                "ih_adapter_listings{{adapter=\"{}\"}} {}",
                r.adapter, r.listings_seen
            );
        }
    }

    if let Ok(rows) = sqlx::query_as::<_, (String, i32, bool)>(
        "SELECT slug, consecutive_failures, enabled FROM sources ORDER BY slug",
    )
    .fetch_all(&state.pool)
    .await
    {
        let _ = writeln!(
            out,
            "# HELP ih_source_consecutive_failures Failed fetches of each feed since its last success."
        );
        let _ = writeln!(out, "# TYPE ih_source_consecutive_failures gauge");
        for (slug, failures, enabled) in rows {
            if enabled {
                let _ = writeln!(
                    out,
                    "ih_source_consecutive_failures{{source=\"{slug}\"}} {failures}"
                );
            }
        }
    }

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        out,
    )
}
