//! Alibaba Qwen (Model Studio): models and prices from the lab's public pricing pages.
//!
//! The page has one table per model family and region. Only the Singapore (international,
//! USD) tables under the "Text generation - Qwen" headings are read: Qwen-Max, Plus, Flash,
//! VL, OCR, Coder, Translation and the open-source models. Omni (priced per modality) and
//! the third-party models resold on Model Studio are skipped. A model priced in tiers by
//! prompt length is listed at its first tier; the others go in `extra.later_tiers`.

use serde_json::json;

use super::{LAB_INTERVAL, LabSpec, text};
use crate::adapters::{ModelListing, round6};

const SOURCE: &str = "https://www.alibabacloud.com/help/en/model-studio/model-pricing";

pub static SPEC: LabSpec = LabSpec {
    id: "lab-qwen",
    name: "Alibaba Qwen (Model Studio)",
    organization: "alibaba-qwen",
    origin: "https://www.alibabacloud.com",
    paths: &["/help/en/model-studio/model-pricing"],
    interval: LAB_INTERVAL,
    enabled: true,
    parse,
};

/// One price tier of a model: prompts up to `max_input_tokens`.
struct Tier {
    max_input_tokens: Option<i64>,
    input: f64,
    output: f64,
    output_thinking: Option<f64>,
}

fn is_model_id(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
}

/// The dollar amount of a price cell: `$0.4`, `List price $0.4 (Limited-time 20% off)`.
/// `None` when the cell states none; the cell is then not a price.
fn cell_price(cell: &str) -> Option<f64> {
    text::first_dollar(cell).map(round6)
}

fn parse(pages: &[String]) -> Result<Vec<ModelListing>, String> {
    let html = pages.first().ok_or("qwen: no page")?;
    let mut out: Vec<ModelListing> = Vec::new();
    for table in tables(html) {
        let in_scope = matches!(
            table.h2.as_str(),
            "Text generation - Qwen" | "Text generation - Qwen (open source)"
        );
        if !in_scope || table.h4 != "Singapore" {
            continue;
        }
        // Header rows are the ones above the first row that starts with a model id.
        let Some(first) = table
            .rows
            .iter()
            .position(|r| r[0].lines().next().is_some_and(|l| is_model_id(l.trim())))
        else {
            continue;
        };
        let width = table.rows[0].len();
        let header: Vec<String> = (0..width)
            .map(|c| {
                table.rows[..first]
                    .iter()
                    .filter_map(|r| r.get(c))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_ascii_lowercase()
            })
            .collect();
        // Multimodal tables price by modality: their header names the modalities.
        if header
            .iter()
            .any(|h| h.contains("audio") || h.contains("image/video"))
        {
            continue;
        }
        let in_col = header
            .iter()
            .position(|h| h.contains("input price") && !h.contains("cache"));
        let out_cols: Vec<usize> = header
            .iter()
            .enumerate()
            .filter(|(_, h)| h.contains("output price"))
            .map(|(c, _)| c)
            .collect();
        let (Some(in_col), Some(&out_col)) = (in_col, out_cols.first()) else {
            continue;
        };
        let think_col = out_cols.get(1).copied().filter(|c| {
            header[*c].contains("thinking mode (") || header[*c].contains("chain of thought")
        });
        let tier_col = header
            .iter()
            .position(|h| h.contains("input tokens per request"));

        let mut i = first;
        while i < table.rows.len() {
            let row = &table.rows[i];
            let id = row[0].lines().next().unwrap_or("").trim().to_owned();
            let mut j = i;
            let mut tiers = Vec::new();
            while j < table.rows.len() && table.rows[j][0].lines().next().unwrap_or("").trim() == id
            {
                let r = &table.rows[j];
                let get = |c: usize| r.get(c).map(String::as_str).unwrap_or("");
                let (inp, outp) = (cell_price(get(in_col)), cell_price(get(out_col)));
                let (inp, outp) = match (inp, outp) {
                    (Some(i), Some(o)) => (i, o),
                    (None, None) => {
                        j += 1;
                        continue;
                    }
                    _ => return Err(format!("qwen: {id} has only one of its two prices")),
                };
                tiers.push(Tier {
                    max_input_tokens: tier_col
                        .map(get)
                        .and_then(|t| t.split_once('≤'))
                        .and_then(|(_, hi)| count(hi)),
                    input: inp,
                    output: outp,
                    output_thinking: think_col.and_then(|c| cell_price(get(c))),
                });
                j += 1;
            }
            i = j.max(i + 1);
            let Some(top) = tiers.first() else { continue };
            if !is_model_id(&id) || out.iter().any(|m| m.external_id == id) {
                continue;
            }
            let mut extra = json!({});
            if let Some(t) = top.output_thinking.filter(|t| *t != top.output) {
                extra["output_thinking_per_mtok"] = json!(t);
            }
            if tiers.len() > 1 {
                extra["later_tiers"] = tiers[1..]
                    .iter()
                    .map(|t| {
                        json!({
                            "max_input_tokens": t.max_input_tokens,
                            "input_per_mtok": t.input,
                            "output_per_mtok": t.output,
                        })
                    })
                    .collect();
            }
            out.push(ModelListing {
                external_id: id.clone(),
                name: id,
                maker: Some("qwen".into()),
                context_window: tiers
                    .last()
                    .and_then(|t| t.max_input_tokens)
                    .and_then(|c| i32::try_from(c).ok()),
                input_per_mtok: top.input,
                output_per_mtok: top.output,
                variant: None,
                source_url: Some(SOURCE.into()),
                extra,
                media: None,
                capabilities: Vec::new(),
            });
        }
    }
    if out.is_empty() {
        return Err("qwen: no priced Singapore text models on the page".into());
    }
    Ok(out)
}

