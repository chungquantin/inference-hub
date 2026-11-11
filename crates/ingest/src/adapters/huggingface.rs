//! Hugging Face Inference Providers: one router in front of many inference
//! companies (Cerebras, Groq, Novita, DeepInfra, Nscale, OVHcloud, ...). Its
//! public model list gives each provider's price for each model, so one
//! request yields a per-provider price table.
//!
//! Each (model, provider) pair is one listing, with the provider in `variant`.

use reqwest::Client;
use serde::Deserialize;

use super::{
    AdapterError, BoxFuture, DAILY, GatewayAdapter, ModelListing, adapter_info, get_text, maker_of,
    round6,
};

const ID: &str = "huggingface";

pub struct HuggingFaceRouterAdapter {
    base_url: String,
}

impl HuggingFaceRouterAdapter {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }
}

adapter_info!(
    HuggingFaceRouterAdapter,
    ID,
    "Hugging Face Inference Providers",
    "hugging-face",
    DAILY
);

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    id: String,
    owned_by: Option<String>,
    #[serde(default)]
    providers: Vec<Provider>,
}

#[derive(Deserialize)]
struct Provider {
    provider: String,
    status: Option<String>,
    context_length: Option<i64>,
    /// USD per million tokens.
    pricing: Option<Pricing>,
    first_token_latency_ms: Option<f64>,
    throughput: Option<f64>,
}

#[derive(Deserialize)]
struct Pricing {
    input: f64,
    output: f64,
}

impl GatewayAdapter for HuggingFaceRouterAdapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>> {
        Box::pin(async move {
            let url = format!("{}/v1/models", self.base_url);
            parse(&get_text(client, ID, &url).await?)
        })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<ModelListing>, AdapterError> {
    let response: ModelsResponse =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    let mut out = Vec::new();
    for model in response.data {
        for p in model.providers {
            // Providers without a price are free tiers or bring-your-own-key.
            let Some(price) = p.pricing else { continue };
            if p.status.as_deref().is_some_and(|s| s != "live") {
                continue;
            }
            if !(price.input.is_finite() && price.output.is_finite())
                || price.input < 0.0
                || price.output < 0.0
            {
                return Err(AdapterError::Format(
                    ID,
                    format!("{} at {} has an invalid price", model.id, p.provider),
                ));
            }
            out.push(ModelListing {
                external_id: model.id.clone(),
                name: model.id.clone(),
                maker: model
                    .owned_by
                    .as_deref()
                    .map(str::to_ascii_lowercase)
                    .or_else(|| maker_of(&model.id)),
                context_window: p.context_length.and_then(|c| i32::try_from(c).ok()),
                input_per_mtok: round6(price.input),
                output_per_mtok: round6(price.output),
                source_url: Some(format!(
                    "https://huggingface.co/{}?inference_provider={}",
                    model.id, p.provider
                )),
                extra: serde_json::json!({
                    "provider": p.provider,
                    "first_token_latency_ms": p.first_token_latency_ms,
                    "throughput_tps": p.throughput,
                }),
                variant: Some(p.provider),
                media: None,
            });
        }
    }
    if out.is_empty() {
        return Err(AdapterError::Format(ID, "no priced providers".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_listing_per_priced_live_provider() {
        let body = r#"{"object":"list","data":[{"id":"Qwen/Qwen3.8-27B","owned_by":"Qwen","providers":[
            {"provider":"novita","status":"live","context_length":1000000,"pricing":{"input":0.42,"output":3},"throughput":25.4},
            {"provider":"cerebras","status":"live","context_length":65536,"pricing":{"input":0.99,"output":1.49}},
            {"provider":"hf-inference","status":"live"},
            {"provider":"old","status":"staging","pricing":{"input":1,"output":1}}
        ]}]}"#;
        let l = parse(body).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].variant.as_deref(), Some("novita"));
        assert_eq!((l[0].input_per_mtok, l[0].output_per_mtok), (0.42, 3.0));
        assert_eq!(l[0].maker.as_deref(), Some("qwen"));
        assert_eq!(l[1].context_window, Some(65_536));
        assert_eq!(l[0].extra["throughput_tps"], 25.4);
    }

    #[test]
    fn rejects_a_changed_format() {
        assert!(parse(r#"{"models":[]}"#).is_err());
        assert!(parse(r#"{"data":[]}"#).is_err());
    }
}
