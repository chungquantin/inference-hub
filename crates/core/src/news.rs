use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Review state of a news item. Only approved items are public.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "news_status", rename_all = "snake_case")]
pub enum NewsStatus {
    Pending,
    Approved,
    Rejected,
}

/// How a news item was linked to an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "news_match", rename_all = "snake_case")]
pub enum NewsMatch {
    /// The item comes from the organization's own feed.
    Source,
    /// The organization is named in the title.
    Title,
    /// The organization is named in the summary.
    Body,
    /// An editor added the link.
    Manual,
}

/// An organization a news item is about.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NewsMention {
    pub slug: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub matched_by: NewsMatch,
}

/// A news item as shown in feeds.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NewsItem {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub publisher: Option<String>,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub author: Option<String>,
    pub published_at: DateTime<Utc>,
    pub status: NewsStatus,
    pub organizations: Vec<NewsMention>,
}

/// A feed source with its fetch health, for the admin view.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SourceStatus {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub url: Option<String>,
    pub enabled: bool,
    pub auto_approve: bool,
    pub fetch_interval_minutes: i32,
    /// The organization that owns the feed, if any.
    pub organization_slug: Option<String>,
    pub organization_name: Option<String>,
    #[sqlx(rename = "organization_logo")]
    pub organization_logo_url: Option<String>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
    /// Items stored from this source.
    pub item_count: i64,
    /// Of which waiting for review.
    pub pending_count: i64,
}

/// Health of one market adapter, for the admin view.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdapterStatus {
    pub adapter: String,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
    /// Listings the last successful run saw.
    pub listings_seen: i32,
    /// Of which mapped to a product of the directory.
    pub mapped: i64,
}

/// A listing an adapter saw that no product is mapped to yet.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct UnmappedListing {
    pub adapter: String,
    /// The id to put under the product's `listings:` in the data.
    pub external_id: String,
    pub name: String,
    pub kind: String,
    pub maker: Option<String>,
    pub price_input_per_mtok: Option<f64>,
    pub price_output_per_mtok: Option<f64>,
    pub gpu_type: Option<String>,
    pub price_per_hour: Option<f64>,
    pub first_seen_at: DateTime<Utc>,
}
