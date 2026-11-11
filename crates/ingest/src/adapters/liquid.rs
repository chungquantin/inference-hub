//! Liquid Inference: an exchange where many providers post offers for each
//! model. Its public order book lists every resting offer with its rates.
//!
//! The directory records the exchange's best (lowest) live price per model, the
//! price a buyer gets through Liquid, with the number of competing offers.
//! Every kind of model in the book is covered: chat models by token rates,
//! embeddings by input tokens, and image, video, speech and transcription
//! models by their own unit (per image, per video second, per character, per
//! audio second).
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

use super::{
    Adapter, AdapterError, BoxFuture, GatewayAdapter, MediaKind, ModelListing, get_text, round_sig,
    round6,
};

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
    fn volatile(&self) -> bool {
        true
    }
}

#[derive(Deserialize)]
struct Offer {
    served_model: String,
    creator: Option<String>,
    provider_slug: String,
    billing_schema: Billing,
    input_length_band: Option<Band>,
    #[serde(default)]
    output_modalities: Vec<Modality>,
    #[serde(default)]
    input_modalities: Vec<Modality>,
    #[serde(default)]
    supported_features: Vec<String>,
    /// Offers with unverified pricing, such as bring-your-own-key routes.
    #[serde(default)]
    unverified: bool,
}

#[derive(Deserialize)]
struct Billing {
    /// USD per million tokens. Absent for models priced per unit.
    rate_uncached_input: Option<f64>,
    rate_generation: Option<f64>,
    /// USD per request.
    #[serde(default)]
    fixed_fee: f64,
    rate_per_image: Option<f64>,
    rate_per_video_second: Option<f64>,
    rate_per_character: Option<f64>,
    rate_per_audio_second: Option<f64>,
}

#[derive(Deserialize)]
struct Modality {
    #[serde(rename = "type")]
    kind: String,
}

/// How an offer is priced, decided from its billing schema.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pricing {
    /// Chat: input and output per million tokens.
    Tokens { input: f64, output: f64 },
    /// One rate per unit of a media kind.
    Unit(MediaKind, f64),
}

impl Pricing {
    /// The price comparable across offers of one model: a reference request
    /// for chat, the unit rate otherwise.
    fn reference_cost(self, fee: f64) -> f64 {
        match self {
            Pricing::Tokens { input, output } => {
                fee + (input * REFERENCE_INPUT + output * REFERENCE_OUTPUT) / 1e6
            }
            Pricing::Unit(_, rate) => rate,
        }
    }

    fn kind(self) -> Option<MediaKind> {
        match self {
            Pricing::Tokens { .. } => None,
            Pricing::Unit(kind, _) => Some(kind),
        }
    }
}

