//! Google Gemini API: models and prices from the lab's public pricing pages.
//!
//! `pricing.md.txt` has one `## Name {% id="model-id" %}` section per model. Its first table
//! (the Standard tab) lists `Input price`, `Output price` and `Context caching price` rows
//! whose last cell is the paid tier, in USD per million tokens. A cell can carry several
//! prices: prompt-length tiers (`$2.00, prompts <= 200k tokens` / `$4.00, prompts > 200k`) or a
//! scheduled change (`$2.00 through November 30, 2026. $4.00 starting December 1, 2026.`).
//! The first price is the one in force; the rest go in `extra`. The Batch tab gives the batch
//! prices. The second page, the models page, gives each new model's context window.
//!
//! Models whose output is not a plain token price (images, audio, video, speech, per-minute,
//! embeddings, free-only open models) are skipped.

use serde_json::{Map, Value, json};

use super::text::{dollars, first_dollar, plain};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const PAGE: &str = "https://ai.google.dev/gemini-api/docs/pricing";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-google",
    name: "Google Gemini API",
    organization: "google",
    origin: "https://ai.google.dev",
    paths: &[
        "/gemini-api/docs/pricing.md.txt",
        "/gemini-api/docs/models.md.txt",
    ],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// Words in an output price cell that mean it is not a plain per-token text price.
const NOT_TEXT: [&str; 7] = [
    "(audio)",
    "(image",
    "(video)",
    "/min",
    "per image",
    "per 10s",
    "per second",
];

/// A table of `Feature | Free | Paid` rows, from the lines of a `{% table %}`.
fn rows(section: &str) -> Vec<Vec<String>> {
    let Some(start) = section.find("{% table %}") else {
        return Vec::new();
    };
    let body = &section[start + 11..];
    let body = &body[..body.find("{% /table %}").unwrap_or(body.len())];
    let mut rows = Vec::new();
    let mut cells: Vec<String> = Vec::new();
    for line in body.lines().map(str::trim) {
        if line == "---" {
            rows.push(std::mem::take(&mut cells));
        } else if let Some(cell) = line.strip_prefix("* ") {
            cells.push(cell.to_owned());
        } else if line == "*" {
            cells.push(String::new());
        } else if !line.is_empty()
            && let Some(last) = cells.last_mut()
        {
            last.push(' ');
            last.push_str(line);
        }
    }
    rows.push(cells);
    rows
}

/// The paid-tier cell of the row whose label starts with `label`.
fn paid(rows: &[Vec<String>], label: &str) -> Option<String> {
    rows.iter()
        .find(|r| r.first().is_some_and(|f| f.starts_with(label)))
        .and_then(|r| r.get(2).cloned())
}

/// The text of the tab titled `title`, or the whole section when it has no tabs.
fn tab<'a>(section: &'a str, title: &str) -> Option<&'a str> {
    let marker = format!("{{% tab title=\"{title}\" %}}");
    match section.find(&marker) {
        Some(at) => {
            let rest = &section[at + marker.len()..];
            Some(&rest[..rest.find("{% /tab %}").unwrap_or(rest.len())])
        }
        None if title == "Standard" && !section.contains("{% tab title=") => Some(section),
        None => None,
    }
}

/// Prices of one cell: the one in force, and what else it states.
struct Cell {
    first: f64,
    all: Vec<f64>,
    /// Date a scheduled price starts, when the cell has one.
    from: Option<String>,
}

fn cell(text: &str) -> Option<Cell> {
    // Only the part before storage prices (`$4.50 / 1,000,000 tokens per hour`).
    let head = text.split("/ 1,000,000").next().unwrap_or(text);
    // The price in force must open the cell; anything else is a layout this reader does not know.
    if !head.trim_start().starts_with('$') {
        return None;
    }
    let first = first_dollar(head)?;
    let from = head.find("starting ").map(|at| {
        let d = &head[at + 9..];
        d[..d.find(". ").or_else(|| d.find('.')).unwrap_or(d.len())]
            .trim()
            .to_owned()
    });
    // The later prices are the ones before the storage line only.
    let mut all = dollars(head);
    all.retain(|v| !text.contains(&format!("${v:.2} / 1,000,000")));
    Some(Cell { first, all, from })
}

/// `1M tokens` -> 1_000_000, `200K` -> 200_000, `1,048,576` -> 1_048_576.
fn tokens(s: &str) -> Option<i32> {
    let s = s.trim().trim_end_matches("tokens").trim().replace(',', "");
    let (num, mult) = match s.chars().last()? {
        'M' | 'm' => (&s[..s.len() - 1], 1_000_000.0),
        'K' | 'k' => (&s[..s.len() - 1], 1_000.0),
        _ => (s.as_str(), 1.0),
    };
    num.trim()
        .parse::<f64>()
        .ok()
        .and_then(|v| i32::try_from((v * mult).round() as i64).ok())
}

