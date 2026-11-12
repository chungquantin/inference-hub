use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{Category, FundingRound, PortfolioCompany, ProductSummary};

/// The roles an organization plays in the market. One organization can hold several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
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
    /// Networks and marketplaces that pool compute from many operators.
    Decentralized,
    /// Observability, evaluation, guardrails and benchmarks around serving.
    Llmops,
    /// Coding agents, agent frameworks and harnesses.
    AgentTool,
    Investor,
    Other,
}

impl OrgKind {
    /// The category an organization of this kind is listed under, if any.
    pub fn category_slug(self) -> Option<&'static str> {
        Some(match self {
            OrgKind::ModelProvider => "model-providers",
            OrgKind::InferenceProvider => "inference-providers",
            OrgKind::LlmGateway => "llm-gateways",
            OrgKind::GpuCloud => "gpu-clouds",
            OrgKind::Hyperscaler => "hyperscalers",
            OrgKind::Hardware => "hardware",
            OrgKind::OpenSource => "open-source",
            OrgKind::Decentralized => "decentralized-compute",
            OrgKind::Llmops => "llmops",
            OrgKind::AgentTool => "agent-tools",
            OrgKind::Investor => "investors",
            OrgKind::Other => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
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

/// A news feed an organization publishes, as the hub reads it.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct OrganizationFeed {
    pub name: String,
    /// The RSS, Atom or JSON feed.
    pub url: String,
    /// Items stored from this feed that are published.
    pub published: i64,
    /// Items stored from this feed that wait for review.
    pub pending: i64,
    /// When the feed was last read successfully.
    pub last_success_at: Option<DateTime<Utc>>,
}

/// The full profile of an organization.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrganizationDetail {
    #[serde(flatten)]
    pub organization: Organization,
    pub description: Option<String>,
    /// Named external links, such as `github`, `x` or `docs`.
    #[schema(value_type = std::collections::HashMap<String, String>)]
    pub links: serde_json::Value,
    pub categories: Vec<Category>,
    /// Products the organization makes.
    pub products: Vec<ProductSummary>,
    /// Rounds the organization raised, newest first.
    pub funding: Vec<FundingRound>,
    /// Companies the organization invested in, most recent first.
    pub portfolio: Vec<PortfolioCompany>,
    /// The news feeds the hub reads for this organization.
    pub feeds: Vec<OrganizationFeed>,
    pub updated_at: DateTime<Utc>,
}
