//! Fetches feed sources and stores their entries as news items.

use std::time::Duration;

use chrono::Utc;
pub use reqwest::Client;
use reqwest::{StatusCode, header};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    article_date,
    feed::{self, FeedEntry},
    tagger::{MatchKind, OrgRule, Tagger},
    url,
};

/// Largest feed body accepted.
const MAX_FEED_BYTES: usize = 10 * 1024 * 1024;
/// Entries dated further in the future than this are clamped to the fetch time.
const MAX_CLOCK_SKEW: chrono::TimeDelta = chrono::TimeDelta::hours(24);

pub fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!(
            "InferenceHubBot/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/chungquantin/inference-hub)"
        ))
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("source {0} not found")]
    SourceNotFound(Uuid),
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("feed answered HTTP {0}")]
    Status(StatusCode),
    #[error("feed is larger than {MAX_FEED_BYTES} bytes")]
    TooLarge,
    #[error(transparent)]
    Parse(#[from] feed::ParseError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// What one fetch did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FetchReport {
    /// The server said nothing changed (HTTP 304).
    pub not_modified: bool,
    pub entries: usize,
    pub inserted: usize,
    pub skipped: usize,
}

#[derive(sqlx::FromRow)]
struct SourceRow {
    id: Uuid,
    name: String,
    url: Option<String>,
    organization_id: Option<Uuid>,
    auto_approve: bool,
    etag: Option<String>,
    last_modified: Option<String>,
}

/// Fetches one source and stores new entries. Records success or failure on the source.
pub async fn fetch_source(
    pool: &PgPool,
    client: &reqwest::Client,
    source_id: Uuid,
) -> Result<FetchReport, IngestError> {
    let source: SourceRow = sqlx::query_as(
        "SELECT id, name, url, organization_id, auto_approve, etag, last_modified
         FROM sources WHERE id = $1",
    )
    .bind(source_id)
    .fetch_optional(pool)
    .await?
    .ok_or(IngestError::SourceNotFound(source_id))?;

    let result = fetch_and_store(pool, client, &source).await;
    match &result {
        Ok(_) => {
            sqlx::query(
                "UPDATE sources SET last_run_at = now(), last_success_at = now(),
                        last_error = NULL, consecutive_failures = 0
                 WHERE id = $1",
            )
            .bind(source.id)
            .execute(pool)
            .await?;
        }
        Err(err) => {
            sqlx::query(
                "UPDATE sources SET last_run_at = now(), last_error = left($2, 2000),
                        consecutive_failures = consecutive_failures + 1
                 WHERE id = $1",
            )
            .bind(source.id)
            .bind(err.to_string())
            .execute(pool)
            .await?;
        }
    }
    result
}

