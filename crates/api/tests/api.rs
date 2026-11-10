//! End-to-end tests of the HTTP API against a real Postgres.
//!
//! `sqlx::test` creates a fresh database per test, runs the migrations and drops
//! it afterwards. It needs `DATABASE_URL` pointing at a server where the user may
//! create databases (the docker compose Postgres works).

use api::AppState;
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

fn app(pool: PgPool) -> Router {
    api::app(AppState::new(pool), &[])
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    send(app, Method::GET, uri, None, None).await
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(json) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
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

const ADMIN_TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn admin_app(pool: PgPool) -> Router {
    api::app(
        AppState {
            pool,
            admin_token: Some(ADMIN_TOKEN.to_owned()),
        },
        &[],
    )
}

/// Three news items: two approved (one about OpenAI), one pending.
async fn seed_news(pool: &PgPool) -> Vec<uuid::Uuid> {
    let source: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO sources (slug, name, source_type, url) VALUES ('press', 'Press', 'rss', 'https://press.example/rss')
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let mut ids = Vec::new();
    for (slug, title, status, day) in [
        ("old", "Older 100% news_item", "approved", 1),
        ("new", "OpenAI ships a model", "approved", 3),
        ("queued", "Pending story", "pending", 2),
    ] {
        let id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO news_items (source_id, url, title, publisher, published_at, status)
             VALUES ($1, $2, $3, 'Press', make_timestamptz(2025, 11, $4, 0, 0, 0), $5::news_status)
             RETURNING id",
        )
        .bind(source)
        .bind(format!("https://press.example/{slug}"))
        .bind(title)
        .bind(day)
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap();
        ids.push(id);
    }
    sqlx::query(
        "INSERT INTO news_item_organizations (news_item_id, organization_id, matched_by)
         SELECT $1, id, 'title' FROM organizations WHERE slug = 'openai'",
    )
    .bind(ids[1])
    .execute(pool)
    .await
    .unwrap();
    ids
}

fn titles(page: &Value) -> Vec<&str> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].as_str().unwrap())
        .collect()
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn public_news_shows_only_approved_items_newest_first(pool: PgPool) {
    seed_news(&pool).await;
    let app = app(pool);

    let (status, body) = get(&app, "/v1/news").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        titles(&body),
        ["OpenAI ships a model", "Older 100% news_item"]
    );
    assert_eq!(body["items"][0]["organizations"][0]["slug"], "openai");
    assert_eq!(body["items"][0]["organizations"][0]["matched_by"], "title");

    let (_, body) = get(&app, "/v1/news?organization=openai").await;
    assert_eq!(titles(&body), ["OpenAI ships a model"]);

    let (_, body) = get(&app, "/v1/news?source=press&limit=1&offset=1").await;
    assert_eq!(titles(&body), ["Older 100% news_item"]);
    assert_eq!(body["total"], 2);

    // LIKE wildcards in the search text match literally.
    let (_, body) = get(&app, "/v1/news?q=100%25").await;
    assert_eq!(titles(&body), ["Older 100% news_item"]);
    let (_, body) = get(&app, "/v1/news?q=_").await;
    assert_eq!(titles(&body), ["Older 100% news_item"]);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn admin_api_is_hidden_without_a_token_and_guarded_with_one(pool: PgPool) {
    let disabled = app(pool.clone());
    let (status, _) = send(
        &disabled,
        Method::GET,
        "/v1/admin/news",
        Some(ADMIN_TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no ADMIN_TOKEN configured");

    let app = admin_app(pool);
    for token in [
        None,
        Some("wrong"),
        Some("0123456789abcdef0123456789abcdeX"),
    ] {
        let (status, body) = send(&app, Method::GET, "/v1/admin/news", token, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
        assert_eq!(body["code"], "unauthorized");
    }
    let (status, _) = send(
        &app,
        Method::GET,
        "/v1/admin/sources",
        Some(ADMIN_TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn admin_reviews_the_queue(pool: PgPool) {
    let ids = seed_news(&pool).await;
    let app = admin_app(pool);
    let token = Some(ADMIN_TOKEN);

    let (_, body) = send(&app, Method::GET, "/v1/admin/news", token, None).await;
    assert_eq!(
        titles(&body),
        ["Pending story"],
        "defaults to the pending queue"
    );

    let review =
        |ids: Vec<String>, status: &str| serde_json::json!({ "ids": ids, "status": status });
    let (status, body) = send(
        &app,
        Method::POST,
        "/v1/admin/news/review",
        token,
        Some(review(
            vec![ids[2].to_string(), uuid::Uuid::nil().to_string()],
            "approved",
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["updated"], 1, "unknown ids are ignored");

    let (_, body) = get(&app, "/v1/news").await;
    assert_eq!(body["total"], 3, "the approved item is now public");

    let (_, body) = send(
        &app,
        Method::GET,
        "/v1/admin/news?status=rejected",
        token,
        None,
    )
    .await;
    assert_eq!(body["total"], 0);

    for bad in [
        review(vec![], "approved"),
        serde_json::json!({ "ids": [ids[0].to_string()], "status": "published" }),
        serde_json::json!({ "ids": ["not-a-uuid"], "status": "approved" }),
    ] {
        let (status, body) = send(
            &app,
            Method::POST,
            "/v1/admin/news/review",
            token,
            Some(bad),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "bad_request");
    }

    let (_, sources) = send(&app, Method::GET, "/v1/admin/sources", token, None).await;
    assert_eq!(sources[0]["slug"], "press");
    assert_eq!(sources[0]["item_count"], 3);
    assert_eq!(sources[0]["pending_count"], 0);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn admin_corrects_organization_tags(pool: PgPool) {
    let ids = seed_news(&pool).await;
    let app = admin_app(pool);
    let token = Some(ADMIN_TOKEN);
    let uri = format!("/v1/admin/news/{}/organizations", ids[1]);

    // Keep OpenAI (its title match stays), add Anthropic as a manual tag.
    let (status, body) = send(
        &app,
        Method::PUT,
        &uri,
        token,
        Some(serde_json::json!({ "organizations": ["openai", "anthropic"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tags: Vec<(String, String)> = body["organizations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            (
                o["slug"].as_str().unwrap().into(),
                o["matched_by"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        tags,
        [
            ("openai".into(), "title".into()),
            ("anthropic".into(), "manual".into())
        ]
    );

    // Removing all tags works; unknown slugs and unknown items are errors.
    let (_, body) = send(
        &app,
        Method::PUT,
        &uri,
        token,
        Some(serde_json::json!({ "organizations": [] })),
    )
    .await;
    assert_eq!(body["organizations"].as_array().unwrap().len(), 0);

    let (status, body) = send(
        &app,
        Method::PUT,
        &uri,
        token,
        Some(serde_json::json!({ "organizations": ["openai", "nope"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap().contains("nope"));

    let missing = format!("/v1/admin/news/{}/organizations", uuid::Uuid::nil());
    let (status, _) = send(
        &app,
        Method::PUT,
        &missing,
        token,
        Some(serde_json::json!({ "organizations": ["openai"] })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
