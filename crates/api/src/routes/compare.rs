use axum::{Json, extract::State};
use ih_core::{GpuPrice, GpuPriceQuery, ModelPrice, ModelPriceQuery, PricePoint, RepoLeader};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
    extract::{Path, Query},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(compare_models))
        .routes(routes!(compare_gpus))
        .routes(routes!(compare_open_source))
        .routes(routes!(price_history))
}

/// Latest token prices of every model at every provider.
///
/// `blended_per_mtok` weighs input and output 3:1, a common chat workload, so
/// models can be ranked by one number.
#[utoipa::path(
    get,
    path = "/compare/models",
    tag = "compare",
    operation_id = "compareModels",
    params(ModelPriceQuery),
    responses(
        (status = 200, body = Vec<ModelPrice>),
        (status = 400, body = ErrorBody),
    )
)]
async fn compare_models(
    State(state): State<AppState>,
    Query(query): Query<ModelPriceQuery>,
) -> ApiResult<Json<Vec<ModelPrice>>> {
    Ok(Json(
        ih_db::compare::model_prices(&state.pool, &query).await?,
    ))
}

#[derive(Serialize, ToSchema)]
pub struct GpuComparison {
    /// GPU types with a current offering, for filters.
    pub gpu_types: Vec<String>,
    /// Cheapest per GPU-hour first.
    pub prices: Vec<GpuPrice>,
}

/// Latest hourly prices of GPU instances at every provider.
#[utoipa::path(
    get,
    path = "/compare/gpus",
    tag = "compare",
    operation_id = "compareGpus",
    params(GpuPriceQuery),
    responses(
        (status = 200, body = GpuComparison),
        (status = 400, body = ErrorBody),
    )
)]
async fn compare_gpus(
    State(state): State<AppState>,
    Query(query): Query<GpuPriceQuery>,
) -> ApiResult<Json<GpuComparison>> {
    Ok(Json(GpuComparison {
        gpu_types: ih_db::compare::gpu_types(&state.pool).await?,
        prices: ih_db::compare::gpu_prices(&state.pool, &query).await?,
    }))
}

/// Open-source projects by GitHub stars, archived ones last.
#[utoipa::path(
    get,
    path = "/compare/open-source",
    tag = "compare",
    operation_id = "compareOpenSource",
    responses((status = 200, body = Vec<RepoLeader>))
)]
async fn compare_open_source(State(state): State<AppState>) -> ApiResult<Json<Vec<RepoLeader>>> {
    Ok(Json(ih_db::compare::repo_leaderboard(&state.pool).await?))
}

/// Every recorded price of a product, oldest first.
#[utoipa::path(
    get,
    path = "/products/{slug}/price-history",
    tag = "products",
    operation_id = "getPriceHistory",
    params(("slug" = String, Path, description = "Product slug")),
    responses(
        (status = 200, body = Vec<PricePoint>),
        (status = 404, body = ErrorBody),
    )
)]
async fn price_history(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Json<Vec<PricePoint>>> {
    ih_db::compare::price_history(&state.pool, &slug)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("product"))
}
