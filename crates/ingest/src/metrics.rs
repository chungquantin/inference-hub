//! Metric sources: public APIs that report numbers about players in the market,
//! beyond prices. Each source returns [`Sample`]s; [`run_source`] stores them as
//! one point per day per series.
//!
//! Sources:
//! - Hugging Face Hub: downloads and likes of the most downloaded models, per lab and per model.
//! - Hugging Face Inference Providers: output speed and time to first token per provider and model.
//! - GitHub: stars and forks of the directory's open-source projects, from the repository sync.

use std::{collections::HashMap, time::Duration};

use chrono::{NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;

use crate::adapters::{AdapterError, get_text};

/// One reading of a metric for one subject.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub metric: &'static str,
    pub subject_type: &'static str,
    pub subject_key: String,
    pub subject_name: String,
    /// Directory slug of the organization the subject belongs to, if known.
    pub organization_slug: Option<String>,
    pub day: NaiveDate,
    pub value: f64,
}

/// A source's run: the samples, and whether they may fill in history.
pub struct Collected {
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone, Copy)]
pub enum Source {
    HuggingFaceHub,
    HuggingFaceSpeed,
    GitHub,
}

pub const SOURCES: &[Source] = &[
    Source::HuggingFaceHub,
    Source::HuggingFaceSpeed,
    Source::GitHub,
];

impl Source {
    pub fn id(self) -> &'static str {
        match self {
            Source::HuggingFaceHub => "huggingface-hub",
            Source::HuggingFaceSpeed => "huggingface-speed",
            Source::GitHub => "github-metrics",
        }
    }

    /// How often the worker runs the source.
    pub fn interval(self) -> Duration {
        match self {
            Source::GitHub => Duration::from_secs(24 * 3600),
            _ => Duration::from_secs(12 * 3600),
        }
    }
}

/// Where the sources call, so tests can point them at a local server.
#[derive(Debug, Clone)]
pub struct Origins {
    pub huggingface: String,
    pub router: String,
}

impl Default for Origins {
    fn default() -> Self {
        Self {
            huggingface: "https://huggingface.co".into(),
            router: "https://router.huggingface.co".into(),
        }
    }
}

pub async fn collect(
    source: Source,
    client: &Client,
    origins: &Origins,
) -> Result<Collected, AdapterError> {
    let samples = match source {
        Source::HuggingFaceHub => hub(client, &origins.huggingface).await?,
        Source::HuggingFaceSpeed => speed(client, &origins.router).await?,
        Source::GitHub => {
            return Ok(Collected {
                samples: Vec::new(),
            });
        }
    };
    Ok(Collected { samples })
}

// ---------------------------------------------------------------------------------------
// Hugging Face Hub

/// Hub author to directory slug, where the slug is not the lower-cased author.
const AUTHORS: &[(&str, &str)] = &[
    ("Qwen", "alibaba-qwen"),
    ("deepseek-ai", "deepseek"),
    ("CohereLabs", "cohere"),
    ("CohereForAI", "cohere"),
    ("moonshotai", "moonshot-ai"),
    ("meta-llama", "meta"),
    ("facebook", "meta"),
    ("MiniMaxAI", "minimax"),
    ("zai-org", "z-ai"),
    ("THUDM", "z-ai"),
    ("mistralai", "mistral-ai"),
    ("stabilityai", "stability-ai"),
    ("allenai", "allen-institute-for-ai"),
    ("ibm-granite", "ibm"),
    ("xai-org", "xai"),
    ("ByteDance-Seed", "bytedance-seed"),
    ("tencent", "tencent"),
    ("nvidia", "nvidia"),
    ("microsoft", "microsoft"),
    ("google", "google"),
    ("openai", "openai"),
    ("black-forest-labs", "black-forest-labs"),
    ("01-ai", "01-ai"),
];

fn author_slug(author: &str) -> String {
    AUTHORS
        .iter()
        .find(|(a, _)| a.eq_ignore_ascii_case(author))
        .map(|(_, s)| (*s).to_owned())
        .unwrap_or_else(|| author.to_ascii_lowercase())
}

#[derive(Deserialize)]
struct HubModel {
    id: String,
    downloads: Option<f64>,
    likes: Option<f64>,
}

