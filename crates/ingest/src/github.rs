//! Syncs GitHub repository stats (stars, forks, license, latest release) for
//! open-source products.

use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode, header};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Where to reach the GitHub API, and how to authenticate.
#[derive(Debug, Clone)]
pub struct GitHub {
    pub api_url: String,
    /// A token raises the limit from 60 to 5,000 requests per hour. Optional.
    pub token: Option<String>,
}

impl GitHub {
    pub fn from_env() -> Self {
        Self {
            api_url: std::env::var("GITHUB_API_URL")
                .unwrap_or_else(|_| "https://api.github.com".to_owned())
                .trim_end_matches('/')
                .to_owned(),
            token: std::env::var("GITHUB_TOKEN")
                .ok()
                .filter(|t| !t.trim().is_empty()),
        }
    }

    fn get(&self, client: &Client, path: &str) -> reqwest::RequestBuilder {
        let request = client
            .get(format!("{}{path}", self.api_url))
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("product {0} has no GitHub repository")]
    NoRepo(Uuid),
    #[error("repository {0} was not found")]
    NotFound(String),
    #[error("GitHub rate limit reached; resets at {0}")]
    RateLimited(String),
    #[error("GitHub answered HTTP {0}")]
    Status(StatusCode),
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    Updated { stars: i32 },
    NotModified,
}

#[derive(Deserialize)]
struct RepoResponse {
    stargazers_count: i32,
    forks_count: i32,
    open_issues_count: i32,
    archived: bool,
    pushed_at: Option<DateTime<Utc>>,
    license: Option<License>,
}

#[derive(Deserialize)]
struct License {
    spdx_id: Option<String>,
}

#[derive(Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    published_at: Option<DateTime<Utc>>,
}

/// Fetches one product's repository and stores its stats. On failure the error
/// is recorded on the existing stats row, so the next sync waits its interval.
pub async fn sync_repo(
    pool: &PgPool,
    client: &Client,
    github: &GitHub,
    product_id: Uuid,
) -> Result<SyncOutcome, SyncError> {
    let result = fetch_and_store(pool, client, github, product_id).await;
    if let Err(err) = &result
        && !matches!(err, SyncError::NoRepo(_))
    {
        // Keeps earlier stats; creates an empty row when there are none yet.
        sqlx::query(
            "INSERT INTO repo_stats (product_id, synced_at, last_error)
             VALUES ($1, now(), left($2, 2000))
             ON CONFLICT (product_id) DO UPDATE SET
                 synced_at = now(), last_error = EXCLUDED.last_error",
        )
        .bind(product_id)
        .bind(err.to_string())
        .execute(pool)
        .await?;
    }
    result
}

async fn fetch_and_store(
    pool: &PgPool,
    client: &Client,
    github: &GitHub,
    product_id: Uuid,
) -> Result<SyncOutcome, SyncError> {
    let (repo, etag): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT p.github_repo, s.etag FROM products p
         LEFT JOIN repo_stats s ON s.product_id = p.id WHERE p.id = $1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();
    let repo = repo.ok_or(SyncError::NoRepo(product_id))?;

    let mut request = github.get(client, &format!("/repos/{repo}"));
    if let Some(etag) = &etag {
        request = request.header(header::IF_NONE_MATCH, etag);
    }
    let response = request.send().await?;
    check_rate_limit(&response)?;
    match response.status() {
        StatusCode::NOT_MODIFIED => {
            // Unchanged, but record today's stars so the history has no gaps.
            sqlx::query(
                "UPDATE repo_stats SET synced_at = now(), last_error = NULL WHERE product_id = $1",
            )
            .bind(product_id)
            .execute(pool)
            .await?;
            record_history(pool, product_id).await?;
            return Ok(SyncOutcome::NotModified);
        }
        StatusCode::NOT_FOUND => return Err(SyncError::NotFound(repo)),
        status if !status.is_success() => return Err(SyncError::Status(status)),
        _ => {}
    }
    let new_etag = response
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let data: RepoResponse = response.json().await?;

    // A repository without releases answers 404 here; that is not an error.
    let release_response = github
        .get(client, &format!("/repos/{repo}/releases/latest"))
        .send()
        .await?;
    check_rate_limit(&release_response)?;
    let release: Option<ReleaseResponse> = match release_response.status() {
        StatusCode::NOT_FOUND => None,
        status if status.is_success() => Some(release_response.json().await?),
        status => return Err(SyncError::Status(status)),
    };

    let license = data
        .license
        .and_then(|l| l.spdx_id)
        .filter(|id| id != "NOASSERTION");
    sqlx::query(
        "INSERT INTO repo_stats
             (product_id, stars, forks, open_issues, license, archived, pushed_at,
              latest_release, released_at, etag, synced_at, last_error)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), NULL)
         ON CONFLICT (product_id) DO UPDATE SET
             stars = EXCLUDED.stars, forks = EXCLUDED.forks,
             open_issues = EXCLUDED.open_issues, license = EXCLUDED.license,
             archived = EXCLUDED.archived, pushed_at = EXCLUDED.pushed_at,
             latest_release = EXCLUDED.latest_release, released_at = EXCLUDED.released_at,
             etag = EXCLUDED.etag, synced_at = now(), last_error = NULL",
    )
    .bind(product_id)
    .bind(data.stargazers_count)
    .bind(data.forks_count)
    .bind(data.open_issues_count)
    .bind(license)
    .bind(data.archived)
    .bind(data.pushed_at)
    .bind(release.as_ref().map(|r| r.tag_name.clone()))
    .bind(release.and_then(|r| r.published_at))
    .bind(new_etag)
    .execute(pool)
    .await?;
    record_history(pool, product_id).await?;
    Ok(SyncOutcome::Updated {
        stars: data.stargazers_count,
    })
}

/// Stores today's star count; a later sync on the same day overwrites it.
async fn record_history(pool: &PgPool, product_id: Uuid) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO repo_star_history (product_id, day, stars)
         SELECT product_id, (now() AT TIME ZONE 'UTC')::date, stars FROM repo_stats
         WHERE product_id = $1 AND stars IS NOT NULL
         ON CONFLICT (product_id, day) DO UPDATE SET stars = EXCLUDED.stars",
    )
    .bind(product_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn check_rate_limit(response: &reqwest::Response) -> Result<(), SyncError> {
    let limited = matches!(
        response.status(),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
    ) && response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        == Some("0");
    if !limited {
        return Ok(());
    }
    let reset = response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .and_then(|secs| DateTime::from_timestamp(secs, 0))
        .map_or_else(|| "an unknown time".to_owned(), |t| t.to_rfc3339());
    Err(SyncError::RateLimited(reset))
}
