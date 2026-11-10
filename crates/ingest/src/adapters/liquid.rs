//! Liquid Inference: an exchange where many providers post offers for each
//! model. Its public order book lists every resting offer with its rates.
//!
//! The directory records the exchange's best (lowest) live price per model, the
//! price a buyer gets through Liquid, with the number of competing offers.
//!
//! Some offers add a fixed fee per request on top of their token rates. A token
//! price cannot express that fee, and showing such an offer's token rates alone
//! would understate what a buyer pays, so the listed price comes only from
//! offers without a fee. Fee offers still count, and the cheapest one (on a
//! typical request of [`REFERENCE_INPUT`] + [`REFERENCE_OUTPUT`] tokens) is kept
//! in `extra` for reference.

use std::collections::BTreeMap;

use reqwest::Client;
use serde::Deserialize;

use super::{Adapter, AdapterError, BoxFuture, GatewayAdapter, ModelListing, get_text, round6};

const ID: &str = "liquid";
/// Tokens of the request offers are compared on: a chat turn with some context.
const REFERENCE_INPUT: f64 = 3_000.0;
const REFERENCE_OUTPUT: f64 = 1_000.0;

pub struct LiquidInferenceAdapter {
    base_url: String,
}

impl LiquidInferenceAdapter {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }
}

impl Adapter for LiquidInferenceAdapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn name(&self) -> &'static str {
        "Liquid Inference"
    }
    fn organization_slug(&self) -> &'static str {
        "liquid-inference"
    }
}

#[derive(Deserialize)]
struct Offer {
    served_model: String,
    creator: Option<String>,
    provider_slug: String,
    billing_schema: Billing,
    input_length_band: Option<Band>,
    /// Offers with unverified pricing, such as bring-your-own-key routes.
    #[serde(default)]
    unverified: bool,
}

#[derive(Deserialize)]
struct Billing {
    /// USD per million tokens.
    rate_uncached_input: f64,
    rate_generation: f64,
    /// USD per request.
    #[serde(default)]
    fixed_fee: f64,
}

impl Billing {
    /// USD for one reference request.
    fn reference_cost(&self) -> f64 {
        self.fixed_fee
            + (self.rate_uncached_input * REFERENCE_INPUT + self.rate_generation * REFERENCE_OUTPUT)
                / 1e6
    }
}

#[derive(Deserialize)]
struct Band {
    max: Option<i64>,
}

