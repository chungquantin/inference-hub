use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::RepoStats;

/// What kind of thing a product is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "product_type", rename_all = "snake_case")]
pub enum ProductType {
    Model,
    InferenceApi,
    Gateway,
    GpuInstance,
    OssProject,
    Hardware,
    Platform,
    Other,
}

/// A short reference to an organization, embedded in product and offering responses.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct OrganizationRef {
    #[sqlx(rename = "organization_slug")]
    pub slug: String,
    #[sqlx(rename = "organization_name")]
    pub name: String,
    /// Path of the organization's logo, such as `/logos/openai.svg`.
    #[sqlx(rename = "organization_logo")]
    pub logo_url: Option<String>,
}

/// A product as shown in listings.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ProductSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub product_type: ProductType,
    pub tagline: Option<String>,
    pub url: Option<String>,
    pub launched_at: Option<NaiveDate>,
    /// The organization that makes the product.
    #[sqlx(flatten)]
    pub organization: OrganizationRef,
}

/// Technical attributes of a model.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ModelSpec {
    pub family: Option<String>,
    /// Total parameters, in billions.
    pub parameters_b: Option<f64>,
    /// Parameters active per token, in billions (mixture-of-experts models).
    pub active_params_b: Option<f64>,
    /// Context window, in tokens.
    pub context_window: Option<i32>,
    /// Input modalities, such as `text` or `image`.
    pub modalities: Vec<String>,
    pub license: Option<String>,
    pub open_weights: bool,
}

/// The latest observation of a provider selling a product.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Offering {
    pub id: Uuid,
    #[sqlx(flatten)]
    pub provider: OrganizationRef,
    /// USD per million input tokens.
    pub price_input_per_mtok: Option<f64>,
    /// USD per million output tokens.
    pub price_output_per_mtok: Option<f64>,
    pub gpu_type: Option<String>,
    pub gpu_count: Option<i16>,
    /// Price per instance-hour.
    pub price_per_hour: Option<f64>,
    pub region: Option<String>,
    /// ISO 4217 currency code of the prices.
    pub currency: String,
    pub source_url: Option<String>,
    /// When this price was last checked: first seen, or last confirmed by an adapter.
    pub observed_at: DateTime<Utc>,
}

/// The full page of a product.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProductDetail {
    #[serde(flatten)]
    pub product: ProductSummary,
    pub description: Option<String>,
    /// Attributes specific to the product type.
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: serde_json::Value,
    /// Present when the product is a model.
    pub model: Option<ModelSpec>,
    /// Where the product can be bought, one entry per provider and configuration.
    pub offerings: Vec<Offering>,
    /// GitHub stats, for products with a synced repository.
    pub repo: Option<RepoStats>,
    pub updated_at: DateTime<Utc>,
}
