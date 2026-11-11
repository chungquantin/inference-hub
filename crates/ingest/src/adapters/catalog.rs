//! Model API providers whose public model list carries prices. Most follow
//! the OpenAI `/v1/models` shape with a provider-specific `pricing` object, so
//! one adapter serves them all: a [`CatalogSpec`] says where the list is and
//! how to read one entry.
//!
//! Every spec here was checked against the live, keyless endpoint. Prices are
//! converted to USD per million tokens; entries without a token price (image,
//! audio or embedding models, routers) are skipped rather than guessed.

use std::time::Duration;

use reqwest::Client;
use serde_json::Value;

use super::{
    Adapter, AdapterError, BoxFuture, DAILY, Endpoints, GatewayAdapter, HOURLY, ModelListing,
    get_text, json_price, maker_of, round6,
};

/// One model as a provider lists it, before validation.
#[derive(Debug, Default)]
pub struct Entry {
    pub id: String,
    pub name: Option<String>,
    pub maker: Option<String>,
    pub context: Option<i64>,
    /// USD per million tokens.
    pub input: f64,
    pub output: f64,
    pub extra: Value,
}

/// How to read one provider's model list.
pub struct CatalogSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub organization: &'static str,
    /// Origin of the API, such as `https://api.deepinfra.com`.
    pub origin: &'static str,
    pub path: &'static str,
    pub interval: Duration,
    /// Page a reader can check the price on. `{id}` is replaced by the model id.
    pub source_url: &'static str,
    /// Reads one element of the list. `Ok(None)` skips it (not a priced text model).
    pub entry: fn(&Value) -> Result<Option<Entry>, String>,
}

pub struct CatalogAdapter {
    spec: &'static CatalogSpec,
    url: String,
}

impl CatalogAdapter {
    pub fn new(spec: &'static CatalogSpec, endpoints: &Endpoints) -> Self {
        Self {
            url: format!("{}{}", endpoints.origin(spec.origin), spec.path),
            spec,
        }
    }
}

impl Adapter for CatalogAdapter {
    fn id(&self) -> &'static str {
        self.spec.id
    }
    fn name(&self) -> &'static str {
        self.spec.name
    }
    fn organization_slug(&self) -> &'static str {
        self.spec.organization
    }
    fn interval(&self) -> Duration {
        self.spec.interval
    }
}

impl GatewayAdapter for CatalogAdapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>> {
        Box::pin(async move {
            let body = get_text(client, self.spec.id, &self.url).await?;
            parse(self.spec, &body)
        })
    }
}

/// Parses a provider's model list with its spec.
pub fn parse(spec: &CatalogSpec, body: &str) -> Result<Vec<ModelListing>, AdapterError> {
    let fail = |msg: String| AdapterError::Format(spec.id, msg);
    let root: Value = serde_json::from_str(body).map_err(|e| fail(e.to_string()))?;
    let items = match &root {
        Value::Array(items) => items,
        Value::Object(o) => o
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| fail("no `data` array".into()))?,
        _ => return Err(fail("not a JSON list".into())),
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Some(e) = (spec.entry)(item).map_err(&fail)? else {
            continue;
        };
        if e.id.trim().is_empty() || !seen.insert(e.id.clone()) {
            continue;
        }
        if !(e.input.is_finite() && e.output.is_finite() && e.input >= 0.0 && e.output >= 0.0) {
            return Err(fail(format!("model `{}` has an invalid price", e.id)));
        }
        out.push(ModelListing {
            source_url: Some(spec.source_url.replace("{id}", &e.id)),
            name: e
                .name
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| e.id.clone()),
            maker: e.maker.or_else(|| maker_of(&e.id)),
            context_window: e
                .context
                .and_then(|c| i32::try_from(c).ok())
                .filter(|c| *c > 0),
            input_per_mtok: round6(e.input),
            output_per_mtok: round6(e.output),
            variant: None,
            extra: e.extra,
            external_id: e.id,
        });
    }
    if out.is_empty() {
        return Err(fail("the list has no priced text models".into()));
    }
    Ok(out)
}

// --- Reading helpers --------------------------------------------------------

fn at<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(v, |v, k| v.get(*k).unwrap_or(&Value::Null))
}

fn str_at(v: &Value, path: &[&str]) -> Option<String> {
    at(v, path).as_str().map(str::to_owned)
}

fn int_at(v: &Value, path: &[&str]) -> Option<i64> {
    let x = at(v, path);
    x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))
}

/// A price at `path`, multiplied by `scale` to reach USD per million tokens.
fn price_at(v: &Value, path: &[&str], scale: f64) -> Result<Option<f64>, String> {
    json_price("catalog", at(v, path))
        .map(|p| p.map(|p| p * scale))
        .map_err(|e| e.to_string())
}