/// One HTML table with the headings above it, spans expanded so every row has one cell per
/// column (a cell spanning rows or columns is repeated).
struct Table {
    h2: String,
    h4: String,
    rows: Vec<Vec<String>>,
}

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

fn tables(html: &str) -> Vec<Table> {
    let lower = html.to_ascii_lowercase();
    let (mut h2, mut h4) = (String::new(), String::new());
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = lower[i..].find('<') {
        let p = i + rel;
        let rest = &lower[p..];
        let heading = ["<h2", "<h4"].into_iter().find(|h| {
            rest.starts_with(h) && rest[3..].starts_with(|c: char| c == '>' || c.is_whitespace())
        });
        if rest.starts_with("<table") {
            let end = rest.find("</table>").map_or(html.len(), |e| p + e);
            out.push(Table {
                h2: h2.clone(),
                h4: h4.clone(),
                rows: grid(&html[p..end], &lower[p..end]),
            });
            i = end.max(p + 1);
        } else if let Some(tag) = heading {
            let close = format!("</{}>", &tag[1..]);
            let Some(e) = rest.find(&close) else {
                i = p + 1;
                continue;
            };
            let t = text::html_lines(&html[p..p + e]).join(" ");
            if tag == "<h2" {
                h2 = t;
            } else {
                h4 = t;
            }
            i = p + e;
        } else {
            i = p + 1;
        }
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

    /// Trimmed excerpts of the live page: the Qwen-Max Singapore table, the Qwen-Plus
    /// Singapore table (tiered, with a thinking-mode output column), a China (Beijing) table
    /// and a Qwen-Omni table, which are skipped, and a third-party section, also skipped.
    const PAGE: &str = r##"<h2 id="a">Text generation - Qwen</h2>
<h3 id="b">Qwen-Max</h3>
<h4 id="c">Singapore</h4>
<table><thead>
<tr><th><strong>Model ID</strong></th><th><strong>Deployment scope</strong></th><th><strong>Mode</strong></th><th><strong>Input tokens per request</strong></th><th><strong>Input price (per 1 million tokens)</strong></th><th><strong>Output price (per 1 million tokens)</strong><blockquote>
<strong>Chain of thought + answer</strong>
</blockquote></th><th><p><strong>Free quota</strong>(Note)</p><sup>Valid for 90 days from the date of Model Studio activation, model release, or application approval, whichever is later</sup></th></tr>
<tr><td><p>qwen3.8-max</p><blockquote>
<p>context caching discount</p>
</blockquote></td><td><p>International</p></td><td><p>Non-Thinking and Thinking modes</p></td><td><p>0&lt;Token≤1M</p></td><td><p>$2</p></td><td><p>$6</p></td><td><p>1 million tokens</p></td></tr>
<tr><td><p>qwen3.7-max</p><blockquote>
<p>Currently equivalent to qwen3.7-max-2026-05-20</p>
</blockquote><blockquote>
<p>context caching discount</p>
</blockquote></td><td><p>International</p></td><td><p>Non-Thinking and Thinking modes</p></td><td><p>0&lt;Token≤1M</p></td><td><p>$2.5</p></td><td><p>$7.5</p></td><td><p>1 million tokens</p></td></tr></tbody></table>
<h4 id="d">China (Beijing)</h4>
<table><thead>
<tr><th><strong>Model ID</strong></th><th><strong>Mode</strong></th><th><strong>Input tokens per request</strong></th><th><strong>Input price (per 1 million tokens)</strong></th><th><strong>Output price (per 1 million tokens)</strong><blockquote>
<strong>Chain of thought + answer</strong>
</blockquote></th></tr>
<tr><td><p>qwen3.8-max</p><blockquote>
<p>50% batch inference discount</p>
</blockquote><blockquote>
<p>context caching discount</p>
</blockquote></td><td><p>Non-Thinking and Thinking modes</p></td><td><p>0&lt;Token≤1M</p></td><td><p>$1.65</p></td><td><p>$4.951</p></td></tr></tbody></table>
<h3 id="e">Qwen-Plus</h3>
<h4 id="f">Singapore</h4>
<table><thead>
<tr><th rowspan="2"><strong>Model ID</strong></th><th rowspan="2"><strong>Deployment scope</strong></th><th rowspan="2"><strong>Input tokens per request</strong></th><th rowspan="2"><strong>Input price (per 1 million tokens)</strong></th><th colspan="2"><strong>Output price (per 1 million tokens)</strong></th><th rowspan="2"><p><strong>Free quota</strong>(Note)</p><sup>Valid for 90 days from the date of Model Studio activation, model release, or application approval, whichever is later</sup></th></tr>
<tr><th><strong>Non-Thinking mode</strong></th><th><strong>Thinking mode (chain of thought + answer)</strong></th></tr>
<tr><td rowspan="2"><p>qwen3.7-plus</p><blockquote>
<p>Currently equivalent to qwen3.7-plus-2026-05-26</p>
</blockquote><blockquote>
<p>context caching discount</p>
</blockquote></td><td rowspan="2"><p>International</p></td><td><p>0&lt;Token≤256K</p></td><td><p>List price $0.4 <strong>(Limited-time 20% off)</strong></p></td><td><p>List price $1.6 <strong>(Limited-time 20% off)</strong></p></td><td><p>List price $1.6 <strong>(Limited-time 20% off)</strong></p></td><td rowspan="2"><p>1 million tokens</p></td></tr>
<tr><td><p>256K&lt;Token≤1M</p></td><td><p>List price $1.2 <strong>(Limited-time 20% off)</strong></p></td><td><p>List price $4.8 <strong>(Limited-time 20% off)</strong></p></td><td><p>List price $4.8 <strong>(Limited-time 20% off)</strong></p></td></tr>
<tr><td rowspan="2"><p>qwen3.7-plus-2026-05-26</p><blockquote>
<p>context caching discount</p>
</blockquote></td><td rowspan="2"><p>International</p></td><td><p>0&lt;Token≤256K</p></td><td><p>$0.4</p></td><td><p>$1.6</p></td><td><p>$1.6</p></td><td rowspan="2"><p>No free quota</p></td></tr>
<tr><td><p>256K&lt;Token≤1M</p></td><td><p>$1.2</p></td><td><p>$4.8</p></td><td><p>$4.8</p></td></tr></tbody></table>
<h3 id="g">Qwen-Omni</h3>
<h4 id="h">Singapore</h4>
<table><thead>
<tr><th rowspan="2"><strong>Model ID</strong></th><th rowspan="2"><strong>Deployment scope</strong></th><th colspan="2"><strong>Input price (per 1 million tokens)</strong></th><th colspan="2"><strong>Output price (per 1 million tokens)</strong></th><th rowspan="2"><p><strong>Free quota</strong>(Note)</p><sup>Valid for 90 days from the date of Model Studio activation, model release, or application approval, whichever is later</sup></th></tr>
<tr><th><strong>Text/Image/video</strong></th><th><strong>Audio</strong></th><th><strong>Text</strong><blockquote>
<p>Multimodal input</p>
</blockquote></th><th><strong>Text + audio</strong><blockquote>
<p>Audio only billed</p>
</blockquote></th></tr>
<tr><td><p>qwen3.5-omni-plus</p><blockquote>
<p>Currently equivalent to qwen3.5-omni-plus-2026-03-15</p>
</blockquote></td><td><p>International</p></td><td><p>$1.4</p></td><td><p>$11</p></td><td><p>$8.3</p></td><td><p>$44</p></td><td><p>1 million tokens</p></td></tr></tbody></table>
<h2 id="i">Text generation - third-party models</h2>
<h4 id="j">Singapore</h4>
<table><thead>
<tr><th><strong>Model ID</strong></th><th><strong>Deployment scope</strong></th><th><strong>Input price (per 1 million tokens)</strong></th><th><strong>Output price (per 1 million tokens)</strong></th><th><p><strong>Free quota</strong>(Note)</p><sup>Valid for 90 days from the date of Model Studio activation, model release, or application approval, whichever is later</sup></th></tr>
<tr><td><p>deepseek-v4.1-flash-1017</p><blockquote>
<p>context caching discount</p>
</blockquote></td><td><p>International</p></td><td><p>Busy hours: $0.3</p><p>Idle hours: $0.15</p></td><td><p>Busy hours: $1.2</p><p>Idle hours: $0.6</p></td><td><p>1 million tokens</p></td></tr></tbody></table>"##;

    fn model<'a>(m: &'a [ModelListing], id: &str) -> &'a ModelListing {
        m.iter()
            .find(|x| x.external_id == id)
            .unwrap_or_else(|| panic!("{id} missing"))
    }

    #[test]
    fn reads_single_tier_models() {
        let m = parse(&[PAGE.to_owned()]).unwrap();
        let max = model(&m, "qwen3.8-max");
        assert_eq!(max.maker.as_deref(), Some("qwen"));
        assert_eq!((max.input_per_mtok, max.output_per_mtok), (2.0, 6.0));
        assert_eq!(max.context_window, Some(1_000_000));
        assert_eq!(max.source_url.as_deref(), Some(SOURCE));
        assert!(max.extra.get("later_tiers").is_none());
        let old = model(&m, "qwen3.7-max");
        assert_eq!((old.input_per_mtok, old.output_per_mtok), (2.5, 7.5));
    }

    #[test]
    fn reads_the_first_tier_and_keeps_the_rest_in_extra() {
        let m = parse(&[PAGE.to_owned()]).unwrap();
        // The alias shows the list price next to a limited-time discount: the list price wins.
        let plus = model(&m, "qwen3.7-plus");
        assert_eq!((plus.input_per_mtok, plus.output_per_mtok), (0.4, 1.6));
        assert_eq!(plus.context_window, Some(1_000_000));
        let tiers = plus.extra["later_tiers"].as_array().unwrap();
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0]["max_input_tokens"], 1_000_000);
        assert_eq!(tiers[0]["input_per_mtok"], 1.2);
        assert_eq!(tiers[0]["output_per_mtok"], 4.8);
        let dated = model(&m, "qwen3.7-plus-2026-05-26");
        assert_eq!(dated.input_per_mtok, 0.4);
    }

    #[test]
    fn skips_other_regions_omni_and_third_party_models() {
        let m = parse(&[PAGE.to_owned()]).unwrap();
        assert!(PAGE.contains("qwen3.5-omni-plus") && PAGE.contains("deepseek-v4.1-flash-1017"));
        assert!(m.iter().all(|x| !x.external_id.contains("omni")));
        assert!(m.iter().all(|x| !x.external_id.starts_with("deepseek")));
        // The China (Beijing) table prices qwen3.8-max at $1.65; the Singapore one wins.
        assert_eq!(model(&m, "qwen3.8-max").input_per_mtok, 2.0);
        let ids: Vec<_> = m.iter().map(|x| x.external_id.as_str()).collect();
        let mut dedup = ids.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len());
    }

    #[test]
    fn a_page_without_the_tables_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()]).is_err());
        assert!(parse(&[]).is_err());
    }

    /// `LAB_FIXTURE_QWEN=/path/to/page.html cargo test -p ih-ingest labs::qwen -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("LAB_FIXTURE_QWEN").expect("LAB_FIXTURE_QWEN");
        let html = std::fs::read_to_string(path).unwrap();
        let m = parse(&[html]).unwrap();
        println!("{} models", m.len());
        for x in m {
            println!(
                "{:<40} in {:<8} out {:<8} ctx {:<10?} {}",
                x.external_id, x.input_per_mtok, x.output_per_mtok, x.context_window, x.extra
            );
        }
    }
}
