//! AI21: models and prices from the lab's public pricing page.
//!
//! The page shows, per model, a name, a blurb, then `$x / 1M input tokens` and
//! `$y / 1M output tokens` lines.

use super::text::{first_dollar, html_lines};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const SOURCE: &str = "https://www.ai21.com/pricing";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-ai21",
    name: "AI21",
    organization: "ai21-labs",
    origin: "https://www.ai21.com",
    paths: &["/pricing"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let mut out: Vec<ModelListing> = Vec::new();
    for i in 2..lines.len().saturating_sub(1) {
        if !lines[i].contains("/ 1M input tokens") {
            continue;
        }
        // Name, blurb, input price, output price.
        let name = lines[i - 2].trim();
        if !name.starts_with("Jamba") {
            continue;
        }
        if !lines[i + 1].contains("/ 1M output tokens") {
            return Err(format!("{name}: no output price after the input price"));
        }
        let (Some(input), Some(output)) = (first_dollar(&lines[i]), first_dollar(&lines[i + 1]))
        else {
            return Err(format!("{name}: price will not parse"));
        };
        let id = name.to_ascii_lowercase().replace(' ', "-");
        if out.iter().any(|l| l.external_id == id) {
            continue;
        }
        out.push(ModelListing {
            external_id: id,
            name: name.to_owned(),
            maker: Some("ai21".into()),
            context_window: None,
            input_per_mtok: round6(input),
            output_per_mtok: round6(output),
            variant: None,
            source_url: Some(SOURCE.to_owned()),
            extra: serde_json::Value::Null,
            media: None,
            capabilities: Vec::new(),
        });
    }
    if out.is_empty() {
        return Err("no Jamba prices found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<section><h2>Pay As You Go</h2>
<div><div>Foundation Models</div><div>Instruct, Chat</div><a>Documentation</a></div>
<div class="card"><h3>Jamba Mini</h3><p>Efficient &amp; lightweight model for a wide range of tasks</p>
<div><span>$0.2 / 1M input tokens</span></div><div><span>$0.4 / 1M output tokens</span></div></div>
<div class="card"><h3>Jamba Large</h3><p>The most powerful and efficient long context model</p>
<div><span>$2 / 1M input tokens</span></div><div><span>$8 / 1M output tokens</span></div></div>
<div class="card"><h3>Maestro</h3><p>Custom agents</p>
<div><span>$5 / 1M input tokens</span></div><div><span>$9 / 1M output tokens</span></div></div>
<h3>What is a Token?</h3></section>"#;

    #[test]
    fn parses_jamba_and_skips_other_products() {
        let l = parse(&[FIXTURE.to_owned()]).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].external_id, "jamba-mini");
        assert_eq!(l[0].name, "Jamba Mini");
        assert_eq!(l[0].maker.as_deref(), Some("ai21"));
        assert_eq!((l[0].input_per_mtok, l[0].output_per_mtok), (0.2, 0.4));
        assert_eq!(l[0].context_window, None);
        assert_eq!(l[1].external_id, "jamba-large");
        assert_eq!((l[1].input_per_mtok, l[1].output_per_mtok), (2.0, 8.0));
    }

    #[test]
    fn errors_when_structure_is_gone() {
        assert!(parse(&["<p>nothing</p>".to_owned()]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_AI21").expect("LAB_FIXTURE_AI21");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page]).unwrap() {
            println!("{l:?}");
        }
    }
}
