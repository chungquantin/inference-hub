use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{AppState, error::ApiResult};

#[derive(Serialize, ToSchema)]
pub struct SitemapEntry {
    pub slug: String,
    pub updated_at: DateTime<Utc>,
}

/// Every page-worthy slug, for building the sitemap.
#[derive(Serialize, ToSchema)]
pub struct SitemapIndex {
    pub categories: Vec<SitemapEntry>,
    pub organizations: Vec<SitemapEntry>,
    pub products: Vec<SitemapEntry>,
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(sitemap))
}

/// List the slugs of every category, organization and product with their last update.
#[utoipa::path(
    get,
    path = "/sitemap",
    tag = "meta",
    operation_id = "getSitemap",
    responses((status = 200, body = SitemapIndex))
)]
async fn sitemap(State(state): State<AppState>) -> ApiResult<Json<SitemapIndex>> {
    let to_entries = |rows: Vec<(String, DateTime<Utc>)>| {
        rows.into_iter()
            .map(|(slug, updated_at)| SitemapEntry { slug, updated_at })
            .collect()
    };
    let pool = &state.pool;
    Ok(Json(SitemapIndex {
        categories: to_entries(ih_db::sitemap::category_slugs(pool).await?),
        organizations: to_entries(ih_db::sitemap::organization_slugs(pool).await?),
        products: to_entries(ih_db::sitemap::product_slugs(pool).await?),
    }))
}
