pub mod admin;
pub mod categories;
pub mod compare;
pub mod health;
pub mod market;
pub mod news;
pub mod organizations;
pub mod products;
pub mod sitemap;

use utoipa_axum::router::OpenApiRouter;

use crate::AppState;

pub fn v1_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(categories::router())
        .merge(organizations::router())
        .merge(products::router())
        .merge(news::router())
        .merge(compare::router())
        .merge(market::router())
        .merge(sitemap::router())
}
