//! Anthropic: models and prices from the lab's public pricing pages.
//!
//! `pricing.md` holds the per-model token table (and the batch table); `overview.md` holds the
//! context window of every model. Rows marked retired are skipped: the Anthropic API no
//! longer serves them.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use super::text::{first_dollar, md_tables, plain};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::ModelListing;

pub static SPEC: LabSpec = LabSpec {
    id: "lab-anthropic",
    name: "Anthropic",
    organization: "anthropic",
    origin: "https://platform.claude.com",
    paths: &[
        "/docs/en/about-claude/pricing.md",
        "/docs/en/about-claude/models/overview.md",
    ],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

const SOURCE: &str = "https://platform.claude.com/docs/en/about-claude/pricing";

/// A model cell of a table: `Claude Haiku 5.5 (for prompts over 100,000 tokens)`.
struct Row {
    name: String,
    /// The prompt-length qualifier, when the model is priced by prompt length.
    tier: Option<String>,
    retired: bool,
    limited: bool,
}

fn row(cell: &str) -> Row {
    let lower = cell.to_ascii_lowercase();
    let text = plain(cell);
    let (name, rest) = match text.find(" (") {
        Some(i) => (text[..i].trim().to_owned(), text[i + 2..].to_owned()),
        None => (text.trim().to_owned(), String::new()),
    };
    let tier = rest.starts_with("for prompts").then(|| {
        rest.trim_start_matches("for prompts")
            .trim_end_matches(')')
            .trim()
            .to_owned()
    });
    Row {
        name,
        tier,
        retired: lower.contains("retired"),
        limited: lower.contains("limited availability"),
    }
}

/// `Claude Opus 4.5` -> `claude-opus-4-5`, the id of the Claude API.
fn api_id(name: &str) -> String {
    name.to_ascii_lowercase().replace(['.', ' '], "-")
}

/// `1M tokens` -> 1_000_000, `200K tokens` -> 200_000.
fn tokens(s: &str) -> Option<i32> {
    let t = s.split_whitespace().next()?.to_ascii_uppercase();
    let (n, mult) = if let Some(n) = t.strip_suffix('M') {
        (n, 1_000_000.0)
    } else {
        (t.strip_suffix('K')?, 1_000.0)
    };
    Some((n.parse::<f64>().ok()? * mult).round() as i32)
}

fn price(cell: &str, what: &str, model: &str) -> Result<f64, String> {
    first_dollar(cell).ok_or_else(|| format!("anthropic: no {what} price for {model}: {cell:?}"))
}

fn col(header: &[String], needle: &str) -> Option<usize> {
    header
        .iter()
        .position(|h| h.to_ascii_lowercase().contains(needle))
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let pricing = pages.first().ok_or("anthropic: no pricing page")?;
    let tables = md_tables(pricing);
    let main = tables
        .iter()
        .find(|t| t.first().is_some_and(|h| col(h, "base input").is_some()))
        .ok_or("anthropic: model pricing table not found")?;
    let h = &main[0];
    let (c_in, c_out) = (col(h, "base input"), col(h, "output tokens"));
    let (Some(c_in), Some(c_out)) = (c_in, c_out) else {
        return Err("anthropic: pricing table lacks input/output columns".into());
    };
    let (c_w5, c_w1, c_hit) = (col(h, "5m cache"), col(h, "1h cache"), col(h, "cache hits"));

    // Batch prices by (model, tier).
    let mut batch: HashMap<(String, Option<String>), (f64, f64)> = HashMap::new();
    if let Some(bt) = tables
        .iter()
        .find(|t| t.first().is_some_and(|h| col(h, "batch input").is_some()))
    {
        let (bi, bo) = (col(&bt[0], "batch input"), col(&bt[0], "batch output"));
        if let (Some(bi), Some(bo)) = (bi, bo) {
            for r in &bt[1..] {
                let (Some(a), Some(b)) = (
                    r.get(bi).and_then(|c| first_dollar(c)),
                    r.get(bo).and_then(|c| first_dollar(c)),
                ) else {
                    continue;
                };
                let k = row(&r[0]);
                batch.insert((k.name, k.tier), (a, b));
            }
        }
    }

    // Context windows by model name.
    let mut context: HashMap<String, i32> = HashMap::new();
    if let Some(overview) = pages.get(1)
        && let Some(t) = md_tables(overview).into_iter().find(|t| {
            t.first()
                .is_some_and(|h| col(h, "context window").is_some())
        })
    {
        let cc = col(&t[0], "context window").unwrap_or(1);
        for r in &t[1..] {
            if let Some(n) = r.get(cc).and_then(|c| tokens(c)) {
                context.insert(row(&r[0]).name, n);
            }
        }
    }

    // Rows in page order; a model priced by prompt length has several, the first wins.
    let mut out: Vec<ModelListing> = Vec::new();
    let mut later_tiers: HashMap<String, Vec<Value>> = HashMap::new();
    for r in &main[1..] {
        let k = row(&r[0]);
        if k.retired || !k.name.starts_with("Claude ") {
            continue;
        }
        let cell = |i: usize| r.get(i).map(String::as_str).unwrap_or("");
        let input = price(cell(c_in), "input", &k.name)?;
        let output = price(cell(c_out), "output", &k.name)?;
        let mut extra = Map::new();
        for (key, c) in [
            ("cache_write_5m_per_mtok", c_w5),
            ("cache_write_1h_per_mtok", c_w1),
            ("cached_input_per_mtok", c_hit),
        ] {
            if let Some(v) = c.and_then(|c| first_dollar(cell(c))) {
                extra.insert(key.into(), json!(v));
            }
        }
        if let Some((bi, bo)) = batch.get(&(k.name.clone(), k.tier.clone())) {
            extra.insert("batch_input_per_mtok".into(), json!(bi));
            extra.insert("batch_output_per_mtok".into(), json!(bo));
        }
        if k.limited {
            extra.insert("limited_availability".into(), json!(true));
        }
        if k.tier.is_some() && out.iter().any(|m| m.name == k.name) {
            let mut tier = extra.clone();
            tier.insert("prompt".into(), json!(k.tier));
            tier.insert("input_per_mtok".into(), json!(input));
            tier.insert("output_per_mtok".into(), json!(output));
            later_tiers
                .entry(k.name)
                .or_default()
                .push(Value::Object(tier));
            continue;
        }
        if let Some(t) = &k.tier {
            extra.insert("first_tier".into(), json!(t));
        }
        out.push(ModelListing {
            external_id: api_id(&k.name),
            context_window: context.get(&k.name).copied(),
            name: k.name,
            maker: Some("anthropic".into()),
            input_per_mtok: input,
            output_per_mtok: output,
            variant: None,
            source_url: Some(SOURCE.into()),
            extra: Value::Object(extra),
            media: None,
            capabilities: Vec::new(),
        });
    }
    for m in &mut out {
        if let (Some(t), Value::Object(e)) = (later_tiers.remove(&m.name), &mut m.extra) {
            e.insert("longer_prompt_tiers".into(), Value::Array(t));
        }
    }
    if out.is_empty() {
        return Err("anthropic: the pricing table holds no models".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRICING: &str = r#"## Model pricing

| Model | Base input tokens | 5m cache writes | 1h cache writes | Cache hits and refreshes | Output tokens |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Claude Fable 5.2 | $8 / MTok | $10 / MTok | $16 / MTok | $0.20 / MTok<sup>1</sup> | $40 / MTok |
| Claude Mythos 5.2 ([limited availability](https://support.claude.com/en/articles/14604842)) | $8 / MTok | $10 / MTok | $16 / MTok | $0.20 / MTok<sup>1</sup> | $40 / MTok |
| Claude Opus 4.5 | $5 / MTok | $6.25 / MTok | $10 / MTok | $0.50 / MTok | $25 / MTok |
| Claude Opus 4 ([retired, except on Claude Models on Google Cloud (Google managed)](https://platform.claude.com/docs/en/about-claude/model-deprecations)) | $15 / MTok | $18.75 / MTok | $30 / MTok | $1.50 / MTok | $75 / MTok |
| Claude Haiku 5.5 (for prompts up to 100,000 tokens) | $0.10 / MTok | $0.125 / MTok | $0.20 / MTok | $0.01 / MTok | $0.50 / MTok |
| Claude Haiku 5.5 (for prompts over 100,000 tokens) | $0.50 / MTok | $0.625 / MTok | $1 / MTok | $0.05 / MTok | $2.50 / MTok |

### Batch processing

| Model | Batch input | Batch output |
| :--- | :--- | :--- |
| Claude Fable 5.2 | $4 / MTok | $20 / MTok |
| Claude Opus 4.5 | $2.50 / MTok | $12.50 / MTok |
| Claude Haiku 5.5 (for prompts up to 100,000 tokens) | $0.05 / MTok | $0.25 / MTok |
| Claude Haiku 5.5 (for prompts over 100,000 tokens) | $0.25 / MTok | $1.25 / MTok |
"#;

    const OVERVIEW: &str = r#"| Model | Capabilities: Context window | Capabilities: Max output | Base tokens: Input | Base tokens: Output |
| :--- | :--- | :--- | :--- | :--- |
| [Claude Fable 5.2](https://platform.claude.com/docs/en/models/fable-5-2/overview) | 1M tokens | 128K tokens | $8 / MTok | $40 / MTok |
| [Claude Mythos 5.2](https://platform.claude.com/docs/en/models/mythos-5-2/overview) ([limited availability](https://support.claude.com/en/articles/14604842)) | 1M tokens | 128K tokens | $8 / MTok | $40 / MTok |
| [Claude Opus 4.5](https://platform.claude.com/docs/en/models/opus-4-5/overview) | 200K tokens | 64K tokens | $5 / MTok | $25 / MTok |
| [Claude Haiku 5.5](https://platform.claude.com/docs/en/models/haiku-5-5/overview) | 1M tokens | 128K tokens | From $0.10 / MTok | From $0.50 / MTok |
"#;

    fn pages() -> Vec<String> {
        vec![PRICING.into(), OVERVIEW.into()]
    }

    #[test]
    fn reads_prices_ids_and_context() {
        let l = parse(&pages()).unwrap();
        let ids: Vec<_> = l.iter().map(|m| m.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "claude-fable-5-2",
                "claude-mythos-5-2",
                "claude-opus-4-5",
                "claude-haiku-5-5"
            ]
        );
        let f = &l[0];
        assert_eq!(f.maker.as_deref(), Some("anthropic"));
        assert_eq!((f.input_per_mtok, f.output_per_mtok), (8.0, 40.0));
        assert_eq!(f.context_window, Some(1_000_000));
        assert_eq!(f.extra["cached_input_per_mtok"], 0.2);
        assert_eq!(f.extra["batch_output_per_mtok"], 20.0);
        let o = &l[2];
        assert_eq!((o.input_per_mtok, o.output_per_mtok), (5.0, 25.0));
        assert_eq!(o.context_window, Some(200_000));
        assert_eq!(o.extra["cache_write_5m_per_mtok"], 6.25);
    }

    #[test]
    fn skips_retired_and_marks_limited_access() {
        let l = parse(&pages()).unwrap();
        assert!(l.iter().all(|m| m.external_id != "claude-opus-4"));
        assert_eq!(l[1].extra["limited_availability"], true);
    }

    #[test]
    fn prompt_length_tiers_keep_the_first_and_record_the_rest() {
        let l = parse(&pages()).unwrap();
        let h = l
            .iter()
            .find(|m| m.external_id == "claude-haiku-5-5")
            .unwrap();
        assert_eq!((h.input_per_mtok, h.output_per_mtok), (0.1, 0.5));
        assert_eq!(h.extra["batch_input_per_mtok"], 0.05);
        let t = &h.extra["longer_prompt_tiers"][0];
        assert_eq!(
            (t["input_per_mtok"].as_f64(), t["output_per_mtok"].as_f64()),
            (Some(0.5), Some(2.5))
        );
        assert_eq!(l.iter().filter(|m| m.name == "Claude Haiku 5.5").count(), 1);
    }

    #[test]
    fn missing_table_or_bad_price_is_an_error() {
        assert!(parse(&["nothing".into()]).is_err());
        let bad = PRICING.replace("$8 / MTok | $10", "TBD | $10");
        assert!(parse(&[bad, OVERVIEW.into()]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let read = |k: &str| std::fs::read_to_string(std::env::var(k).unwrap()).unwrap();
        let pages = vec![
            read("LAB_FIXTURE_ANTHROPIC"),
            read("LAB_FIXTURE_ANTHROPIC_OVERVIEW"),
        ];
        for m in parse(&pages).unwrap() {
            println!(
                "{:<22} {:<30} ctx={:?} in={} out={} {}",
                m.external_id,
                m.name,
                m.context_window,
                m.input_per_mtok,
                m.output_per_mtok,
                m.extra
            );
        }
    }
}
