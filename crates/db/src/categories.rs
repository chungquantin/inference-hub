use ih_core::Category;
use sqlx::PgPool;
use uuid::Uuid;

// A macro (not a const) so it can be spliced with `concat!` into static SQL strings.
macro_rules! select_category {
    () => {
        "SELECT c.id, c.slug, c.name, c.description, c.parent_id, \
                (SELECT count(DISTINCT oc.organization_id) \
                   FROM organization_categories oc \
                   JOIN categories sub ON sub.id = oc.category_id \
                  WHERE sub.id = c.id OR sub.parent_id = c.id) AS organization_count \
         FROM categories c "
    };
}

pub async fn list(pool: &PgPool) -> sqlx::Result<Vec<Category>> {
    sqlx::query_as(concat!(
        select_category!(),
        "GROUP BY c.id ORDER BY c.sort_order, c.name"
    ))
    .fetch_all(pool)
    .await
}

pub async fn get_by_slug(pool: &PgPool, slug: &str) -> sqlx::Result<Option<Category>> {
    sqlx::query_as(concat!(
        select_category!(),
        "WHERE c.slug = $1 GROUP BY c.id"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn list_for_organization(pool: &PgPool, org_id: Uuid) -> sqlx::Result<Vec<Category>> {
    sqlx::query_as(concat!(
        select_category!(),
        "WHERE c.id IN (SELECT category_id FROM organization_categories WHERE organization_id = $1) \
         GROUP BY c.id ORDER BY c.sort_order, c.name"
    ))
    .bind(org_id)
    .fetch_all(pool)
    .await
}
