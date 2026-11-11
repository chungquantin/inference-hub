//! Site-wide search.

use axum::{Json, extract::State};
use ih_core::{SearchQuery, SearchResults};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
    extract::Query,
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(search))
}

/// Search organizations, products, models (chat and media), GPU types and
/// news at once. Results come grouped by kind, best match first.
#[utoipa::path(get, path = "/search", tag = "search", operation_id = "search",
    params(SearchQuery),
    responses((status = 200, body = SearchResults), (status = 400, body = ErrorBody)))]
async fn search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<SearchResults>> {
    if q.q.chars().count() > 100 {
        return Err(ApiError::BadRequest(
            "q must be at most 100 characters".into(),
        ));
    }
    let kinds: Vec<&str> = q
        .kinds
        .as_deref()
        .map(|k| {
            k.split(',')
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if let Some(bad) = kinds.iter().find(|k| !ih_db::search::KINDS.contains(k)) {
        return Err(ApiError::BadRequest(format!(
            "unknown kind `{bad}`; use {}",
            ih_db::search::KINDS.join(", ")
        )));
    }
    let per_kind = q.per_kind.unwrap_or(6).clamp(1, 50);
    Ok(Json(
        ih_db::search::search(&state.pool, &q.q, &kinds, per_kind).await?,
    ))
}
