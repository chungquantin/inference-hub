//! End-to-end tests of the HTTP API against a real Postgres.
//!
//! `sqlx::test` creates a fresh database per test, runs the migrations and drops
//! it afterwards. It needs `DATABASE_URL` pointing at a server where the user may
//! create databases (the docker compose Postgres works).

use api::AppState;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

fn app(pool: PgPool) -> Router {
    api::app(AppState { pool }, &[])
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        panic!(
            "{uri} returned non-JSON: {}",
            String::from_utf8_lossy(&bytes)
        )
    });
    (status, body)
}

fn slugs(page: &Value) -> Vec<&str> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slug"].as_str().unwrap())
        .collect()
}

const LAB: &str = r#"
slug: test-lab
name: Test Lab
kinds: [model_provider]
tagline: Builds test models.
products:
  - slug: test-model-open
    name: Test Model Open
    type: model
    launched_at: 2025-03-01
    model:
      parameters_b: 70
      context_window: 131072
      open_weights: true
    offerings:
      - provider: test-cloud
        input: 0.5
        output: 2.0
        observed_on: 2025-01-01
      - provider: test-cloud
        input: 0.4
        output: 1.5
        observed_on: 2025-06-01
      - provider: test-lab
        input: 1.0
        output: 1.0
        observed_on: 2025-06-01
  - slug: test-model-closed
    name: Test Model Closed
    type: model
    model:
      open_weights: false
"#;

const CLOUD: &str = r#"
slug: test-cloud
name: Test Cloud
kinds: [inference_provider, gpu_cloud]
"#;

async fn seed_fixtures(pool: &PgPool) {
    let files = vec![
        (
            "test-cloud.yaml".to_owned(),
            serde_saphyr::from_str(CLOUD).unwrap(),
        ),
        (
            "test-lab.yaml".to_owned(),
            serde_saphyr::from_str(LAB).unwrap(),
        ),
    ];
    seed::import(pool, &seed::DataSet::from_organizations(files))
        .await
        .expect("fixtures import");
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn health_reports_database(pool: PgPool) {
    let (status, body) = get(&app(pool), "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["database"], "ok");
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn categories_list_and_detail(pool: PgPool) {
    let app = app(pool);
    let (status, body) = get(&app, "/v1/categories").await;
    assert_eq!(status, StatusCode::OK);
    let categories = body.as_array().unwrap();
    assert_eq!(categories.len(), 8);
    assert_eq!(categories[0]["slug"], "model-providers");

    let (status, body) = get(&app, "/v1/categories/gpu-clouds").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "GPU Clouds");

    let (status, body) = get(&app, "/v1/categories/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn organizations_filter_search_and_page(pool: PgPool) {
    let app = app(pool);

    let (_, body) = get(&app, "/v1/organizations?kind=gpu_cloud").await;
    assert_eq!(slugs(&body), ["coreweave", "lambda", "together-ai"]);

    let (_, body) = get(&app, "/v1/organizations?category=investors").await;
    assert_eq!(slugs(&body), ["a16z", "nvidia"]);

    let (_, body) = get(&app, "/v1/organizations?q=serving%20engine").await;
    assert_eq!(slugs(&body), ["vllm"]);

    let (_, body) = get(&app, "/v1/organizations?limit=5&offset=10").await;
    assert_eq!(body["total"], 12);
    assert_eq!(body["limit"], 5);
    assert_eq!(slugs(&body).len(), 2);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn invalid_parameters_get_json_errors(pool: PgPool) {
    let app = app(pool);
    for uri in [
        "/v1/organizations?kind=bogus",
        "/v1/organizations?limit=abc",
        "/v1/products?type=spaceship",
        "/v1/products?open_weights=maybe",
    ] {
        let (status, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(body["code"], "bad_request", "{uri}");
    }
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn organization_detail_lists_made_and_served_products(pool: PgPool) {
    seed_fixtures(&pool).await;
    let app = app(pool);

    let (status, body) = get(&app, "/v1/organizations/test-cloud").await;
    assert_eq!(status, StatusCode::OK);
    let categories: Vec<&str> = body["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["slug"].as_str().unwrap())
        .collect();
    assert_eq!(categories, ["inference-providers", "gpu-clouds"]);
    assert_eq!(body["products"].as_array().unwrap().len(), 0);
    assert_eq!(body["served_products"][0]["slug"], "test-model-open");

    let (_, body) = get(&app, "/v1/organizations/test-lab").await;
    assert_eq!(body["products"].as_array().unwrap().len(), 2);
    assert_eq!(body["served_products"].as_array().unwrap().len(), 0);

    let (status, _) = get(&app, "/v1/organizations/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn products_filter_by_type_maker_and_open_weights(pool: PgPool) {
    seed_fixtures(&pool).await;
    let app = app(pool);

    let (_, body) = get(&app, "/v1/products?organization=test-lab").await;
    assert_eq!(slugs(&body), ["test-model-open", "test-model-closed"]);
    assert_eq!(body["items"][0]["organization"]["name"], "Test Lab");
    assert_eq!(body["items"][0]["product_type"], "model");

    let (_, body) = get(
        &app,
        "/v1/products?organization=test-lab&open_weights=false",
    )
    .await;
    assert_eq!(slugs(&body), ["test-model-closed"]);

    let (_, body) = get(&app, "/v1/products?type=hardware&organization=test-lab").await;
    assert_eq!(body["total"], 0);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn product_detail_shows_latest_offering_per_provider(pool: PgPool) {
    seed_fixtures(&pool).await;
    let app = app(pool);

    let (status, body) = get(&app, "/v1/products/test-model-open").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["model"]["parameters_b"], 70.0);
    assert_eq!(body["model"]["context_window"], 131072);

    // Two observations from test-cloud collapse to the newest; cheapest output first.
    let offerings = body["offerings"].as_array().unwrap();
    assert_eq!(offerings.len(), 2);
    assert_eq!(offerings[0]["provider"]["slug"], "test-lab");
    assert_eq!(offerings[0]["price_output_per_mtok"], 1.0);
    assert_eq!(offerings[1]["provider"]["slug"], "test-cloud");
    assert_eq!(offerings[1]["price_output_per_mtok"], 1.5);

    let (status, body) = get(&app, "/v1/products/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["message"], "product not found");
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn sitemap_lists_every_slug(pool: PgPool) {
    seed_fixtures(&pool).await;
    let (status, body) = get(&app(pool), "/v1/sitemap").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["categories"].as_array().unwrap().len(), 8);
    assert_eq!(body["organizations"].as_array().unwrap().len(), 14);
    assert_eq!(body["products"].as_array().unwrap().len(), 2);
}
