use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A node of the market taxonomy, such as "GPU Clouds".
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Category {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    /// Number of organizations listed in this category or in one of its sub-categories.
    pub organization_count: i64,
}
