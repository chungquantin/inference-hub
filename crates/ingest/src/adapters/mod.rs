//! Adapters: one per data provider, each turning its public API or page into
//! a common shape.
//!
//! - [`GatewayAdapter`]: model API marketplaces and routers that resell token
//!   access to many models (OpenRouter, Liquid Inference). They return
//!   [`ModelListing`]s.
//! - [`NeoCloudAdapter`]: GPU clouds that rent instances by the hour (RunPod,
//!   Hyperbolic, CoreWeave). They return [`GpuListing`]s.
//! - [`NewsAdapter`]: sources of articles (feeds). They return [`FeedEntry`]s.
//!
//! Every adapter also implements [`Adapter`], which names it and the
//! organization of the directory it reports for. Adding a provider means
//! writing one adapter and registering it in [`market_adapters`].

pub mod coreweave;
pub mod gpu;
pub mod hyperbolic;
pub mod liquid;
pub mod openrouter;
pub mod rss;
pub mod runpod;

use std::{future::Future, pin::Pin};

use reqwest::Client;
use serde::Serialize;

use crate::feed::FeedEntry;

/// A boxed future, so traits with async methods stay object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What every adapter declares about itself.
pub trait Adapter: Send + Sync {
    /// Stable id, stored with everything the adapter records. Never rename it.
    fn id(&self) -> &'static str;
    /// Human-readable name for logs and the admin page.
    fn name(&self) -> &'static str;
    /// Slug of the organization the adapter reports prices for. Gateways
    /// report their own prices; neoclouds their own instances.
    fn organization_slug(&self) -> &'static str;
}

/// A model API sold through a gateway, at the gateway's price.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelListing {
    /// The gateway's model id, such as `meta-llama/llama-3.3-70b-instruct`.
    pub external_id: String,
    pub name: String,
    /// Who made the model, as the gateway says it, lower case.
    pub maker: Option<String>,
    pub context_window: Option<i32>,
    /// USD per million input tokens.
    pub input_per_mtok: f64,
    /// USD per million output tokens.
    pub output_per_mtok: f64,
    /// Region or provider variant when the gateway has several prices for one model.
    pub variant: Option<String>,
    pub source_url: Option<String>,
    pub extra: serde_json::Value,
}

/// A GPU configuration rented by the hour.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GpuListing {
    /// The cloud's id of the GPU type, such as `NVIDIA H100 80GB HBM3`.
    pub external_id: String,
    /// The cloud's name for the configuration.
    pub name: String,
    /// Normalized GPU name, such as `H100 SXM`. See [`gpu::normalize`].
    pub gpu_type: String,
    pub gpu_count: i16,
    /// On-demand USD per hour for the whole configuration.
    pub price_per_hour: f64,
    pub region: Option<String>,
    pub source_url: Option<String>,
    /// Other prices (spot, community) and details, kept for reference.
    pub extra: serde_json::Value,
}

/// Why a fetch failed.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0} answered HTTP {1}")]
    Status(&'static str, reqwest::StatusCode),
    /// The response arrived but was not what the adapter expects: the
    /// provider changed its API or page. Better to fail than to store junk.
    #[error("unexpected response from {0}: {1}")]
    Format(&'static str, String),
}

/// A source of model API prices.
pub trait GatewayAdapter: Adapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>>;
}

/// A source of GPU instance prices.
pub trait NeoCloudAdapter: Adapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>>;
}

/// A source of articles.
pub trait NewsAdapter: Adapter {
    fn fetch_entries<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<FeedEntry>, AdapterError>>;
}

/// One market adapter of either kind, as the worker runs them.
pub enum MarketAdapter {
    Gateway(Box<dyn GatewayAdapter>),
    NeoCloud(Box<dyn NeoCloudAdapter>),
}

impl MarketAdapter {
    pub fn info(&self) -> &dyn Adapter {
        match self {
            MarketAdapter::Gateway(a) => a.as_ref(),
            MarketAdapter::NeoCloud(a) => a.as_ref(),
        }
    }
}

/// Base URLs of every provider API, so tests can point adapters at local servers.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub openrouter: String,
    pub liquid: String,
    pub runpod: String,
    pub hyperbolic: String,
    pub coreweave: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            openrouter: "https://openrouter.ai".into(),
            liquid: "https://router.inference.ai.exchange".into(),
            runpod: "https://api.runpod.io".into(),
            hyperbolic: "https://api.hyperbolic.xyz".into(),
            coreweave: "https://www.coreweave.com".into(),
        }
    }
}

/// Every market adapter, in the order the worker schedules them.
pub fn market_adapters(endpoints: &Endpoints) -> Vec<MarketAdapter> {
    vec![
        MarketAdapter::Gateway(Box::new(openrouter::OpenRouterAdapter::new(
            &endpoints.openrouter,
        ))),
        MarketAdapter::Gateway(Box::new(liquid::LiquidInferenceAdapter::new(
            &endpoints.liquid,
        ))),
        MarketAdapter::NeoCloud(Box::new(runpod::RunPodAdapter::new(&endpoints.runpod))),
        MarketAdapter::NeoCloud(Box::new(hyperbolic::HyperbolicAdapter::new(
            &endpoints.hyperbolic,
        ))),
        MarketAdapter::NeoCloud(Box::new(coreweave::CoreWeaveAdapter::new(
            &endpoints.coreweave,
        ))),
    ]
}

/// GETs a URL and returns the body, failing on non-success statuses.
pub(crate) async fn get_text(
    client: &Client,
    adapter: &'static str,
    url: &str,
) -> Result<String, AdapterError> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(AdapterError::Status(adapter, response.status()));
    }
    Ok(response.text().await?)
}

/// Parses a decimal price string, such as OpenRouter's per-token `"0.00000071"`.
pub(crate) fn parse_price(adapter: &'static str, s: &str) -> Result<f64, AdapterError> {
    let value: f64 = s
        .trim()
        .parse()
        .map_err(|_| AdapterError::Format(adapter, format!("price `{s}` is not a number")))?;
    if !value.is_finite() || value < 0.0 {
        return Err(AdapterError::Format(
            adapter,
            format!("price `{s}` is out of range"),
        ));
    }
    Ok(value)
}

/// Rounds a price to 6 decimals, so per-token × 1M conversions do not leave
/// floating-point noise such as 0.7100000000000001.
pub(crate) fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}
