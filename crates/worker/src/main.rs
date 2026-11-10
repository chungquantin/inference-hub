//! `worker [--once] [--retag]`
//!
//! Without flags, runs the scheduler and job runners until SIGINT or SIGTERM.
//! `--once` schedules due feed fetches and repository syncs, runs them and exits.
//! `--retag` re-runs the organization tagger on all stored news and exits.

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use worker::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = ih_db::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    ih_db::migrate(&pool).await.context("running migrations")?;
    let http = ih_ingest::http_client().context("building the HTTP client")?;
    let github = worker::GitHub::from_env();
    if github.token.is_none() {
        tracing::info!("GITHUB_TOKEN is not set; GitHub allows 60 requests per hour");
    }
    let ctx = worker::Context::new(http, github);

    if flag("--retag") {
        let items = ih_ingest::retag_all(&pool).await?;
        tracing::info!(items, "retagged news");
        return Ok(());
    }

    let config = Config {
        concurrency: env_parse("WORKER_CONCURRENCY", 4),
        ..Config::default()
    };

    if flag("--once") {
        let scheduled = worker::schedule_all(&pool, &ctx).await?;
        let ran = worker::drain(&pool, &ctx, &config.worker_id).await?;
        tracing::info!(scheduled, ran, "done");
        return Ok(());
    }

    let cancel = CancellationToken::new();
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            shutdown_signal().await;
            tracing::info!("shutting down after running jobs finish");
            cancel.cancel();
        }
    });
    tracing::info!(concurrency = config.concurrency, "worker started");
    worker::run(pool, ctx, config, cancel).await;
    Ok(())
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
