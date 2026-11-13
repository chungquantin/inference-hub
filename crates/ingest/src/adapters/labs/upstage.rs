//! Upstage: models and prices from the lab's public pricing pages.
//!
//! Each Solar model has a docs page (`/docs/models/<model>.md`) that lists its prices, context
//! length and API alias. The docs index (`llms.txt`) does not carry prices. Document and
//! structured-decision models (Solar Decide, Document Parse, ...) are not chat models and
//! are not listed here.

use serde_json::{Map, Value, json};

use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::ModelListing;

pub static SPEC: LabSpec = LabSpec {
    id: "lab-upstage",
    name: "Upstage",
    organization: "upstage",
    origin: "https://console.upstage.ai",
    paths: &[
        "/docs/models/solar-pro-4.md",
        "/docs/models/solar-mini-4.md",
    ],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

const SOURCE: &str = "https://console.upstage.ai/docs/models";

/// `$0.30/1M tokens (Input Token)` -> (`Input`, 0.30)
fn price_line(line: &str) -> Option<(String, f64)> {
    let rest = line.trim().strip_prefix("- **Price**:")?.trim();
    let usd = rest.strip_prefix('$')?;
    let end = usd.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let value = usd[..end].parse().ok()?;
    let kind = rest.split('(').nth(1)?.split_whitespace().next()?;
    Some((kind.to_owned(), value))
}

/// `- 512K context length` -> 512_000
fn context(line: &str) -> Option<i32> {
    let t = line.trim().strip_prefix("- ")?;
    let word = t.strip_suffix(" context length")?;
    let (n, mult) = match word.chars().last()? {
        'K' | 'k' => (&word[..word.len() - 1], 1_000.0),
        'M' | 'm' => (&word[..word.len() - 1], 1_000_000.0),
        _ => return None,
    };
    Some((n.parse::<f64>().ok()? * mult).round() as i32)
}

/// The alias of the newest version: `### solar-pro4-260806 (Alias: solar-pro4)`.
fn alias(line: &str) -> Option<String> {
    let h = line.trim().strip_prefix("### ")?;
    let a = h.split("(Alias:").nth(1)?;
    Some(a.trim_end_matches(')').trim().to_owned())
}

fn one(page: &str) -> Result<ModelListing, String> {
    let name = page
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .ok_or("upstage: model page has no title")?
        .trim()
        .to_owned();
    let (mut input, mut output, mut cached) = (None, None, None);
    let (mut ctx, mut id) = (None, None);
    for line in page.lines() {
        if let Some((kind, v)) = price_line(line) {
            match kind.as_str() {
                "Input" => input = input.or(Some(v)),
                "Output" => output = output.or(Some(v)),
                "Cached" => cached = cached.or(Some(v)),
                _ => {}
            }
        } else if ctx.is_none() {
            ctx = context(line);
        }
        if id.is_none() {
            id = alias(line);
        }
    }
    let id = id.ok_or_else(|| format!("upstage: no API alias for {name}"))?;
    let (Some(input), Some(output)) = (input, output) else {
        return Err(format!("upstage: no input/output price for {name}"));
    };
    let mut extra = Map::new();
    if let Some(c) = cached {
        extra.insert("cached_input_per_mtok".into(), json!(c));
    }
    Ok(ModelListing {
        external_id: id,
        name,
        maker: Some("upstage".into()),
        context_window: ctx,
        input_per_mtok: input,
        output_per_mtok: output,
        variant: None,
        source_url: Some(SOURCE.into()),
        extra: Value::Object(extra),
        media: None,
        capabilities: Vec::new(),
    })
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    if pages.is_empty() {
        return Err("upstage: no model pages".into());
    }
    pages.iter().map(|p| one(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRO: &str = r#"> For the complete documentation index, see [/llms.txt](/llms.txt).

# Solar Pro 4

> Source: /docs/models/solar-pro-4

Upstage's flagship model, specialized for agentic use

- **Intelligence**: 5 (Higher)
- **Speed**: 5 (Fast)
- **Price**: $0.30/1M tokens (Input Token)
- **Price**: $1.20/1M tokens (Output Token)
- **Price**: $0.06/1M tokens (Cached Token)

- 512K context length
- Undisclosed parameters

## Versions

### solar-pro4-260806 (Alias: solar-pro4)

Released on 2026-08-06
"#;

    const MINI: &str = r#"# Solar Mini 4

> Source: /docs/models/solar-mini-4

- **Price**: $0.10/1M tokens (Input Token)
- **Price**: $0.40/1M tokens (Output Token)
- **Price**: $0.01/1M tokens (Cached Token)

- 512K context length
- 35B / 3B active parameters

### solar-mini4-260922 (Alias: solar-mini4)
"#;

    const DECIDE: &str = r#"# Solar Decide

- **Price**: $0.1/1M tokens (Input Token)
- **Price**: Free (Output Token)
- **Price**: $0.1/1M tokens (Cached Token)

- 512K context length

### solar-mini4-260922 (Alias: solar-mini4)
"#;

    #[test]
    fn reads_prices_alias_and_context() {
        let l = parse(&[PRO.into(), MINI.into()]).unwrap();
        assert_eq!(l.len(), 2);
        let p = &l[0];
        assert_eq!(p.external_id, "solar-pro4");
        assert_eq!(p.name, "Solar Pro 4");
        assert_eq!(p.maker.as_deref(), Some("upstage"));
        assert_eq!((p.input_per_mtok, p.output_per_mtok), (0.3, 1.2));
        assert_eq!(p.context_window, Some(512_000));
        assert_eq!(p.extra["cached_input_per_mtok"], 0.06);
        let m = &l[1];
        assert_eq!(m.external_id, "solar-mini4");
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (0.1, 0.4));
    }

    #[test]
    fn a_model_without_token_prices_is_an_error() {
        assert!(one(DECIDE).is_err());
        assert!(parse(&[]).is_err());
        assert!(parse(&["# Page\n\nnothing".into()]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let paths = std::env::var("LAB_FIXTURE_UPSTAGE").unwrap();
        let pages: Vec<String> = paths
            .split(':')
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect();
        for m in parse(&pages).unwrap() {
            println!(
                "{:<14} {:<14} ctx={:?} in={} out={} {}",
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
