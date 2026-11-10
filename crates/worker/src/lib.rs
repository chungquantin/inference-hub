//! The ingestion worker: a scheduler that enqueues due work and a pool of
//! runners that execute jobs from the Postgres queue.

use std::time::Duration;

use ih_db::jobs::{self, Job, NewJob};
pub use ih_ingest::ingest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const FETCH_SOURCE: &str = "fetch_source";

/// How long a job may run before another worker may take it over.
pub const LEASE: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchSourcePayload {
    pub source_id: Uuid,
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

/// Runs one job. Errors are recorded on the job by the caller.
pub async fn run_job(pool: &PgPool, http: &Client, job: &Job) -> anyhow::Result<()> {
    match job.kind.as_str() {
        FETCH_SOURCE => {
            let payload: FetchSourcePayload = serde_json::from_value(job.payload.clone())?;
            let report = ih_ingest::fetch_source(pool, http, payload.source_id).await?;
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
pub async fn drain(pool: &PgPool, http: &Client, worker_id: &str) -> sqlx::Result<usize> {
    let mut ran = 0;
    while let Some(job) = jobs::claim(pool, worker_id).await? {
        execute(pool, http, &job).await?;
        ran += 1;
    }
    Ok(ran)
}

async fn execute(pool: &PgPool, http: &Client, job: &Job) -> sqlx::Result<()> {
    match run_job(pool, http, job).await {
        Ok(()) => jobs::complete(pool, job.id).await,
        Err(err) => {
            tracing::warn!(job = job.id, kind = %job.kind, attempt = job.attempts, error = %err, "job failed");
            jobs::fail(pool, job, &format!("{err:#}")).await
        }
    }
}

/// Runs the scheduler and the runners until `cancel` fires, then lets running jobs finish.
pub async fn run(pool: PgPool, http: Client, config: Config, cancel: CancellationToken) {
    let mut tasks = tokio::task::JoinSet::new();

    {
        let (pool, cancel, every) = (pool.clone(), cancel.clone(), config.schedule_every);
        tasks.spawn(async move {
            let mut tick = tokio::time::interval(every);
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {}
                }
                if let Err(err) = maintenance(&pool).await {
                    tracing::error!(error = %err, "scheduler pass failed");
                }
            }
        });
    }

    for n in 0..config.concurrency.max(1) {
        let (pool, http, cancel) = (pool.clone(), http.clone(), cancel.clone());
        let worker_id = format!("{}-{n}", config.worker_id);
        let poll_every = config.poll_every;
        tasks.spawn(async move {
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                match jobs::claim(&pool, &worker_id).await {
                    Ok(Some(job)) => {
                        if let Err(err) = execute(&pool, &http, &job).await {
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

async fn maintenance(pool: &PgPool) -> sqlx::Result<()> {
    let requeued = jobs::requeue_stale(pool, LEASE).await?;
    if requeued > 0 {
        tracing::warn!(requeued, "requeued jobs whose lease expired");
    }
    let scheduled = schedule_due_sources(pool).await?;
    if scheduled > 0 {
        tracing::info!(scheduled, "scheduled source fetches");
    }
    jobs::prune_done(pool, Duration::from_secs(7 * 24 * 3600)).await?;
    Ok(())
}
