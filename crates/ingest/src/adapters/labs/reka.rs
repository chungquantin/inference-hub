//! Reka: models and prices from the lab's public pricing pages.
//!
//! `developer.reka.ai/models` lists one card per model: a `Maker : Name` heading, the
//! context window, the API id, then `$x/M input tokens $y/M output tokens [$z/M cached]`.

use serde_json::json;

use super::text::{dollars, html_lines};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const SOURCE: &str = "https://developer.reka.ai/models";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-reka",
    name: "Reka",
    organization: "reka-ai",
    origin: "https://developer.reka.ai",
    paths: &["/models"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// `Maker : Model name` heading of a card, when the next line is its context size.
fn card_heading(lines: &[String], i: usize) -> Option<(&str, &str)> {
    let (maker, name) = lines[i].split_once(" : ")?;
    lines.get(i + 1)?.strip_suffix(" context")?;
    Some((maker.trim(), name.trim()))
}

/// `262k` -> 262144, `1M` -> 1048576, `16k` -> 16384: the page rounds binary sizes, so a
/// size within 6% of a power of two is that power of two.
fn context(s: &str) -> Option<i32> {
    let s = s.trim();
    let (num, mult) = if let Some(n) = s.strip_suffix(['k', 'K']) {
        (n, 1e3)
    } else if let Some(n) = s.strip_suffix(['m', 'M']) {
        (n, 1e6)
    } else {
        (s, 1.0)
    };
    let v = num.replace(',', "").parse::<f64>().ok()? * mult;
    let pow = 2f64.powf(v.log2().round());
    let v = if (pow - v).abs() / v < 0.06 { pow } else { v };
    Some(v.round() as i32)
}

/// The maker prefix OpenRouter uses for the model's family.
fn maker(heading: &str) -> String {
    match heading.to_ascii_lowercase().as_str() {
        "reka" => "rekaai".into(),
        "glm" => "z-ai".into(),
        "gemma" => "google".into(),
        other => other.replace(' ', ""),
    }
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let mut out = Vec::new();
    let mut cards = 0;
    for i in 0..lines.len() {
        let Some((heading, name)) = card_heading(&lines, i) else {
            continue;
        };
        cards += 1;
        let window = context(lines[i + 1].trim_end_matches(" context"));
        // The card runs to the next heading.
        let end = (i + 2..lines.len())
            .find(|&j| card_heading(&lines, j).is_some())
            .unwrap_or(lines.len());
        let Some(p) = (i + 2..end).find(|&j| lines[j].contains("/M input tokens")) else {
            continue;
        };
        let id = lines[p - 1].trim();
        if id.is_empty() || id.contains(' ') {
            return Err(format!("{name}: no model id before its price"));
        }
        let line = &lines[p];
        let (_, rest) = line.split_once("/M input tokens").unwrap_or_default();
        if !rest.contains("/M output tokens") {
            return Err(format!("{id}: no output price in {line:?}"));
        }
        let prices = dollars(line);
        if prices.len() < 2 {
            return Err(format!("{id}: price will not parse in {line:?}"));
        }
        let extra = if line.contains("/M cached") && prices.len() >= 3 {
            json!({ "cached_input_per_mtok": round6(prices[2]) })
        } else {
            serde_json::Value::Null
        };
        out.push(ModelListing {
            external_id: id.to_owned(),
            name: name.to_owned(),
            maker: Some(maker(heading)),
            context_window: window,
            input_per_mtok: round6(prices[0]),
            output_per_mtok: round6(prices[1]),
            variant: None,
            source_url: Some(SOURCE.to_owned()),
            extra,
            media: None,
            capabilities: Vec::new(),
        });
    }
    if cards == 0 {
        return Err("no model cards found".into());
    }
    if out.is_empty() {
        return Err("model cards hold no prices".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<main><h1>Models</h1>
<div class="card"><h2>DeepSeek : DeepSeek V4 Flash</h2><div>262k context</div>
<div><span>Chat</span><span>Streaming</span></div>
<p class="mlist-blurb">DeepSeek V4 Flash is an open-weight reasoning model with a 262k context window, hosted by Reka.</p>
<div class="mlist-meta"><span class="mlist-id"><code>deepseek4-flash</code></span>
<div class="mlist-price"><span>$0.11/M input tokens</span> <span>$0.66/M output tokens</span> <span>$0.007/M cached</span> <span>131,072 max output</span></div></div></div>
<div class="card"><h2>GLM : GLM 5.3 Flash</h2><div>1M context</div>
<p class="mlist-blurb">GLM 5.3 Flash is an open-weight reasoning model.</p>
<div class="mlist-meta"><span class="mlist-id"><code>glm5.3-flash</code></span>
<div class="mlist-price"><span>$0.15/M input tokens</span> <span>$0.50/M output tokens</span> <span>$0.015/M cached</span></div></div></div>
<div class="card"><h2>Reka : Reka Flash 3</h2><div>64k context</div>
<p class="mlist-blurb">Reka Flash 3 is a 21B general-purpose reasoning LLM.</p>
<div class="mlist-meta"><span class="mlist-id"><code>reka-flash-3</code></span>
<div class="mlist-price"><span>$0.10/M input tokens</span> <span>$0.20/M output tokens</span> <span>65,536 max output</span></div></div></div>
<div class="card"><h2>Reka : Reka Preview</h2><div>16k context</div>
<p class="mlist-blurb">Not priced yet.</p>
<div class="mlist-meta"><span class="mlist-id"><code>reka-preview</code></span></div></div>
<p>Note Hover an icon to see what the capability means.</p></main>"#;

    #[test]
    fn parses_cards_and_skips_unpriced() {
        let l = parse(&[FIXTURE.to_owned()]).unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].external_id, "deepseek4-flash");
        assert_eq!(l[0].name, "DeepSeek V4 Flash");
        assert_eq!(l[0].maker.as_deref(), Some("deepseek"));
        assert_eq!(l[0].context_window, Some(262144));
        assert_eq!((l[0].input_per_mtok, l[0].output_per_mtok), (0.11, 0.66));
        assert_eq!(l[0].extra["cached_input_per_mtok"], 0.007);
        assert_eq!(l[1].external_id, "glm5.3-flash");
        assert_eq!(l[1].maker.as_deref(), Some("z-ai"));
        assert_eq!(l[1].context_window, Some(1048576));
        assert_eq!(l[2].external_id, "reka-flash-3");
        assert_eq!(l[2].maker.as_deref(), Some("rekaai"));
        assert_eq!((l[2].input_per_mtok, l[2].output_per_mtok), (0.10, 0.20));
        assert!(l[2].extra.is_null());
    }

    #[test]
    fn errors_when_structure_is_gone() {
        assert!(parse(&["<p>nothing</p>".to_owned()]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_REKA").expect("LAB_FIXTURE_REKA");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page]).unwrap() {
            println!("{l:?}");
        }
    }
}
