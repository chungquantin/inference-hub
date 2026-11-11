//! The ingestion worker: a scheduler that enqueues due work and a pool of
//! runners that execute jobs from the Postgres queue.

pub mod health;

use std::sync::Arc;
use std::time::Duration;

use ih_db::jobs::{self, Job, NewJob};
pub use ih_ingest::adapters::{Endpoints, MarketAdapter, market_adapters};
pub use ih_ingest::{github::GitHub, ingest::Client};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const FETCH_SOURCE: &str = "fetch_source";
pub const SYNC_REPO: &str = "sync_repo";
pub const RUN_ADAPTER: &str = "run_adapter";

fn market_adapters_from(adapters: Arc<Vec<MarketAdapter>>) -> Vec<MarketAdapter> {
    Arc::try_unwrap(adapters).unwrap_or_else(|_| panic!("adapters are shared"))
}

/// How long a job may run before another worker may take it over.
pub const LEASE: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchSourcePayload {
    pub source_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRepoPayload {
    pub product_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunAdapterPayload {
    pub adapter: String,
}

/// What jobs need besides the database.
#[derive(Clone)]
pub struct Context {
    pub http: Client,
    pub github: GitHub,
    /// How long repository stats stay fresh.
    pub repo_sync_every: Duration,
    /// Market adapters, shared by every runner.
    pub adapters: Arc<Vec<MarketAdapter>>,
    /// Runs every market adapter at this interval instead of its own (tests).
    pub adapter_every: Option<Duration>,
    /// Counters served on `/metrics`.
    pub metrics: Arc<health::Metrics>,
}

impl Context {
    pub fn new(http: Client, github: GitHub) -> Self {
        Self::with_endpoints(http, github, &Endpoints::default())
    }

    /// Points the market adapters at other base URLs (tests use local servers).
    pub fn with_endpoints(http: Client, github: GitHub, endpoints: &Endpoints) -> Self {
        Self {
            http,
            github,
            repo_sync_every: Duration::from_secs(6 * 3600),
            adapters: Arc::new(market_adapters(endpoints)),
            adapter_every: None,
            metrics: Arc::default(),
        }
    }

    /// Keeps only the adapters with these ids (tests that fake some providers).
    pub fn only_adapters(mut self, ids: &[&str]) -> Self {
        let adapters = market_adapters_from(std::mem::take(&mut self.adapters));
        self.adapters = Arc::new(
            adapters
                .into_iter()
                .filter(|a| ids.contains(&a.info().id()))
                .collect(),
        );
        self
    }

    fn adapter(&self, id: &str) -> Option<&MarketAdapter> {
        self.adapters.iter().find(|a| a.info().id() == id)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub worker_id: String,
    /// Jobs run at the same time.
    pub concurrency: usize,
    /// How often the scheduler looks for due sources.
    pub schedule_every: Duration,
    /// How long an idle runner sleeps before polling again.
    pub poll_every: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            worker_id: format!("worker-{}", std::process::id()),
            concurrency: 4,
            schedule_every: Duration::from_secs(60),
            poll_every: Duration::from_secs(2),
        }
    }
}

/// Enqueues a fetch for every enabled feed source that is due.
///
/// A source is due when it never ran or its interval passed since the last run.
/// Sources that keep failing back off: the interval doubles per consecutive
/// failure, up to 24 hours. The dedupe key keeps one pending fetch per source.
pub async fn schedule_due_sources(pool: &PgPool) -> sqlx::Result<usize> {
    let due: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM sources
         WHERE enabled AND source_type = 'rss' AND url IS NOT NULL
           AND (last_run_at IS NULL
                OR last_run_at + make_interval(mins => least(
                       fetch_interval_minutes * power(2, least(consecutive_failures, 10))::int,
                       24 * 60)) <= now())",
    )
    .fetch_all(pool)
    .await?;

    let mut enqueued = 0;
    for source_id in due {
        let key = format!("{FETCH_SOURCE}:{source_id}");
        let id = jobs::enqueue(
            pool,
            NewJob {
                kind: FETCH_SOURCE,
                payload: serde_json::to_value(FetchSourcePayload { source_id }).unwrap_or_default(),
                dedupe_key: Some(&key),
                // The source records its own failures and is rescheduled, so one try is enough.
                max_attempts: Some(1),
                ..NewJob::default()
            },
        )
        .await?;
        enqueued += usize::from(id.is_some());
    }
    Ok(enqueued)
}

