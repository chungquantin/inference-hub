//! Slug listings for the sitemap.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

type Rows = sqlx::Result<Vec<(String, DateTime<Utc>)>>;

pub async fn category_slugs(pool: &PgPool) -> Rows {
    sqlx::query_as("SELECT slug, updated_at FROM categories ORDER BY slug")
        .fetch_all(pool)
        .await
}

pub async fn organization_slugs(pool: &PgPool) -> Rows {
    sqlx::query_as("SELECT slug, updated_at FROM organizations ORDER BY slug")
        .fetch_all(pool)
        .await
}

pub async fn product_slugs(pool: &PgPool) -> Rows {
    sqlx::query_as("SELECT slug, updated_at FROM products ORDER BY slug")
        .fetch_all(pool)
        .await
}
