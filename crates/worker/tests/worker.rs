//! Worker tests against a real Postgres and a local feed server.

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use ih_db::jobs::{self, NewJob};
use sqlx::PgPool;
use uuid::Uuid;

const FEED_V1: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Acme Blog</title>
  <item>
    <title>Acme launches GPUs with OpenAI</title>
    <link>https://acme.example/blog/gpus?utm_source=rss</link>
    <guid>1</guid>
    <pubDate>Mon, 03 Nov 2025 09:00:00 GMT</pubDate>
    <description>&lt;p&gt;Now serving models on AWS Lambda and Lambda.&lt;/p&gt;</description>
  </item>
  <item>
    <title>Second post</title>
    <link>https://acme.example/blog/second</link>
    <pubDate>Tue, 04 Nov 2025 09:00:00 GMT</pubDate>
  </item>
</channel></rss>"#;

/// Same as V1 plus one item; one item URL differs only by tracking parameters.
const FEED_V2: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Acme Blog</title>
  <item>
    <title>Acme launches GPUs with OpenAI</title>
    <link>https://www.acme.example/blog/gpus/</link>
  </item>
  <item>
    <title>Second post</title>
    <link>https://acme.example/blog/second</link>
  </item>
  <item>
    <title>Third post</title>
    <link>https://acme.example/blog/third</link>
    <pubDate>Wed, 05 Nov 2025 09:00:00 GMT</pubDate>
  </item>
</channel></rss>"#;

/// A feed server whose content and behavior the test controls.
#[derive(Clone, Default)]
struct FeedServer {
    version: Arc<AtomicUsize>,
    hits: Arc<AtomicUsize>,
    conditional_hits: Arc<AtomicUsize>,
}

