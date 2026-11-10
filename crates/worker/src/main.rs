//! Ingestion worker. Phase 2 adds the job queue and the RSS, GitHub and pricing
//! sources; for now it only connects and reports how many sources are enabled.

use anyhow::Context;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = ih_db::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    let enabled: i64 = ih_db::sqlx::query_scalar("SELECT count(*) FROM sources WHERE enabled")
        .fetch_one(&pool)
        .await
        .context("reading sources (have migrations run?)")?;
    tracing::info!(
        enabled_sources = enabled,
        "worker ready; no jobs are implemented yet"
    );
    Ok(())
}
