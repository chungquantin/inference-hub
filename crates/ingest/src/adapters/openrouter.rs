//! OpenRouter: a gateway that resells many labs' models. Its public model list
//! carries per-token prices.

use reqwest::Client;
use serde::Deserialize;

use super::{
    Adapter, AdapterError, BoxFuture, GatewayAdapter, ModelListing, get_text, parse_price, round6,
};

const ID: &str = "openrouter";

pub struct OpenRouterAdapter {
    base_url: String,
}

impl OpenRouterAdapter {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }
}

impl Adapter for OpenRouterAdapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn name(&self) -> &'static str {
        "OpenRouter"
    }
    fn organization_slug(&self) -> &'static str {
        "openrouter"
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    id: String,
    name: String,
    context_length: Option<i64>,
    pricing: Pricing,
}

/// USD per token, as decimal strings. `-1` marks variable pricing (routers).
#[derive(Deserialize)]
struct Pricing {
    prompt: String,
    completion: String,
}

impl GatewayAdapter for OpenRouterAdapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>> {
        Box::pin(async move {
            let url = format!("{}/api/v1/models", self.base_url);
            let body = get_text(client, ID, &url).await?;
            parse(&body)
        })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<ModelListing>, AdapterError> {
    let response: ModelsResponse =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    let mut listings = Vec::with_capacity(response.data.len());
    for model in response.data {
        // Routers such as `openrouter/auto` price per request, not per token.
        if model.pricing.prompt.starts_with('-') || model.pricing.completion.starts_with('-') {
            continue;
        }
        let input = parse_price(ID, &model.pricing.prompt)? * 1e6;
        let output = parse_price(ID, &model.pricing.completion)? * 1e6;
        // The maker is the part of the id before the slash: `meta-llama/llama-3.3-70b`.
        let maker = model.id.split_once('/').map(|(m, _)| m.to_owned());
        listings.push(ModelListing {
            source_url: Some(format!("https://openrouter.ai/{}", model.id)),
            external_id: model.id,
            name: model.name,
            maker,
            context_window: model.context_length.and_then(|c| i32::try_from(c).ok()),
            input_per_mtok: round6(input),
            output_per_mtok: round6(output),
            variant: None,
            extra: serde_json::Value::Null,
        });
    }
    Ok(listings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_per_token_prices_and_skips_variable_pricing() {
        let body = r#"{"data":[
            {"id":"meta-llama/llama-3.3-70b-instruct","name":"Meta: Llama 3.3 70B Instruct",
             "context_length":131072,"pricing":{"prompt":"0.00000071","completion":"0.00000071"}},
            {"id":"openrouter/auto","name":"Auto Router","context_length":2000000,
             "pricing":{"prompt":"-1","completion":"-1"}},
            {"id":"vendor/free-model:free","name":"Free","context_length":null,
             "pricing":{"prompt":"0","completion":"0"}}
        ]}"#;
        let listings = parse(body).unwrap();
        assert_eq!(listings.len(), 2);
        assert_eq!(listings[0].input_per_mtok, 0.71);
        assert_eq!(listings[0].output_per_mtok, 0.71);
        assert_eq!(listings[0].maker.as_deref(), Some("meta-llama"));
        assert_eq!(listings[0].context_window, Some(131_072));
        assert_eq!(
            listings[0].source_url.as_deref(),
            Some("https://openrouter.ai/meta-llama/llama-3.3-70b-instruct")
        );
        assert_eq!(listings[1].input_per_mtok, 0.0);
    }

    #[test]
    fn rejects_a_changed_format() {
        assert!(parse(r#"{"models":[]}"#).is_err());
        assert!(parse(r#"{"data":[{"id":"a/b","name":"B","pricing":{"prompt":"cheap","completion":"0"}}]}"#).is_err());
    }
}
