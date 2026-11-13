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
pub mod clouds;
pub mod coreweave;
pub mod cudo;
pub mod gpu;
pub mod huggingface;
pub mod hyperbolic;
pub mod ionet;
pub mod labs;
pub mod lambda;
pub mod linode;
pub mod liquid;
pub mod models_dev;
pub mod openrouter;
pub mod oracle;
pub mod ovhcloud;
pub mod paging;
pub mod rss;
pub mod runpod;
pub mod scaleway;
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
    /// Whether the list is the inventory available right now (a marketplace, or a cloud that
    /// lists only the configurations in stock), which can shrink a lot between runs with
    /// nothing broken. The store then refuses only an empty run, not one that is under half
    /// of the last. Volatile sources are inventory by nature.
    fn inventory_varies(&self) -> bool {
        self.volatile()
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
    /// For models not priced by chat tokens: what they make and the unit
    /// price. `input_per_mtok` and `output_per_mtok` are then not stored.
    pub media: Option<MediaPrice>,
    /// What the model can do at this provider, in the [`caps`] vocabulary.
    pub capabilities: Vec<String>,
}

/// What a non-chat model produces, which decides its unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// Priced per million input tokens.
    Embedding,
    /// Priced per image.
    Image,
    /// Priced per second of video.
    Video,
    /// Text to speech, priced per input character.
    Speech,
    /// Speech to text, priced per second of audio.
    Transcription,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Embedding => "embedding",
            MediaKind::Image => "image",
            MediaKind::Video => "video",
            MediaKind::Speech => "speech",
            MediaKind::Transcription => "transcription",
        }
    }

    /// The unit `price_per_unit` is per.
    pub fn unit(self) -> &'static str {
        match self {
            MediaKind::Embedding => "mtok_input",
            MediaKind::Image => "image",
            MediaKind::Video => "video_second",
            MediaKind::Speech => "character",
            MediaKind::Transcription => "audio_second",
        }
    }

    /// Plausible upper bound of one unit, in USD. Catches unit mistakes
    /// (cents read as dollars, per-minute read as per-second), not high prices.
    pub fn max_price(self) -> f64 {
        match self {
            MediaKind::Embedding => 200.0,
            MediaKind::Image => 5.0,
            MediaKind::Video => 20.0,
            MediaKind::Speech => 0.01,
            MediaKind::Transcription => 0.1,
        }
    }
}

/// The price of a non-chat model in its own unit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MediaPrice {
    pub kind: MediaKind,
    /// USD per unit (see [`MediaKind::unit`]).
    pub price: f64,
}

