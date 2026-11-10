use std::net::SocketAddr;

use anyhow::Context;
use axum::http::HeaderValue;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub addr: SocketAddr,
    pub cors_origins: Vec<HeaderValue>,
    pub run_migrations: bool,
    /// Enables the admin API. Must be at least 32 characters.
    pub admin_token: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let host = env_or("API_HOST", "127.0.0.1");
        let port = env_or("API_PORT", "4000");
        let addr = format!("{host}:{port}")
            .parse()
            .with_context(|| format!("invalid API_HOST/API_PORT: {host}:{port}"))?;

        let cors_origins = env_or("CORS_ORIGINS", "http://localhost:3000")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| HeaderValue::from_str(s).with_context(|| format!("invalid CORS origin: {s}")))
            .collect::<anyhow::Result<_>>()?;

        let run_migrations = matches!(
            env_or("RUN_MIGRATIONS", "true")
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes"
        );

        let admin_token = std::env::var("ADMIN_TOKEN")
            .ok()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty());
        if let Some(token) = &admin_token {
            anyhow::ensure!(
                token.len() >= 32,
                "ADMIN_TOKEN must be at least 32 characters (try `openssl rand -hex 32`)"
            );
        }

        Ok(Self {
            database_url,
            addr,
            cors_origins,
            run_migrations,
            admin_token,
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}
