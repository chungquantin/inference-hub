//! Voyage AI: embedding prices from the lab's public pricing page.
//!
//! The text-embedding tables list model ids, a price per thousand tokens and a price per
//! million tokens; one price cell can cover several ids. Rerankers and the multimodal
//! endpoint (priced per pixel) have no unit in our schema and are skipped.

use serde_json::Value;

use super::text::{dollars, html_lines};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{MediaKind, ModelListing, round6};

const SOURCE: &str = "https://docs.voyageai.com/docs/pricing";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-voyage",
    name: "Voyage AI",
    organization: "voyage-ai",
    origin: "https://docs.voyageai.com",
    paths: &["/docs/pricing"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

fn is_model_id(line: &str) -> bool {
    (line.starts_with("voyage-") || line.starts_with("rerank"))
        && line
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '.'))
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let mut out: Vec<ModelListing> = Vec::new();
    let mut active = false;
    let mut seen_section = false;
    let mut pending: Vec<&str> = Vec::new();
    let mut per_k: Option<f64> = None;
    for line in &lines {
        match line.as_str() {
            "Text Embeddings" | "Older models" => {
                active = true;
                seen_section = true;
                pending.clear();
                per_k = None;
                continue;
            }
            "Multimodal Embeddings" | "Rerankers" | "Batch and Files API" => {
                active = false;
                continue;
            }
            _ => {}
        }
        if !active {
            continue;
        }
        if is_model_id(line) {
            pending.push(line);
        } else if line.starts_with('$') && !pending.is_empty() {
            let Some(price) = dollars(line).first().copied() else {
                return Err(format!("price will not parse: {line:?}"));
            };
            let Some(k) = per_k.take() else {
                per_k = Some(price);
                continue;
            };
            if (k * 1000.0 - price).abs() > 1e-6 {
                return Err(format!(
                    "{}: per-thousand ${k} and per-million ${price} disagree",
                    pending.join(", ")
                ));
            }
            for id in pending.drain(..) {
                if id.starts_with("rerank") || out.iter().any(|l| l.external_id == id) {
                    continue;
                }
                out.push(ModelListing::media(
                    id.to_owned(),
                    id.to_owned(),
                    Some("voyage".into()),
                    MediaKind::Embedding,
                    round6(price),
                    Some(SOURCE.to_owned()),
                    Value::Null,
                ));
            }
        }
    }
    if !seen_section {
        return Err("no embedding price section found".into());
    }
    if out.is_empty() {
        return Err("no embedding prices found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<h2>Text Embeddings</h2><p>We charge for requests to the Voyage text embedding endpoint.</p>
<table><thead><tr><th>Model</th><th>Price per thousand tokens</th><th>Price per million tokens</th><th>Number of free tokens</th></tr></thead><tbody>
<tr><td><code>voyage-4-large</code></td><td>$0.00012</td><td>$0.12</td><td>200 million</td></tr>
<tr><td><code>voyage-4-lite</code></td><td>$0.00002</td><td>$0.02</td><td>200 million</td></tr>
<tr><td><code>voyage-finance-2</code><br/><code>voyage-law-2</code></td><td>$0.00012</td><td>$0.12</td><td>50 million</td></tr></tbody></table>
<h2>Multimodal Embeddings</h2>
<table><tbody><tr><td><code>voyage-multimodal-3.5</code></td><td>$0.12</td><td>$0.60</td></tr></tbody></table>
<h2>Rerankers</h2>
<table><tbody><tr><td><code>rerank-3</code></td><td>$0.00005</td><td>$0.05</td><td>$0.0025</td></tr></tbody></table>
<h2>Batch and Files API</h2><p>Storage is priced at $0.05 per GB per month.</p>
<h2>Older models</h2>
<table><tbody><tr><td><code>voyage-3</code></td><td>$0.00006</td><td>$0.06</td></tr>
<tr><td><code>rerank-2</code></td><td>$0.00005</td><td>$0.05</td></tr></tbody></table>"#;

    #[test]
    fn parses_embeddings_and_skips_rerankers_and_multimodal() {
        let l = parse(&[FIXTURE.to_owned()]).unwrap();
        let ids: Vec<_> = l.iter().map(|m| m.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "voyage-4-large",
                "voyage-4-lite",
                "voyage-finance-2",
                "voyage-law-2",
                "voyage-3"
            ]
        );
        assert_eq!(l[0].maker.as_deref(), Some("voyage"));
        let m = l[0].media.unwrap();
        assert_eq!(m.kind, MediaKind::Embedding);
        assert_eq!(m.price, 0.12);
        assert_eq!(l[1].media.unwrap().price, 0.02);
        assert_eq!(l[3].media.unwrap().price, 0.12);
        assert_eq!(l[4].media.unwrap().price, 0.06);
        assert_eq!(l[0].context_window, None);
    }

    #[test]
    fn errors_when_prices_disagree_or_structure_is_gone() {
        assert!(parse(&["<p>nothing</p>".to_owned()]).is_err());
        let bad = "<h2>Text Embeddings</h2><p>voyage-4</p><p>$0.00006</p><p>$0.6</p>";
        assert!(parse(&[bad.to_owned()]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_VOYAGE").expect("LAB_FIXTURE_VOYAGE");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page]).unwrap() {
            println!("{l:?}");
        }
    }
}
