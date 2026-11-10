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
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
    /// Items stored from this source.
    pub item_count: i64,
    /// Of which waiting for review.
    pub pending_count: i64,
}
