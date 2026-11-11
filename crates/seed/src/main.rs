//! `cargo run -p seed -- [--check] [DATA_DIR]`
//!
//! Validates the curated YAML in `DATA_DIR` (default `data`) and imports it.
//! With `--check`, it only parses and validates the files and writes nothing.
//! Both modes connect to Postgres, to read the category list.

use std::path::PathBuf;

use anyhow::Context;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let registry = tracing_subscriber::registry().with(filter);
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    let mut check_only = false;
    let mut archive: Option<&str> = None;
    let mut dir = PathBuf::from("data");
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--check" => check_only = true,
            "--export-archive" => archive = Some("export"),
            "--import-archive" => archive = Some("import"),
            "-h" | "--help" => {
                println!(
                    "usage: seed [--check | --export-archive | --import-archive] [DATA_DIR]\n\
                     \n  --export-archive  write fetched data to DATA_DIR/archive/*.jsonl\
                     \n  --import-archive  load DATA_DIR/archive/*.jsonl, then rebuild market summaries"
                );
                return Ok(());
            }
            other => dir = PathBuf::from(other),
        }
    }

    if let Some(mode) = archive {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let pool =
            ih_db::connect_with_retry(&database_url, std::time::Duration::from_secs(120)).await?;
        ih_db::migrate(&pool).await.context("running migrations")?;
        let archive_dir = dir.join("archive");
        let counts = if mode == "export" {
            seed::archive::export(&pool, &archive_dir).await?
        } else {
            let counts = seed::archive::import(&pool, &archive_dir).await?;
            let report = ih_db::market_refresh::refresh(&pool).await?;
            tracing::info!(?report, "market summaries rebuilt");
            counts
        };
        for c in &counts {
            tracing::info!(table = c.table, rows = c.rows, "{mode}ed");
        }
        return Ok(());
    }

    let data = seed::load(&dir)?;
    tracing::info!(
        organizations = data.organizations.len(),
        shared_feeds = data.sources.feeds.len(),
        dir = %dir.display(),
        "loaded seed files"
    );

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = ih_db::connect_with_retry(&database_url, std::time::Duration::from_secs(120))
        .await
        .context("connecting to Postgres")?;
    ih_db::migrate(&pool).await.context("running migrations")?;

    if check_only {
        let known = seed::known_categories(&pool).await?;
        let issues = data.validate(&known);
        if issues.is_empty() {
            tracing::info!("data is valid");
            return Ok(());
        }
        for issue in &issues {
            eprintln!("{issue}");
        }
        anyhow::bail!("{} problem(s) found", issues.len());
    }

    let summary = seed::import(&pool, &data).await?;
    tracing::info!(
        organizations = summary.organizations,
        products = summary.products,
        offerings_added = summary.offerings_added,
        sources = summary.sources,
        funding_rounds = summary.funding_rounds,
        "import complete"
    );
    Ok(())
}
