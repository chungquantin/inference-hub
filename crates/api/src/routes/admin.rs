//! Editorial endpoints: the news review queue and source health.
//! Every route requires `Authorization: Bearer <ADMIN_TOKEN>`.

use axum::{extract::State, http::StatusCode};
use ih_core::{
    AdapterStatus, FailedJob, HoldResolution, NewsItem, NewsStatus, Page, PageParams, PriceHold,
    SourceStatus, UnmappedListing,
};
use ih_db::news::NewsFilter;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
    extract::{Json, Path, Query},
};

/// Most items one review request may change.
const MAX_BATCH: usize = 200;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_admin_news))
        .routes(routes!(review_news))
        .routes(routes!(set_news_organizations))
        .routes(routes!(list_sources))
        .routes(routes!(list_adapters))
        .routes(routes!(list_unmapped))
        .routes(routes!(list_holds))
        .routes(routes!(resolve_hold))
        .routes(routes!(list_failed_jobs))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct AdminNewsQuery {
    /// Review state to list. Default `pending`.
    status: Option<NewsStatus>,
    /// Keep items from this source (slug).
    source: Option<String>,
    /// Keep items about this organization (slug).
    organization: Option<String>,
    /// Text that the title or summary must contain.
    q: Option<String>,
}

/// List news in any review state. Defaults to the pending queue.
#[utoipa::path(
    get,
    path = "/admin/news",
    tag = "admin",
    operation_id = "adminListNews",
    params(AdminNewsQuery, PageParams),
    security(("admin_token" = [])),
    responses(
        (status = 200, body = Page<NewsItem>),
        (status = 401, body = ErrorBody),
    )
)]
async fn list_admin_news(
    State(state): State<AppState>,
    Query(query): Query<AdminNewsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<axum::Json<Page<NewsItem>>> {
    let filter = NewsFilter {
        status: Some(query.status.unwrap_or(NewsStatus::Pending)),
        organization_slug: query.organization,
        source_slug: query.source,
        query: query.q,
    };
    Ok(axum::Json(
        ih_db::news::list(&state.pool, &filter, page).await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReviewRequest {
    /// Items to change, at most 200.
    pub ids: Vec<Uuid>,
    pub status: NewsStatus,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewResult {
    /// Items that exist and were set.
    pub updated: u64,
}

/// Approve, reject or return items to the queue.
#[utoipa::path(
    post,
    path = "/admin/news/review",
    tag = "admin",
    operation_id = "adminReviewNews",
    request_body = ReviewRequest,
    security(("admin_token" = [])),
    responses(
        (status = 200, body = ReviewResult),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
    )
)]
async fn review_news(
    State(state): State<AppState>,
    Json(request): Json<ReviewRequest>,
) -> ApiResult<axum::Json<ReviewResult>> {
    if request.ids.is_empty() || request.ids.len() > MAX_BATCH {
        return Err(ApiError::BadRequest(format!(
            "ids must hold between 1 and {MAX_BATCH} items"
        )));
    }
    let updated = ih_db::news::set_status(&state.pool, &request.ids, request.status).await?;
    tracing::info!(updated, status = ?request.status, "news reviewed");
    Ok(axum::Json(ReviewResult { updated }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetOrganizationsRequest {
    /// Slugs of the organizations the item is about. Unknown slugs are rejected.
    pub organizations: Vec<String>,
}

/// Replace the organizations a news item is linked to.
#[utoipa::path(
    put,
    path = "/admin/news/{id}/organizations",
    tag = "admin",
    operation_id = "adminSetNewsOrganizations",
    params(("id" = Uuid, Path, description = "News item id")),
    request_body = SetOrganizationsRequest,
    security(("admin_token" = [])),
    responses(
        (status = 200, body = NewsItem),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
async fn set_news_organizations(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<SetOrganizationsRequest>,
) -> ApiResult<(StatusCode, axum::Json<NewsItem>)> {
    let known: Vec<String> =
        ih_db::sqlx::query_scalar("SELECT slug FROM organizations WHERE slug = ANY ($1)")
            .bind(&request.organizations)
            .fetch_all(&state.pool)
            .await?;
    let unknown: Vec<&str> = request
        .organizations
        .iter()
        .filter(|s| !known.contains(s))
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "unknown organizations: {}",
            unknown.join(", ")
        )));
    }

    if !ih_db::news::set_organizations(&state.pool, id, &request.organizations).await? {
        return Err(ApiError::NotFound("news item"));
    }
    let item = ih_db::news::get(&state.pool, id)
        .await?
        .ok_or(ApiError::NotFound("news item"))?;
    Ok((StatusCode::OK, axum::Json(item)))
}

/// List feed sources with their fetch health, failing ones first.
#[utoipa::path(
    get,
    path = "/admin/sources",
    tag = "admin",
    operation_id = "adminListSources",
    security(("admin_token" = [])),
    responses(
        (status = 200, body = Vec<SourceStatus>),
        (status = 401, body = ErrorBody),
    )
)]
async fn list_sources(State(state): State<AppState>) -> ApiResult<axum::Json<Vec<SourceStatus>>> {
    Ok(axum::Json(ih_db::news::list_sources(&state.pool).await?))
}

/// List market adapters with their health, failing ones first.
#[utoipa::path(
    get,
    path = "/admin/adapters",
    tag = "admin",
    operation_id = "adminListAdapters",
    security(("admin_token" = [])),
    responses(
        (status = 200, body = Vec<AdapterStatus>),
        (status = 401, body = ErrorBody),
    )
)]
async fn list_adapters(State(state): State<AppState>) -> ApiResult<axum::Json<Vec<AdapterStatus>>> {
    Ok(axum::Json(ih_db::news::list_adapters(&state.pool).await?))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct UnmappedQuery {
    /// Keep listings of this adapter, such as `openrouter`.
    adapter: Option<String>,
    /// `model` or `gpu`.
    kind: Option<String>,
}

/// List live listings that no product is mapped to: candidates to add to the directory.
#[utoipa::path(
    get,
    path = "/admin/listings/unmapped",
    tag = "admin",
    operation_id = "adminListUnmapped",
    params(UnmappedQuery, PageParams),
    security(("admin_token" = [])),
    responses(
        (status = 200, body = Page<UnmappedListing>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
    )
)]
async fn list_unmapped(
    State(state): State<AppState>,
    Query(query): Query<UnmappedQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<axum::Json<Page<UnmappedListing>>> {
    if let Some(kind) = query.kind.as_deref()
        && !matches!(kind, "model" | "gpu")
    {
        return Err(ApiError::BadRequest("kind must be `model` or `gpu`".into()));
    }
    Ok(axum::Json(
        ih_db::news::list_unmapped(
            &state.pool,
            query.adapter.as_deref(),
            query.kind.as_deref(),
            page,
        )
        .await?,
    ))
}

/// Prices that failed a sanity check and wait for a decision.
#[utoipa::path(
    get,
    path = "/admin/holds",
    tag = "admin",
    operation_id = "adminListHolds",
    security(("admin_token" = [])),
    responses((status = 200, body = Vec<PriceHold>), (status = 401, body = ErrorBody))
)]
async fn list_holds(State(state): State<AppState>) -> ApiResult<axum::Json<Vec<PriceHold>>> {
    Ok(axum::Json(ih_db::market::open_holds(&state.pool).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveHoldRequest {
    pub resolution: HoldResolution,
}

/// Accept a held price (stored on the adapter's next run) or reject it.
#[utoipa::path(
    post,
    path = "/admin/holds/{id}",
    tag = "admin",
    operation_id = "adminResolveHold",
    params(("id" = i64, Path, description = "Hold id")),
    request_body = ResolveHoldRequest,
    security(("admin_token" = [])),
    responses(
        (status = 204),
        (status = 401, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
async fn resolve_hold(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ResolveHoldRequest>,
) -> ApiResult<StatusCode> {
    if ih_db::market::resolve_hold(&state.pool, id, request.resolution).await? {
        tracing::info!(id, resolution = ?request.resolution, "price hold resolved");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("open hold"))
    }
}

/// Jobs that failed every attempt, newest first.
#[utoipa::path(
    get,
    path = "/admin/jobs/failed",
    tag = "admin",
    operation_id = "adminListFailedJobs",
    security(("admin_token" = [])),
    responses((status = 200, body = Vec<FailedJob>), (status = 401, body = ErrorBody))
)]
async fn list_failed_jobs(State(state): State<AppState>) -> ApiResult<axum::Json<Vec<FailedJob>>> {
    Ok(axum::Json(
        ih_db::market::failed_jobs(&state.pool, 100).await?,
    ))
}
