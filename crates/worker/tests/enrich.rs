//! News enrichment through a fake OpenAI-compatible exchange.

use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
use ih_ingest::llm::{LlmConfig, enrich_pending};
use serde_json::{Value, json};
use sqlx::PgPool;

#[derive(Clone, Default)]
struct Exchange {
    /// Requests received, for assertions.
    seen: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
}

/// Answers by title: CoreWeave news is core, the rest off topic. Names one
/// slug outside the list, which must be dropped.
async fn chat(
    State(x): State<Exchange>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let user = body["messages"][1]["content"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    x.seen.lock().unwrap().push((headers, body));
    let answer = if user.contains("CoreWeave") {
        json!({"topic":"partnership","relevance":"core","verdict":"approve","confidence":0.9,
               "summary":"CoreWeave signed a GPU deal with OpenAI.",
               "organizations":["coreweave","openai","not-a-real-org"]})
    } else {
        json!({"topic":"other","relevance":"off_topic","verdict":"reject","confidence":0.95,
               "summary":"A recipe.","organizations":[]})
    };
    Json(json!({
        "model": "fake-model",
        "choices": [{"message": {"role": "assistant", "content": answer.to_string()}}],
        "exchange": {"charge": {"total_usd": 0.002}}
    }))
}

async fn start() -> (String, Exchange) {
    let x = Exchange::default();
    let app = Router::new()
        .route("/v1/chat/completions", post(chat))
        .with_state(x.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, x)
}

fn config(base: &str) -> LlmConfig {
    LlmConfig {
        base_url: base.into(),
        api_key: "test-key".into(),
        model: "liquid.auto".into(),
        cap_usd_per_call: 0.01,
        max_usd_per_run: 1.0,
        max_usd_per_day: 1.0,
        batch: 10,
    }
}

async fn seed(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO organizations (slug, name, kinds) VALUES
             ('coreweave', 'CoreWeave', '{gpu_cloud}'), ('openai', 'OpenAI', '{model_provider}')
         ON CONFLICT (slug) DO NOTHING",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sources (slug, name, source_type, url, organization_id) VALUES
             ('coreweave-blog', 'CoreWeave Blog', 'rss', 'https://cw.example/feed',
              (SELECT id FROM organizations WHERE slug = 'coreweave')),
             ('outlet', 'Outlet', 'rss', 'https://outlet.example/feed', NULL)",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO news_items (source_id, url, title, published_at, status) VALUES
           ((SELECT id FROM sources WHERE slug = 'coreweave-blog'), 'https://cw.example/1',
            'CoreWeave signs GPU deal with OpenAI', now(), 'pending'),
           ((SELECT id FROM sources WHERE slug = 'outlet'), 'https://outlet.example/2',
            'CoreWeave mentioned on a shared outlet', now() - interval '1 hour', 'pending'),
           ((SELECT id FROM sources WHERE slug = 'outlet'), 'https://outlet.example/3',
            'Best sourdough recipe', now() - interval '2 hours', 'pending')",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn enriches_pending_news_within_the_closed_slug_list(pool: PgPool) {
    seed(&pool).await;
    let (base, exchange) = start().await;
    let http = ih_ingest::http_client().unwrap();

    let report = enrich_pending(&pool, &http, &config(&base)).await.unwrap();
    assert_eq!(report.enriched, 3);
    assert_eq!(
        report.approved, 1,
        "only the company's own feed is promoted"
    );
    assert!((report.cost_usd - 0.006).abs() < 1e-9);

    let status: Vec<(String, String)> =
        sqlx::query_as("SELECT url, status::text FROM news_items ORDER BY url")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        status,
        [
            ("https://cw.example/1".into(), "approved".into()),
            ("https://outlet.example/2".into(), "pending".into()),
            ("https://outlet.example/3".into(), "pending".into()),
        ]
    );

    let llm_tags: Vec<String> = sqlx::query_scalar(
        "SELECT o.slug FROM news_item_organizations nio JOIN organizations o ON o.id = nio.organization_id
         WHERE nio.matched_by = 'llm' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        llm_tags,
        ["coreweave", "coreweave", "openai", "openai"],
        "made-up slugs are dropped"
    );

    // The request carries the cap, a strict schema and the closed slug list.
    let seen = exchange.seen.lock().unwrap().clone();
    let (headers, body) = &seen[0];
    assert_eq!(headers["x-liquid-cap-usd"], "0.01");
    assert_eq!(headers["authorization"], "Bearer test-key");
    assert_eq!(body["response_format"]["type"], "json_schema");
    let user = body["messages"][1]["content"].as_str().unwrap();
    let list = user.lines().next().unwrap();
    assert!(
        list.starts_with("Organizations (slugs): ")
            && list.contains("coreweave")
            && list.contains("openai")
    );

    // Enriched items are not enriched again.
    let again = enrich_pending(&pool, &http, &config(&base)).await.unwrap();
    assert_eq!(again.enriched, 0);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn stops_when_the_daily_budget_is_spent(pool: PgPool) {
    seed(&pool).await;
    let (base, exchange) = start().await;
    let http = ih_ingest::http_client().unwrap();
    // Room for one call: after it costs $0.002, less than the $0.01 cap is left.
    let cfg = LlmConfig {
        max_usd_per_day: 0.011,
        ..config(&base)
    };
    let report = enrich_pending(&pool, &http, &cfg).await.unwrap();
    assert_eq!(report.enriched, 1);
    assert!(report.budget_exhausted);
    assert_eq!(exchange.seen.lock().unwrap().len(), 1);

    // The next pass sees the stored spend and makes no call.
    let next = enrich_pending(&pool, &http, &cfg).await.unwrap();
    assert_eq!((next.enriched, next.budget_exhausted), (0, true));
    assert_eq!(exchange.seen.lock().unwrap().len(), 1);
}
