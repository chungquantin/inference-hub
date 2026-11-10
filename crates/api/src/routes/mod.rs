pub mod categories;
pub mod health;
pub mod organizations;

use utoipa_axum::router::OpenApiRouter;

use crate::AppState;

pub fn v1_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(categories::router())
        .merge(organizations::router())
}
