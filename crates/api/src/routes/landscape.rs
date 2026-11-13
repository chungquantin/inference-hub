//! The landscape map: every organization, with its funding rounds.

use axum::{Json, extract::State};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{AppState, error::ApiResult};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(landscape))
}

/// Every organization with its roles, logo, summary and funding rounds, for the landscape map.
#[utoipa::path(get, path = "/landscape", tag = "organizations", operation_id = "landscape",
    responses((status = 200, body = ih_core::landscape::Landscape)))]
async fn landscape(
    State(state): State<AppState>,
) -> ApiResult<Json<ih_core::landscape::Landscape>> {
    Ok(Json(ih_db::landscape::landscape(&state.pool).await?))
}