async fn fetch_and_store(
    pool: &PgPool,
    client: &reqwest::Client,
    source: &SourceRow,
) -> Result<FetchReport, IngestError> {
    let Some(feed_url) = source.url.as_deref() else {
        return Ok(FetchReport::default());
    };

    let mut request = client.get(feed_url).header(
        header::ACCEPT,
        "application/rss+xml, application/atom+xml, application/feed+json, application/xml;q=0.9, */*;q=0.5",
    );
    if let Some(etag) = &source.etag {
        request = request.header(header::IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = &source.last_modified {
        request = request.header(header::IF_MODIFIED_SINCE, last_modified);
    }

    let mut response = request.send().await?;
    if response.status() == StatusCode::NOT_MODIFIED {
        return Ok(FetchReport {
            not_modified: true,
            ..FetchReport::default()
        });
    }
    if !response.status().is_success() {
        return Err(IngestError::Status(response.status()));
    }
    if response
        .content_length()
        .is_some_and(|len| len as usize > MAX_FEED_BYTES)
    {
        return Err(IngestError::TooLarge);
    }

    let etag = header_str(&response, header::ETAG);
    let last_modified = header_str(&response, header::LAST_MODIFIED);
    let final_url = response.url().to_string();

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_FEED_BYTES {
            return Err(IngestError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }

    let parsed = feed::parse(&body, &final_url)?;
    let tagger = load_tagger(pool).await?;
    let publisher = parsed
        .title
        .clone()
        .filter(|t| !t.is_empty())
        .or_else(|| url::host(feed_url))
        .unwrap_or_else(|| source.name.clone());

    let mut report = FetchReport {
        entries: parsed.entries.len(),
        skipped: parsed.skipped,
        ..FetchReport::default()
    };
    for entry in &parsed.entries {
        let mut entry = entry.clone();
        if !news_exists(pool, &entry.url).await? {
            // The feed's date is a fallback: many feeds report the last edit, not the first publication.
            if let Some(date) = article_date::fetch_published(client, &entry.url).await {
                entry.published_at = Some(date);
            }
        }
        if store_entry(pool, source, &publisher, &entry, &tagger).await? {
            report.inserted += 1;
        }
    }

    // Save the validators only after the entries are stored, so a failed run refetches.
    sqlx::query("UPDATE sources SET etag = $2, last_modified = $3 WHERE id = $1")
        .bind(source.id)
        .bind(etag)
        .bind(last_modified)
        .execute(pool)
        .await?;

    Ok(report)
}

async fn news_exists(pool: &PgPool, url: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM news_items WHERE url = $1)")
        .bind(url)
        .fetch_one(pool)
        .await
}

/// What `redate_all` did.
#[derive(Debug, Default, Clone, Copy)]
pub struct RedateReport {
    pub checked: usize,
    pub found: usize,
    pub changed: usize,
}

/// Re-reads the publication date of stored news from the article pages and corrects
/// `published_at` where the page disagrees. Items whose page declares no date keep theirs.
pub async fn redate_all(
    pool: &PgPool,
    client: &reqwest::Client,
    concurrency: usize,
) -> sqlx::Result<RedateReport> {
    let items: Vec<(Uuid, String, chrono::DateTime<Utc>)> =
        sqlx::query_as("SELECT id, url, published_at FROM news_items ORDER BY published_at DESC")
            .fetch_all(pool)
            .await?;
    let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
    let mut tasks = tokio::task::JoinSet::new();
    for (id, url, stored) in items {
        let client = client.clone();
        let gate = gate.clone();
        tasks.spawn(async move {
            let _permit = gate.acquire_owned().await.ok();
            (
                id,
                stored,
                article_date::fetch_published(&client, &url).await,
            )
        });
    }
    let mut report = RedateReport::default();
    while let Some(done) = tasks.join_next().await {
        let Ok((id, stored, page)) = done else {
            continue;
        };
        report.checked += 1;
        let Some(page) = page else { continue };
        report.found += 1;
        // A date-only value is stored as noon UTC; do not rewrite a same-day timestamp.
        if (page - stored).abs() < chrono::TimeDelta::hours(24) {
            continue;
        }
        sqlx::query("UPDATE news_items SET published_at = $2 WHERE id = $1")
            .bind(id)
            .bind(page)
            .execute(pool)
            .await?;
        report.changed += 1;
        if report.checked % 200 == 0 {
            tracing::info!(?report, "redating");
        }
    }
    Ok(report)
}

fn header_str(response: &reqwest::Response, name: header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// Inserts an entry unless its URL is already stored. Returns whether it was new.
async fn store_entry(
    pool: &PgPool,
    source: &SourceRow,
    publisher: &str,
    entry: &FeedEntry,
    tagger: &Tagger,
) -> sqlx::Result<bool> {
    let now = Utc::now();
    let published_at = entry
        .published_at
        .filter(|d| *d <= now + MAX_CLOCK_SKEW)
        .unwrap_or(now);
    let status = if source.auto_approve {
        "approved"
    } else {
        "pending"
    };

    let mut tx = pool.begin().await?;
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO news_items
             (source_id, url, title, publisher, summary, image_url, published_at, status, guid, author)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::news_status, $9, $10)
         ON CONFLICT (url) DO NOTHING
         RETURNING id",
    )
    .bind(source.id)
    .bind(&entry.url)
    .bind(&entry.title)
    .bind(publisher)
    .bind(&entry.summary)
    .bind(&entry.image_url)
    .bind(published_at)
    .bind(status)
    .bind(&entry.guid)
    .bind(&entry.author)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(id) = id else {
        return Ok(false);
    };

    let mut tags: Vec<(Uuid, &str)> = tagger
        .tag(&entry.title, entry.summary.as_deref().unwrap_or(""))
        .into_iter()
        .map(|(org, kind)| {
            (
                org,
                match kind {
                    MatchKind::Title => "title",
                    MatchKind::Body => "body",
                },
            )
        })
        .collect();
    if let Some(org) = source.organization_id {
        tags.retain(|(id, _)| *id != org);
        tags.push((org, "source"));
    }
    for (org, matched_by) in tags {
        sqlx::query(
            "INSERT INTO news_item_organizations (news_item_id, organization_id, matched_by)
             VALUES ($1, $2, $3::news_match) ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(org)
        .bind(matched_by)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

#[derive(sqlx::FromRow)]
struct TaggingRow {
    id: Uuid,
    name: String,
    aliases: Vec<String>,
    exclude_phrases: Vec<String>,
    match_case: bool,
    match_name: bool,
}

/// Builds the tagger from the organizations in the database.
pub async fn load_tagger(pool: &PgPool) -> sqlx::Result<Tagger> {
    let rows: Vec<TaggingRow> = sqlx::query_as(
        "SELECT id, name, aliases, exclude_phrases, match_case, match_name FROM organizations",
    )
    .fetch_all(pool)
    .await?;
    Ok(Tagger::new(
        rows.into_iter()
            .map(|row| OrgRule {
                id: row.id,
                terms: row
                    .match_name
                    .then_some(row.name)
                    .into_iter()
                    .chain(row.aliases)
                    .collect(),
                exclude_phrases: row.exclude_phrases,
                match_case: row.match_case,
            })
            .collect(),
    ))
}

/// Re-runs the tagger on stored items, replacing their title and body matches.
/// Source and manual tags are kept. Returns how many items were processed.
pub async fn retag_all(pool: &PgPool) -> sqlx::Result<usize> {
    let tagger = load_tagger(pool).await?;
    let items: Vec<(Uuid, String, Option<String>)> =
        sqlx::query_as("SELECT id, title, summary FROM news_items")
            .fetch_all(pool)
            .await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM news_item_organizations WHERE matched_by IN ('title', 'body')")
        .execute(&mut *tx)
        .await?;
    for (id, title, summary) in &items {
        for (org, kind) in tagger.tag(title, summary.as_deref().unwrap_or("")) {
            sqlx::query(
                "INSERT INTO news_item_organizations (news_item_id, organization_id, matched_by)
                 VALUES ($1, $2, $3::news_match) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(org)
            .bind(match kind {
                MatchKind::Title => "title",
                MatchKind::Body => "body",
            })
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(items.len())
}
