//! HTTP API for Inference Hub.

mod auth;
pub mod config;
pub mod error;
mod extract;
mod routes;

use axum::Router;
use ih_db::PgPool;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Bearer token of the admin API. `None` disables the admin API.
    pub admin_token: Option<String>,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            admin_token: None,
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Inference Hub API",
        description = "Products, services and news of the inference market."
    ),
    modifiers(&AdminTokenScheme),
    // Enums used only as query parameters are not collected from the routes.
    components(schemas(ih_core::ModelPriceSort, ih_core::NewsStatus)),
    tags(
        (name = "meta", description = "Service health"),
        (name = "categories", description = "Market taxonomy"),
        (name = "organizations", description = "Companies, projects and investors"),
        (name = "products", description = "Models, APIs, GPU instances and projects"),
        (name = "news", description = "Published news about the market"),
        (name = "compare", description = "Prices and stats across providers"),
        (name = "admin", description = "Editorial endpoints; need the admin token"),
    )
)]
struct ApiDoc;

struct AdminTokenScheme;

impl Modify for AdminTokenScheme {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "admin_token",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            );
        }
    }
}

/// The routes with their documentation. `state` is needed to attach the admin
/// guard; without it (when only the document is wanted) the guard is left out.
fn api_router(state: Option<&AppState>) -> OpenApiRouter<AppState> {
    let mut admin = routes::admin::router();
    if let Some(state) = state {
        admin = admin.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));
    }
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(routes::health::router())
        .nest("/v1", routes::v1_router().merge(admin))
}

/// The OpenAPI document, used to generate the TypeScript client.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api_router(None).into_openapi()
}

pub fn app(state: AppState, cors_origins: &[axum::http::HeaderValue]) -> Router {
    let (router, openapi) = api_router(Some(&state)).split_for_parts();

    // Browsers only read public data directly; admin calls go through the web server.
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
