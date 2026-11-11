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

pub mod akash;
pub mod azure;
pub mod catalog;
pub mod coreweave;
pub mod cudo;
pub mod gpu;
pub mod huggingface;
pub mod hyperbolic;
pub mod ionet;
pub mod lambda;
pub mod linode;
pub mod liquid;
pub mod openrouter;
pub mod oracle;
pub mod rss;
pub mod runpod;
pub mod shadeform;
pub(crate) mod simple;
pub mod vastai;
pub mod verda;
pub mod vultr;

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

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
    /// How often the worker runs the adapter. Marketplaces whose prices move
    /// by the hour run hourly; price lists that change monthly run daily.
    fn interval(&self) -> Duration {
        Duration::from_secs(6 * 3600)
    }
    /// Whether prices come from a live order book or host offers, which can
    /// move several-fold between runs. The store then skips its jump check
    /// and keeps only the plausible-range check.
    fn volatile(&self) -> bool {
        false
    }
}

/// Run cadences, so adapters agree on what "hourly" means.
pub const HOURLY: Duration = Duration::from_secs(3600);
pub const DAILY: Duration = Duration::from_secs(24 * 3600);

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
    /// The provider asked us to slow down (HTTP 429).
    #[error("{adapter} is rate limiting us{}", retry_after.map(|d| format!("; retry after {}s", d.as_secs())).unwrap_or_default())]
    RateLimited {
        adapter: &'static str,
        retry_after: Option<Duration>,
    },
    /// The endpoint is gone (HTTP 404 or 410): the provider moved its API.
    #[error("{0} endpoint not found: {1}")]
    NotFound(&'static str, String),
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

/// Where the adapters send their requests.
///
/// By default every adapter calls its provider. Tests point all of them at
/// one local server with [`Endpoints::all`]; each adapter then requests its
/// usual path on that server.
#[derive(Debug, Clone, Default)]
pub struct Endpoints {
    base: Option<String>,
}

impl Endpoints {
    /// Sends every adapter to `base` (scheme, host and port) instead of its provider.
    pub fn all(base: impl Into<String>) -> Self {
        Self {
            base: Some(base.into().trim_end_matches('/').to_owned()),
        }
    }

    /// The origin an adapter should call: its provider's, or the override.
    pub fn origin(&self, default: &str) -> String {
        self.base
            .clone()
            .unwrap_or_else(|| default.trim_end_matches('/').to_owned())
    }
}

/// Every market adapter, in the order the worker schedules them: model APIs
/// first, then GPU clouds.
pub fn market_adapters(endpoints: &Endpoints) -> Vec<MarketAdapter> {
    let mut adapters = vec![
        MarketAdapter::Gateway(Box::new(openrouter::OpenRouterAdapter::new(
            &endpoints.origin("https://openrouter.ai"),
        ))),
        MarketAdapter::Gateway(Box::new(liquid::LiquidInferenceAdapter::new(
            &endpoints.origin("https://router.inference.ai.exchange"),
        ))),
        MarketAdapter::Gateway(Box::new(huggingface::HuggingFaceRouterAdapter::new(
            &endpoints.origin("https://router.huggingface.co"),
        ))),
    ];
    for spec in catalog::CATALOGS {
        adapters.push(MarketAdapter::Gateway(Box::new(
            catalog::CatalogAdapter::new(spec, endpoints),
        )));
    }
    adapters.extend([
        MarketAdapter::NeoCloud(Box::new(runpod::RunPodAdapter::new(
            &endpoints.origin("https://api.runpod.io"),
        ))),
        MarketAdapter::NeoCloud(Box::new(hyperbolic::HyperbolicAdapter::new(
            &endpoints.origin("https://api.hyperbolic.xyz"),
        ))),
        MarketAdapter::NeoCloud(Box::new(coreweave::CoreWeaveAdapter::new(
            &endpoints.origin("https://www.coreweave.com"),
        ))),
        MarketAdapter::NeoCloud(Box::new(vastai::VastAiAdapter::new(
            &endpoints.origin("https://console.vast.ai"),
        ))),
        MarketAdapter::NeoCloud(Box::new(vultr::VultrAdapter::new(
            &endpoints.origin("https://api.vultr.com"),
        ))),
        MarketAdapter::NeoCloud(Box::new(linode::LinodeAdapter::new(
            &endpoints.origin("https://api.linode.com"),
        ))),
        MarketAdapter::NeoCloud(Box::new(verda::VerdaAdapter::new(
            &endpoints.origin("https://api.datacrunch.io"),
        ))),
        MarketAdapter::NeoCloud(Box::new(akash::AkashAdapter::new(
            &endpoints.origin("https://console-api.akash.network"),
        ))),
        MarketAdapter::NeoCloud(Box::new(ionet::IoNetAdapter::new(
            &endpoints.origin("https://api.io.solutions"),
        ))),
        MarketAdapter::NeoCloud(Box::new(cudo::CudoAdapter::new(
            &endpoints.origin("https://rest.compute.cudo.org"),
        ))),
        MarketAdapter::NeoCloud(Box::new(shadeform::ShadeformAdapter::new(
            &endpoints.origin("https://api.shadeform.ai"),
        ))),
        MarketAdapter::NeoCloud(Box::new(oracle::OracleAdapter::new(
            &endpoints.origin("https://apexapps.oracle.com"),
        ))),
        MarketAdapter::NeoCloud(Box::new(lambda::LambdaAdapter::new(
            &endpoints.origin("https://lambda.ai"),
        ))),
        MarketAdapter::NeoCloud(Box::new(azure::AzureAdapter::new(
            &endpoints.origin("https://prices.azure.com"),
        ))),
    ]);
    adapters
}

/// Least time between two requests to the same host, across all adapters.
/// Some adapters make many requests to one host; this keeps us polite.
const MIN_GAP_PER_HOST: Duration = Duration::from_millis(500);

static LAST_REQUEST: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

/// Waits until the host may be called again, and books the slot.
async fn pace(url: &str) {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| format!("{h}:{}", u.port_or_known_default().unwrap_or(0)))
        })
        .unwrap_or_default();
    let wait = {
        let mut last = LAST_REQUEST.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let slot = last
            .get(&host)
            .map(|t| (*t + MIN_GAP_PER_HOST).max(now))
            .unwrap_or(now);
        last.insert(host, slot);
        slot - now
    };
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
}

