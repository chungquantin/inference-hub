//! Writer: models and prices from the lab's public pricing pages.
//!
//! The models page holds one comparison table with a column per Palmyra model: API model id,
//! input and output price per 1M tokens, context window.

use serde_json::json;

use super::{LAB_INTERVAL, LabSpec, text};
use crate::adapters::{ModelListing, round6};

const SOURCE: &str = "https://dev.writer.com/home/models";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-writer",
    name: "Writer",
    organization: "writer",
    origin: "https://dev.writer.com",
    paths: &["/home/models"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let html = pages.first().ok_or("writer: no page")?;
    let rows = tables(html)
        .into_iter()
        .find(|t| t.iter().any(|r| r[0] == "API model ID"))
        .ok_or("writer: the model comparison table is not on the page")?;
    let row = |label: &str| rows.iter().find(|r| r[0].starts_with(label));
    let header = &rows[0];
    let ids = row("API model ID").ok_or("writer: no API model ID row")?;
    let input = row("Pricing (Input)").ok_or("writer: no input price row")?;
    let output = row("Pricing (Output)").ok_or("writer: no output price row")?;
    let context = row("Context window");
    let max_output = row("Max output");

    let mut out = Vec::new();
    for (k, id) in ids.iter().enumerate().skip(1) {
        let id = id.trim();
        if id.is_empty() || id.contains(char::is_whitespace) {
            continue;
        }
        let cell = |r: &Vec<String>| r.get(k).cloned().unwrap_or_default();
        let prices = (
            text::first_dollar(&cell(input)),
            text::first_dollar(&cell(output)),
        );
        let (inp, outp) = match prices {
            (Some(i), Some(o)) => (round6(i), round6(o)),
            (None, None) => continue,
            _ => return Err(format!("writer: {id} has only one of its two prices")),
        };
        let mut extra = json!({});
        if let Some(m) = max_output.and_then(|r| count(&cell(r))) {
            extra["max_output_tokens"] = json!(m);
        }
        let name = header.get(k).filter(|n| !n.is_empty());
        out.push(ModelListing {
            external_id: id.to_owned(),
            name: name.cloned().unwrap_or_else(|| id.to_owned()),
            maker: Some("writer".into()),
            context_window: context
                .and_then(|r| count(&cell(r)))
                .and_then(|c| i32::try_from(c).ok()),
            input_per_mtok: inp,
            output_per_mtok: outp,
            variant: None,
            source_url: Some(SOURCE.into()),
            extra,
            media: None,
            capabilities: Vec::new(),
        });
    }
    if out.is_empty() {
        return Err("writer: the table lists no priced models".into());
    }
    Ok(out)
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

    /// The comparison table of the live page, with a fourth column added for a model that
    /// states no price (the page writes a dash for values that do not apply).
    const PAGE: &str = r##"<table><thead>
<tr><th>Feature</th><th>Palmyra X6</th><th>Palmyra X5</th><th>Palmyra X4</th><th>Palmyra Fin</th></tr></thead><tbody>
<tr><td><strong>Description</strong></td><td>Agentic model for WRITER Agent and long-horizon enterprise workflows with sub-agents, grounding, and MCP tool use</td><td>General-purpose model with a 1M token context window and adaptive reasoning</td><td>Language model for complex tasks that require extended context</td><td>Domain model</td></tr>
<tr><td><strong>API model ID</strong></td><td><code>palmyra-x6</code></td><td><code>palmyra-x5</code></td><td><code>palmyra-x4</code></td><td><code>palmyra-fin</code></td></tr>
<tr><td><strong>Bedrock model ID</strong></td><td>—</td><td><code>writer.palmyra-x5-v1:0</code></td><td><code>writer.palmyra-x4-v1:0</code></td><td>—</td></tr>
<tr><td><strong>Pricing (Input)</strong></td><td>$2.00 / 1M tokens</td><td>$0.60 / 1M tokens</td><td>$2.50 / 1M tokens</td><td>—</td></tr>
<tr><td><strong>Pricing (Output)</strong></td><td>$8.00 / 1M tokens</td><td>$6.00 / 1M tokens</td><td>$10.00 / 1M tokens</td><td>—</td></tr>
<tr><td><strong>Context window</strong></td><td>1M tokens</td><td>1M tokens</td><td>128k tokens</td><td>—</td></tr>
<tr><td><strong>Max output</strong></td><td>8,192 tokens</td><td>8,192 tokens</td><td>4,096 tokens</td><td>—</td></tr></tbody>
</table>"##;

    #[test]
    fn parses_every_priced_model() {
        let m = parse(&[PAGE.to_owned()]).unwrap();
        assert_eq!(m.len(), 3);
        let x6 = &m[0];
        assert_eq!(x6.external_id, "palmyra-x6");
        assert_eq!(x6.name, "Palmyra X6");
        assert_eq!(x6.maker.as_deref(), Some("writer"));
        assert_eq!(x6.input_per_mtok, 2.0);
        assert_eq!(x6.output_per_mtok, 8.0);
        assert_eq!(x6.context_window, Some(1_000_000));
        assert_eq!(x6.extra["max_output_tokens"], 8192);
        let x5 = &m[1];
        assert_eq!(x5.external_id, "palmyra-x5");
        assert_eq!((x5.input_per_mtok, x5.output_per_mtok), (0.6, 6.0));
        let x4 = &m[2];
        assert_eq!(x4.external_id, "palmyra-x4");
        assert_eq!((x4.input_per_mtok, x4.output_per_mtok), (2.5, 10.0));
        assert_eq!(x4.context_window, Some(128_000));
        assert!(m.iter().all(|x| x.source_url.as_deref() == Some(SOURCE)));
    }

    #[test]
    fn a_model_without_a_price_is_skipped() {
        assert!(PAGE.contains("palmyra-fin"));
        let m = parse(&[PAGE.to_owned()]).unwrap();
        assert!(m.iter().all(|x| x.external_id != "palmyra-fin"));
    }

    #[test]
    fn a_page_without_the_table_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()]).is_err());
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn a_half_priced_model_is_an_error() {
        assert!(parse(&[PAGE.replace("$8.00 / 1M tokens", "contact us")]).is_err());
    }

    /// `LAB_FIXTURE_WRITER=/path/to/page.html cargo test -p ih-ingest labs::writer -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_WRITER").expect("LAB_FIXTURE_WRITER");
        let html = std::fs::read_to_string(path).unwrap();
        for m in parse(&[html]).unwrap() {
            println!("{m:#?}");
        }
    }
}
