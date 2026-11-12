//! The market: every live price the adapters collect, across all providers.

use axum::{Json, extract::State};
use ih_core::{
    GpuIndexRow, MarketDetail, MarketIndexQuery, MarketOverview, MarketProvider, MarketSource,
    MediaIndexQuery, MediaIndexRow, ModelIndexRow, Page,
};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
    extract::{Path, Query},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(overview))
        .routes(routes!(analytics))
        .routes(routes!(gpu_index))
        .routes(routes!(gpu_detail))
        .routes(routes!(model_index))
        .routes(routes!(model_detail))
        .routes(routes!(model_makers))
        .routes(routes!(media_index))
        .routes(routes!(providers))
        .routes(routes!(provider))
        .routes(routes!(provider_offerings))
        .routes(routes!(sources))
}

/// Headline numbers, the most traded GPUs and models, and source health.
#[utoipa::path(get, path = "/market", tag = "market", operation_id = "marketOverview",
    responses((status = 200, body = MarketOverview)))]
async fn overview(State(state): State<AppState>) -> ApiResult<Json<MarketOverview>> {
    Ok(Json(ih_db::market::overview(&state.pool).await?))
}

/// Rankings and distributions: models by price spread, makers, GPU price matrix and
/// providers scored against the market median.
#[utoipa::path(get, path = "/market/analytics", tag = "market", operation_id = "marketAnalytics",
    responses((status = 200, body = ih_core::MarketAnalytics)))]
async fn analytics(State(state): State<AppState>) -> ApiResult<Json<ih_core::MarketAnalytics>> {
    Ok(Json(ih_db::analytics::analytics(&state.pool).await?))
}

/// GPU types across every cloud: min, median and max price per GPU-hour.
#[utoipa::path(get, path = "/market/gpus", tag = "market", operation_id = "marketGpus",
    params(MarketIndexQuery),
    responses((status = 200, body = Vec<GpuIndexRow>), (status = 400, body = ErrorBody)))]
async fn gpu_index(
    State(state): State<AppState>,
    Query(q): Query<MarketIndexQuery>,
) -> ApiResult<Json<Vec<GpuIndexRow>>> {
    Ok(Json(ih_db::market::gpu_index(&state.pool, &q).await?))
}

/// Every cloud's price for one GPU type, such as `H100 SXM`.
#[utoipa::path(get, path = "/market/gpus/{gpu_type}", tag = "market", operation_id = "marketGpu",
    params(("gpu_type" = String, Path, description = "GPU type, such as `H100 SXM`")),
    responses((status = 200, body = MarketDetail), (status = 404, body = ErrorBody)))]
async fn gpu_detail(
    State(state): State<AppState>,
    Path(gpu_type): Path<String>,
) -> ApiResult<Json<MarketDetail>> {
    ih_db::market::gpu_detail(&state.pool, &gpu_type)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("GPU type"))
}

/// Models across every provider: cheapest and median price per million tokens.
#[utoipa::path(get, path = "/market/models", tag = "market", operation_id = "marketModels",
    params(MarketIndexQuery),
    responses((status = 200, body = Page<ModelIndexRow>), (status = 400, body = ErrorBody)))]
async fn model_index(
    State(state): State<AppState>,
    Query(q): Query<MarketIndexQuery>,
) -> ApiResult<Json<Page<ModelIndexRow>>> {
    Ok(Json(ih_db::market::model_index(&state.pool, &q).await?))
}

/// Every provider's price for one model, by its normalized key.
#[utoipa::path(get, path = "/market/models/{key}", tag = "market", operation_id = "marketModel",
    params(("key" = String, Path, description = "Normalized model key, such as `deepseek-v3.2`")),
    responses((status = 200, body = MarketDetail), (status = 404, body = ErrorBody)))]
async fn model_detail(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> ApiResult<Json<MarketDetail>> {
    ih_db::market::model_detail(&state.pool, &key)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("model"))
}

/// Providers by coverage, with how often they are the cheapest.
#[utoipa::path(get, path = "/market/providers", tag = "market", operation_id = "marketProviders",
    responses((status = 200, body = Vec<MarketProvider>)))]
async fn providers(State(state): State<AppState>) -> ApiResult<Json<Vec<MarketProvider>>> {
    Ok(Json(ih_db::market::providers(&state.pool).await?))
}

/// Every data source and its health.
#[utoipa::path(get, path = "/market/sources", tag = "market", operation_id = "marketSources",
    responses((status = 200, body = Vec<MarketSource>)))]
async fn sources(State(state): State<AppState>) -> ApiResult<Json<Vec<MarketSource>>> {
    Ok(Json(ih_db::market::sources(&state.pool).await?))
}

/// Embedding, image, video, speech and transcription models across
/// providers, priced in their own unit.
#[utoipa::path(get, path = "/market/media", tag = "market", operation_id = "marketMedia",
    params(MediaIndexQuery),
    responses((status = 200, body = Page<MediaIndexRow>), (status = 400, body = ErrorBody)))]
async fn media_index(
    State(state): State<AppState>,
    Query(q): Query<MediaIndexQuery>,
) -> ApiResult<Json<Page<MediaIndexRow>>> {
    if let Some(kind) = q.kind.as_deref()
        && !kind.is_empty()
        && !matches!(
            kind,
            "embedding" | "image" | "video" | "speech" | "transcription"
        )
    {
        return Err(ApiError::BadRequest(
            "kind must be embedding, image, video, speech or transcription".into(),
        ));
    }
    Ok(Json(ih_db::market::media_index(&state.pool, &q).await?))
}

/// One provider's catalog in the market: model, media and GPU counts.
#[utoipa::path(get, path = "/market/providers/{slug}", tag = "market", operation_id = "marketProvider",
    params(("slug" = String, Path, description = "Organization slug")),
    responses((status = 200, body = MarketProvider), (status = 404, body = ErrorBody)))]
async fn provider(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Json<MarketProvider>> {
    ih_db::market::provider(&state.pool, &slug)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("market provider"))
}

/// Everything one provider sells, collected from its catalog and from the
/// curated data, with the market's price next to its own.
#[utoipa::path(get, path = "/market/providers/{slug}/offerings", tag = "market",
    operation_id = "marketProviderOfferings",
    params(("slug" = String, Path, description = "Organization slug"), ih_core::ProviderOfferingsQuery),
    responses((status = 200, body = ih_core::ProviderOfferings), (status = 404, body = ErrorBody)))]
async fn provider_offerings(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(q): Query<ih_core::ProviderOfferingsQuery>,
) -> ApiResult<Json<ih_core::ProviderOfferings>> {
    ih_db::market::provider_offerings(&state.pool, &slug, &q)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("provider offerings"))
}

/// Makers of chat models in the market, most models first, for filters.
#[utoipa::path(get, path = "/market/makers", tag = "market", operation_id = "marketMakers",
    params(("min_providers" = Option<i64>, Query, description = "Count models sold by at least this many providers. Default 1.")),
    responses((status = 200, body = Vec<ih_core::MakerCount>)))]
async fn model_makers(
    State(state): State<AppState>,
    Query(q): Query<MakersQuery>,
) -> ApiResult<Json<Vec<ih_core::MakerCount>>> {
    Ok(Json(
        ih_db::market::model_makers(&state.pool, q.min_providers.unwrap_or(1)).await?,
    ))
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct MakersQuery {
    min_providers: Option<i64>,
}
