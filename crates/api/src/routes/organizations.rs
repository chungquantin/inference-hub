use axum::{Json, extract::State};
use ih_core::{OrgKind, Organization, OrganizationDetail, Page, PageParams};
use ih_db::organizations::OrganizationFilter;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    AppState,
    error::{ApiError, ApiResult, ErrorBody},
    extract::{Path, Query},
};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_organizations))
        .routes(routes!(get_organization))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListOrganizationsQuery {
    /// Keep organizations that play this role.
    kind: Option<OrgKind>,
    /// Keep organizations listed in this category.
    category: Option<String>,
    /// Free-text search over name, tagline and description.
    q: Option<String>,
}

/// List organizations, optionally filtered and searched.
#[utoipa::path(
    get,
    path = "/organizations",
    tag = "organizations",
    operation_id = "listOrganizations",
    params(ListOrganizationsQuery, PageParams),
    responses(
        (status = 200, body = Page<Organization>),
        (status = 400, body = ErrorBody),
    )
)]
async fn list_organizations(
    State(state): State<AppState>,
    Query(query): Query<ListOrganizationsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Page<Organization>>> {
    let filter = OrganizationFilter {
        kind: query.kind,
        category_slug: query.category,
        query: query.q,
    };
    Ok(Json(
        ih_db::organizations::list(&state.pool, &filter, page).await?,
    ))
}

/// Get the full profile of one organization.
#[utoipa::path(
    get,
    path = "/organizations/{slug}",
    tag = "organizations",
    operation_id = "getOrganization",
    params(("slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, body = OrganizationDetail),
        (status = 404, body = ErrorBody),
    )
)]
async fn get_organization(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> ApiResult<Json<OrganizationDetail>> {
    ih_db::organizations::get_by_slug(&state.pool, &slug)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("organization"))
}
