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
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut check_only = false;
    let mut dir = PathBuf::from("data");
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--check" => check_only = true,
            "-h" | "--help" => {
                println!("usage: seed [--check] [DATA_DIR]");
                return Ok(());
            }
            other => dir = PathBuf::from(other),
        }
    }

    let data = seed::load(&dir)?;
    tracing::info!(
        organizations = data.organizations.len(),
        shared_feeds = data.sources.feeds.len(),
        dir = %dir.display(),
        "loaded seed files"
    );

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = ih_db::connect(&database_url)
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
