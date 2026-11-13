//! xAI: models and prices from the lab's public models page.
//!
//! The page inlines `globalThis.__XAI_PUBLIC_MODELS__={"clusterConfigs":[{"languageModels":[..]}]}`.
//! Prices there are integers as strings in units of 1/10000 USD per million tokens:
//! `"20000"` is $2.00 per 1M. Evidence (checked against the live page): `grok-4.5`/`4.6`/`4.7`
//! carry `promptTextTokenPrice` 20000 and `completionTextTokenPrice` 60000, the visible card
//! text reads "Input $2.00 / 1M tokens, Output $6.00 / 1M tokens", and OpenRouter lists
//! `x-ai/grok-4.7` at $2 in / $6 out per 1M; `grok-4.3` 12500/25000 matches OpenRouter's
//! $1.25/$2.50 and `grok-build-0.1` 10000/20000 matches $1/$2.
//! `*LongContext` prices apply above `longContextThreshold` tokens and go in `extra`.

use serde_json::{Value, json};

use super::text::embedded_json;
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const URL: &str = "https://docs.x.ai/developers/models";
const MARKER: &str = "__XAI_PUBLIC_MODELS__=";
/// Price units per USD per million tokens.
const SCALE: f64 = 10_000.0;

pub static SPEC: LabSpec = LabSpec {
    id: "lab-xai",
    name: "xAI",
    organization: "xai",
    origin: "https://docs.x.ai",
    paths: &["/developers/models"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// A price field in USD per million tokens: absent -> None, present but unreadable -> Err.
fn price(m: &Value, key: &str) -> Result<Option<f64>, String> {
    match m.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let n = match v {
                Value::String(s) => s.trim().parse::<f64>().ok(),
                Value::Number(n) => n.as_f64(),
                _ => None,
            };
            n.map(|x| round6(x / SCALE))
                .map(Some)
                .ok_or_else(|| format!("{key}: {v} is not a price"))
        }
    }
}

