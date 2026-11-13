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
    /// When set, admin requests must also come from one of these networks.
    pub admin_allowed: Option<crate::auth::AllowList>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let host = env_or("API_HOST", "127.0.0.1");
        // Platforms such as Railway assign the port in `PORT`.
        let port = std::env::var("PORT").unwrap_or_else(|_| env_or("API_PORT", "4000"));
        let addr = bind_address(&host, &port)
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

        let admin_allowed = match std::env::var("ADMIN_ALLOWED_IPS") {
            Ok(list) if !list.trim().is_empty() => Some(
                crate::auth::AllowList::parse(&list, std::env::var("CLIENT_IP_HEADER").ok())
                    .context("ADMIN_ALLOWED_IPS")?,
            ),
            _ => None,
        };

        Ok(Self {
            database_url,
            addr,
            cors_origins,
            run_migrations,
            admin_token,
            admin_allowed,
        })
    }
}

/// `host:port`, with an IPv6 host (such as `::`, which also takes IPv4 on Linux) in brackets.
fn bind_address(host: &str, port: &str) -> Result<SocketAddr, std::net::AddrParseError> {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}").parse()
    } else {
        format!("{host}:{port}").parse()
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

#[cfg(test)]
mod tests {
    use super::bind_address;

    #[test]
    fn binds_ipv4_and_ipv6_hosts() {
        assert_eq!(
            bind_address("0.0.0.0", "4000").unwrap().to_string(),
            "0.0.0.0:4000"
        );
        assert_eq!(bind_address("127.0.0.1", "8080").unwrap().port(), 8080);
        // Railway's private network is IPv6; `::` listens there, and on IPv4 too.
        assert_eq!(bind_address("::", "4000").unwrap().to_string(), "[::]:4000");
        assert_eq!(
            bind_address("[::]", "4000").unwrap().to_string(),
            "[::]:4000"
        );
        assert!(bind_address("not a host", "4000").is_err());
    }
}