impl FeedServer {
    async fn start(self) -> SocketAddr {
        let state = self.clone();
        let app = Router::new()
            .route(
                "/feed.xml",
                get(move |headers: HeaderMap| {
                    let state = state.clone();
                    async move { state.serve(&headers) }
                }),
            )
            .route(
                "/broken.xml",
                get(|| async { (StatusCode::OK, "<html>nope</html>") }),
            )
            .route("/gone.xml", get(|| async { StatusCode::GONE }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        addr
    }

    fn serve(&self, headers: &HeaderMap) -> axum::response::Response {
        self.hits.fetch_add(1, Ordering::SeqCst);
        let version = self.version.load(Ordering::SeqCst);
        let etag = format!("\"v{version}\"");
        if headers
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(&etag)
        {
            self.conditional_hits.fetch_add(1, Ordering::SeqCst);
            return StatusCode::NOT_MODIFIED.into_response();
        }
        let body = if version == 1 { FEED_V1 } else { FEED_V2 };
        (
            [
                (header::CONTENT_TYPE, "application/rss+xml"),
                (header::ETAG, etag.as_str()),
            ],
            body,
        )
            .into_response()
    }
}

/// Creates or updates an organization; the seed migration already holds some slugs.
async fn insert_org(
    pool: &PgPool,
    slug: &str,
    name: &str,
    exclude: &[&str],
    match_case: bool,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO organizations (slug, name, kinds, exclude_phrases, match_case)
         VALUES ($1, $2, '{other}', $3, $4)
         ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name,
             exclude_phrases = EXCLUDED.exclude_phrases, match_case = EXCLUDED.match_case
         RETURNING id",
    )
    .bind(slug)
    .bind(name)
    .bind(exclude.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    .bind(match_case)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_source(
    pool: &PgPool,
    slug: &str,
    url: &str,
    org: Option<Uuid>,
    auto_approve: bool,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO sources (slug, name, source_type, url, organization_id, auto_approve)
         VALUES ($1, $1, 'rss', $2, $3, $4) RETURNING id",
    )
    .bind(slug)
    .bind(url)
    .bind(org)
    .bind(auto_approve)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn run_due(pool: &PgPool) -> usize {
    let http = ih_ingest::http_client().unwrap();
    worker::schedule_due_sources(pool).await.unwrap();
    worker::drain(pool, &http, "test").await.unwrap()
}

/// Makes every source due again, as if its interval had passed.
async fn age_sources(pool: &PgPool) {
    sqlx::query("UPDATE sources SET last_run_at = now() - interval '1 day'")
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn ingests_tags_dedupes_and_uses_conditional_requests(pool: PgPool) {
    let server = FeedServer::default();
    server.version.store(1, Ordering::SeqCst);
    let addr = server.clone().start().await;

    let acme = insert_org(&pool, "acme", "Acme", &[], false).await;
    let openai = insert_org(&pool, "openai", "OpenAI", &[], false).await;
    let lambda = insert_org(&pool, "lambda", "Lambda", &["AWS Lambda"], true).await;
    let source = insert_source(
        &pool,
        "acme-blog",
        &format!("http://{addr}/feed.xml"),
        Some(acme),
        true,
    )
    .await;

    // First run stores both items with canonical URLs and tags.
    assert_eq!(run_due(&pool).await, 1);
    let items: Vec<(String, String, String)> =
        sqlx::query_as("SELECT url, status::text, publisher FROM news_items ORDER BY published_at")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        items,
        [
            (
                "https://acme.example/blog/gpus".into(),
                "approved".into(),
                "Acme Blog".into()
            ),
            (
                "https://acme.example/blog/second".into(),
                "approved".into(),
                "Acme Blog".into()
            ),
        ]
    );

    let mut tags: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT no.organization_id, no.matched_by::text FROM news_item_organizations no
         JOIN news_items n ON n.id = no.news_item_id
         WHERE n.url = 'https://acme.example/blog/gpus'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    tags.sort();
    let mut expected = vec![
        (acme, "source".to_owned()),
        (openai, "title".to_owned()),
        (lambda, "body".to_owned()),
    ];
    expected.sort();
    assert_eq!(
        tags, expected,
        "source wins over the title match; AWS Lambda is excluded"
    );

    // A run before the interval does nothing.
    assert_eq!(run_due(&pool).await, 0);

    // Unchanged feed: the stored ETag gets a 304 and nothing is parsed.
    age_sources(&pool).await;
    assert_eq!(run_due(&pool).await, 1);
    assert_eq!(server.conditional_hits.load(Ordering::SeqCst), 1);

    // Changed feed: only the new item is added; the www/trailing-slash variant is a duplicate.
    server.version.store(2, Ordering::SeqCst);
    age_sources(&pool).await;
    run_due(&pool).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM news_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);

    let (failures, last_error, success): (i32, Option<String>, bool) = sqlx::query_as(
        "SELECT consecutive_failures, last_error, last_success_at IS NOT NULL FROM sources WHERE id = $1",
    )
    .bind(source)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((failures, last_error, success), (0, None, true));
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn shared_sources_go_to_review_and_failures_back_off(pool: PgPool) {
    let addr = FeedServer {
        version: Arc::new(AtomicUsize::new(1)),
        ..FeedServer::default()
    }
    .start()
    .await;
    insert_source(
        &pool,
        "press",
        &format!("http://{addr}/feed.xml"),
        None,
        false,
    )
    .await;
    let broken = insert_source(
        &pool,
        "broken",
        &format!("http://{addr}/broken.xml"),
        None,
        false,
    )
    .await;
    let gone = insert_source(
        &pool,
        "gone",
        &format!("http://{addr}/gone.xml"),
        None,
        false,
    )
    .await;

    assert_eq!(run_due(&pool).await, 3);

    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM news_items WHERE status = 'pending'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending, 2, "items from a shared source wait for review");

    for (id, expected) in [(broken, "not a valid feed"), (gone, "HTTP 410")] {
        let (failures, error): (i32, String) =
            sqlx::query_as("SELECT consecutive_failures, last_error FROM sources WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(failures, 1);
        assert!(error.contains(expected), "{error}");
    }

    // 90 minutes later the healthy source is past its 60-minute interval, but the
    // failing ones have one failure each, which doubles their interval to 120 minutes.
    sqlx::query("UPDATE sources SET last_run_at = now() - interval '90 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(worker::schedule_due_sources(&pool).await.unwrap(), 1);

    // A failed fetch job is marked failed, not retried by the queue: the source reschedules it.
    let failed: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE status = 'failed'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(failed, 2);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn queue_claims_once_dedupes_and_retries_with_backoff(pool: PgPool) {
    let new = |key: &'static str| NewJob {
        kind: "test",
        dedupe_key: Some(key),
        max_attempts: Some(2),
        ..NewJob::default()
    };
    let first = jobs::enqueue(&pool, new("k")).await.unwrap();
    assert!(first.is_some());
    assert_eq!(
        jobs::enqueue(&pool, new("k")).await.unwrap(),
        None,
        "deduped while pending"
    );

    // Two concurrent claimers never get the same job.
    let (a, b) = tokio::join!(jobs::claim(&pool, "a"), jobs::claim(&pool, "b"));
    let claimed: Vec<_> = [a.unwrap(), b.unwrap()].into_iter().flatten().collect();
    assert_eq!(claimed.len(), 1);
    let job = &claimed[0];
    assert_eq!(job.attempts, 1);

    // First failure: back in the queue, but not due for 30 seconds.
    jobs::fail(&pool, job, "boom").await.unwrap();
    assert!(jobs::claim(&pool, "a").await.unwrap().is_none());
    sqlx::query("UPDATE jobs SET run_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    let retry = jobs::claim(&pool, "a")
        .await
        .unwrap()
        .expect("retry is due");
    assert_eq!(retry.attempts, 2);

    // Last attempt fails: the job is failed for good and the key is free again.
    jobs::fail(&pool, &retry, "boom again").await.unwrap();
    let (status, error): (String, String) =
        sqlx::query_as("SELECT status::text, last_error FROM jobs WHERE id = $1")
            .bind(retry.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), error.as_str()), ("failed", "boom again"));
    assert!(jobs::enqueue(&pool, new("k")).await.unwrap().is_some());
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn stale_running_jobs_are_requeued(pool: PgPool) {
    jobs::enqueue(
        &pool,
        NewJob {
            kind: "test",
            ..NewJob::default()
        },
    )
    .await
    .unwrap();
    let job = jobs::claim(&pool, "dead-worker").await.unwrap().unwrap();
    assert_eq!(
        jobs::requeue_stale(&pool, Duration::from_secs(600))
            .await
            .unwrap(),
        0
    );

    sqlx::query("UPDATE jobs SET locked_at = now() - interval '11 minutes' WHERE id = $1")
        .bind(job.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        jobs::requeue_stale(&pool, Duration::from_secs(600))
            .await
            .unwrap(),
        1
    );
    assert!(jobs::claim(&pool, "live-worker").await.unwrap().is_some());
}
