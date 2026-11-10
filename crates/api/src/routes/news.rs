use axum::{Json, extract::State};
use ih_core::{NewsItem, Page, PageParams};
use ih_db::news::NewsFilter;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiResult, ErrorBody},
    extract::Query,
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list_news))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListNewsQuery {
    /// Keep items about this organization (slug).
    organization: Option<String>,
    /// Keep items from this source (slug).
    source: Option<String>,
    /// Text that the title or summary must contain.
    q: Option<String>,
}

/// List published news, newest first.
#[utoipa::path(
    get,
    path = "/news",
    tag = "news",
    operation_id = "listNews",
    params(ListNewsQuery, PageParams),
    responses(
        (status = 200, body = Page<NewsItem>),
        (status = 400, body = ErrorBody),
    )
)]
async fn list_news(
    State(state): State<AppState>,
    Query(query): Query<ListNewsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Page<NewsItem>>> {
    let filter = NewsFilter {
        status: None,
        organization_slug: query.organization,
        source_slug: query.source,
        query: query.q,
    };
    Ok(Json(ih_db::news::list(&state.pool, &filter, page).await?))
}
