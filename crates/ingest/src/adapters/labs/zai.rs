//! Z.ai: models and prices from the lab's public pricing pages.
//!
//! The pricing page is server-rendered: one HTML table per model family with the columns
//! `Model | Input | Cached Input | Cached Input Storage | Output`, in USD per million tokens.
//! `Free` is a stated price of zero. Tables without both an input and an output column
//! (image, video, audio, tools, agents) are not token-priced and are skipped.

use serde_json::json;

use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const PAGE: &str = "https://docs.z.ai/guides/overview/pricing";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-zai",
    name: "Z.ai",
    organization: "z-ai",
    origin: "https://docs.z.ai",
    paths: &["/guides/overview/pricing"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// Every `<table>` of the page as rows of cell text.
fn html_tables(html: &str) -> Vec<Vec<Vec<String>>> {
    let mut tables = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<table") {
        let Some(end) = rest[start..].find("</table>") else {
            break;
        };
        let table = &rest[start..start + end];
        let rows = table
            .split("<tr")
            .skip(1)
            .map(|row| {
                row.split("</tr>")
                    .next()
                    .unwrap_or("")
                    .split("</t")
                    .filter_map(|cell| {
                        let open = cell.rfind("<td").or_else(|| cell.rfind("<th"))?;
                        let body = &cell[open..];
                        let body = &body[body.find('>')? + 1..];
                        Some(super::text::html_lines(body).join(" "))
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|r| !r.is_empty())
            .collect();
        tables.push(rows);
        rest = &rest[start + end..];
    }
    tables
}

/// `$1.4` -> 1.4, `Free` -> 0. Anything else is an error.
fn price(cell: &str, model: &str) -> Result<f64, String> {
    let c = cell.trim();
    if c.eq_ignore_ascii_case("free") {
        return Ok(0.0);
    }
    super::text::first_dollar(c).ok_or_else(|| format!("{model}: price {c:?} does not parse"))
}

/// A cached-input price when the cell states one (`-` and `\` mean none).
fn optional_price(cell: Option<&String>) -> Option<f64> {
    let c = cell?.trim();
    if c.eq_ignore_ascii_case("free") {
        return Some(0.0);
    }
    super::text::first_dollar(c)
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let mut out: Vec<ModelListing> = Vec::new();
    for table in html_tables(page) {
        let Some(head) = table.first() else { continue };
        let col = |name: &str| head.iter().position(|h| h.eq_ignore_ascii_case(name));
        let (Some(m), Some(i), Some(o)) = (col("Model"), col("Input"), col("Output")) else {
            continue;
        };
        let cached = col("Cached Input");
        for row in &table[1..] {
            let (Some(model), Some(input), Some(output)) = (row.get(m), row.get(i), row.get(o))
            else {
                continue;
            };
            let id = model.trim().to_ascii_lowercase();
            if id.is_empty() || out.iter().any(|l| l.external_id == id) {
                continue;
            }
            let mut extra = serde_json::Map::new();
            if let Some(c) = cached.and_then(|c| optional_price(row.get(c))) {
                extra.insert("cached_input_per_mtok".into(), json!(round6(c)));
            }
            out.push(ModelListing {
                external_id: id,
                name: model.trim().to_owned(),
                maker: Some("z-ai".into()),
                context_window: None,
                input_per_mtok: round6(price(input, model)?),
                output_per_mtok: round6(price(output, model)?),
                variant: None,
                source_url: Some(PAGE.into()),
                extra: serde_json::Value::Object(extra),
                media: None,
                capabilities: Vec::new(),
            });
        }
    }
    if out.is_empty() {
        return Err("no Model/Input/Output price table found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<h3 id="latest-models">Latest Models</h3>
<table class="m-0"><thead><tr><th style="text-align:left">Model</th><th>Input</th><th>Cached Input</th><th>Cached Input Storage</th><th>Output</th></tr></thead><tbody>
<tr><td style="text-align:left">GLM-5.3</td><td data-numeric="true">$1.4</td><td data-numeric="true">$0.26</td><td>Limited-time Free</td><td data-numeric="true">$4.4</td></tr>
<tr><td>GLM-5.3-Flash</td><td>$0.15</td><td>$0.03</td><td>Limited-time Free</td><td>$0.50</td></tr></tbody></table>
<h3>Text Models</h3>
<table><thead><tr><th>Model</th><th>Input</th><th>Cached Input</th><th>Cached Input Storage</th><th>Output</th></tr></thead><tbody>
<tr><td>GLM-4-32B-0414-128K</td><td>$0.1</td><td>-</td><td>-</td><td>$0.1</td></tr>
<tr><td>GLM-4.7-Flash</td><td>Free</td><td>Free</td><td>Free</td><td>Free</td></tr></tbody></table>
<h3>Image Generation Models</h3>
<table><thead><tr><th>Model</th><th>Price</th></tr></thead><tbody><tr><td>GLM-Image</td><td>$0.015</td></tr></tbody></table>
<h3>Agents</h3>
<table><thead><tr><th>Agent</th><th>Price</th></tr></thead><tbody><tr><td>General-Purpose Translation</td><td>$3 / MTok</td></tr></tbody></table>"#;

    #[test]
    fn reads_token_priced_models_and_skips_the_rest() {
        let l = parse(&[FIXTURE.into()]).unwrap();
        let ids: Vec<_> = l.iter().map(|x| x.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "glm-5.3",
                "glm-5.3-flash",
                "glm-4-32b-0414-128k",
                "glm-4.7-flash"
            ]
        );
        assert_eq!(l[0].name, "GLM-5.3");
        assert_eq!(l[0].maker.as_deref(), Some("z-ai"));
        assert_eq!((l[0].input_per_mtok, l[0].output_per_mtok), (1.4, 4.4));
        assert_eq!(l[0].extra["cached_input_per_mtok"], 0.26);
        assert_eq!(l[0].context_window, None);
        assert_eq!((l[1].input_per_mtok, l[1].output_per_mtok), (0.15, 0.5));
        assert_eq!((l[2].input_per_mtok, l[2].output_per_mtok), (0.1, 0.1));
        assert!(l[2].extra.get("cached_input_per_mtok").is_none());
        assert_eq!((l[3].input_per_mtok, l[3].output_per_mtok), (0.0, 0.0));
        assert_eq!(l[0].source_url.as_deref(), Some(PAGE));
    }

    #[test]
    fn a_page_without_price_tables_is_an_error() {
        assert!(parse(&["<p>moved</p>".into()]).is_err());
        let bad = FIXTURE.replace("$1.4", "call us");
        assert!(parse(&[bad]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_ZAI").expect("LAB_FIXTURE_ZAI");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page]).unwrap() {
            println!("{l:?}");
        }
    }
}