/// A dated segment such as `0309` or `20250101` in a model id.
fn is_dated(id: &str) -> bool {
    id.split('-')
        .any(|p| matches!(p.len(), 4 | 8) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// The id the API takes, undated when possible: the name unless it is dated, then the
/// shortest alias that is neither dated, a `-latest` pointer nor a beta.
fn pick_id(name: &str, aliases: &[&str]) -> String {
    if !is_dated(name) {
        return name.to_owned();
    }
    aliases
        .iter()
        .filter(|a| !is_dated(a) && !a.contains("latest") && !a.contains("beta"))
        .min_by_key(|a| a.len())
        .map_or_else(|| name.to_owned(), |a| (*a).to_owned())
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let data = embedded_json(page, MARKER).ok_or("no __XAI_PUBLIC_MODELS__ data on the page")?;
    let clusters = data["clusterConfigs"]
        .as_array()
        .ok_or("no clusterConfigs in the models data")?;

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for model in clusters
        .iter()
        .flat_map(|c| c["languageModels"].as_array().into_iter().flatten())
    {
        let name = model["name"]
            .as_str()
            .ok_or("language model without a name")?;
        let (Some(input), Some(output)) = (
            price(model, "promptTextTokenPrice").map_err(|e| format!("{name}: {e}"))?,
            price(model, "completionTextTokenPrice").map_err(|e| format!("{name}: {e}"))?,
        ) else {
            continue;
        };
        let aliases: Vec<&str> = model["aliases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let id = pick_id(name, &aliases);
        if !seen.insert(id.clone()) {
            continue;
        }
        let p = |key: &str| price(model, key).map_err(|e| format!("{name}: {e}"));
        let mut extra = serde_json::Map::new();
        if let Some(v) = p("cachedPromptTokenPrice")? {
            extra.insert("cached_input_per_mtok".into(), json!(v));
        }
        if let (Some(i), Some(o)) = (
            p("promptTextTokenPriceLongContext")?,
            p("completionTokenPriceLongContext")?,
        ) {
            let mut long = json!({"input_per_mtok": i, "output_per_mtok": o});
            if let Some(c) = p("cachedPromptTokenPriceLongContext")? {
                long["cached_input_per_mtok"] = json!(c);
            }
            if let Some(t) = model["longContextThreshold"]
                .as_str()
                .and_then(|t| t.parse::<u64>().ok())
            {
                long["above_tokens"] = json!(t);
            }
            extra.insert("long_context".into(), long);
        }
        if let Some(d) = model["batchDiscountPercent"].as_f64() {
            extra.insert("batch_discount_percent".into(), json!(d));
        }
        extra.insert("api_name".into(), json!(name));
        extra.insert("aliases".into(), json!(aliases));

        out.push(ModelListing {
            external_id: id.clone(),
            name: id,
            maker: Some("x-ai".into()),
            context_window: model["maxPromptLength"]
                .as_i64()
                .and_then(|n| i32::try_from(n).ok()),
            input_per_mtok: input,
            output_per_mtok: output,
            variant: None,
            source_url: Some(URL.into()),
            extra: Value::Object(extra),
            media: None,
            capabilities: Vec::new(),
        });
    }
    if out.is_empty() {
        return Err("the models data holds no priced language models".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from the live page: three language models and an image model with no token price.
    const FIXTURE: &str = r#"<script>globalThis.__XAI_PUBLIC_MODELS__={"clusterConfigs":[{"languageModels":[{"name":"grok-build-0.1","version":"1.0","inputModalities":["TEXT","IMAGE"],"outputModalities":["TEXT"],"promptTextTokenPrice":"10000","promptImageTokenPrice":"10000","promptTextTokenPriceLongContext":"20000","cachedPromptTokenPrice":"2000","cachedPromptTokenPriceLongContext":"4000","completionTextTokenPrice":"20000","completionTokenPriceLongContext":"40000","rps":"37","cluster":"us-east-1","maxPromptLength":256000,"aliases":["grok-code-fast-1","grok-code-fast","grok-code-fast-1-0825"],"features":{"functionCalling":true},"longContextThreshold":"200000","batchDiscountPercent":20},{"name":"grok-4.7","version":"1.0","inputModalities":["TEXT","IMAGE"],"outputModalities":["TEXT"],"promptTextTokenPrice":"20000","promptTextTokenPriceLongContext":"40000","cachedPromptTokenPrice":"5000","cachedPromptTokenPriceLongContext":"10000","completionTextTokenPrice":"60000","completionTokenPriceLongContext":"120000","cluster":"us-east-1","maxPromptLength":500000,"aliases":[],"longContextThreshold":"200000"},{"name":"grok-4.20-0309-reasoning","version":"1.0","inputModalities":["TEXT","IMAGE"],"outputModalities":["TEXT"],"promptTextTokenPrice":"12500","cachedPromptTokenPrice":"2000","completionTextTokenPrice":"25000","cluster":"us-east-1","maxPromptLength":1000000,"aliases":["grok-4.20-reasoning-latest","grok-4.20","grok-4.20-reasoning"]}],"imageGenerationModels":[{"name":"grok-imagine-image-2.0","imagePrice":"600000000"}],"clusterName":"us-east-1"}]};</script>"#;

    fn by_id<'a>(l: &'a [ModelListing], id: &str) -> &'a ModelListing {
        l.iter().find(|m| m.external_id == id).unwrap()
    }

    #[test]
    fn reads_language_models_at_the_confirmed_scale() {
        let l = parse(&[FIXTURE.to_owned()]).unwrap();
        assert_eq!(l.len(), 3, "the image model has no token price");
        let m = by_id(&l, "grok-4.7");
        assert_eq!(m.maker.as_deref(), Some("x-ai"));
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (2.0, 6.0));
        assert_eq!(m.context_window, Some(500_000));
        assert_eq!(m.extra["cached_input_per_mtok"], 0.5);
        assert_eq!(m.extra["long_context"]["input_per_mtok"], 4.0);
        assert_eq!(m.extra["long_context"]["above_tokens"], 200_000);
        let m = by_id(&l, "grok-build-0.1");
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (1.0, 2.0));
        assert_eq!(m.context_window, Some(256_000));
        assert_eq!(m.extra["batch_discount_percent"], 20.0);
        let m = by_id(&l, "grok-4.20");
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (1.25, 2.5));
        assert_eq!(m.extra["api_name"], "grok-4.20-0309-reasoning");
        assert_eq!(m.source_url.as_deref(), Some(URL));
    }

    #[test]
    fn errors_when_the_data_is_missing_or_a_price_is_text() {
        assert!(parse(&["<html></html>".to_owned()]).is_err());
        let bad = FIXTURE.replace(
            "\"promptTextTokenPrice\":\"12500\"",
            "\"promptTextTokenPrice\":\"free\"",
        );
        assert!(parse(&[bad]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_XAI").expect("LAB_FIXTURE_XAI");
        let page = std::fs::read_to_string(path).unwrap();
        for m in parse(&[page]).unwrap() {
            println!(
                "{} in={} out={} ctx={:?} extra={}",
                m.external_id, m.input_per_mtok, m.output_per_mtok, m.context_window, m.extra
            );
        }
    }
}
