//! HTTP API for Inference Hub.

pub mod config;
pub mod error;
mod routes;

use axum::Router;
use ih_db::PgPool;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Inference Hub API",
        description = "Products, services and news of the inference market."
    ),
    tags(
        (name = "meta", description = "Service health"),
        (name = "categories", description = "Market taxonomy"),
        (name = "organizations", description = "Companies, projects and investors"),
    )
)]
struct ApiDoc;

fn api_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(routes::health::router())
        .nest("/v1", routes::v1_router())
}

/// The OpenAPI document, used to generate the TypeScript client.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api_router().into_openapi()
}

pub fn app(state: AppState, cors_origins: &[axum::http::HeaderValue]) -> Router {
    let (router, openapi) = api_router().split_for_parts();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(cors_origins.iter().cloned()))
        .allow_methods([axum::http::Method::GET])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    router
        .merge(Scalar::with_url("/docs", openapi.clone()))
        .route(
            "/openapi.json",
            axum::routing::get(move || async move { axum::Json(openapi) }),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