impl GatewayAdapter for LiquidInferenceAdapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>> {
        Box::pin(async move {
            let url = format!("{}/v1/exchange/book?kind=text&limit=5000", self.base_url);
            let body = get_text(client, ID, &url).await?;
            parse(&body)
        })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<ModelListing>, AdapterError> {
    let offers: Vec<Offer> =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;

    #[derive(Default)]
    struct Model {
        offers: usize,
        /// Cheapest offer priced by tokens alone.
        best: Option<Offer>,
        /// Cheapest offer that adds a fee per request.
        best_with_fee: Option<Offer>,
    }
    let cheaper = |a: &Offer, b: &Option<Offer>| {
        b.as_ref()
            .is_none_or(|b| a.billing_schema.reference_cost() < b.billing_schema.reference_cost())
    };

    let mut models: BTreeMap<String, Model> = BTreeMap::new();
    for offer in offers {
        let b = &offer.billing_schema;
        // Bring-your-own-key and unverified offers are not a price anyone can buy at.
        if offer.unverified
            || offer.provider_slug.starts_with("byok")
            || [b.rate_uncached_input, b.rate_generation, b.fixed_fee]
                .iter()
                .any(|r| !r.is_finite() || *r < 0.0)
        {
            continue;
        }
        let model = models.entry(offer.served_model.clone()).or_default();
        model.offers += 1;
        if offer.billing_schema.fixed_fee > 0.0 {
            if cheaper(&offer, &model.best_with_fee) {
                model.best_with_fee = Some(offer);
            }
        } else if cheaper(&offer, &model.best) {
            model.best = Some(offer);
        }
    }

    Ok(models
        .into_iter()
        // A model sold only with per-request fees has no token price to list.
        .filter_map(|(name, model)| {
            let best = model.best?;
            let fee_offer = model.best_with_fee.map(|o| {
                serde_json::json!({
                    "provider": o.provider_slug,
                    "input_per_mtok": round6(o.billing_schema.rate_uncached_input),
                    "output_per_mtok": round6(o.billing_schema.rate_generation),
                    "fixed_fee_per_request": o.billing_schema.fixed_fee,
                })
            });
            Some(ModelListing {
                source_url: Some(format!("https://liquidinference.ai/models/{name}")),
                maker: best.creator.clone(),
                context_window: best
                    .input_length_band
                    .as_ref()
                    .and_then(|b| b.max)
                    .and_then(|m| i32::try_from(m).ok()),
                input_per_mtok: round6(best.billing_schema.rate_uncached_input),
                output_per_mtok: round6(best.billing_schema.rate_generation),
                variant: None,
                extra: serde_json::json!({
                    "offers": model.offers,
                    "best_provider": best.provider_slug,
                    "cheapest_with_request_fee": fee_offer,
                }),
                external_id: name.clone(),
                name,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(model: &str, provider: &str, input: f64, output: f64, unverified: bool) -> String {
        offer_with_fee(model, provider, input, output, 0.0, unverified)
    }

    fn offer_with_fee(
        model: &str,
        provider: &str,
        input: f64,
        output: f64,
        fee: f64,
        unverified: bool,
    ) -> String {
        format!(
            r#"{{"served_model":"{model}","creator":"openai","provider_slug":"{provider}",
                 "billing_schema":{{"rate_uncached_input":{input},"rate_generation":{output},"fixed_fee":{fee}}},
                 "input_length_band":{{"min":0,"max":131072}},"unverified":{unverified}}}"#
        )
    }

    #[test]
    fn keeps_the_best_buyable_offer_per_model() {
        let body = format!(
            "[{},{},{},{}]",
            offer("gpt-oss-120b", "akash", 0.037, 0.187, false),
            offer("gpt-oss-120b", "sokkan", 0.09, 0.46, false),
            offer("gpt-oss-120b", "byok-bedrock", 0.0, 0.0, true),
            offer("other", "x", 1.0, 2.0, false),
        );
        let listings = parse(&body).unwrap();
        assert_eq!(listings.len(), 2);
        let oss = listings
            .iter()
            .find(|l| l.external_id == "gpt-oss-120b")
            .unwrap();
        assert_eq!((oss.input_per_mtok, oss.output_per_mtok), (0.037, 0.187));
        assert_eq!(oss.extra["offers"], 2, "the BYOK offer is not counted");
        assert_eq!(oss.extra["best_provider"], "akash");
        assert_eq!(oss.context_window, Some(131_072));
        assert_eq!(oss.maker.as_deref(), Some("openai"));
    }

    #[test]
    fn offers_with_per_request_fees_are_not_listed_as_token_prices() {
        // pera's token rates are lower, but it adds a fee per request: the listed
        // token price is akash's, and pera's offer is kept for reference.
        let body = format!(
            "[{},{}]",
            offer_with_fee("m", "pera", 0.037289062, 0.0034, 0.000163949, false),
            offer("m", "akash", 0.037, 0.187, false),
        );
        let listings = parse(&body).unwrap();
        assert_eq!(listings.len(), 1);
        let m = &listings[0];
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (0.037, 0.187));
        assert_eq!(m.extra["best_provider"], "akash");
        assert_eq!(m.extra["offers"], 2);
        assert_eq!(m.extra["cheapest_with_request_fee"]["provider"], "pera");
        assert_eq!(
            m.extra["cheapest_with_request_fee"]["fixed_fee_per_request"],
            0.000163949
        );

        // A model sold only with fees has no token price to list.
        let body = format!(
            "[{}]",
            offer_with_fee("only-fee", "pera", 0.03, 0.003, 0.0002, false)
        );
        assert!(parse(&body).unwrap().is_empty());
    }

    #[test]
    fn rejects_a_changed_format() {
        assert!(parse(r#"{"offers":[]}"#).is_err());
    }
}