impl ModelListing {
    /// A listing priced per unit rather than by chat tokens.
    pub fn media(
        external_id: String,
        name: String,
        maker: Option<String>,
        kind: MediaKind,
        price: f64,
        source_url: Option<String>,
        extra: serde_json::Value,
    ) -> Self {
        Self {
            external_id,
            name,
            maker,
            context_window: None,
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
            variant: None,
            source_url,
            extra,
            media: Some(MediaPrice { kind, price }),
            capabilities: Vec::new(),
        }
    }
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
    /// The provider publishes prices that apply from a later date and shows no current ones.
    /// Not a failure: nothing is recorded, the adapter keeps its schedule, and the next run
    /// after the date picks the prices up.
    #[error("{0} publishes prices that take effect on {1}; nothing is recorded before then")]
    NotYet(&'static str, chrono::NaiveDate),
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
    for spec in models_dev::PROVIDERS {
        adapters.push(MarketAdapter::Gateway(Box::new(
            models_dev::ModelsDevAdapter::new(spec, endpoints),
        )));
    }
    for spec in labs::LABS.iter().filter(|s| s.enabled) {
        adapters.push(MarketAdapter::Gateway(Box::new(labs::LabAdapter::new(
            spec, endpoints,
        ))));
    }
    adapters.extend(
        clouds::CLOUDS.iter().filter(|s| s.enabled).map(|spec| {
            MarketAdapter::NeoCloud(Box::new(clouds::CloudAdapter::new(spec, endpoints)))
        }),
    );
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
        MarketAdapter::NeoCloud(Box::new(ovhcloud::OvhCloudAdapter::new(
            &endpoints.origin("https://api.ovh.com"),
            &endpoints.origin("https://api.frankfurter.dev"),
        ))),
        MarketAdapter::NeoCloud(Box::new(scaleway::ScalewayAdapter::new(
            &endpoints.origin("https://api.scaleway.com"),
            &endpoints.origin("https://api.frankfurter.dev"),
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
    // One retry for transport errors (a dropped connection mid-body, a reset):
    // they are common on large catalogs and say nothing about the provider.
    // HTTP errors are not retried here; the scheduler backs off on them.
    let mut attempt = 0;
    loop {
        attempt += 1;
        pace(url).await;
        let result = async {
            let response = client.get(url).send().await?;
            Ok::<_, AdapterError>(check(adapter, url, response).await?.text().await?)
        }
        .await;
        match result {
            Err(AdapterError::Http(err)) if attempt < 2 && !err.is_status() => {
                tracing::warn!(adapter, url, error = %err, "transport error; retrying once");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            other => return other,
        }
    }
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

/// Rounds a small unit price (per character, per audio second) to 10
/// significant digits: keeps real precision, drops float noise from unit
/// conversions such as cents × 0.01.
pub(crate) fn round_sig(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let digits = 10 - (x.abs().log10().floor() as i32) - 1;
    let factor = 10f64.powi(digits);
    (x * factor).round() / factor
}

/// Rounds a price to 6 decimals, so per-token × 1M conversions do not leave
/// floating-point noise such as 0.7100000000000001.
pub(crate) fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// What a model can do, in one vocabulary across providers.
///
/// Input and output modalities are `in:<m>` and `out:<m>` with `m` one of
/// `text`, `image`, `audio`, `video`, `file`. Features: `tools` (function
/// calling), `json` (JSON mode or structured outputs), `reasoning`, `web`
/// (built-in web search) and `caching` (prompt caching).
pub mod caps {
    pub const MODALITIES: [&str; 5] = ["text", "image", "audio", "video", "file"];
    pub const FEATURES: [&str; 5] = ["tools", "json", "reasoning", "web", "caching"];

    /// Maps one provider term to the vocabulary, or `None` when it names
    /// nothing we show. `direction` is `in` or `out` for modality lists.
    pub fn modality(direction: &str, raw: &str) -> Option<String> {
        let m = match raw.trim().to_ascii_lowercase().as_str() {
            "text" | "transcript" => "text",
            "image" | "images" | "vision" => "image",
            "audio" | "speech" => "audio",
            "video" => "video",
            "file" | "files" | "pdf" | "document" => "file",
            _ => return None,
        };
        Some(format!("{direction}:{m}"))
    }

    /// Maps a feature name from any provider (`function-calling`,
    /// `structured_outputs`, `response_format.json_schema`, `tool-use`, ...).
    pub fn feature(raw: &str) -> Option<&'static str> {
        let r = raw.trim().to_ascii_lowercase().replace(['_', '-', ' '], "");
        Some(match r.as_str() {
            "tools" | "tool" | "tooluse" | "toolcalling" | "functioncalling" | "function"
            | "functions" | "toolchoice" => "tools",
            "json"
            | "jsonmode"
            | "structuredoutputs"
            | "structuredoutput"
            | "responseformat"
            | "responseformat.jsonobject"
            | "responseformat.jsonschema"
            | "responseschema"
            | "outputjsonobject"
            | "outputjsonschema" => "json",
            "reasoning"
            | "includereasoning"
            | "reasoningeffort"
            | "thinking"
            | "reasoning.effort"
            | "reasoning.budgettokens" => "reasoning",
            "websearch" | "web" | "search" => "web",
            "caching" | "promptcaching" | "cache" => "caching",
            _ => return None,
        })
    }

    /// Sorted, deduplicated, in display order: inputs, outputs, features.
    pub fn normalize(mut caps: Vec<String>) -> Vec<String> {
        let rank = |c: &str| -> usize {
            if let Some(m) = c.strip_prefix("in:") {
                MODALITIES.iter().position(|x| *x == m).unwrap_or(9)
            } else if let Some(m) = c.strip_prefix("out:") {
                10 + MODALITIES.iter().position(|x| *x == m).unwrap_or(9)
            } else {
                20 + FEATURES.iter().position(|x| *x == c).unwrap_or(9)
            }
        };
        caps.sort_by_key(|c| (rank(c), c.clone()));
        caps.dedup();
        caps
    }

    /// Reads capabilities from common JSON shapes: modality lists (strings or
    /// `{type}` objects), feature lists, and `supports_*` booleans.
    pub fn from_json(v: &serde_json::Value) -> Vec<String> {
        use serde_json::Value;
        let mut out = Vec::new();
        let strings = |x: &Value| -> Vec<String> {
            x.as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| match e {
                    Value::String(s) => Some(s.clone()),
                    Value::Object(o) => o.get("type").and_then(Value::as_str).map(str::to_owned),
                    _ => None,
                })
                .collect()
        };
        let at = |path: &[&str]| path.iter().try_fold(v, |v, k| v.get(*k));
        for (dir, paths) in [
            (
                "in",
                [
                    &["input_modalities"][..],
                    &["architecture", "input_modalities"],
                    &["modalities", "input"],
                ],
            ),
            (
                "out",
                [
                    &["output_modalities"][..],
                    &["architecture", "output_modalities"],
                    &["modalities", "output"],
                ],
            ),
        ] {
            for p in paths {
                if let Some(x) = at(p) {
                    out.extend(strings(x).iter().filter_map(|m| modality(dir, m)));
                }
            }
        }
        for p in [
            &["supported_features"][..],
            &["supported_parameters"],
            &["features"],
            &["tags"],
        ] {
            if let Some(x) = at(p) {
                out.extend(
                    strings(x)
                        .iter()
                        .filter_map(|f| feature(f))
                        .map(str::to_owned),
                );
            }
        }
        // `supports_vision`, `supportsFunctionCalling`, `supports_tools`, ...
        let flags = [
            ("vision", "in:image"),
            ("audioinput", "in:audio"),
            ("videoinput", "in:video"),
            ("toolcalling", "tools"),
            ("tools", "tools"),
            ("functioncalling", "tools"),
            ("structuredoutput", "json"),
            ("responseschema", "json"),
            ("outputjsonschema", "json"),
            ("outputjsonobject", "json"),
            ("reasoning", "reasoning"),
            ("websearch", "web"),
            ("caching", "caching"),
            ("imagegeneration", "out:image"),
        ];
        for obj in [
            Some(v),
            at(&["capabilities"]),
            at(&["model_spec", "capabilities"]),
        ]
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        {
            for (k, val) in obj {
                if val.as_bool() != Some(true) {
                    continue;
                }
                let key = k.to_ascii_lowercase().replace('_', "");
                let Some(name) = key.strip_prefix("supports") else {
                    continue;
                };
                if let Some((_, cap)) = flags.iter().find(|(f, _)| *f == name) {
                    out.push((*cap).to_owned());
                }
            }
        }
        normalize(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        #[test]
        fn reads_each_providers_shape() {
            // OpenRouter
            let or = json!({"architecture":{"input_modalities":["text","image"],"output_modalities":["text"]},
                            "supported_parameters":["tools","response_format","include_reasoning","top_p"]});
            assert_eq!(
                from_json(&or),
                [
                    "in:text",
                    "in:image",
                    "out:text",
                    "tools",
                    "json",
                    "reasoning"
                ]
            );
            // Liquid: objects, dotted features
            let lq = json!({"input_modalities":[{"type":"text"},{"type":"audio"}],"output_modalities":[{"type":"text"}],
                            "supported_features":["tools","response_format.json_schema","reasoning.effort","sampling.seed"]});
            assert_eq!(
                from_json(&lq),
                [
                    "in:text",
                    "in:audio",
                    "out:text",
                    "tools",
                    "json",
                    "reasoning"
                ]
            );
            // Requesty / Venice flags
            let rq = json!({"supports_vision":true,"supports_tool_calling":true,"supports_web_search":true,
                            "supports_caching":true,"supports_image_generation":false});
            assert_eq!(from_json(&rq), ["in:image", "tools", "web", "caching"]);
            let ve = json!({"model_spec":{"capabilities":{"supportsVision":true,"supportsFunctionCalling":true,
                            "supportsReasoning":true,"supportsVideoInput":true}}});
            assert_eq!(
                from_json(&ve),
                ["in:image", "in:video", "tools", "reasoning"]
            );
            // Novita, Vercel
            let nv = json!({"features":["function-calling","structured-outputs"],"input_modalities":["text","image"]});
            assert_eq!(from_json(&nv), ["in:text", "in:image", "tools", "json"]);
            let vc = json!({"modalities":{"input":["text","file"],"output":["text"]},"tags":["tool-use","reasoning"]});
            assert_eq!(
                from_json(&vc),
                ["in:text", "in:file", "out:text", "tools", "reasoning"]
            );
        }
    }
}
