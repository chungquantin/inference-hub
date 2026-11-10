use anyhow::Context;
use api::{AppState, config::Config};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let pool = ih_db::connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;

    if config.run_migrations {
        ih_db::migrate(&pool).await.context("running migrations")?;
        tracing::info!("migrations applied");
    }

    if config.admin_token.is_none() {
        tracing::info!("ADMIN_TOKEN is not set; the admin API is disabled");
    }
    let state = AppState {
        pool,
        admin_token: config.admin_token.clone(),
    };
    let app = api::app(state, &config.cors_origins);
    let listener = tokio::net::TcpListener::bind(config.addr)
        .await
        .with_context(|| format!("binding {}", config.addr))?;
    tracing::info!("listening on http://{} (docs at /docs)", config.addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
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
    tracing::info!("shutting down");
}
