//! `worker [--once] [--retag] [--enrich-backlog]`
//!
//! Without flags, runs the scheduler and job runners until SIGINT or SIGTERM.
//! `--once` schedules due feed fetches and repository syncs, runs them and exits.
//! `--retag` re-runs the organization tagger on all stored news and exits.
//! `--enrich-backlog` enriches pending news through the LLM until none is
//! left or a budget runs out, and exits.
//!
//! The long-running mode also serves `/healthz`, `/readyz` and `/metrics` on
//! `WORKER_METRICS_PORT` (default 9090), for the platform's health checks.

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use worker::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = ih_db::connect_with_retry(&database_url, std::time::Duration::from_secs(120))
        .await
        .context("connecting to Postgres")?;
    ih_db::migrate(&pool).await.context("running migrations")?;
    let http = ih_ingest::http_client().context("building the HTTP client")?;
    let github = worker::GitHub::from_env();
    if github.token.is_none() {
        tracing::info!("GITHUB_TOKEN is not set; GitHub allows 60 requests per hour");
    }
    let mut ctx = worker::Context::new(http, github);
    ctx.llm = ih_ingest::llm::LlmConfig::from_env();
    match &ctx.llm {
        Some(cfg) => tracing::info!(
            model = %cfg.model,
            max_usd_per_day = cfg.max_usd_per_day,
            "news enrichment is on"
        ),
        None => {
            tracing::info!("LIQUID_BASE_URL or LIQUID_API_KEY is not set; news enrichment is off")
        }
    }

    if flag("--enrich-backlog") {
        let Some(cfg) = &ctx.llm else {
            anyhow::bail!("--enrich-backlog needs LIQUID_BASE_URL and LIQUID_API_KEY");
        };
        // Passes until nothing is left or a budget runs out.
        loop {
            let report = ih_ingest::llm::enrich_pending(&pool, &ctx.http, cfg).await?;
            tracing::info!(?report, "enrichment pass");
            if report.enriched == 0 || report.budget_exhausted {
                break;
            }
        }
        return Ok(());
    }

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
        // Adapter runs schedule a debounced refresh; a one-shot run does it now.
        let refresh = ih_db::market_refresh::refresh(&pool).await?;
        tracing::info!(scheduled, ran, ?refresh, "done");
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
    let port: u16 = env_parse("WORKER_METRICS_PORT", 9090);
    let health = tokio::spawn(worker::health::serve(
        std::net::SocketAddr::from(([0, 0, 0, 0], port)),
        pool.clone(),
        ctx.metrics.clone(),
        cancel.clone(),
    ));
    tracing::info!(concurrency = config.concurrency, "worker started");
    worker::run(pool, ctx, config, cancel).await;
    if let Ok(Err(err)) = health.await {
        tracing::warn!(error = %err, "health server stopped with an error");
    }
    Ok(())
}

/// Pretty logs by default; one JSON object per line when `LOG_FORMAT=json`,
/// for log collectors.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let registry = tracing_subscriber::registry().with(filter);
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
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
