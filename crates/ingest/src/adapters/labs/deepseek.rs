//! DeepSeek: models and prices from the lab's public pricing pages.
//!
//! The pricing page is one server-rendered table with a column per model. Prices come as
//! peak and off-peak rows; the peak (list) price is the price and the off-peak price goes in
//! `extra`.

use serde_json::json;

use super::{LAB_INTERVAL, LabSpec, text};
use crate::adapters::{ModelListing, round6};

const SOURCE: &str = "https://api-docs.deepseek.com/quick_start/pricing";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-deepseek",
    name: "DeepSeek",
    organization: "deepseek",
    origin: "https://api-docs.deepseek.com",
    paths: &["/quick_start/pricing"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// Prices of one model column: peak (the list price) and off-peak.
#[derive(Default, Clone, Copy)]
struct Prices {
    hit: Option<f64>,
    miss: Option<f64>,
    output: Option<f64>,
    off_hit: Option<f64>,
    off_miss: Option<f64>,
    off_output: Option<f64>,
}

fn price(cell: &str) -> Result<f64, String> {
    text::first_dollar(cell).ok_or_else(|| format!("deepseek: price {cell:?} does not parse"))
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let html = pages.first().ok_or("deepseek: no page")?;
    let rows = tables(html)
        .into_iter()
        .find(|t| {
            t.iter().any(|r| r[0] == "MODEL") && t.iter().any(|r| r[0].starts_with("PRICING"))
        })
        .ok_or("deepseek: the model and pricing table is not on the page")?;

    let head = rows
        .iter()
        .find(|r| r[0] == "MODEL")
        .ok_or("deepseek: no MODEL row")?;
    // Leading label columns: MODEL spans several.
    let label = head.iter().take_while(|c| *c == "MODEL").count();
    let n = head.len() - label;
    if n == 0 {
        return Err("deepseek: the MODEL row names no models".into());
    }
    let ids: Vec<String> = head[label..]
        .iter()
        .map(|c| c.split_whitespace().next().unwrap_or("").to_owned())
        .collect();
    if ids.iter().any(|id| id.is_empty() || id.contains('$')) {
        return Err("deepseek: a model column has no id".into());
    }

    let cells = |label_text: &str| -> Option<&[String]> {
        rows.iter()
            .find(|r| r[0] == label_text && r.len() == label + n)
            .map(|r| &r[label..])
    };
    let versions = cells("MODEL VERSION");
    let context = cells("CONTEXT LENGTH");
    let max_output = cells("MAX OUTPUT");

    let mut prices = vec![Prices::default(); n];
    for r in rows.iter().filter(|r| r.len() == label + n) {
        let section = r[1].to_ascii_uppercase();
        let off_peak = r[label - 1].eq_ignore_ascii_case("OFF-PEAK");
        let slot = if section.contains("CACHE HIT") {
            0
        } else if section.contains("CACHE MISS") {
            1
        } else if section.contains("OUTPUT") {
            2
        } else {
            continue;
        };
        if !r[label..].iter().all(|c| c.contains('$')) {
            continue;
        }
        for (k, cell) in r[label..].iter().enumerate() {
            let v = round6(price(cell)?);
            let p = &mut prices[k];
            match (slot, off_peak) {
                (0, false) => p.hit = Some(v),
                (0, true) => p.off_hit = Some(v),
                (1, false) => p.miss = Some(v),
                (1, true) => p.off_miss = Some(v),
                (_, false) => p.output = Some(v),
                (_, true) => p.off_output = Some(v),
            }
        }
    }

    let mut out = Vec::new();
    for (k, id) in ids.iter().enumerate() {
        let p = prices[k];
        let (Some(input), Some(output)) = (p.miss, p.output) else {
            return Err(format!("deepseek: {id} has no input or output price"));
        };
        let name = versions
            .and_then(|v| v.get(k))
            .map_or_else(|| id.clone(), |v| display_name(v));
        let context_window = context
            .and_then(|c| c.get(k))
            .and_then(|c| count(c))
            .and_then(|c| i32::try_from(c).ok());
        let mut extra = json!({});
        let map = extra.as_object_mut().expect("object");
        if let Some(v) = p.hit {
            map.insert("cached_input_per_mtok".into(), json!(v));
        }
        let off = json!({
            "input_per_mtok": p.off_miss,
            "output_per_mtok": p.off_output,
            "cached_input_per_mtok": p.off_hit,
        });
        if p.off_miss.is_some() || p.off_output.is_some() {
            map.insert("off_peak".into(), off);
        }
        if let Some(m) = max_output.and_then(|m| m.get(k)).and_then(|m| count(m)) {
            map.insert("max_output_tokens".into(), json!(m));
        }
        out.push(ModelListing {
            external_id: id.clone(),
            name,
            maker: Some("deepseek".into()),
            context_window,
            input_per_mtok: input,
            output_per_mtok: output,
            variant: None,
            source_url: Some(SOURCE.into()),
            extra,
            media: None,
            capabilities: Vec::new(),
        });
    }
    Ok(out)
}

/// `DeepSeek-V4.1-Flash-1017` -> `DeepSeek V4.1 Flash`: the snapshot suffix is dropped.
fn display_name(version: &str) -> String {
    let v = version.split_whitespace().next().unwrap_or(version);
    let v = match v.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 4 && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => v,
    };
    v.replace('-', " ")
}

