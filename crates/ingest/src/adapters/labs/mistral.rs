//! Mistral AI: models and prices from the lab's public pricing pages.
//!
//! One table per model family; each row has the API name and a price cell whose lines read
//! `Input $0.15 /M Tokens<br>Cached input $0.03 /M Tokens<br>Output $0.5 /M Tokens`. Rows
//! priced per page, minute or character, or without a price, are skipped. The page also
//! resells a few partner models (Z.ai, DeepSeek); they take their own maker.

use serde_json::{Map, Value, json};

use super::text::{first_dollar, md_tables, plain};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::ModelListing;

pub static SPEC: LabSpec = LabSpec {
    id: "lab-mistral",
    name: "Mistral AI",
    organization: "mistral-ai",
    origin: "https://docs.mistral.ai",
    paths: &["/inference/pricing.md"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

const SOURCE: &str = "https://docs.mistral.ai/inference/pricing";

fn maker(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.starts_with("z.ai") || n.starts_with("glm") {
        "z-ai"
    } else if n.starts_with("deepseek") {
        "deepseek"
    } else {
        "mistralai"
    }
}

/// The price of a `/M Tokens` line, when the line is priced per million tokens.
fn per_mtok(line: &str) -> Result<Option<f64>, String> {
    if !line.to_ascii_lowercase().contains("/m tokens") {
        return Ok(None);
    }
    first_dollar(line)
        .map(Some)
        .ok_or_else(|| format!("mistral: unreadable price {line:?}"))
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let body = pages.first().ok_or("mistral: no pricing page")?;
    let mut out = Vec::new();
    let mut saw_table = false;
    for table in md_tables(body) {
        let Some(head) = table.first() else { continue };
        let lower: Vec<_> = head.iter().map(|h| h.to_ascii_lowercase()).collect();
        let (Some(c_name), Some(c_id), Some(c_price)) = (
            lower.iter().position(|h| h == "model"),
            lower.iter().position(|h| h == "api name"),
            lower.iter().position(|h| h == "price"),
        ) else {
            continue;
        };
        saw_table = true;
        for r in &table[1..] {
            let (Some(name), Some(id), Some(price)) = (r.get(c_name), r.get(c_id), r.get(c_price))
            else {
                continue;
            };
            let (name, id) = (plain(name), plain(id));
            if id.is_empty() {
                continue;
            }
            let (mut input, mut output, mut cached) = (None, None, None);
            for line in price.split("<br>") {
                let line = line.trim();
                let l = line.to_ascii_lowercase();
                if l.starts_with("cached input") {
                    cached = cached.or(per_mtok(line)?);
                } else if l.starts_with("input") {
                    input = input.or(per_mtok(line)?);
                } else if l.starts_with("output") {
                    output = output.or(per_mtok(line)?);
                }
            }
            let (Some(input), Some(output)) = (input, output) else {
                continue;
            };
            let mut extra = Map::new();
            if let Some(c) = cached {
                extra.insert("cached_input_per_mtok".into(), json!(c));
            }
            out.push(ModelListing {
                external_id: id,
                maker: Some(maker(&name).into()),
                name,
                context_window: None,
                input_per_mtok: input,
                output_per_mtok: output,
                variant: None,
                source_url: Some(SOURCE.into()),
                extra: Value::Object(extra),
                media: None,
                capabilities: Vec::new(),
            });
        }
    }
    if !saw_table {
        return Err("mistral: no model price table found".into());
    }
    if out.is_empty() {
        return Err("mistral: the price tables hold no per-token models".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"# API pricing

## Generalist models

| Model | API name | Price |
| --- | --- | --- |
| [Z.ai GLM 5.3 Flash](/models/zai-glm-5-3-flash) | `zai-glm-5-3-flash` | Input $0.15 /M Tokens<br>Cached input $0.03 /M Tokens<br>Output $0.5 /M Tokens |
| [Mistral Small 4](/models/mistral-small-4-0-26-03) | `mistral-small-2603` | Input $0.15 /M Tokens<br>Output $0.6 /M Tokens |
| [Mistral Large 4](/models/mistral-large-4-0) | `mistral-large-4` | Input $0.7 /M Tokens<br>Cached input $0.07 /M Tokens<br>Output $2.1 /M Tokens |

## OCR models

| Model | API name | Price |
| --- | --- | --- |
| [OCR 3](/models/ocr-3-25-12) | `mistral-ocr-2512` | Input $2 /1000 Pages<br>Input $3 /1000 Annotated Pages |

## Audio models

| Model | API name | Price |
| --- | --- | --- |
| [Voxtral Mini Transcribe 3](/models/voxtral-mini-transcribe-3-0) | `voxtral-mini-transcribe-3-0` | Input $0.0035 /Min |

## Embedding models

| Model | API name | Price |
| --- | --- | --- |
| [Mistral Embed](/models/mistral-embed-23-12) | `mistral-embed-2312` | Input $0.1 /M Tokens |

## Time series models

| Model | API name | Price |
| --- | --- | --- |
| [Timestral 1.0 5M](/models/timestral-1-0-5m) |  |  |
"#;

    #[test]
    fn reads_token_priced_models_only() {
        let l = parse(&[PAGE.into()]).unwrap();
        let ids: Vec<_> = l.iter().map(|m| m.external_id.as_str()).collect();
        assert_eq!(
            ids,
            ["zai-glm-5-3-flash", "mistral-small-2603", "mistral-large-4"]
        );
        let s = &l[1];
        assert_eq!(s.name, "Mistral Small 4");
        assert_eq!(s.maker.as_deref(), Some("mistralai"));
        assert_eq!((s.input_per_mtok, s.output_per_mtok), (0.15, 0.6));
        assert_eq!(s.context_window, None);
        let big = &l[2];
        assert_eq!((big.input_per_mtok, big.output_per_mtok), (0.7, 2.1));
        assert_eq!(big.extra["cached_input_per_mtok"], 0.07);
        assert_eq!(l[0].maker.as_deref(), Some("z-ai"));
    }

    #[test]
    fn page_without_tables_or_with_bad_price_is_an_error() {
        assert!(parse(&["# nothing".into()]).is_err());
        let bad = PAGE.replace("Output $0.6 /M Tokens", "Output $x /M Tokens");
        assert!(parse(&[bad]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let body = std::fs::read_to_string(std::env::var("LAB_FIXTURE_MISTRAL").unwrap()).unwrap();
        for m in parse(&[body]).unwrap() {
            println!(
                "{:<30} {:<28} {:<10} in={} out={} {}",
                m.external_id,
                m.name,
                m.maker.unwrap_or_default(),
                m.input_per_mtok,
                m.output_per_mtok,
                m.extra
            );
        }
    }
}