/// Checks a response's status, turning 429 and 404 into their own errors.
async fn check(
    adapter: &'static str,
    url: &str,
    response: reqwest::Response,
) -> Result<reqwest::Response, AdapterError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    Err(match status.as_u16() {
        429 => AdapterError::RateLimited {
            adapter,
            retry_after: response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs),
        },
        404 | 410 => AdapterError::NotFound(adapter, url.to_owned()),
        _ => AdapterError::Status(adapter, status),
    })
}

/// POSTs a JSON body and returns the response body.
pub(crate) async fn post_json(
    client: &Client,
    adapter: &'static str,
    url: &str,
    body: &serde_json::Value,
) -> Result<String, AdapterError> {
    pace(url).await;
    let response = client.post(url).json(body).send().await?;
    Ok(check(adapter, url, response).await?.text().await?)
}

/// GETs a URL and returns the body, failing on non-success statuses.
pub(crate) async fn get_text(
    client: &Client,
    adapter: &'static str,
    url: &str,
) -> Result<String, AdapterError> {
    pace(url).await;
    let response = client.get(url).send().await?;
    Ok(check(adapter, url, response).await?.text().await?)
}

/// A price from JSON that may be a number or a decimal string.
pub(crate) fn json_price(
    adapter: &'static str,
    v: &serde_json::Value,
) -> Result<Option<f64>, AdapterError> {
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => {
            let x = n.as_f64().unwrap_or(f64::NAN);
            if !x.is_finite() || x < 0.0 {
                return Err(AdapterError::Format(
                    adapter,
                    format!("price {n} is out of range"),
                ));
            }
            Ok(Some(x))
        }
        serde_json::Value::String(s) if s.trim().is_empty() => Ok(None),
        serde_json::Value::String(s) => parse_price(adapter, s).map(Some),
        other => Err(AdapterError::Format(
            adapter,
            format!("price {other} is not a number"),
        )),
    }
}

/// The maker part of a model id, `meta-llama` in `meta-llama/llama-3.3-70b`.
pub(crate) fn maker_of(id: &str) -> Option<String> {
    id.split_once('/')
        .map(|(m, _)| m.to_ascii_lowercase())
        .filter(|m| !m.is_empty())
}

/// Declares the [`Adapter`] impl of a struct with a fixed id, name, organization and interval.
macro_rules! adapter_info {
    ($ty:ty, $id:expr, $name:expr, $org:expr, $interval:expr) => {
        impl $crate::adapters::Adapter for $ty {
            fn id(&self) -> &'static str {
                $id
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn organization_slug(&self) -> &'static str {
                $org
            }
            fn interval(&self) -> std::time::Duration {
                $interval
            }
        }
    };
    ($ty:ty, $id:expr, $name:expr, $org:expr, $interval:expr, volatile) => {
        impl $crate::adapters::Adapter for $ty {
            fn id(&self) -> &'static str {
                $id
            }
            fn name(&self) -> &'static str {
                $name
            }
            fn organization_slug(&self) -> &'static str {
                $org
            }
            fn interval(&self) -> std::time::Duration {
                $interval
            }
            fn volatile(&self) -> bool {
                true
            }
        }
    };
}
pub(crate) use adapter_info;

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