/// `model id -> context window` from the cards of the models page.
fn contexts(models: &str) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    let mut id: Option<String> = None;
    for line in models.lines() {
        if let Some(rest) = line.split("*Model code* **").nth(1) {
            id = rest.split("**").next().map(str::to_owned);
        } else if let Some(rest) = line.split("*Context* **").nth(1)
            && let (Some(id), Some(n)) = (id.take(), rest.split("**").next().and_then(tokens))
        {
            out.push((id, n));
        }
    }
    out
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let ctx = pages.get(1).map(|p| contexts(p)).unwrap_or_default();
    let mut out: Vec<ModelListing> = Vec::new();
    let mut sections = 0;
    for section in page.split("\n## ").skip(1) {
        let (title, body) = section.split_once('\n').unwrap_or((section, ""));
        let Some((name, attr)) = title.split_once("{% id=\"") else {
            continue;
        };
        let Some(id) = attr.split('"').next().filter(|i| i.starts_with("gemini-")) else {
            continue;
        };
        sections += 1;
        let standard = rows(tab(body, "Standard").unwrap_or(""));
        let (Some(input), Some(output)) = (
            paid(&standard, "Input price"),
            paid(&standard, "Output price"),
        ) else {
            continue;
        };
        // Not a per-token text price: media, speech, per-minute, free-only or not applicable.
        let lower = output.to_ascii_lowercase();
        if first_dollar(&input).is_none()
            || first_dollar(&output).is_none()
            || NOT_TEXT.iter().any(|w| lower.contains(w))
        {
            if first_dollar(&input).is_some()
                && first_dollar(&output).is_none()
                && !lower.contains("not applicable")
                && !lower.contains("not available")
            {
                return Err(format!("{id}: output price {output:?} does not parse"));
            }
            continue;
        }
        let (Some(i), Some(o)) = (cell(&input), cell(&output)) else {
            return Err(format!("{id}: prices {input:?} / {output:?} do not parse"));
        };
        let mut extra = Map::new();
        if input.contains("prompts") {
            extra.insert("input_tiers_per_mtok".into(), json!(i.all));
            extra.insert("output_tiers_per_mtok".into(), json!(o.all));
            extra.insert("tiers_by_prompt_tokens".into(), json!(plain(&input)));
        } else if let Some(from) = &i.from {
            extra.insert(
                "scheduled_price".into(),
                json!({
                    "starting": from,
                    "input_per_mtok": i.all.get(1),
                    "output_per_mtok": o.all.get(1),
                }),
            );
        }
        if let Some(c) = paid(&standard, "Context caching price")
            .as_deref()
            .and_then(cell)
        {
            extra.insert("cached_input_per_mtok".into(), json!(round6(c.first)));
        }
        let batch = rows(tab(body, "Batch").unwrap_or(""));
        if let (Some(bi), Some(bo)) = (
            paid(&batch, "Input price").as_deref().and_then(cell),
            paid(&batch, "Output price").as_deref().and_then(cell),
        ) {
            extra.insert(
                "batch".into(),
                json!({"input_per_mtok": round6(bi.first), "output_per_mtok": round6(bo.first)}),
            );
        }
        let name: String = plain(name).chars().filter(char::is_ascii).collect();
        out.push(ModelListing {
            external_id: id.to_owned(),
            name: name.trim().to_owned(),
            maker: Some("google".into()),
            context_window: ctx.iter().find(|(c, _)| c == id).map(|(_, n)| *n),
            input_per_mtok: round6(i.first),
            output_per_mtok: round6(o.first),
            variant: None,
            source_url: Some(PAGE.into()),
            extra: Value::Object(extra),
            media: None,
            capabilities: Vec::new(),
        });
    }
    if sections == 0 || out.is_empty() {
        return Err("no model sections with prices found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRICING: &str = r#"# Gemini Developer API pricing

## Gemini 4 Carbon  {% id="gemini-4-carbon" %}

{% tabs %}
  {% tab title="Standard" %}
  {% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Not available
  * $2.00 through November 30, 2026.
  $4.00 starting December 1, 2026.

  ---

  * Output price (including thinking tokens)
  * Not available
  * $10.00 through November 30, 2026.
  $20.00 starting December 1, 2026.

  ---

  * Context caching price
  * Not available
  * $0.10 through November 30, 2026.
  $0.20 starting December 1, 2026.
  Implicit caching only (explicit caching is not supported).
  {% /table %}
  {% /tab %}

  {% tab title="Batch" %}
  {% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Not available
  * $1.00 through November 30, 2026.
  $2.00 starting December 1, 2026.

  ---

  * Output price (including thinking tokens)
  * Not available
  * $5.00 through November 30, 2026.
  $10.00 starting December 1, 2026.
  {% /table %}
  {% /tab %}
{% /tabs %}

## Gemini 3.5 Flash-Lite  {% id="gemini-3.5-flash-lite" %}

{% tabs %}
  {% tab title="Standard" %}
  {% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Free of charge
  * $0.30 (text / image / video / audio)

  ---

  * Output price (including thinking tokens)
  * Free of charge
  * $2.50

  ---

  * Context caching price
  * Not available
  * $0.03
  $1.00 / 1,000,000 tokens per hour (storage price)
  {% /table %}
  {% /tab %}
{% /tabs %}

## Gemini 3.1 Pro Preview  {% id="gemini-3.1-pro-preview" %}

{% tabs %}
  {% tab title="Standard" %}
  {% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Not available
  * $2.00, prompts <= 200k tokens
  $4.00, prompts > 200k tokens

  ---

  * Output price (including thinking tokens)
  * Not available
  * $12.00, prompts <= 200k tokens
  $18.00, prompts > 200k

  ---

  * Context caching price
  * Not available
  * $0.20, prompts <= 200k tokens
  $0.40, prompts > 200k
  $4.50 / 1,000,000 tokens per hour (storage price)
  {% /table %}
  {% /tab %}
{% /tabs %}

## Gemini Nano Banana 2.1 🍌  {% id="gemini-nano-banana-2.1" %}

{% tabs %}
  {% tab title="Standard" %}
  {% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Not available
  * $1.50 (text/image/video)

  ---

  * Output price
  * Not available
  * $7.50 (text and thinking)
  $30.00 (images)
  Equivalent to $0.0336 per 1K image.
  {% /table %}
  {% /tab %}
{% /tabs %}

## Gemini 3.5 Transcribe Live  {% id="gemini-3.5-transcribe-live" %}

{% tabs %}
  {% tab title="Standard" %}
  {% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Free of charge
  * $3.50 or $0.005/min{% sup %}*{% /sup %} (audio)

  ---

  * Output price (including thinking tokens)
  * Free of charge
  * $21.00 or $0.004/min{% sup %}*{% /sup %} (text)
  {% /table %}
  {% /tab %}
{% /tabs %}

## [Gemma 4](https://ai.google.dev/gemma/docs/core/model_card_4) {% id="gemma-4" %}

{% table %}
  * Feature
  * Free Tier
  * Paid Tier, per 1M tokens in USD

  ---

  * Input price
  * Free of charge
  * Not available

  ---

  * Output price
  * Free of charge
  * Not available
{% /table %}

## Pricing for tools  {% id="pricing-for-tools" %}
"#;

    const MODELS: &str = r#"## Gemini 4  {% id="gemini-4" %}
  *   *Model code* **gemini-4-carbon**
  *   *Context* **1M tokens**
  *   *Max output* **65K / 1M tokens**
"#;

    fn parsed() -> Vec<ModelListing> {
        parse(&[PRICING.into(), MODELS.into()]).unwrap()
    }

    #[test]
    fn reads_current_prices_and_skips_media_models() {
        let l = parsed();
        let ids: Vec<_> = l.iter().map(|m| m.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "gemini-4-carbon",
                "gemini-3.5-flash-lite",
                "gemini-3.1-pro-preview"
            ]
        );

        let c = &l[0];
        assert_eq!(c.name, "Gemini 4 Carbon");
        assert_eq!(c.maker.as_deref(), Some("google"));
        assert_eq!((c.input_per_mtok, c.output_per_mtok), (2.0, 10.0));
        assert_eq!(c.context_window, Some(1_000_000));
        assert_eq!(c.extra["cached_input_per_mtok"], 0.1);
        assert_eq!(c.extra["batch"]["input_per_mtok"], 1.0);
        assert_eq!(c.extra["scheduled_price"]["input_per_mtok"], 4.0);
        assert_eq!(c.extra["scheduled_price"]["starting"], "December 1, 2026");

        let f = &l[1];
        assert_eq!((f.input_per_mtok, f.output_per_mtok), (0.3, 2.5));
        assert_eq!(f.extra["cached_input_per_mtok"], 0.03);
        assert_eq!(f.context_window, None);

        let p = &l[2];
        assert_eq!((p.input_per_mtok, p.output_per_mtok), (2.0, 12.0));
        assert_eq!(p.extra["input_tiers_per_mtok"][1], 4.0);
        assert_eq!(p.extra["cached_input_per_mtok"], 0.2);
    }

    #[test]
    fn a_page_that_changed_is_an_error() {
        assert!(parse(&["# moved".into()]).is_err());
        let bad = PRICING.replace("$12.00, prompts", "TBD, prompts");
        assert!(parse(&[bad]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_GOOGLE").expect("LAB_FIXTURE_GOOGLE");
        let mut pages = vec![std::fs::read_to_string(&path).unwrap()];
        if let Ok(models) = std::env::var("LAB_FIXTURE_GOOGLE_MODELS") {
            pages.push(std::fs::read_to_string(models).unwrap());
        }
        for l in parse(&pages).unwrap() {
            println!("{l:?}");
        }
    }
}