/// Both prices, or `None` when either is missing.
fn prices(
    v: &Value,
    input: &[&str],
    output: &[&str],
    scale: f64,
) -> Result<Option<(f64, f64)>, String> {
    Ok(
        match (price_at(v, input, scale)?, price_at(v, output, scale)?) {
            (Some(i), Some(o)) => Some((i, o)),
            _ => None,
        },
    )
}

const PER_TOKEN: f64 = 1e6;
const PER_MTOK: f64 = 1.0;

/// The common OpenRouter-style shape: `pricing.prompt` / `pricing.completion`
/// in USD per token, `context_length`.
fn openai_style(v: &Value, scale: f64) -> Result<Option<Entry>, String> {
    let Some(id) = str_at(v, &["id"]) else {
        return Err("model without an id".into());
    };
    let Some((input, output)) =
        prices(v, &["pricing", "prompt"], &["pricing", "completion"], scale)?
    else {
        return Ok(None);
    };
    Ok(Some(Entry {
        name: str_at(v, &["name"]),
        context: int_at(v, &["context_length"]),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

// --- Providers --------------------------------------------------------------

fn deepinfra(v: &Value) -> Result<Option<Entry>, String> {
    if str_at(v, &["type"]).as_deref() != Some("text-generation")
        || str_at(v, &["pricing", "type"]).as_deref() != Some("tokens")
        || at(v, &["deprecated"]).as_bool() == Some(true)
        || at(v, &["private"]).as_bool() == Some(true)
    {
        return Ok(None);
    }
    let Some(id) = str_at(v, &["model_name"]) else {
        return Err("model without model_name".into());
    };
    // US cents per token: × 1e6 tokens ÷ 100 cents.
    let Some((input, output)) = prices(
        v,
        &["pricing", "cents_per_input_token"],
        &["pricing", "cents_per_output_token"],
        1e4,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(Entry {
        context: int_at(v, &["max_tokens"]),
        extra: serde_json::json!({ "quantization": at(v, &["quantization"]) }),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

fn novita(v: &Value) -> Result<Option<Entry>, String> {
    let Some(id) = str_at(v, &["id"]) else {
        return Err("model without an id".into());
    };
    let Some((input, output)) = prices(
        v,
        &["pricing", "prompt", "price_per_m_decimal"],
        &["pricing", "completion", "price_per_m_decimal"],
        PER_MTOK,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(Entry {
        name: str_at(v, &["title"]),
        context: int_at(v, &["context_size"]),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

fn chutes(v: &Value) -> Result<Option<Entry>, String> {
    let mut e = match openai_style(v, PER_MTOK)? {
        Some(e) => e,
        None => return Ok(None),
    };
    e.extra = serde_json::json!({
        "quantization": at(v, &["quantization"]),
        "confidential_compute": at(v, &["confidential_compute"]),
    });
    Ok(Some(e))
}

fn per_token(v: &Value) -> Result<Option<Entry>, String> {
    openai_style(v, PER_TOKEN)
}

fn atlascloud(v: &Value) -> Result<Option<Entry>, String> {
    let mut e = match openai_style(v, PER_TOKEN)? {
        Some(e) => e,
        None => return Ok(None),
    };
    e.extra = serde_json::json!({ "quantization": at(v, &["quantization"]) });
    Ok(Some(e))
}

fn featherless(v: &Value) -> Result<Option<Entry>, String> {
    let Some(id) = str_at(v, &["id"]) else {
        return Err("model without an id".into());
    };
    if at(v, &["is_gated"]).as_bool() == Some(true) {
        return Ok(None);
    }
    let Some((input, output)) = prices(v, &["pricing", "input"], &["pricing", "output"], PER_MTOK)?
    else {
        return Ok(None);
    };
    Ok(Some(Entry {
        context: int_at(v, &["context_length"]),
        extra: serde_json::json!({ "model_class": at(v, &["model_class"]) }),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

fn vercel(v: &Value) -> Result<Option<Entry>, String> {
    if str_at(v, &["type"]).as_deref() != Some("language") {
        return Ok(None);
    }
    let Some(id) = str_at(v, &["id"]) else {
        return Err("model without an id".into());
    };
    let Some((input, output)) =
        prices(v, &["pricing", "input"], &["pricing", "output"], PER_TOKEN)?
    else {
        return Ok(None);
    };
    Ok(Some(Entry {
        name: str_at(v, &["name"]),
        maker: str_at(v, &["owned_by"]),
        context: int_at(v, &["context_window"]),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

fn venice(v: &Value) -> Result<Option<Entry>, String> {
    if str_at(v, &["type"]).as_deref() != Some("text") {
        return Ok(None);
    }
    let Some(id) = str_at(v, &["id"]) else {
        return Err("model without an id".into());
    };
    let Some((input, output)) = prices(
        v,
        &["model_spec", "pricing", "input", "usd"],
        &["model_spec", "pricing", "output", "usd"],
        PER_MTOK,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(Entry {
        name: str_at(v, &["model_spec", "name"]),
        context: int_at(v, &["model_spec", "availableContextTokens"]),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

fn requesty(v: &Value) -> Result<Option<Entry>, String> {
    if str_at(v, &["api"]).as_deref() != Some("chat") {
        return Ok(None);
    }
    let Some(id) = str_at(v, &["id"]) else {
        return Err("model without an id".into());
    };
    let Some((input, output)) = prices(v, &["input_price"], &["output_price"], PER_TOKEN)? else {
        return Ok(None);
    };
    Ok(Some(Entry {
        context: int_at(v, &["context_window"]),
        input,
        output,
        id,
        ..Entry::default()
    }))
}

/// Every catalog provider, small to large is not an order here: the worker
/// runs them on their own intervals.
pub static CATALOGS: &[CatalogSpec] = &[
    CatalogSpec {
        id: "deepinfra",
        name: "DeepInfra",
        organization: "deepinfra",
        origin: "https://api.deepinfra.com",
        path: "/models/list",
        interval: HOURLY,
        source_url: "https://deepinfra.com/{id}",
        entry: deepinfra,
    },
    CatalogSpec {
        id: "novita",
        name: "Novita AI",
        organization: "novita-ai",
        origin: "https://api.novita.ai",
        path: "/v3/openai/models",
        interval: HOURLY,
        source_url: "https://novita.ai/models/llm/{id}",
        entry: novita,
    },
    CatalogSpec {
        id: "chutes",
        name: "Chutes",
        organization: "chutes",
        origin: "https://llm.chutes.ai",
        path: "/v1/models",
        interval: HOURLY,
        source_url: "https://chutes.ai/app",
        entry: chutes,
    },
    CatalogSpec {
        id: "sambanova",
        name: "SambaNova Cloud",
        organization: "sambanova",
        origin: "https://api.sambanova.ai",
        path: "/v1/models",
        interval: DAILY,
        source_url: "https://cloud.sambanova.ai/plans/pricing",
        entry: per_token,
    },
    CatalogSpec {
        id: "atlascloud",
        name: "Atlas Cloud",
        organization: "atlas-cloud",
        origin: "https://api.atlascloud.ai",
        path: "/v1/models",
        interval: DAILY,
        source_url: "https://www.atlascloud.ai/models",
        entry: atlascloud,
    },
    CatalogSpec {
        id: "inferencenet",
        name: "Inference.net",
        organization: "inference-net",
        origin: "https://api.inference.net",
        path: "/v1/models",
        interval: DAILY,
        source_url: "https://inference.net/models",
        entry: per_token,
    },
    CatalogSpec {
        id: "featherless",
        name: "Featherless",
        organization: "featherless",
        origin: "https://api.featherless.ai",
        path: "/v1/models",
        interval: DAILY,
        source_url: "https://featherless.ai/models/{id}",
        entry: featherless,
    },
    CatalogSpec {
        id: "venice",
        name: "Venice",
        organization: "venice",
        origin: "https://api.venice.ai",
        path: "/api/v1/models",
        interval: DAILY,
        source_url: "https://venice.ai/pricing",
        entry: venice,
    },
    CatalogSpec {
        id: "vercel",
        name: "Vercel AI Gateway",
        organization: "vercel",
        origin: "https://ai-gateway.vercel.sh",
        path: "/v1/models",
        interval: DAILY,
        source_url: "https://vercel.com/ai-gateway/models",
        entry: vercel,
    },
    CatalogSpec {
        id: "requesty",
        name: "Requesty",
        organization: "requesty",
        origin: "https://router.requesty.ai",
        path: "/v1/models",
        interval: DAILY,
        source_url: "https://www.requesty.ai/solution/llm-routing/models",
        entry: requesty,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str) -> &'static CatalogSpec {
        CATALOGS.iter().find(|s| s.id == id).unwrap()
    }

    fn one(id: &str, body: &str) -> ModelListing {
        let mut l = parse(spec(id), body).unwrap();
        assert_eq!(l.len(), 1, "{l:?}");
        l.remove(0)
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = CATALOGS.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CATALOGS.len());
    }

    #[test]
    fn deepinfra_converts_cents_per_token_and_skips_other_types() {
        let l = one(
            "deepinfra",
            r#"[{"model_name":"zai-org/GLM-4.7-Flash","type":"text-generation","max_tokens":202752,
                 "pricing":{"type":"tokens","cents_per_input_token":0.000006,"cents_per_output_token":0.00004}},
                {"model_name":"black-forest-labs/FLUX","type":"text-to-image","pricing":{"type":"images"}},
                {"model_name":"old/model","type":"text-generation","deprecated":true,
                 "pricing":{"type":"tokens","cents_per_input_token":1,"cents_per_output_token":1}}]"#,
        );
        assert_eq!(l.external_id, "zai-org/GLM-4.7-Flash");
        assert_eq!((l.input_per_mtok, l.output_per_mtok), (0.06, 0.4));
        assert_eq!(l.maker.as_deref(), Some("zai-org"));
        assert_eq!(l.context_window, Some(202_752));
        assert_eq!(
            l.source_url.as_deref(),
            Some("https://deepinfra.com/zai-org/GLM-4.7-Flash")
        );
    }

    #[test]
    fn novita_reads_decimal_prices_per_million() {
        let l = one(
            "novita",
            r#"{"data":[{"id":"tencent/hy4","title":"tencent/hy4","context_size":1000000,
                "pricing":{"prompt":{"price_per_m_decimal":"0.417"},"completion":{"price_per_m_decimal":"1.2505"}}}]}"#,
        );
        assert_eq!((l.input_per_mtok, l.output_per_mtok), (0.417, 1.2505));
        assert_eq!(l.context_window, Some(1_000_000));
    }

    #[test]
    fn chutes_prices_are_already_per_million() {
        let l = one(
            "chutes",
            r#"{"data":[{"id":"Qwen/Qwen3.8-27B-TEE","pricing":{"prompt":0.039,"completion":1.25},
                "context_length":262144,"quantization":"fp8","confidential_compute":true}]}"#,
        );
        assert_eq!((l.input_per_mtok, l.output_per_mtok), (0.039, 1.25));
        assert_eq!(l.extra["quantization"], "fp8");
    }

    #[test]
    fn per_token_strings_are_scaled() {
        for id in ["sambanova", "atlascloud", "inferencenet"] {
            let l = one(
                id,
                r#"{"data":[{"id":"GLM-5.3","context_length":131072,
                    "pricing":{"prompt":"0.00000280","completion":"0.00000880"}}]}"#,
            );
            assert_eq!((l.input_per_mtok, l.output_per_mtok), (2.8, 8.8), "{id}");
            assert_eq!(l.maker, None, "{id}: an id without a slash names no maker");
        }
    }

    #[test]
    fn vercel_keeps_language_models_only() {
        let l = one(
            "vercel",
            r#"{"data":[{"id":"alibaba/qwen-3-14b","owned_by":"alibaba","type":"language","context_window":40960,
                         "pricing":{"input":"0.00000012","output":"0.00000024"}},
                        {"id":"bfl/flux","type":"image","pricing":{"image":"0.04"}}]}"#,
        );
        assert_eq!((l.input_per_mtok, l.output_per_mtok), (0.12, 0.24));
        assert_eq!(l.maker.as_deref(), Some("alibaba"));
    }

    #[test]
    fn venice_requesty_and_featherless() {
        let l = one(
            "venice",
            r#"{"data":[{"id":"gemini-4-argon","type":"text","model_spec":{"name":"Gemini 4 Argon",
                "availableContextTokens":1000000,"pricing":{"input":{"usd":2.5},"output":{"usd":12.5}}}}]}"#,
        );
        assert_eq!(
            (l.name.as_str(), l.input_per_mtok, l.output_per_mtok),
            ("Gemini 4 Argon", 2.5, 12.5)
        );

        let l = one(
            "requesty",
            r#"{"data":[{"api":"chat","id":"vertex/gemini-4-carbon","input_price":0.000002,"output_price":0.00001,
                         "context_window":1048576},
                        {"api":"image","id":"x/y","input_price":1,"output_price":1}]}"#,
        );
        assert_eq!((l.input_per_mtok, l.output_per_mtok), (2.0, 10.0));

        let l = one(
            "featherless",
            r#"{"data":[{"id":"recursal/EagleX_1-7T","is_gated":false,"context_length":16384,
                         "pricing":{"input":0.1,"output":0.2}},
                        {"id":"meta/gated","is_gated":true,"pricing":{"input":0.1,"output":0.2}}]}"#,
        );
        assert_eq!((l.input_per_mtok, l.output_per_mtok), (0.1, 0.2));
    }

    #[test]
    fn rejects_changed_formats() {
        assert!(parse(spec("novita"), r#"{"models":[]}"#).is_err());
        assert!(
            parse(spec("novita"), r#"{"data":[]}"#).is_err(),
            "an empty list is a broken API"
        );
        assert!(
            parse(
                spec("sambanova"),
                r#"{"data":[{"id":"a","pricing":{"prompt":"cheap","completion":"1"}}]}"#
            )
            .is_err()
        );
        assert!(
            parse(
                spec("sambanova"),
                r#"{"data":[{"id":"a","pricing":{"prompt":"-1","completion":"1"}}]}"#
            )
            .is_err()
        );
    }
}
