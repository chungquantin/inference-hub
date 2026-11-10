//! A small job queue on Postgres.
//!
//! Workers claim jobs with `FOR UPDATE SKIP LOCKED`, so several workers can run
//! at once without taking the same job. A failed job is retried with
//! exponential backoff until `max_attempts`, then marked `failed`. A job whose
//! worker died is put back in the queue once its lock is older than the lease.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub payload: Value,
    /// Attempts including the current one.
    pub attempts: i32,
    pub max_attempts: i32,
}

/// Options for [`enqueue`].
#[derive(Debug, Clone, Default)]
pub struct NewJob<'a> {
    pub kind: &'a str,
    pub payload: Value,
    /// While a queued or running job has this key, enqueueing another is a no-op.
    pub dedupe_key: Option<&'a str>,
    pub run_at: Option<DateTime<Utc>>,
    pub max_attempts: Option<i32>,
}

/// Adds a job. Returns its id, or `None` when a job with the same dedupe key is pending.
pub async fn enqueue(pool: &PgPool, job: NewJob<'_>) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar(
        "INSERT INTO jobs (kind, payload, dedupe_key, run_at, max_attempts)
         VALUES ($1, $2, $3, coalesce($4, now()), coalesce($5, 5))
         ON CONFLICT (dedupe_key) WHERE status IN ('queued', 'running') DO NOTHING
         RETURNING id",
    )
    .bind(job.kind)
    .bind(job.payload)
    .bind(job.dedupe_key)
    .bind(job.run_at)
    .bind(job.max_attempts)
    .fetch_optional(pool)
    .await
}

/// Claims the next due job, or returns `None` when there is nothing to do.
pub async fn claim(pool: &PgPool, worker_id: &str) -> sqlx::Result<Option<Job>> {
    sqlx::query_as(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1,
                         locked_at = now(), locked_by = $1
         WHERE id = (
             SELECT id FROM jobs
             WHERE status = 'queued' AND run_at <= now()
             ORDER BY run_at, id
             LIMIT 1
             FOR UPDATE SKIP LOCKED)
         RETURNING id, kind, payload, attempts, max_attempts",
    )
    .bind(worker_id)
    .fetch_optional(pool)
    .await
}

pub async fn complete(pool: &PgPool, id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'done', locked_at = NULL, locked_by = NULL, last_error = NULL
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Records a failure: requeues with backoff, or marks the job failed after its last attempt.
pub async fn fail(pool: &PgPool, job: &Job, error: &str) -> sqlx::Result<()> {
    let retry = job.attempts < job.max_attempts;
    sqlx::query(
        "UPDATE jobs SET
             status = CASE WHEN $2 THEN 'queued'::job_status ELSE 'failed'::job_status END,
             run_at = CASE WHEN $2 THEN now() + make_interval(secs => $3) ELSE run_at END,
             locked_at = NULL, locked_by = NULL, last_error = left($4, 2000)
         WHERE id = $1",
    )
    .bind(job.id)
    .bind(retry)
    .bind(backoff(job.attempts).as_secs_f64())
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Puts jobs whose lock is older than `lease` back in the queue. Returns how many.
pub async fn requeue_stale(pool: &PgPool, lease: Duration) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE jobs SET status = 'queued', locked_at = NULL, locked_by = NULL,
                         last_error = 'lease expired'
         WHERE status = 'running' AND locked_at < now() - make_interval(secs => $1)",
    )
    .bind(lease.as_secs_f64())
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Deletes finished jobs older than `age`. Failed jobs are kept for inspection.
pub async fn prune_done(pool: &PgPool, age: Duration) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM jobs WHERE status = 'done' AND updated_at < now() - make_interval(secs => $1)",
    )
    .bind(age.as_secs_f64())
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// 30s, 2m, 8m, 32m, … capped at 6h.
pub fn backoff(attempts: i32) -> Duration {
    let exp = attempts.clamp(1, 10) as u32 - 1;
    Duration::from_secs((30u64 * 4u64.pow(exp)).min(6 * 3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(30));
        assert_eq!(backoff(2), Duration::from_secs(120));
        assert_eq!(backoff(3), Duration::from_secs(480));
        assert_eq!(backoff(10), Duration::from_secs(6 * 3600));
        assert_eq!(backoff(0), Duration::from_secs(30));
    }
}
