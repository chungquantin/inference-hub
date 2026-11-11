//! The worker's platform endpoints.

use std::sync::Arc;

use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use worker::health::{Metrics, router};

async fn get(app: &axum::Router, path: &str) -> (u16, String) {
    let res = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status().as_u16();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn health_ready_and_metrics(pool: PgPool) {
    sqlx::query(
        "INSERT INTO adapter_state (adapter, last_success_at, consecutive_failures, listings_seen)
         VALUES ('openrouter', '2026-01-01T00:00:00Z', 0, 457), ('coreweave', NULL, 3, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let metrics = Arc::new(Metrics::default());
    metrics.job_finished("run_adapter", true);
    metrics.job_finished("run_adapter", true);
    metrics.job_finished("fetch_source", false);
    let app = router(pool, metrics);

    assert_eq!(get(&app, "/healthz").await, (200, "ok".into()));
    assert_eq!(get(&app, "/readyz").await.0, 200);

    let (status, body) = get(&app, "/metrics").await;
    assert_eq!(status, 200);
    for line in [
        "ih_worker_jobs_total{kind=\"run_adapter\",outcome=\"ok\"} 2",
        "ih_worker_jobs_total{kind=\"fetch_source\",outcome=\"failed\"} 1",
        "ih_adapter_last_success_timestamp_seconds{adapter=\"openrouter\"} 1767225600",
        "ih_adapter_consecutive_failures{adapter=\"coreweave\"} 3",
        "ih_adapter_listings{adapter=\"openrouter\"} 457",
    ] {
        assert!(body.contains(line), "missing `{line}` in:\n{body}");
    }
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn not_ready_when_the_database_is_gone(pool: PgPool) {
    let app = router(pool.clone(), Arc::default());
    pool.close().await;
    assert_eq!(get(&app, "/readyz").await.0, 503);
    assert_eq!(
        get(&app, "/healthz").await.0,
        200,
        "liveness does not depend on the database"
    );
}
