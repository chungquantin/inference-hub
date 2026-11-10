use axum::{Json, extract::State};
use ih_core::{Page, PageParams, ProductDetail, ProductSummary, ProductType};
use ih_db::products::ProductFilter;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
    extract::{Path, Query},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_products))
        .routes(routes!(get_product))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListProductsQuery {
    /// Keep products of this type.
    #[serde(rename = "type")]
    #[param(rename = "type")]
    product_type: Option<ProductType>,
    /// Keep products made by this organization (slug).
    organization: Option<String>,
    /// Keep models with (true) or without (false) open weights.
    open_weights: Option<bool>,
    /// Free-text search over name, tagline and description.
    q: Option<String>,
}

/// List products, optionally filtered and searched.
#[utoipa::path(
    get,
    path = "/products",
    tag = "products",
    operation_id = "listProducts",
    params(ListProductsQuery, PageParams),
    responses(
        (status = 200, body = Page<ProductSummary>),
        (status = 400, body = ErrorBody),
    )
)]
async fn list_products(
    State(state): State<AppState>,
    Query(query): Query<ListProductsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Page<ProductSummary>>> {
    let filter = ProductFilter {
        product_type: query.product_type,
        organization_slug: query.organization,
        open_weights: query.open_weights,
        query: query.q,
    };
    Ok(Json(
        ih_db::products::list(&state.pool, &filter, page).await?,
    ))
}

/// Get one product with its model attributes and current offerings.
#[utoipa::path(
    get,
    path = "/products/{slug}",
    tag = "products",
    operation_id = "getProduct",
    params(("slug" = String, Path, description = "Product slug")),
    responses(
        (status = 200, body = ProductDetail),
        (status = 404, body = ErrorBody),
    )
)]
async fn get_product(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Json<ProductDetail>> {
    ih_db::products::get_by_slug(&state.pool, &slug)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("product"))
}
