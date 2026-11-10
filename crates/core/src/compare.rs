use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{OrganizationRef, ProductType};

/// A row of the model price comparison: one model at one provider, latest price.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ModelPrice {
    pub product_slug: String,
    pub product_name: String,
    /// The lab that makes the model.
    #[sqlx(flatten)]
    pub maker: MakerRef,
    #[sqlx(flatten)]
    pub provider: OrganizationRef,
    pub open_weights: bool,
    pub context_window: Option<i32>,
    pub parameters_b: Option<f64>,
    /// USD per million input tokens.
    pub price_input_per_mtok: Option<f64>,
    /// USD per million output tokens.
    pub price_output_per_mtok: Option<f64>,
    /// Price of 3 input tokens per output token, per million tokens: a single
    /// number for sorting chat-style workloads.
    pub blended_per_mtok: Option<f64>,
    pub currency: String,
    pub source_url: Option<String>,
    pub observed_at: DateTime<Utc>,
}

/// The organization that makes a product, read from `maker_*` columns.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MakerRef {
    #[sqlx(rename = "maker_slug")]
    pub slug: String,
    #[sqlx(rename = "maker_name")]
    pub name: String,
}

/// A row of the GPU price comparison: one configuration at one provider.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct GpuPrice {
    pub product_slug: String,
    pub product_name: String,
    #[sqlx(flatten)]
    pub provider: OrganizationRef,
    pub gpu_type: String,
    pub gpu_count: i16,
    pub price_per_hour: f64,
    /// Price per hour divided by the number of GPUs.
    pub price_per_gpu_hour: f64,
    pub region: Option<String>,
    pub currency: String,
    pub source_url: Option<String>,
    pub observed_at: DateTime<Utc>,
}

/// Sort orders of the model comparison.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelPriceSort {
    #[default]
    Blended,
    Input,
    Output,
    ContextWindow,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ModelPriceQuery {
    /// Keep models with open weights.
    pub open_weights: Option<bool>,
    /// Keep models made by this organization (slug).
    pub maker: Option<String>,
    /// Keep offerings from this provider (slug).
    pub provider: Option<String>,
    /// Keep models with at least this context window, in tokens.
    pub min_context: Option<i32>,
    /// Default `blended`, cheapest first.
    pub sort: Option<ModelPriceSort>,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GpuPriceQuery {
    /// Keep offerings of this GPU type, such as `H100 SXM` (case-insensitive).
    pub gpu: Option<String>,
    /// Keep offerings from this provider (slug).
    pub provider: Option<String>,
}

/// One observation of a provider's price for a product, for price history.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct PricePoint {
    #[sqlx(flatten)]
    pub provider: OrganizationRef,
    /// Distinguishes configurations of one provider, such as `8× H100 SXM` or a region.
    pub configuration: Option<String>,
    pub price_input_per_mtok: Option<f64>,
    pub price_output_per_mtok: Option<f64>,
    pub price_per_hour: Option<f64>,
    pub currency: String,
    pub observed_at: DateTime<Utc>,
}

/// A funding round as shown on the company's page.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FundingRound {
    pub round: String,
    pub announced_on: NaiveDate,
    /// Amount raised, in `currency`.
    pub amount: Option<i64>,
    /// Post-money valuation, in `currency`.
    pub valuation: Option<i64>,
    pub currency: String,
    pub source_url: Option<String>,
    pub notes: Option<String>,
    pub investors: Vec<RoundInvestor>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoundInvestor {
    pub slug: String,
    pub name: String,
    pub lead: bool,
}

/// A company an investor backed, with the rounds it joined.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PortfolioCompany {
    #[serde(flatten)]
    pub organization: OrganizationRef,
    pub rounds: Vec<PortfolioRound>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PortfolioRound {
    pub round: String,
    pub announced_on: NaiveDate,
    pub amount: Option<i64>,
    pub currency: String,
    pub lead: bool,
}

/// Latest GitHub stats of an open-source product.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RepoStats {
    /// `owner/name`.
    pub repo: String,
    pub stars: Option<i32>,
    pub forks: Option<i32>,
    pub open_issues: Option<i32>,
    /// SPDX identifier, such as `Apache-2.0`.
    pub license: Option<String>,
    pub archived: bool,
    pub pushed_at: Option<DateTime<Utc>>,
    pub latest_release: Option<String>,
    pub released_at: Option<DateTime<Utc>>,
    /// Stars gained over the last 30 days of recorded history, when there is any.
    pub stars_30d: Option<i32>,
    pub synced_at: DateTime<Utc>,
}

/// A row of the open-source leaderboard.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RepoLeader {
    pub product_slug: String,
    pub product_name: String,
    pub product_type: ProductType,
    pub organization: OrganizationRef,
    #[serde(flatten)]
    pub stats: RepoStats,
}
