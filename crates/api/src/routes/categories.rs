use axum::{
    Json,
    extract::{Path, State},
};
use ih_core::Category;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_categories))
        .routes(routes!(get_category))
}

/// List all categories of the market taxonomy.
#[utoipa::path(
    get,
    path = "/categories",
    tag = "categories",
    operation_id = "listCategories",
    responses((status = 200, body = Vec<Category>))
)]
async fn list_categories(State(state): State<AppState>) -> ApiResult<Json<Vec<Category>>> {
    Ok(Json(ih_db::categories::list(&state.pool).await?))
}

/// Get one category by slug.
#[utoipa::path(
    get,
    path = "/categories/{slug}",
    tag = "categories",
    operation_id = "getCategory",
    params(("slug" = String, Path, description = "Category slug")),
    responses(
        (status = 200, body = Category),
        (status = 404, body = ErrorBody),
    )
)]
async fn get_category(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Json<Category>> {
    ih_db::categories::get_by_slug(&state.pool, &slug)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("category"))
}