/// Rows of an HTML table with spans expanded so every row has one cell per column (a cell
/// spanning rows or columns is repeated).
fn span(tag: &str, name: &str) -> usize {
    tag.find(&format!("{name}=\""))
        .map(|p| &tag[p + name.len() + 2..])
        .and_then(|r| r.split('"').next())
        .and_then(|n| n.trim().parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 64)
}

fn next_cell(row: &str, from: usize) -> Option<usize> {
    let mut i = from;
    while let Some(p) = row[i..].find("<t") {
        let at = i + p;
        let rest = &row[at + 2..];
        if (rest.starts_with('d') || rest.starts_with('h'))
            && rest[1..].starts_with(|c: char| c == '>' || c.is_whitespace())
        {
            return Some(at);
        }
        i = at + 2;
    }
    None
}

fn grid(html: &str, lower: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut pending: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while let Some(r) = lower[i..].find("<tr") {
        let start = i + r;
        let end = lower[start..]
            .find("</tr>")
            .map_or(lower.len(), |e| start + e);
        i = end.max(start + 3);
        let (row_html, row_lower) = (&html[start..end], &lower[start..end]);
        let mut row: Vec<String> = Vec::new();
        let (mut col, mut j) = (0, 0);
        loop {
            while pending.get(col).is_some_and(|p| p.0 > 0) {
                row.push(pending[col].1.clone());
                pending[col].0 -= 1;
                col += 1;
            }
            let Some(cs) = next_cell(row_lower, j) else {
                break;
            };
            let tag_end = row_lower[cs..]
                .find('>')
                .map_or(row_lower.len(), |e| cs + e + 1);
            let tag = &row_lower[cs..tag_end];
            let cell_end = ["</td>", "</th>"]
                .iter()
                .filter_map(|c| row_lower[tag_end..].find(c))
                .min()
                .map_or(row_lower.len(), |e| tag_end + e);
            let text = text::html_lines(&row_html[tag_end..cell_end]).join("\n");
            let (rowspan, colspan) = (span(tag, "rowspan"), span(tag, "colspan"));
            for _ in 0..colspan {
                if pending.len() <= col {
                    pending.resize(col + 1, (0, String::new()));
                }
                pending[col] = (rowspan - 1, text.clone());
                row.push(text.clone());
                col += 1;
            }
            j = cell_end;
        }
        while pending.get(col).is_some_and(|p| p.0 > 0) {
            row.push(pending[col].1.clone());
            pending[col].0 -= 1;
            col += 1;
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    rows
}

fn tables(html: &str) -> Vec<Vec<Vec<String>>> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = lower[i..].find("<table") {
        let p = i + p;
        let end = lower[p..].find("</table>").map_or(html.len(), |e| p + e);
        out.push(grid(&html[p..end], &lower[p..end]));
        i = end.max(p + 1);
    }
    out
}

/// A count such as `1M`, `256K`, `128k tokens` or `8,192`, as a number.
fn count(s: &str) -> Option<i64> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let rest = &s[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.'))
        .unwrap_or(rest.len());
    let n: f64 = rest[..end].replace(',', "").parse().ok()?;
    let mult = match rest[end..].chars().next() {
        Some('K' | 'k') => 1e3,
        Some('M' | 'm') => 1e6,
        _ => 1.0,
    };
    Some((n * mult).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The model and pricing table of the live page, and the search table below it (which
    /// prices requests, not tokens, and must be ignored).
    const PAGE: &str = r##"<table style="text-align:center"><tr><td colspan="3" style="text-align:center">MODEL</td><td>deepseek-flash<sup>(1)</sup></td><td>deepseek-v4-pro</td></tr><tr><td colspan="3">BASE URL (OpenAI Format)</td><td colspan="2"><a href="https://api.deepseek.com" target="_blank" rel="noopener noreferrer">https://api.deepseek.com</a></td></tr><tr><td colspan="3">BASE URL (Anthropic Format)</td><td colspan="2"><a href="https://api.deepseek.com/anthropic" target="_blank" rel="noopener noreferrer">https://api.deepseek.com/anthropic</a></td></tr><tr><td colspan="3" style="text-align:center">MODEL VERSION</td><td>DeepSeek-V4.1-Flash-1017</td><td>DeepSeek-V4-Pro-0813</td></tr><tr><td colspan="3">THINKING MODE</td><td colspan="2">Supports both non-thinking and thinking (default) modes<br>See <a href="/guides/thinking_mode">Thinking Mode</a> for how to switch</td></tr><tr><td colspan="3">CONTEXT LENGTH</td><td colspan="2">1M</td></tr><tr><td colspan="3">MAX OUTPUT</td><td colspan="2">MAXIMUM: 384K</td></tr><tr><td rowspan="7">FEATURES</td><td colspan="2"><a href="/guides/json_mode">Json Output</a></td><td>✓</td><td>✓</td></tr><tr><td colspan="2"><a href="/guides/tool_calls">Tool Calls</a></td><td>✓</td><td>✓</td></tr><tr><td colspan="2"><a href="/guides/responses_api">Responses API</a></td><td>✓</td><td>✓</td></tr><tr><td colspan="2"><a href="/guides/anthropic_api">Anthropic API</a></td><td>✓</td><td>✓</td></tr><tr><td colspan="2"><a href="/guides/chat_prefix_completion">Chat Prefix Completion（Beta）</a></td><td>✓</td><td>✓</td></tr><tr><td colspan="2"><a href="/guides/fim_completion">FIM Completion（Beta）</a></td><td>Non-thinking mode only</td><td>Non-thinking mode only</td></tr><tr><td colspan="2"><a href="/guides/vision">Vision</a></td><td>✓</td><td>Not supported</td></tr><tr><td rowspan="6">PRICING<sup>(2)</sup></td><td rowspan="2">1M INPUT TOKENS<br>(CACHE HIT)</td><td>OFF-PEAK</td><td>$0.003</td><td>$0.022</td></tr><tr><td>PEAK</td><td>$0.006</td><td>$0.044</td></tr><tr><td rowspan="2">1M INPUT TOKENS<br>(CACHE MISS)</td><td>OFF-PEAK</td><td>$0.15</td><td>$0.66</td></tr><tr><td>PEAK</td><td>$0.3</td><td>$1.32</td></tr><tr><td rowspan="2">1M OUTPUT TOKENS</td><td>OFF-PEAK</td><td>$0.6</td><td>$1.98</td></tr><tr><td>PEAK</td><td>$1.2</td><td>$3.96</td></tr><tr><td colspan="3">Concurrency Limit<sup>(3)</sup></td><td>2500</td><td>500</td></tr></table>
<table><thead><tr><th>API</th><th>Unit</th><th>Price</th></tr></thead><tbody><tr><td>Search</td><td>Per 1,000 successful requests</td><td>$0.75</td></tr><tr><td>Fetch</td><td>Per 1,000 successful requests</td><td>$0.75</td></tr></tbody></table>"##;

    #[test]
    fn parses_peak_prices_and_keeps_off_peak_in_extra() {
        let m = parse(&[PAGE.to_owned()]).unwrap();
        assert_eq!(m.len(), 2);
        let flash = &m[0];
        assert_eq!(flash.external_id, "deepseek-flash");
        assert_eq!(flash.name, "DeepSeek V4.1 Flash");
        assert_eq!(flash.maker.as_deref(), Some("deepseek"));
        assert_eq!(flash.input_per_mtok, 0.3);
        assert_eq!(flash.output_per_mtok, 1.2);
        assert_eq!(flash.context_window, Some(1_000_000));
        assert_eq!(flash.extra["cached_input_per_mtok"], 0.006);
        assert_eq!(flash.extra["off_peak"]["input_per_mtok"], 0.15);
        assert_eq!(flash.extra["off_peak"]["output_per_mtok"], 0.6);
        assert_eq!(flash.extra["max_output_tokens"], 384_000);
        let pro = &m[1];
        assert_eq!(pro.external_id, "deepseek-v4-pro");
        assert_eq!(pro.input_per_mtok, 1.32);
        assert_eq!(pro.output_per_mtok, 3.96);
        assert_eq!(pro.extra["off_peak"]["cached_input_per_mtok"], 0.022);
        assert!(m.iter().all(|x| x.source_url.as_deref() == Some(SOURCE)));
    }

    #[test]
    fn a_page_without_the_table_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()]).is_err());
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn an_unparsable_price_is_an_error() {
        let broken = PAGE.replace("$1.32", "soon");
        assert!(parse(&[broken]).is_err());
    }

    /// `LAB_FIXTURE_DEEPSEEK=/path/to/page.html cargo test -p ih-ingest labs::deepseek -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_DEEPSEEK").expect("LAB_FIXTURE_DEEPSEEK");
        let html = std::fs::read_to_string(path).unwrap();
        for m in parse(&[html]).unwrap() {
            println!("{m:#?}");
        }
    }
}
