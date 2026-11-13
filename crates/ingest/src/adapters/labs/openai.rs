//! OpenAI: models and prices from the lab's public pricing page.
//!
//! The page is server-rendered with Astro islands. Each `TextTokenPricingTables` island
//! carries its table as an HTML-escaped `props="..."` attribute in Astro's serialization, where
//! every value is wrapped as `[tag, payload]` (tag 0 = plain value or object, tag 1 = array).
//! One island exists per service tier (`standard`, `batch`, `fast`, `ultrafast`); a row is
//! `[model, input, cached input, output]` or `[model, input, cached input, cache write, output]`,
//! in USD per million tokens. The `standard` tier is the price; `batch` and `fast` go in `extra`.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const URL: &str = "https://platform.openai.com/docs/pricing";
const ISLAND: &str = "component-export=\"TextTokenPricingTables\"";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-openai",
    name: "OpenAI",
    organization: "openai",
    origin: "https://platform.openai.com",
    paths: &["/docs/pricing"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// One table row: model name and the prices it states, in USD per million tokens.
struct Row {
    name: String,
    input: Option<f64>,
    cached: Option<f64>,
    write: Option<f64>,
    output: Option<f64>,
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Astro island props: `[0, x]` is x (objects hold encoded values), `[1, [..]]` an array.
fn decode(v: &Value) -> Value {
    match v.as_array().map(Vec::as_slice) {
        Some([tag, payload]) if tag == 0 => match payload {
            Value::Object(m) => {
                Value::Object(m.iter().map(|(k, x)| (k.clone(), decode(x))).collect())
            }
            other => other.clone(),
        },
        Some([tag, Value::Array(items)]) if tag == 1 => {
            Value::Array(items.iter().map(decode).collect())
        }
        _ => v.clone(),
    }
}

/// A price cell: a number, or `-`/null when the page states none.
fn cell(v: &Value) -> Result<Option<f64>, String> {
    match v {
        Value::Number(n) => Ok(n.as_f64()),
        Value::Null => Ok(None),
        Value::String(s) if s.trim() == "-" || s.trim().is_empty() => Ok(None),
        other => Err(format!("price cell {other} is not a number")),
    }
}

/// The tables on the page by tier name.
fn tables(page: &str) -> Result<HashMap<String, Vec<Row>>, String> {
    let mut out = HashMap::new();
    let mut from = 0;
    while let Some(p) = page[from..].find(ISLAND) {
        let at = from + p + ISLAND.len();
        from = at;
        let Some(q) = page[at..].find("props=\"") else {
            break;
        };
        let start = at + q + 7;
        let Some(end) = page[start..].find('"') else {
            break;
        };
        let raw: Value = serde_json::from_str(&unescape(&page[start..start + end]))
            .map_err(|e| format!("pricing island props are not JSON: {e}"))?;
        // The props object itself is bare; only its values are wrapped.
        let props = decode(&json!([0, raw]));
        let tier = props["tier"].as_str().unwrap_or_default().to_owned();
        let rows = props["rows"]
            .as_array()
            .ok_or("pricing island has no rows")?;
        let mut parsed = Vec::new();
        for r in rows {
            let r = r.as_array().ok_or("pricing row is not an array")?;
            let name = r
                .first()
                .and_then(Value::as_str)
                .ok_or("pricing row has no model name")?
                .to_owned();
            let (input, cached, write, output) = match r.len() {
                4 => (&r[1], &r[2], &Value::Null, &r[3]),
                5 => (&r[1], &r[2], &r[3], &r[4]),
                n => return Err(format!("{name}: unexpected {n} columns")),
            };
            parsed.push(Row {
                input: cell(input).map_err(|e| format!("{name}: {e}"))?,
                cached: cell(cached).map_err(|e| format!("{name}: {e}"))?,
                write: cell(write).map_err(|e| format!("{name}: {e}"))?,
                output: cell(output).map_err(|e| format!("{name}: {e}"))?,
                name,
            });
        }
        out.entry(tier).or_insert(parsed);
    }
    Ok(out)
}

/// `gpt-5.5 (<272K context length)` -> (`gpt-5.5`, Some(`<272K context length`)).
fn split_name(raw: &str) -> (String, Option<String>) {
    match raw.split_once(" (") {
        Some((id, note)) => (
            id.trim().to_owned(),
            Some(note.trim_end_matches(')').to_owned()),
        ),
        None => (raw.trim().to_owned(), None),
    }
}

fn tier_prices(rows: Option<&Vec<Row>>) -> HashMap<String, (Option<f64>, Option<f64>)> {
    rows.into_iter()
        .flatten()
        .map(|r| (split_name(&r.name).0, (r.input, r.output)))
        .collect()
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let mut tables = tables(page)?;
    let standard = tables
        .remove("standard")
        .ok_or("the pricing page has no standard text-token table")?;
    let batch = tier_prices(tables.get("batch"));
    let fast = tier_prices(tables.get("fast"));

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for r in standard {
        let (id, note) = split_name(&r.name);
        // No token price stated for this row: nothing to list.
        let (Some(input), Some(output)) = (r.input, r.output) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let mut extra = serde_json::Map::new();
        if let Some(v) = r.cached {
            extra.insert("cached_input_per_mtok".into(), json!(round6(v)));
        }
        if let Some(v) = r.write {
            extra.insert("cache_write_per_mtok".into(), json!(round6(v)));
        }
        if let Some(n) = note {
            extra.insert("price_note".into(), json!(n));
        }
        if let Some((Some(i), Some(o))) = batch.get(&id) {
            extra.insert(
                "batch".into(),
                json!({"input_per_mtok": round6(*i), "output_per_mtok": round6(*o)}),
            );
        }
        if let Some((Some(i), Some(o))) = fast.get(&id) {
            extra.insert(
                "fast".into(),
                json!({"input_per_mtok": round6(*i), "output_per_mtok": round6(*o)}),
            );
        }
        out.push(ModelListing {
            external_id: id.clone(),
            name: id,
            maker: Some("openai".into()),
            context_window: None,
            input_per_mtok: round6(input),
            output_per_mtok: round6(output),
            variant: None,
            source_url: Some(URL.into()),
            extra: Value::Object(extra),
            media: None,
            capabilities: Vec::new(),
        });
    }
    if out.is_empty() {
        return Err("the standard table holds no priced models".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from the live page: the standard and batch islands, plus a row with no price.
    const FIXTURE: &str = r#"<div data-value="standard"><astro-island uid="Z2e4d4p" component-url="/_astro/pricing.js" component-export="TextTokenPricingTables" renderer-url="/_astro/client.js" props="{&quot;tier&quot;:[0,&quot;standard&quot;],&quot;rows&quot;:[1,[[1,[[0,&quot;gpt-5.5 (&lt;272K context length)&quot;],[0,5],[0,0.5],[0,&quot;-&quot;],[0,30]]],[1,[[0,&quot;gpt-5.4-mini&quot;],[0,0.75],[0,0.075],[0,4.5]]],[1,[[0,&quot;gpt-5-pro&quot;],[0,15],[0,null],[0,120]]],[1,[[0,&quot;gpt-realtime-x&quot;],[0,&quot;-&quot;],[0,null],[0,&quot;-&quot;]]],[1,[[0,&quot;gpt-4o-mini&quot;],[0,0.15],[0,0.075],[0,0.6]]]]]}" ssr client="load"></astro-island></div>
<div data-value="batch"><astro-island component-export="TextTokenPricingTables" renderer-url="/_astro/client.js" props="{&quot;tier&quot;:[0,&quot;batch&quot;],&quot;rows&quot;:[1,[[1,[[0,&quot;gpt-5.5 (&lt;272K context length)&quot;],[0,2.5],[0,0.25],[0,&quot;-&quot;],[0,15]]],[1,[[0,&quot;gpt-5.4-mini&quot;],[0,0.375],[0,0.0375],[0,2.25]]]]]}" ssr client="load"></astro-island></div>"#;

    fn by_id<'a>(l: &'a [ModelListing], id: &str) -> &'a ModelListing {
        l.iter().find(|m| m.external_id == id).unwrap()
    }

    #[test]
    fn reads_the_standard_table() {
        let l = parse(&[FIXTURE.to_owned()]).unwrap();
        assert_eq!(l.len(), 4, "the row without a price is skipped");
        assert!(l.iter().all(|m| m.external_id != "gpt-realtime-x"));
        let m = by_id(&l, "gpt-5.5");
        assert_eq!(m.maker.as_deref(), Some("openai"));
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (5.0, 30.0));
        assert_eq!(m.context_window, None);
        assert_eq!(m.extra["cached_input_per_mtok"], 0.5);
        assert_eq!(m.extra["price_note"], "<272K context length");
        assert_eq!(m.extra["batch"]["input_per_mtok"], 2.5);
        let m = by_id(&l, "gpt-5.4-mini");
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (0.75, 4.5));
        assert_eq!(m.extra["batch"]["output_per_mtok"], 2.25);
        let m = by_id(&l, "gpt-5-pro");
        assert_eq!((m.input_per_mtok, m.output_per_mtok), (15.0, 120.0));
        assert!(m.extra.get("cached_input_per_mtok").is_none());
        assert_eq!(by_id(&l, "gpt-4o-mini").source_url.as_deref(), Some(URL));
    }

    #[test]
    fn errors_when_the_structure_is_missing_or_a_price_is_text() {
        assert!(parse(&["<html></html>".to_owned()]).is_err());
        let bad = FIXTURE.replace("[0,0.75]", "[0,&quot;free&quot;]");
        assert!(parse(&[bad]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_OPENAI").expect("LAB_FIXTURE_OPENAI");
        let page = std::fs::read_to_string(path).unwrap();
        for m in parse(&[page]).unwrap() {
            println!(
                "{} in={} out={} extra={}",
                m.external_id, m.input_per_mtok, m.output_per_mtok, m.extra
            );
        }
    }
}
