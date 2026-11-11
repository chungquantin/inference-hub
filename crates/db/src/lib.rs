//! Postgres access: connection pool, migrations and queries.

pub mod categories;
pub mod compare;
pub mod jobs;
pub mod news;
pub mod organizations;
pub mod products;
pub mod sitemap;

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
pub use sqlx::{self, PgPool};

/// Migrations embedded from the workspace `migrations/` directory.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

/// Connects, retrying with backoff while the database is not reachable yet.
///
/// In a deployment the database may still be starting, or restarting after a
/// redeploy (a volume can only be mounted by one Postgres at a time), when the
/// API or the worker boots. Gives up after `deadline`.
pub async fn connect_with_retry(
    database_url: &str,
    deadline: Duration,
) -> Result<PgPool, sqlx::Error> {
    let started = std::time::Instant::now();
    let mut wait = Duration::from_millis(500);
    loop {
        match connect(database_url).await {
            Ok(pool) => return Ok(pool),
            Err(err) if started.elapsed() + wait < deadline && is_transient(&err) => {
                tracing::warn!(error = %err, retry_in = ?wait, "database not reachable yet");
                tokio::time::sleep(wait).await;
                wait = (wait * 2).min(Duration::from_secs(10));
            }
            Err(err) => return Err(err),
        }
    }
}

/// Errors worth retrying: the server is down or refusing connections, not a
/// wrong password or a bad URL.
fn is_transient(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::Tls(_) => true,
        // 57P03: the database system is starting up.
        sqlx::Error::Database(db) => db.code().as_deref() == Some("57P03"),
        _ => false,
    }
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}