/// Enqueues a sync for every product with a GitHub repository whose stats are
/// missing or older than `every`.
pub async fn schedule_due_repos(pool: &PgPool, every: Duration) -> sqlx::Result<usize> {
    let due: Vec<Uuid> = sqlx::query_scalar(
        "SELECT p.id FROM products p LEFT JOIN repo_stats s ON s.product_id = p.id
         WHERE p.github_repo IS NOT NULL
           AND (s.synced_at IS NULL OR s.synced_at <= now() - make_interval(secs => $1))",
    )
    .bind(every.as_secs_f64())
    .fetch_all(pool)
    .await?;

    let mut enqueued = 0;
    for product_id in due {
        let key = format!("{SYNC_REPO}:{product_id}");
        let id = jobs::enqueue(
            pool,
            NewJob {
                kind: SYNC_REPO,
                payload: serde_json::to_value(SyncRepoPayload { product_id }).unwrap_or_default(),
                dedupe_key: Some(&key),
                max_attempts: Some(1),
                ..NewJob::default()
            },
        )
        .await?;
        enqueued += usize::from(id.is_some());
    }
    Ok(enqueued)
}

/// Enqueues a run of every market adapter that has not run within its interval.
/// A failing adapter backs off like a feed: the interval doubles per failure, up to a day.
pub async fn schedule_due_adapters(pool: &PgPool, ctx: &Context) -> sqlx::Result<usize> {
    ih_ingest::market::register_adapters(pool, &ctx.adapters).await?;
    let mut enqueued = 0;
    for adapter in ctx.adapters.iter() {
        let id = adapter.info().id();
        let due: bool = sqlx::query_scalar(
            "SELECT coalesce((
                 SELECT last_run_at + make_interval(secs => least(
                            $2 * power(2, least(consecutive_failures, 10)), 86400)) <= now()
                 FROM adapter_state WHERE adapter = $1), true)",
        )
        .bind(id)
        .bind(
            ctx.adapter_every
                .unwrap_or_else(|| adapter.info().interval())
                .as_secs_f64(),
        )
        .fetch_one(pool)
        .await?;
        if !due {
            continue;
        }
        let key = format!("{RUN_ADAPTER}:{id}");
        let job = jobs::enqueue(
            pool,
            NewJob {
                kind: RUN_ADAPTER,
                payload: serde_json::to_value(RunAdapterPayload {
                    adapter: id.to_owned(),
                })
                .unwrap_or_default(),
                dedupe_key: Some(&key),
                max_attempts: Some(1),
                ..NewJob::default()
            },
        )
        .await?;
        enqueued += usize::from(job.is_some());
    }
    Ok(enqueued)
}

/// Schedules every kind of due work. Returns how many jobs were enqueued.
pub async fn schedule_all(pool: &PgPool, ctx: &Context) -> sqlx::Result<usize> {
    Ok(schedule_due_sources(pool).await?
        + schedule_due_repos(pool, ctx.repo_sync_every).await?
        + schedule_due_adapters(pool, ctx).await?)
}

