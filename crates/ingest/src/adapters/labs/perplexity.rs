//! Perplexity: models and prices from the lab's public pricing pages.
//!
//! The Agent API models page lists, per provider tab, a table of
//! `Model | Input ($/1M) | Output ($/1M) | Cache read ($/1M) | Service tiers | Docs`. Ids are
//! written `provider/model`; they are kept as written, since the Agent API takes them so.
//! Third-party models resold by Perplexity are Perplexity's listings at Perplexity's rates.

use serde_json::json;

use super::text::{md_tables, plain};
use super::{LAB_INTERVAL, LabSpec};
use crate::adapters::{ModelListing, round6};

const PAGE: &str = "https://docs.perplexity.ai/docs/agent-api/models";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-perplexity",
    name: "Perplexity",
    organization: "perplexity",
    origin: "https://docs.perplexity.ai",
    paths: &["/docs/agent-api/models.md"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// Every plain decimal in a cell (`2.00 (≤272k) 4.00 (>272k)` -> `[2.0, 4.0]`); thresholds
/// such as `(≤272k)` are not numbers and are ignored.
fn numbers(cell: &str) -> Vec<f64> {
    cell.split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

/// The model's display name: the docs link text when it names this very model, else the
/// id's tail.
fn display_name(id: &str, docs: &str) -> String {
    let tail = id.split('/').next_back().unwrap_or(id);
    let squash = |s: &str| {
        s.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase()
    };
    let label = plain(docs);
    if !label.is_empty() && squash(&label) == squash(tail) {
        label
    } else {
        tail.to_owned()
    }
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let page = pages.first().ok_or("no page")?;
    // Tables sit inside blockquoted tabs (`> | ... |`): drop the quote markers.
    let unquoted = page
        .lines()
        .map(|l| l.trim_start().trim_start_matches('>').trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    let mut out: Vec<ModelListing> = Vec::new();
    for table in md_tables(&unquoted) {
        let head = &table[0];
        let col = |prefix: &str| head.iter().position(|h| h.starts_with(prefix));
        let (Some(m), Some(i), Some(o)) = (col("Model"), col("Input"), col("Output")) else {
            continue;
        };
        let (cache, docs) = (col("Cache read"), col("Docs"));
        for row in &table[1..] {
            let id = row.get(m).map_or("", |c| c.trim_matches('`').trim());
            let (Some((prefix, _)), Some(input), Some(output)) =
                (id.split_once('/'), row.get(i), row.get(o))
            else {
                return Err(format!("row {row:?} has no provider/model id and prices"));
            };
            let ins = numbers(input);
            let outs = numbers(output);
            let (Some(&input_price), Some(&output_price)) = (ins.first(), outs.first()) else {
                return Err(format!("{id}: prices {input:?} / {output:?} do not parse"));
            };
            let mut extra = serde_json::Map::new();
            if ins.len() > 1 || outs.len() > 1 {
                extra.insert("input_tiers_per_mtok".into(), json!(ins));
                extra.insert("output_tiers_per_mtok".into(), json!(outs));
                extra.insert("tiers_by_prompt_tokens".into(), json!(input));
            }
            if let Some(cell) = cache.and_then(|c| row.get(c)) {
                if let Some(&c) = numbers(cell).first() {
                    extra.insert("cached_input_per_mtok".into(), json!(round6(c)));
                } else if let Some(pct) = cell
                    .split('%')
                    .next()
                    .filter(|_| cell.contains("% off"))
                    .and_then(|p| p.trim().parse::<f64>().ok())
                {
                    extra.insert(
                        "cached_input_per_mtok".into(),
                        json!(round6(input_price * (1.0 - pct / 100.0))),
                    );
                }
            }
            if let Some(tiers) = row.get(col("Service tiers").unwrap_or(usize::MAX)) {
                let names: Vec<&str> = tiers
                    .split(',')
                    .map(|t| t.trim().trim_matches('`'))
                    .filter(|t| t.chars().all(|c| c.is_ascii_lowercase()) && !t.is_empty())
                    .collect();
                if !names.is_empty() {
                    extra.insert("service_tiers".into(), json!(names));
                }
            }
            if out.iter().any(|l| l.external_id == id) {
                continue;
            }
            let maker = if prefix == "xai" { "x-ai" } else { prefix };
            out.push(ModelListing {
                external_id: id.to_owned(),
                name: display_name(id, docs.and_then(|d| row.get(d)).map_or("", |s| s)),
                maker: Some(maker.to_owned()),
                context_window: None,
                input_per_mtok: round6(input_price),
                output_per_mtok: round6(output_price),
                variant: None,
                source_url: Some(PAGE.into()),
                extra: serde_json::Value::Object(extra),
                media: None,
                capabilities: Vec::new(),
            });
        }
    }
    if out.is_empty() {
        return Err("no model price table found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"# Agent API Models

> [tab] Anthropic
>
> | Model | Input (\$/1M) | Output (\$/1M) | Cache read (\$/1M) | Service tiers | Docs |
> | - | - | - | - | - | - |
> | `anthropic/claude-sonnet-5-5` | 2.00 | 10.00 | 0.10 | — | [Claude Sonnet 5.5](https://www.anthropic.com/claude-sonnet-5-5) |
> | `anthropic/claude-haiku-5-5` | 0.10 (≤100k) 0.50 (>100k) | 0.50 (≤100k) 2.50 (>100k) | 0.01 (≤100k) 0.05 (>100k) | — | [Claude Haiku 5.5](https://platform.claude.com/docs/en/about-claude/models/overview) |

> [tab] OpenAI
>
> | Model | Input (\$/1M) | Output (\$/1M) | Cache read (\$/1M) | Service tiers | Docs |
> | - | - | - | - | - | - |
> | `openai/gpt-6.1-sol` | 2.00 (≤272k) 4.00 (>272k) | 10.00 (≤272k) 15.00 (>272k) | 95% off input | `flex`, `priority` | [GPT-6.1 Sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol) |

> [tab] xAI
>
> | Model | Input (\$/1M) | Output (\$/1M) | Cache read (\$/1M) | Service tiers | Docs |
> | - | - | - | - | - | - |
> | `xai/grok-4.7` | 2.00 (\<200k) 4.00 (≥200k) | 6.00 (\<200k) 12.00 (≥200k) | 0.50 (\<200k) 1.00 (≥200k) | — | [Grok 4.7](https://docs.x.ai/developers/models/grok-4.7) |

> [tab] Perplexity
>
> | Model | Input (\$/1M) | Output (\$/1M) | Cache read (\$/1M) | Service tiers | Docs |
> | - | - | - | - | - | - |
> | `perplexity/sonar` | 0.25 | 2.50 | 0.0625 | — | — |

## Search API

| Endpoint | Price per 1K requests |
| - | - |
| `POST /search` | 5.00 |
"#;

    fn by_id<'a>(l: &'a [ModelListing], id: &str) -> &'a ModelListing {
        l.iter().find(|m| m.external_id == id).unwrap()
    }

    #[test]
    fn reads_resold_and_own_models() {
        let l = parse(&[FIXTURE.into()]).unwrap();
        assert_eq!(l.len(), 5, "{l:?}");
        let s = by_id(&l, "anthropic/claude-sonnet-5-5");
        assert_eq!(s.name, "Claude Sonnet 5.5");
        assert_eq!(s.maker.as_deref(), Some("anthropic"));
        assert_eq!((s.input_per_mtok, s.output_per_mtok), (2.0, 10.0));
        assert_eq!(s.extra["cached_input_per_mtok"], 0.1);
        assert_eq!(s.context_window, None);
        assert_eq!(s.source_url.as_deref(), Some(PAGE));

        let h = by_id(&l, "anthropic/claude-haiku-5-5");
        assert_eq!((h.input_per_mtok, h.output_per_mtok), (0.1, 0.5));
        assert_eq!(h.extra["output_tiers_per_mtok"][1], 2.5);

        let g = by_id(&l, "openai/gpt-6.1-sol");
        assert_eq!((g.input_per_mtok, g.output_per_mtok), (2.0, 10.0));
        assert_eq!(g.extra["cached_input_per_mtok"], 0.1);
        assert_eq!(g.extra["service_tiers"][0], "flex");

        let x = by_id(&l, "xai/grok-4.7");
        assert_eq!(x.maker.as_deref(), Some("x-ai"));
        assert_eq!((x.input_per_mtok, x.output_per_mtok), (2.0, 6.0));

        let p = by_id(&l, "perplexity/sonar");
        assert_eq!(p.maker.as_deref(), Some("perplexity"));
        assert_eq!(p.name, "sonar");
        assert_eq!((p.input_per_mtok, p.output_per_mtok), (0.25, 2.5));
    }

    #[test]
    fn a_page_that_changed_is_an_error() {
        assert!(parse(&["# moved".into()]).is_err());
        let bad = FIXTURE.replace("| 2.00 | 10.00 |", "| tbd | 10.00 |");
        assert!(parse(&[bad]).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_PERPLEXITY").expect("LAB_FIXTURE_PERPLEXITY");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page]).unwrap() {
            println!("{l:?}");
        }
    }
}