async fn hub(client: &Client, origin: &str) -> Result<Vec<Sample>, AdapterError> {
    const ID: &str = "huggingface-hub";
    let url = format!("{origin}/api/models?sort=downloads&direction=-1&limit=1000");
    let models: Vec<HubModel> = serde_json::from_str(&get_text(client, ID, &url).await?)
        .map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    Ok(hub_samples(&models, Utc::now().date_naive()))
}

fn hub_samples(models: &[HubModel], day: NaiveDate) -> Vec<Sample> {
    let mut by_author: HashMap<&str, (f64, f64)> = HashMap::new();
    let mut out = Vec::new();
    for m in models {
        let Some((author, _)) = m.id.split_once('/') else {
            continue;
        };
        let entry = by_author.entry(author).or_default();
        entry.0 += m.downloads.unwrap_or(0.0);
        entry.1 += m.likes.unwrap_or(0.0);
    }
    for m in models.iter().take(200) {
        let Some((author, _)) = m.id.split_once('/') else {
            continue;
        };
        if let Some(d) = m.downloads.filter(|d| *d > 0.0) {
            out.push(Sample {
                metric: "model_downloads_30d",
                subject_type: "model",
                subject_key: m.id.clone(),
                subject_name: m.id.clone(),
                organization_slug: Some(author_slug(author)),
                day,
                value: d,
            });
        }
    }
    for (author, (downloads, likes)) in by_author {
        for (metric, value) in [("hf_downloads_30d", downloads), ("hf_likes", likes)] {
            if value > 0.0 {
                out.push(Sample {
                    metric,
                    subject_type: "organization",
                    subject_key: author.to_owned(),
                    subject_name: author.to_owned(),
                    organization_slug: Some(author_slug(author)),
                    day,
                    value,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Hugging Face Inference Providers: speed

/// Router provider id to directory slug, where they differ.
const ROUTER_PROVIDERS: &[(&str, &str)] = &[
    ("novita", "novita-ai"),
    ("featherless-ai", "featherless"),
    ("together", "together-ai"),
    ("zai-org", "z-ai"),
    ("cohere", "cohere"),
    ("ovhcloud", "ovhcloud"),
];

fn provider_slug(provider: &str) -> String {
    ROUTER_PROVIDERS
        .iter()
        .find(|(p, _)| *p == provider)
        .map(|(_, s)| (*s).to_owned())
        .unwrap_or_else(|| provider.to_owned())
}

#[derive(Deserialize)]
struct RouterList {
    data: Vec<RouterModel>,
}

#[derive(Deserialize)]
struct RouterModel {
    id: String,
    #[serde(default)]
    providers: Vec<RouterProvider>,
}

#[derive(Deserialize)]
struct RouterProvider {
    provider: String,
    status: Option<String>,
    first_token_latency_ms: Option<f64>,
    throughput: Option<f64>,
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    })
}

async fn speed(client: &Client, origin: &str) -> Result<Vec<Sample>, AdapterError> {
    const ID: &str = "huggingface-speed";
    let body = get_text(client, ID, &format!("{origin}/v1/models")).await?;
    let list: RouterList =
        serde_json::from_str(&body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    Ok(speed_samples(&list, Utc::now().date_naive()))
}

fn speed_samples(list: &RouterList, day: NaiveDate) -> Vec<Sample> {
    let mut tps: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut ttft: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut out = Vec::new();
    for m in &list.data {
        let mut best: Option<f64> = None;
        for p in m
            .providers
            .iter()
            .filter(|p| p.status.as_deref() == Some("live"))
        {
            if let Some(t) = p.throughput.filter(|t| *t > 0.0) {
                tps.entry(&p.provider).or_default().push(t);
                best = Some(best.map_or(t, |b: f64| b.max(t)));
            }
            if let Some(t) = p.first_token_latency_ms.filter(|t| *t > 0.0) {
                ttft.entry(&p.provider).or_default().push(t);
            }
        }
        if let Some(best) = best {
            out.push(Sample {
                metric: "model_throughput",
                subject_type: "model",
                subject_key: m.id.clone(),
                subject_name: m.id.clone(),
                organization_slug: None,
                day,
                value: (best * 10.0).round() / 10.0,
            });
        }
    }
    for (metric, map) in [("provider_throughput", tps), ("provider_first_token", ttft)] {
        for (provider, mut values) in map {
            if let Some(v) = median(&mut values) {
                out.push(Sample {
                    metric,
                    subject_type: "organization",
                    subject_key: provider.to_owned(),
                    subject_name: provider.to_owned(),
                    organization_slug: Some(provider_slug(provider)),
                    day,
                    value: (v * 10.0).round() / 10.0,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Store

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error(transparent)]
    Source(#[from] AdapterError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RunReport {
    pub series: usize,
    pub points: usize,
}

/// Runs one source and stores what it returned. Records the outcome so failing
/// sources back off.
pub async fn run_source(
    pool: &PgPool,
    client: &Client,
    origins: &Origins,
    source: Source,
) -> Result<RunReport, MetricsError> {
    let result = async {
        let mut report = match source {
            Source::GitHub => github(pool).await?,
            _ => {
                store(
                    pool,
                    source.id(),
                    collect(source, client, origins).await?.samples,
                )
                .await?
            }
        };
        if matches!(source, Source::GitHub) {
            report.points = report.series;
        }
        Ok::<_, MetricsError>(report)
    }
    .await;
    match &result {
        Ok(r) => {
            sqlx::query(
                "INSERT INTO metric_source_state (source, last_run_at, last_success_at, series)
                 VALUES ($1, now(), now(), $2)
                 ON CONFLICT (source) DO UPDATE SET last_run_at = now(), last_success_at = now(),
                     consecutive_failures = 0, last_error = NULL, series = $2",
            )
            .bind(source.id())
            .bind(i32::try_from(r.series).unwrap_or(i32::MAX))
            .execute(pool)
            .await?;
        }
        Err(e) => {
            sqlx::query(
                "INSERT INTO metric_source_state (source, last_run_at, consecutive_failures, last_error)
                 VALUES ($1, now(), 1, $2)
                 ON CONFLICT (source) DO UPDATE SET last_run_at = now(),
                     consecutive_failures = metric_source_state.consecutive_failures + 1,
                     last_error = $2",
            )
            .bind(source.id())
            .bind(e.to_string())
            .execute(pool)
            .await?;
        }
    }
    result
}

async fn store(
    pool: &PgPool,
    source: &str,
    samples: Vec<Sample>,
) -> Result<RunReport, MetricsError> {
    let slugs: HashMap<String, uuid::Uuid> =
        sqlx::query_as::<_, (String, uuid::Uuid)>("SELECT slug, id FROM organizations")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let mut tx = pool.begin().await?;
    let mut series_ids: HashMap<(&'static str, &'static str, String), i64> = HashMap::new();
    let mut points = 0;
    for s in &samples {
        let key = (s.metric, s.subject_type, s.subject_key.clone());
        let id = match series_ids.get(&key) {
            Some(id) => *id,
            None => {
                let org = s
                    .organization_slug
                    .as_ref()
                    .and_then(|slug| slugs.get(slug))
                    .copied();
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO metric_series
                         (metric, subject_type, subject_key, subject_name, organization_id, source,
                          latest_value, latest_day)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                     ON CONFLICT (metric, subject_type, subject_key) DO UPDATE SET
                         subject_name = EXCLUDED.subject_name,
                         organization_id = coalesce(EXCLUDED.organization_id, metric_series.organization_id)
                     RETURNING id",
                )
                .bind(s.metric)
                .bind(s.subject_type)
                .bind(&s.subject_key)
                .bind(&s.subject_name)
                .bind(org)
                .bind(source)
                .bind(s.value)
                .bind(s.day)
                .fetch_one(&mut *tx)
                .await?;
                series_ids.insert(key, id);
                id
            }
        };
        sqlx::query(
            "INSERT INTO metric_points (series_id, day, value) VALUES ($1, $2, $3)
             ON CONFLICT (series_id, day) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(id)
        .bind(s.day)
        .bind(s.value)
        .execute(&mut *tx)
        .await?;
        points += 1;
    }
    // The latest value is the newest point, whatever order the source listed them in.
    sqlx::query(
        "UPDATE metric_series s SET latest_value = l.value, latest_day = l.day
         FROM (SELECT DISTINCT ON (series_id) series_id, day, value FROM metric_points
               WHERE series_id = ANY($1) ORDER BY series_id, day DESC) l
         WHERE s.id = l.series_id",
    )
    .bind(series_ids.values().copied().collect::<Vec<_>>())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(RunReport {
        series: series_ids.len(),
        points,
    })
}

/// Stars and forks come from the repository sync, which already ran: copy them into series.
async fn github(pool: &PgPool) -> Result<RunReport, MetricsError> {
    type RepoRow = (String, String, uuid::Uuid, Option<i32>, Option<i32>);
    let rows: Vec<RepoRow> = sqlx::query_as(
        "SELECT p.slug, p.name, p.organization_id, r.stars, r.forks
         FROM repo_stats r JOIN products p ON p.id = r.product_id
         WHERE r.stars IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    let today = Utc::now().date_naive();
    let mut tx = pool.begin().await?;
    let mut series = 0;
    for (slug, name, org, stars, forks) in rows {
        for (metric, value) in [("github_stars", stars), ("github_forks", forks)] {
            let Some(value) = value.filter(|v| *v > 0) else {
                continue;
            };
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO metric_series
                     (metric, subject_type, subject_key, subject_name, organization_id, source,
                      latest_value, latest_day)
                 VALUES ($1, 'project', $2, $3, $4, 'github-metrics', $5, $6)
                 ON CONFLICT (metric, subject_type, subject_key) DO UPDATE SET
                     subject_name = EXCLUDED.subject_name, organization_id = EXCLUDED.organization_id,
                     latest_value = EXCLUDED.latest_value, latest_day = EXCLUDED.latest_day
                 RETURNING id",
            )
            .bind(metric)
            .bind(&slug)
            .bind(&name)
            .bind(org)
            .bind(f64::from(value))
            .bind(today)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO metric_points (series_id, day, value) VALUES ($1, $2, $3)
                 ON CONFLICT (series_id, day) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(id)
            .bind(today)
            .bind(f64::from(value))
            .execute(&mut *tx)
            .await?;
            series += 1;
        }
    }
    // Star history the repository sync kept: fill the days before today.
    sqlx::query(
        "INSERT INTO metric_points (series_id, day, value)
         SELECT s.id, h.day, h.stars FROM repo_star_history h
         JOIN products p ON p.id = h.product_id
         JOIN metric_series s ON s.metric = 'github_stars' AND s.subject_type = 'project'
                             AND s.subject_key = p.slug
         WHERE h.day < $1
         ON CONFLICT (series_id, day) DO NOTHING",
    )
    .bind(today)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(RunReport {
        series,
        points: series,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 12).unwrap()
    }

    #[test]
    fn hub_sums_downloads_per_author_and_maps_aliases() {
        let models: Vec<HubModel> = serde_json::from_str(
            r#"[{"id":"Qwen/Qwen3-8B","downloads":100,"likes":5},
                {"id":"Qwen/Qwen3-32B","downloads":50,"likes":1},
                {"id":"gpt2","downloads":9,"likes":1}]"#,
        )
        .unwrap();
        let samples = hub_samples(&models, day());
        let qwen = samples
            .iter()
            .find(|s| s.metric == "hf_downloads_30d" && s.subject_key == "Qwen")
            .unwrap();
        assert_eq!(qwen.value, 150.0);
        assert_eq!(qwen.organization_slug.as_deref(), Some("alibaba-qwen"));
        assert_eq!(
            samples
                .iter()
                .filter(|s| s.metric == "model_downloads_30d")
                .count(),
            2
        );
    }

    #[test]
    fn speed_takes_provider_medians_and_the_fastest_model_speed() {
        let list: RouterList = serde_json::from_str(
            r#"{"data":[
              {"id":"a/m1","providers":[
                {"provider":"novita","status":"live","first_token_latency_ms":100,"throughput":50},
                {"provider":"groq","status":"live","first_token_latency_ms":20,"throughput":400}]},
              {"id":"a/m2","providers":[
                {"provider":"novita","status":"live","first_token_latency_ms":300,"throughput":30},
                {"provider":"groq","status":"offline","first_token_latency_ms":1,"throughput":9999}]}]}"#,
        )
        .unwrap();
        let s = speed_samples(&list, day());
        let get = |metric: &str, key: &str| {
            s.iter()
                .find(|s| s.metric == metric && s.subject_key == key)
                .map(|s| s.value)
        };
        assert_eq!(get("provider_throughput", "novita"), Some(40.0));
        assert_eq!(get("provider_first_token", "novita"), Some(200.0));
        assert_eq!(get("provider_throughput", "groq"), Some(400.0));
        assert_eq!(get("model_throughput", "a/m1"), Some(400.0));
        assert_eq!(get("model_throughput", "a/m2"), Some(30.0));
        let novita = s.iter().find(|s| s.subject_key == "novita").unwrap();
        assert_eq!(novita.organization_slug.as_deref(), Some("novita-ai"));
    }
}