/// Runs one job. Errors are recorded on the job by the caller.
pub async fn run_job(pool: &PgPool, ctx: &Context, job: &Job) -> anyhow::Result<()> {
    match job.kind.as_str() {
        RUN_ADAPTER => {
            let payload: RunAdapterPayload = serde_json::from_value(job.payload.clone())?;
            let adapter = ctx
                .adapter(&payload.adapter)
                .ok_or_else(|| anyhow::anyhow!("unknown adapter `{}`", payload.adapter))?;
            let report = ih_ingest::market::run_adapter(pool, &ctx.http, adapter).await?;
            tracing::info!(job = job.id, adapter = %payload.adapter, ?report, "ran market adapter");
            Ok(())
        }
        SYNC_REPO => {
            let payload: SyncRepoPayload = serde_json::from_value(job.payload.clone())?;
            let outcome =
                ih_ingest::github::sync_repo(pool, &ctx.http, &ctx.github, payload.product_id)
                    .await?;
            tracing::info!(job = job.id, product = %payload.product_id, ?outcome, "synced repository");
            Ok(())
        }
        FETCH_SOURCE => {
            let payload: FetchSourcePayload = serde_json::from_value(job.payload.clone())?;
            let report = ih_ingest::fetch_source(pool, &ctx.http, payload.source_id).await?;
            tracing::info!(
                job = job.id,
                source = %payload.source_id,
                not_modified = report.not_modified,
                entries = report.entries,
                inserted = report.inserted,
                "fetched source"
            );
            Ok(())
        }
        other => anyhow::bail!("unknown job kind `{other}`"),
    }
}

/// Claims and runs jobs until there are none due. Returns how many ran. Used by
/// `--once` and by tests.
pub async fn drain(pool: &PgPool, ctx: &Context, worker_id: &str) -> sqlx::Result<usize> {
    let mut ran = 0;
    while let Some(job) = jobs::claim(pool, worker_id).await? {
        execute(pool, ctx, &job).await?;
        ran += 1;
    }
    Ok(ran)
}

async fn execute(pool: &PgPool, ctx: &Context, job: &Job) -> sqlx::Result<()> {
    match run_job(pool, ctx, job).await {
        Ok(()) => {
            ctx.metrics.job_finished(&job.kind, true);
            jobs::complete(pool, job.id).await
        }
        Err(err) => {
            ctx.metrics.job_finished(&job.kind, false);
            tracing::warn!(job = job.id, kind = %job.kind, attempt = job.attempts, error = %err, "job failed");
            jobs::fail(pool, job, &format!("{err:#}")).await
        }
    }
}

/// Runs the scheduler and the runners until `cancel` fires, then lets running jobs finish.
pub async fn run(pool: PgPool, ctx: Context, config: Config, cancel: CancellationToken) {
    let mut tasks = tokio::task::JoinSet::new();

    {
        let (pool, ctx, cancel, every) = (
            pool.clone(),
            ctx.clone(),
            cancel.clone(),
            config.schedule_every,
        );
        tasks.spawn(async move {
            let mut tick = tokio::time::interval(every);
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {}
                }
                if let Err(err) = maintenance(&pool, &ctx).await {
                    tracing::error!(error = %err, "scheduler pass failed");
                }
            }
        });
    }

    for n in 0..config.concurrency.max(1) {
        let (pool, ctx, cancel) = (pool.clone(), ctx.clone(), cancel.clone());
        let worker_id = format!("{}-{n}", config.worker_id);
        let poll_every = config.poll_every;
        tasks.spawn(async move {
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                match jobs::claim(&pool, &worker_id).await {
                    Ok(Some(job)) => {
                        if let Err(err) = execute(&pool, &ctx, &job).await {
                            tracing::error!(job = job.id, error = %err, "could not record job result");
                        }
                    }
                    Ok(None) => {
                        tokio::select! {
                            () = cancel.cancelled() => break,
                            () = tokio::time::sleep(poll_every) => {}
                        }
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "could not claim a job");
                        tokio::select! {
                            () = cancel.cancelled() => break,
                            () = tokio::time::sleep(poll_every * 5) => {}
                        }
                    }
                }
            }
        });
    }

    while tasks.join_next().await.is_some() {}
}

async fn maintenance(pool: &PgPool, ctx: &Context) -> sqlx::Result<()> {
    let requeued = jobs::requeue_stale(pool, LEASE).await?;
    if requeued > 0 {
        tracing::warn!(requeued, "requeued jobs whose lease expired");
    }
    let scheduled = schedule_all(pool, ctx).await?;
    if scheduled > 0 {
        tracing::info!(scheduled, "scheduled jobs");
    }
    jobs::prune_done(pool, Duration::from_secs(7 * 24 * 3600)).await?;
    ctx.metrics.schedule_ok();
    Ok(())
}
