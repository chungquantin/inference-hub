//! Market-wide data: everything the adapters see across providers, whether or
//! not an editor has mapped it to a product of the directory.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::OrganizationRef;

/// Headline numbers of the whole market.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MarketOverview {
    /// Organizations with at least one live listing.
    pub providers: i64,
    pub model_providers: i64,
    pub gpu_providers: i64,
    /// Live (provider, model) price listings.
    pub model_listings: i64,
    /// Distinct models across providers, by normalized id.
    pub distinct_models: i64,
    /// Of which sold by two or more providers: the comparable market.
    pub multi_provider_models: i64,
    /// Live GPU configurations with an hourly price.
    pub gpu_listings: i64,
    pub gpu_types: i64,
    /// Live listings of models priced per unit (embedding, image, video,
    /// speech, transcription), and the distinct models among them.
    pub media_listings: i64,
    pub media_models: i64,
    /// When the freshest listing was last seen.
    pub updated_at: Option<DateTime<Utc>>,
    /// The most widely sold GPUs, for the headline index.
    pub gpu_index: Vec<GpuIndexRow>,
    /// The most widely served models.
    pub model_index: Vec<ModelIndexRow>,
    pub sources: Vec<MarketSource>,
}

/// One GPU type across every cloud: price per GPU-hour.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct GpuIndexRow {
    pub gpu_type: String,
    /// Organizations selling it.
    pub providers: i64,
    pub listings: i64,
    pub min_per_gpu_hour: f64,
    pub median_per_gpu_hour: f64,
    pub max_per_gpu_hour: f64,
    /// Who sells it cheapest.
    #[sqlx(flatten)]
    pub cheapest: OrganizationRef,
}

/// One model across every provider: price per million tokens.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ModelIndexRow {
    /// Normalized id shared by providers, such as `deepseek-v3.2`.
    pub model_key: String,
    /// A display name: the most common name providers give it.
    pub name: String,
    pub maker: Option<String>,
    pub providers: i64,
    pub min_input_per_mtok: f64,
    pub min_output_per_mtok: f64,
    /// Cheapest 3:1 blend of input and output.
    pub min_blended_per_mtok: f64,
    pub median_blended_per_mtok: f64,
    /// The largest any provider states, or what models.dev knows when none does.
    pub max_context_window: Option<i32>,
    /// When the model was released, from models.dev.
    pub released_on: Option<chrono::NaiveDate>,
    pub open_weights: Option<bool>,
    /// What providers say it can do: `in:<modality>`, `out:<modality>` and
    /// features (`tools`, `json`, `reasoning`, `web`, `caching`).
    pub capabilities: Vec<String>,
    #[sqlx(flatten)]
    pub cheapest: OrganizationRef,
}

/// One data source of the market: a provider's adapter and its health.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MarketSource {
    pub adapter: String,
    #[sqlx(flatten)]
    pub organization: OrganizationRef,
    /// `model` or `gpu`; `None` before the first successful run.
    pub kind: Option<String>,
    pub listings: i64,
    pub last_success_at: Option<DateTime<Utc>>,
    pub healthy: bool,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MarketIndexQuery {
    /// Text the GPU type or model id must contain (case-insensitive).
    pub q: Option<String>,
    /// Keep rows sold by at least this many providers. Default 1.
    pub min_providers: Option<i64>,
    /// Most rows to return. Default 100, at most 500.
    pub limit: Option<i64>,
    /// Rows to skip, for paging. Default 0.
    pub offset: Option<i64>,
    /// Keep models (or GPUs) this organization sells, by slug. Prices and
    /// provider counts still cover the whole market.
    pub provider: Option<String>,
    /// Models only: keep this maker (as providers name it, such as `deepseek-ai`).
    pub maker: Option<String>,
    /// Lowest price at most / at least this: USD per 1M tokens (blend) for
    /// models, USD per GPU-hour for GPUs.
    pub max_price: Option<f64>,
    pub min_price: Option<f64>,
    /// Models only: context window of at least this many tokens.
    pub min_context: Option<i64>,
    /// `providers` (default), `price`, `median`, `input`, `output`,
    /// `context`, `released` or `name`.
    pub sort: Option<String>,
    /// `asc` or `desc`; each sort has a natural default.
    pub dir: Option<String>,
    /// Models only: keep those with every listed capability, comma-separated,
    /// such as `tools,in:image`.
    pub caps: Option<String>,
}

/// A maker and how many models it has, for filters.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MakerCount {
    pub maker: String,
    pub models: i64,
}

/// One provider's live price for a model or GPU, as the adapter saw it.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MarketListing {
    pub adapter: String,
    #[sqlx(flatten)]
    pub provider: OrganizationRef,
    pub external_id: String,
    pub name: String,
    /// Reseller detail: the hosting provider behind a router, or a region or plan.
    pub variant: Option<String>,
    pub context_window: Option<i32>,
    pub price_input_per_mtok: Option<f64>,
    pub price_output_per_mtok: Option<f64>,
    pub blended_per_mtok: Option<f64>,
    pub gpu_count: Option<i16>,
    pub price_per_hour: Option<f64>,
    pub price_per_gpu_hour: Option<f64>,
    /// Non-chat models: what the price is per, and the price.
    pub unit: Option<String>,
    pub price_per_unit: Option<f64>,
    /// What this provider says the model can do.
    pub capabilities: Vec<String>,
    pub source_url: Option<String>,
    pub last_seen_at: DateTime<Utc>,
}

