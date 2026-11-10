//! Postgres access: connection pool, migrations and queries.

pub mod categories;
pub mod organizations;
pub mod products;
pub mod sitemap;

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
pub use sqlx::{self, PgPool};

/// Migrations embedded from the workspace `migrations/` directory.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}