impl Offer {
    fn pricing(&self) -> Option<Pricing> {
        let b = &self.billing_schema;
        let outputs: Vec<&str> = self
            .output_modalities
            .iter()
            .map(|m| m.kind.as_str())
            .collect();
        let unit = |kind, rate: Option<f64>| rate.map(|r| Pricing::Unit(kind, r));
        if let Some(p) = unit(MediaKind::Image, b.rate_per_image)
            .or(unit(MediaKind::Video, b.rate_per_video_second))
            .or(unit(MediaKind::Speech, b.rate_per_character))
            .or(unit(MediaKind::Transcription, b.rate_per_audio_second))
        {
            return Some(p);
        }
        if outputs.contains(&"embedding") {
            return unit(MediaKind::Embedding, b.rate_uncached_input);
        }
        // Decision models answer typed questions; they have no comparable market here.
        if outputs.contains(&"decision") {
            return None;
        }
        Some(Pricing::Tokens {
            input: b.rate_uncached_input?,
            output: b.rate_generation?,
        })
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
            // The whole book: every kind of model. The endpoint answers with
            // every offer at once; it takes no paging parameters.
            let url = format!("{}/v1/exchange/book", self.base_url);
            let body = get_text(client, ID, &url).await?;
            parse(&body)
        })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<ModelListing>, AdapterError> {
    let offers: Vec<Offer> =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;

    struct Priced {
        offer: Offer,
        pricing: Pricing,
    }
    #[derive(Default)]
    struct Model {
        offers: usize,
        /// Cheapest offer without a fee per request.
        best: Option<Priced>,
        /// Cheapest chat offer that adds a fee per request.
        best_with_fee: Option<Priced>,
    }
    let cost = |p: &Priced| p.pricing.reference_cost(p.offer.billing_schema.fixed_fee);
    let cheaper = |a: &Priced, b: &Option<Priced>| b.as_ref().is_none_or(|b| cost(a) < cost(b));

    // One model can be offered as two kinds (rarely); key by both.
    let mut models: BTreeMap<(String, Option<MediaKind>), Model> = BTreeMap::new();
    for offer in offers {
        // Bring-your-own-key and unverified offers are not a price anyone can buy at.
        if offer.unverified || offer.provider_slug.starts_with("byok") {
            continue;
        }
        let Some(pricing) = offer.pricing() else {
            continue;
        };
        let fee = offer.billing_schema.fixed_fee;
        let valid = |x: f64| x.is_finite() && x >= 0.0;
        let ok = valid(fee)
            && match pricing {
                Pricing::Tokens { input, output } => valid(input) && valid(output),
                // Unit-priced offers with a per-request fee are not comparable per unit.
                Pricing::Unit(_, rate) => valid(rate) && rate > 0.0 && fee == 0.0,
            };
        if !ok {
            continue;
        }
        let model = models
            .entry((offer.served_model.clone(), pricing.kind()))
            .or_default();
        model.offers += 1;
        let priced = Priced { offer, pricing };
        if fee > 0.0 {
            if cheaper(&priced, &model.best_with_fee) {
                model.best_with_fee = Some(priced);
            }
        } else if cheaper(&priced, &model.best) {
            model.best = Some(priced);
        }
    }

    let listings: Vec<ModelListing> = models
        .into_iter()
        // A model sold only with per-request fees has no token price to list.
        .filter_map(|((name, _), model)| {
            let best = model.best?;
            let fee_offer = model.best_with_fee.map(|p| {
                let b = &p.offer.billing_schema;
                serde_json::json!({
                    "provider": p.offer.provider_slug,
                    "input_per_mtok": b.rate_uncached_input.map(round6),
                    "output_per_mtok": b.rate_generation.map(round6),
                    "fixed_fee_per_request": b.fixed_fee,
                })
            });
            let extra = serde_json::json!({
                "offers": model.offers,
                "best_provider": best.offer.provider_slug,
                "cheapest_with_request_fee": fee_offer,
            });
            let source_url = Some(format!("https://liquidinference.ai/models/{name}"));
            let o = &best.offer;
            let capabilities = super::caps::normalize(
                o.input_modalities
                    .iter()
                    .filter_map(|m| super::caps::modality("in", &m.kind))
                    .chain(
                        o.output_modalities
                            .iter()
                            .filter_map(|m| super::caps::modality("out", &m.kind)),
                    )
                    .chain(
                        o.supported_features
                            .iter()
                            .filter_map(|f| super::caps::feature(f))
                            .map(str::to_owned),
                    )
                    // `input.image` style feature names
                    .chain(o.supported_features.iter().filter_map(|f| {
                        f.strip_prefix("input.")
                            .and_then(|m| super::caps::modality("in", m))
                    }))
                    .collect(),
            );
            let maker = best.offer.creator.clone();
            Some(match best.pricing {
                Pricing::Tokens { input, output } => ModelListing {
                    capabilities: capabilities.clone(),
                    source_url,
                    maker,
                    context_window: best
                        .offer
                        .input_length_band
                        .as_ref()
                        .and_then(|b| b.max)
                        .and_then(|m| i32::try_from(m).ok()),
                    input_per_mtok: round6(input),
                    output_per_mtok: round6(output),
                    variant: None,
                    media: None,

                    extra,
                    external_id: name.clone(),
                    name,
                },
                Pricing::Unit(kind, rate) => ModelListing {
                    capabilities,
                    ..ModelListing::media(
                        name.clone(),
                        name,
                        maker,
                        kind,
                        round_sig(rate),
                        source_url,
                        extra,
                    )
                },
            })
        })
        .collect();
    if listings.is_empty() {
        return Err(AdapterError::Format(
            ID,
            "the book has no buyable offers".into(),
        ));
    }
    Ok(listings)
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
            "[{},{}]",
            offer_with_fee("only-fee", "pera", 0.03, 0.003, 0.0002, false),
            offer("m", "akash", 0.037, 0.187, false),
        );
        let listed: Vec<_> = parse(&body)
            .unwrap()
            .into_iter()
            .map(|l| l.external_id)
            .collect();
        assert_eq!(listed, ["m"]);
    }

    #[test]
    fn rejects_a_changed_format() {
        assert!(parse(r#"{"offers":[]}"#).is_err());
        assert!(parse("[]").is_err(), "an empty book is a broken API");
    }

    fn media(model: &str, provider: &str, billing: &str, outputs: &str) -> String {
        format!(
            r#"{{"served_model":"{model}","creator":"acme","provider_slug":"{provider}",
                 "billing_schema":{{"fixed_fee":0.0,"rate_uncached_input":null,"rate_generation":null,{billing}}},
                 "output_modalities":[{outputs}],"unverified":false}}"#
        )
    }

    #[test]
    fn prices_every_kind_of_model_in_its_own_unit() {
        let body = format!(
            "[{}]",
            [
                media("seedream-4.5", "a", r#""rate_per_image":0.08"#, r#"{"type":"image"}"#),
                media("seedream-4.5", "b", r#""rate_per_image":0.05"#, r#"{"type":"image"}"#),
                media("wan-video", "a", r#""rate_per_video_second":0.29"#, r#"{"type":"video"}"#),
                media("glm-tts", "a", r#""rate_per_character":0.0002"#, r#"{"type":"audio"}"#),
                media("asr", "a", r#""rate_per_audio_second":0.00007"#, r#"{"type":"transcript"}"#),
                r#"{"served_model":"embed","creator":"acme","provider_slug":"a",
                    "billing_schema":{"fixed_fee":0,"rate_uncached_input":0.25,"rate_generation":null},
                    "output_modalities":[{"type":"embedding"}],"unverified":false}"#
                    .to_owned(),
                r#"{"served_model":"judge","creator":"acme","provider_slug":"a",
                    "billing_schema":{"fixed_fee":0,"rate_uncached_input":0.02,"rate_generation":null},
                    "output_modalities":[{"type":"decision"}],"unverified":false}"#
                    .to_owned(),
                offer("chat", "a", 0.1, 0.2, false),
            ]
            .join(",")
        );
        let l = parse(&body).unwrap();
        let get = |id: &str| l.iter().find(|x| x.external_id == id).unwrap();
        assert_eq!(l.len(), 6, "decision models are left out");
        let img = get("seedream-4.5").media.unwrap();
        assert_eq!((img.kind, img.price), (MediaKind::Image, 0.05));
        assert_eq!(get("seedream-4.5").extra["offers"], 2);
        assert_eq!(get("wan-video").media.unwrap().kind, MediaKind::Video);
        assert_eq!(get("glm-tts").media.unwrap().price, 0.0002);
        assert_eq!(get("asr").media.unwrap().kind, MediaKind::Transcription);
        assert_eq!(get("embed").media.unwrap().kind, MediaKind::Embedding);
        assert!(get("chat").media.is_none());
    }
}