/// Every provider's price for one model or GPU type.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MarketDetail {
    pub key: String,
    pub name: String,
    /// Models: the maker as most providers name it, such as `deepseek-ai`.
    pub maker: Option<String>,
    /// Cheapest first.
    pub listings: Vec<MarketListing>,
}

/// A provider in the market leaderboard.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MarketProvider {
    #[sqlx(flatten)]
    pub organization: OrganizationRef,
    /// Distinct chat models with a price (variants such as `:free` or
    /// `@region` count once; models only offered free are left out).
    pub models: i64,
    /// Chat model listings as the provider lists them, variants included.
    pub model_listings: i64,
    /// Embedding, image, video, speech and transcription models.
    pub media_models: i64,
    pub gpu_configurations: i64,
    /// GPU types or models sold by two or more providers where this one is cheapest.
    pub cheapest_count: i64,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// A price an adapter reported that failed a sanity check, waiting for an editor.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct PriceHold {
    pub id: i64,
    pub adapter: String,
    pub external_id: String,
    pub kind: String,
    pub name: String,
    /// `out_of_band` or `jump`.
    pub reason: String,
    pub detail: Option<String>,
    pub price_input_per_mtok: Option<f64>,
    pub price_output_per_mtok: Option<f64>,
    pub price_per_hour: Option<f64>,
    pub previous_input_per_mtok: Option<f64>,
    pub previous_output_per_mtok: Option<f64>,
    pub previous_per_hour: Option<f64>,
    pub gpu_count: Option<i16>,
    pub held_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// An editor's decision on a held price.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoldResolution {
    /// Store the price on the adapter's next run.
    Accepted,
    /// Keep the old price; the same held price stays silent.
    Rejected,
}

/// A job that failed every attempt.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FailedJob {
    pub id: i64,
    pub job_id: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub failed_at: DateTime<Utc>,
}

/// One non-chat model across providers: its unit price.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MediaIndexRow {
    pub model_key: String,
    pub name: String,
    pub maker: Option<String>,
    /// `embedding`, `image`, `video`, `speech` or `transcription`.
    pub kind: String,
    /// What the price is per: `mtok_input`, `image`, `video_second`,
    /// `character` or `audio_second`.
    pub unit: String,
    pub providers: i64,
    pub min_price: f64,
    pub median_price: f64,
    pub capabilities: Vec<String>,
    #[sqlx(flatten)]
    pub cheapest: OrganizationRef,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MediaIndexQuery {
    /// `embedding`, `image`, `video`, `speech` or `transcription`. Default all.
    pub kind: Option<String>,
    /// Text the model id or name must contain (case-insensitive).
    pub q: Option<String>,
    /// Keep rows sold by at least this many providers. Default 1.
    pub min_providers: Option<i64>,
    /// Default 100, at most 500.
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Keep models this organization sells, by slug.
    pub provider: Option<String>,
}

/// One thing a provider sells: its price next to the market's.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ProviderOffering {
    /// `model`, `gpu`, `embedding`, `image`, `video`, `speech` or `transcription`.
    pub kind: String,
    /// The market key: a model key, or a GPU type.
    pub item_key: String,
    pub name: String,
    pub maker: Option<String>,
    /// The curated product page, when editors mapped this item.
    pub product_slug: Option<String>,
    /// `adapter` (collected from the provider) and/or `curated` (the data files).
    pub sources: Vec<String>,
    /// Listings of this item at the provider: variants, regions, sizes.
    pub variants: i32,
    /// The provider's lowest price: blended USD per 1M tokens for chat models,
    /// USD per GPU-hour for GPUs, USD per `unit` for the rest.
    pub price: Option<f64>,
    pub input_per_mtok: Option<f64>,
    pub output_per_mtok: Option<f64>,
    pub unit: Option<String>,
    /// Providers selling it, and the market's lowest and median price.
    pub market_providers: Option<i32>,
    pub market_min: Option<f64>,
    pub market_median: Option<f64>,
    /// 1 = cheapest in the market.
    pub rank: Option<i32>,
    pub capabilities: Vec<String>,
    pub context_window: Option<i32>,
    pub source_url: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// How many items a provider sells of each kind, for tabs.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct KindCount {
    pub kind: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProviderOfferingsQuery {
    /// One kind; default all kinds.
    pub kind: Option<String>,
    /// Text the name or key must contain (case-insensitive).
    pub q: Option<String>,
    /// Default 50, at most 500.
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// `providers` (default), `price`, `rank`, `gap` (how far above the
    /// market low), or `name`.
    pub sort: Option<String>,
    /// `asc` or `desc`.
    pub dir: Option<String>,
    /// Only items where this provider is the cheapest.
    pub cheapest: Option<bool>,
}

/// A provider's offerings, one page, with counts per kind.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProviderOfferings {
    pub kinds: Vec<KindCount>,
    pub items: Vec<ProviderOffering>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
