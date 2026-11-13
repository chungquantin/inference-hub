pub mod admin;
pub mod categories;
pub mod compare;
pub mod health;
pub mod landscape;
pub mod market;
pub mod news;
pub mod organizations;
pub mod products;
pub mod search;
pub mod sitemap;

use utoipa_axum::router::OpenApiRouter;

use crate::AppState;

pub fn v1_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(categories::router())
        .merge(organizations::router())
        .merge(landscape::router())
        .merge(products::router())
        .merge(news::router())
        .merge(compare::router())
        .merge(market::router())
        .merge(search::router())
        .merge(sitemap::router())
}
