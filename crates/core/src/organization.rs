use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::Category;

/// The roles an organization plays in the market. One organization can hold several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "org_kind", rename_all = "snake_case")]
pub enum OrgKind {
    ModelProvider,
    InferenceProvider,
    LlmGateway,
    GpuCloud,
    Hyperscaler,
    Hardware,
    OpenSource,
    Investor,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "org_status", rename_all = "snake_case")]
pub enum OrgStatus {
    Active,
    Acquired,
    Defunct,
}

/// An organization as shown in listings.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub kinds: Vec<OrgKind>,
    pub tagline: Option<String>,
    pub website: Option<String>,
    pub logo_url: Option<String>,
    /// ISO 3166-1 alpha-2 country code.
    pub hq_country: Option<String>,
    pub founded_year: Option<i16>,
    pub status: OrgStatus,
}

/// A product as listed on its organization's page.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ProductSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    /// One of: model, inference_api, gateway, gpu_instance, oss_project, hardware, platform, other.
    pub product_type: String,
    pub tagline: Option<String>,
    pub url: Option<String>,
    pub launched_at: Option<NaiveDate>,
}

/// The full profile of an organization.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrganizationDetail {
    #[serde(flatten)]
    pub organization: Organization,
    pub description: Option<String>,
    /// Named external links, such as `github`, `x` or `docs`.
    #[schema(value_type = Object)]
    pub links: serde_json::Value,
    pub categories: Vec<Category>,
    pub products: Vec<ProductSummary>,
    pub updated_at: DateTime<Utc>,
}
