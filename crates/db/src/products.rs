use chrono::{DateTime, Utc};
use ih_core::{ModelSpec, Offering, Page, PageParams, ProductDetail, ProductSummary, ProductType};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Filters for listing products. All filters combine with AND.
#[derive(Debug, Clone, Default)]
pub struct ProductFilter {
    pub product_type: Option<ProductType>,
    /// Keep products made by this organization.
    pub organization_slug: Option<String>,
    /// Keep models with open weights.
    pub open_weights: Option<bool>,
    /// Free text, matched with full-text search and name matching.
    pub query: Option<String>,
}

// Macros (not consts) so the fragments splice with `concat!` into static SQL.
macro_rules! product_columns {
    () => {
        "p.id, p.slug, p.name, p.product_type, p.tagline, p.url, p.launched_at, \
         o.slug AS organization_slug, o.name AS organization_name"
    };
}

macro_rules! filter_where {
    () => {
        " WHERE ($1::product_type IS NULL OR p.product_type = $1)
            AND ($2::text IS NULL OR o.slug = $2)
            AND ($3::boolean IS NULL OR coalesce(m.open_weights, false) = $3)
            AND ($4::text IS NULL
                 OR p.search @@ websearch_to_tsquery('english', $4)
                 OR p.name ILIKE '%' || $4 || '%') "
    };
}

macro_rules! from_products {
    () => {
        " FROM products p
          JOIN organizations o ON o.id = p.organization_id
          LEFT JOIN models m ON m.product_id = p.id "
    };
}

pub async fn list(
    pool: &PgPool,
    filter: &ProductFilter,
    page: PageParams,
) -> sqlx::Result<Page<ProductSummary>> {
    let query = filter
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty());

    let items: Vec<ProductSummary> = sqlx::query_as(concat!(
        "SELECT ",
        product_columns!(),
        from_products!(),
        filter_where!(),
        "ORDER BY
           CASE WHEN $4::text IS NULL THEN 0
                ELSE ts_rank(p.search, websearch_to_tsquery('english', $4)) END DESC,
           p.launched_at DESC NULLS LAST, p.name
         LIMIT $5 OFFSET $6"
    ))
    .bind(filter.product_type)
    .bind(filter.organization_slug.as_deref())
    .bind(filter.open_weights)
    .bind(query)
    .bind(page.limit())
    .bind(page.offset())
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(concat!(
        "SELECT count(*)",
        from_products!(),
        filter_where!()
    ))
    .bind(filter.product_type)
    .bind(filter.organization_slug.as_deref())
    .bind(filter.open_weights)
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
struct ProductRow {
    #[sqlx(flatten)]
    product: ProductSummary,
    description: Option<String>,
    metadata: serde_json::Value,
    updated_at: DateTime<Utc>,
}

pub async fn get_by_slug(pool: &PgPool, slug: &str) -> sqlx::Result<Option<ProductDetail>> {
    let Some(row): Option<ProductRow> = sqlx::query_as(concat!(
        "SELECT ",
        product_columns!(),
        ", p.description, p.metadata, p.updated_at
         FROM products p JOIN organizations o ON o.id = p.organization_id
         WHERE p.slug = $1"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let product_id = row.product.id;
    let model: Option<ModelSpec> = sqlx::query_as(
        "SELECT family, parameters_b::float8 AS parameters_b,
                active_params_b::float8 AS active_params_b,
                context_window, modalities, license, open_weights
         FROM models WHERE product_id = $1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?;
    let offerings = latest_offerings(pool, product_id).await?;

    Ok(Some(ProductDetail {
        product: row.product,
        description: row.description,
        metadata: row.metadata,
        model,
        offerings,
        updated_at: row.updated_at,
    }))
}

/// The newest observation for each (provider, configuration) of a product,
/// cheapest first. Older rows are kept as price history.
pub async fn latest_offerings(pool: &PgPool, product_id: Uuid) -> sqlx::Result<Vec<Offering>> {
    sqlx::query_as(
        "SELECT * FROM (
             SELECT DISTINCT ON (f.provider_id, coalesce(f.gpu_type, ''),
                                 coalesce(f.gpu_count, 0), coalesce(f.region, ''))
                    f.id, o.slug AS organization_slug, o.name AS organization_name,
                    f.price_input_per_mtok::float8 AS price_input_per_mtok,
                    f.price_output_per_mtok::float8 AS price_output_per_mtok,
                    f.gpu_type, f.gpu_count,
                    f.price_per_hour::float8 AS price_per_hour,
                    f.region, f.currency::text AS currency, f.source_url, f.observed_at
             FROM offerings f
             JOIN organizations o ON o.id = f.provider_id
             WHERE f.product_id = $1
             ORDER BY f.provider_id, coalesce(f.gpu_type, ''), coalesce(f.gpu_count, 0),
                      coalesce(f.region, ''), f.observed_at DESC
         ) latest
         ORDER BY coalesce(price_output_per_mtok, price_per_hour) NULLS LAST,
                  organization_name",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await
}

/// Products an organization makes.
pub async fn list_made_by(
    pool: &PgPool,
    organization_id: Uuid,
) -> sqlx::Result<Vec<ProductSummary>> {
    sqlx::query_as(concat!(
        "SELECT ",
        product_columns!(),
        " FROM products p JOIN organizations o ON o.id = p.organization_id
          WHERE p.organization_id = $1
          ORDER BY p.launched_at DESC NULLS LAST, p.name"
    ))
    .bind(organization_id)
    .fetch_all(pool)
    .await
}

/// Products made by others that an organization offers.
pub async fn list_served_by(
    pool: &PgPool,
    organization_id: Uuid,
) -> sqlx::Result<Vec<ProductSummary>> {
    sqlx::query_as(concat!(
        "SELECT ",
        product_columns!(),
        " FROM products p JOIN organizations o ON o.id = p.organization_id
          WHERE p.organization_id <> $1
            AND EXISTS (SELECT 1 FROM offerings f
                        WHERE f.product_id = p.id AND f.provider_id = $1)
          ORDER BY p.launched_at DESC NULLS LAST, p.name"
    ))
    .bind(organization_id)
    .fetch_all(pool)
    .await
}
