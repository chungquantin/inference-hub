use chrono::{DateTime, Utc};
use ih_core::{OrgKind, Organization, OrganizationDetail, Page, PageParams};
use sqlx::{FromRow, PgPool};

use crate::{categories, compare, products};

/// Filters for listing organizations. All filters combine with AND.
#[derive(Debug, Clone, Default)]
pub struct OrganizationFilter {
    pub kind: Option<OrgKind>,
    pub category_slug: Option<String>,
    /// Free text, matched with full-text search and fuzzy name matching.
    pub query: Option<String>,
}

// Macros (not consts) so the fragments splice with `concat!` into static SQL,
// which keeps every query a literal string.
macro_rules! org_columns {
    () => {
        "o.id, o.slug, o.name, o.kinds, o.tagline, o.website, o.logo_url, \
         o.hq_country::text AS hq_country, o.founded_year, o.status"
    };
}

// Every filter is optional: a NULL parameter disables its clause.
macro_rules! filter_where {
    () => {
        " WHERE ($1::org_kind IS NULL OR $1 = ANY (o.kinds))
            AND ($2::text IS NULL OR EXISTS (
                  SELECT 1 FROM organization_categories oc
                  JOIN categories c ON c.id = oc.category_id
                  LEFT JOIN categories parent ON parent.id = c.parent_id
                  WHERE oc.organization_id = o.id AND $2 IN (c.slug, parent.slug)))
            AND ($3::text IS NULL
                 OR o.search @@ websearch_to_tsquery('english', $3)
                 OR o.name ILIKE '%' || $3 || '%') "
    };
}

pub async fn list(
    pool: &PgPool,
    filter: &OrganizationFilter,
    page: PageParams,
) -> sqlx::Result<Page<Organization>> {
    let query = filter
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty());

    let items: Vec<Organization> = sqlx::query_as(concat!(
        "SELECT ",
        org_columns!(),
        " FROM organizations o",
        filter_where!(),
        "ORDER BY
           CASE WHEN $3::text IS NULL THEN 0
                ELSE ts_rank(o.search, websearch_to_tsquery('english', $3)) END DESC,
           o.name
         LIMIT $4 OFFSET $5"
    ))
    .bind(filter.kind)
    .bind(filter.category_slug.as_deref())
    .bind(query)
    .bind(page.limit())
    .bind(page.offset())
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(concat!(
        "SELECT count(*) FROM organizations o",
        filter_where!()
    ))
    .bind(filter.kind)
    .bind(filter.category_slug.as_deref())
    .bind(query)
    .fetch_one(pool)
    .await?;

    Ok(Page {
        items,
        total,
        limit: page.limit(),
        offset: page.offset(),
    })
}

#[derive(FromRow)]
struct OrganizationRow {
    #[sqlx(flatten)]
    organization: Organization,
    description: Option<String>,
    links: serde_json::Value,
    updated_at: DateTime<Utc>,
}

pub async fn get_by_slug(pool: &PgPool, slug: &str) -> sqlx::Result<Option<OrganizationDetail>> {
    let Some(row): Option<OrganizationRow> = sqlx::query_as(concat!(
        "SELECT ",
        org_columns!(),
        ", o.description, o.links, o.updated_at FROM organizations o WHERE o.slug = $1"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let org_id = row.organization.id;
    let categories = categories::list_for_organization(pool, org_id).await?;
    let products = products::list_made_by(pool, org_id).await?;
    let served_products = products::list_served_by(pool, org_id).await?;
    let funding = compare::funding_rounds(pool, org_id).await?;
    let portfolio = compare::portfolio(pool, org_id).await?;

    Ok(Some(OrganizationDetail {
        organization: row.organization,
        description: row.description,
        links: row.links,
        categories,
        products,
        served_products,
        funding,
        portfolio,
        updated_at: row.updated_at,
    }))
}
