//! Market analytics: rankings and distributions across models, GPUs and providers.
//!
//! Everything here is derived from the live listings the adapters collect, by the
//! same refresh that builds the market index tables, so a page of rankings costs
//! one read and never aggregates at request time.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::OrganizationRef;

/// One chat model sold by two or more providers.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ModelRank {
    pub model_key: String,
    pub name: String,
    pub maker: Option<String>,
    /// Providers selling it at a price.
    pub providers: i64,
    /// Cheapest, median and priciest provider price, USD per million tokens (3:1 input:output blend).
    pub min_blended_per_mtok: f64,
    pub median_blended_per_mtok: f64,
    pub max_blended_per_mtok: f64,
    pub min_input_per_mtok: Option<f64>,
    pub min_output_per_mtok: Option<f64>,
    /// Priciest provider over the cheapest: what a buyer saves by shopping around.
    pub spread: f64,
    pub max_context_window: Option<i32>,
    pub capabilities: Vec<String>,
    /// Who sells it cheapest.
    #[sqlx(flatten)]
    pub cheapest: OrganizationRef,
}

/// A price band of the model histogram.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct PriceBucket {
    /// Lower bound, inclusive, USD per million tokens.
    pub from: f64,
    /// Upper bound, exclusive.
    pub to: f64,
    /// Chat models whose cheapest price falls in the band.
    pub models: i64,
    /// Of which sold by two or more providers.
    pub contested: i64,
}

/// A model maker's footprint in the market.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MakerRank {
    pub maker: String,
    /// Priced chat models, variants merged.
    pub models: i64,
    /// Of which sold by two or more providers.
    pub contested_models: i64,
    /// Mean number of providers per model.
    pub avg_providers: f64,
    /// Median over the maker's models of the cheapest blended price, USD per million tokens.
    pub median_min_price: f64,
    /// Share of the maker's models sold by two or more providers, 0 to 1.
    pub contested_share: f64,
}

/// One GPU type across clouds.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct GpuRank {
    pub gpu_type: String,
    pub providers: i64,
    pub listings: i64,
    pub min_per_gpu_hour: f64,
    pub median_per_gpu_hour: f64,
    pub max_per_gpu_hour: f64,
    /// Priciest listing over the cheapest.
    pub spread: f64,
    #[sqlx(flatten)]
    pub cheapest: OrganizationRef,
}

/// One cloud's lowest price for one GPU type: a cell of the price matrix.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct GpuCell {
    pub gpu_type: String,
    #[sqlx(flatten)]
    pub provider: OrganizationRef,
    pub per_gpu_hour: f64,
    /// 1 is the cheapest in the market.
    pub rank: i32,
    /// This price over the market median for the GPU type: below 1 is cheaper than most.
    pub vs_median: f64,
}

/// A provider scored across the items it shares with other providers.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ProviderRank {
    #[sqlx(flatten)]
    pub organization: OrganizationRef,
    /// Priced chat models in its catalog.
    pub models: i64,
    pub media_models: i64,
    pub gpu_configurations: i64,
    /// Chat models and GPU types it sells that at least one other provider sells too.
    pub contested_models: i64,
    pub contested_gpus: i64,
    /// Contested items where its price is the lowest in the market.
    pub model_wins: i64,
    pub gpu_wins: i64,
    /// Geometric mean of its price over the market median on contested items.
    /// 1.0 is the market, 0.5 is half the median; null when it sells none.
    pub model_price_index: Option<f64>,
    pub gpu_price_index: Option<f64>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Everything the rankings dashboard shows.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MarketAnalytics {
    /// When the summaries these were computed from were refreshed.
    pub refreshed_at: Option<DateTime<Utc>>,
    /// Chat models sold by two or more providers, widest sold first.
    pub models: Vec<ModelRank>,
    pub price_histogram: Vec<PriceBucket>,
    pub makers: Vec<MakerRank>,
    pub gpus: Vec<GpuRank>,
    pub gpu_prices: Vec<GpuCell>,
    pub providers: Vec<ProviderRank>,
}
