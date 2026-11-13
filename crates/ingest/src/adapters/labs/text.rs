//! Reading helpers shared by the lab parsers: markdown tables, page text, dollar amounts and
//! JSON embedded in a page.

use serde_json::Value;

/// Every row of every pipe table in a markdown document, as trimmed cells. Separator rows
/// (`| --- | --- |`) are dropped. Table boundaries are kept: a blank line starts a new table.
pub fn md_tables(body: &str) -> Vec<Vec<Vec<String>>> {
    let mut tables = Vec::new();
    let mut current: Vec<Vec<String>> = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('|') {
            let cells: Vec<String> = t
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_owned())
                .collect();
            let is_rule = cells
                .iter()
                .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')));
            if !is_rule {
                current.push(cells);
            }
        } else if !current.is_empty() {
            tables.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tables.push(current);
    }
    tables
}

/// Markdown with its decoration removed: links become their text, code ticks, bold and
/// italics are dropped, `<br>` becomes a space.
pub fn plain(md: &str) -> String {
    let mut s = md
        .replace("<br>", " ")
        .replace("<br/>", " ")
        .replace("<br />", " ");
    // [text](url) -> text
    while let Some(open) = s.find("](") {
        let Some(start) = s[..open].rfind('[') else {
            break;
        };
        let Some(close) = s[open..].find(')') else {
            break;
        };
        let label = s[start + 1..open].to_owned();
        s.replace_range(start..open + close + 1, &label);
    }
    s.replace(['`', '*'], "").trim().to_owned()
}

/// Visible text of an HTML page, one line per block element. Scripts, styles and tags are
/// removed; the common entities are decoded.
pub fn html_lines(html: &str) -> Vec<String> {
    let mut out = String::with_capacity(html.len() / 2);
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Skip whole <script> and <style> elements.
            let rest = &lower[i..];
            let skip_to = if rest.starts_with("<script") {
                rest.find("</script>").map(|p| p + 9)
            } else if rest.starts_with("<style") {
                rest.find("</style>").map(|p| p + 8)
            } else {
                None
            };
            if let Some(n) = skip_to {
                i += n;
                continue;
            }
            let end = html[i..].find('>').map_or(html.len(), |p| i + p + 1);
            let tag = &lower[i..end];
            let block = [
                "<br", "<p", "</p", "<div", "</div", "<li", "<tr", "</tr", "<h1", "<h2", "<h3",
                "<h4", "<td", "<th",
            ]
            .iter()
            .any(|t| tag.starts_with(t));
            out.push(if block { '\n' } else { ' ' });
            i = end;
        } else {
            let next = html[i..].find('<').map_or(html.len(), |p| i + p);
            out.push_str(&html[i..next]);
            i = next;
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'");
    out.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Every dollar amount in `s` (`$0.15`, `$1,200`, `US$2`), in order of appearance.
pub fn dollars(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.' || b[j] == b',') {
                j += 1;
            }
            let num: String = s[i + 1..j].chars().filter(|c| *c != ',').collect();
            if let Ok(v) = num.trim_end_matches('.').parse::<f64>() {
                out.push(v);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// The first dollar amount in `s`.
pub fn first_dollar(s: &str) -> Option<f64> {
    dollars(s).into_iter().next()
}

/// The JSON value that follows `marker` (such as `__XAI_PUBLIC_MODELS__=`) in a page: the
/// balanced `{...}` or `[...]` starting at the next brace, string-aware.
pub fn embedded_json(body: &str, marker: &str) -> Option<Value> {
    let start = body.find(marker)? + marker.len();
    let rest = &body[start..];
    let open = rest.find(['{', '['])?;
    let bytes = rest.as_bytes();
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    for (k, &c) in bytes.iter().enumerate().skip(open) {
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str(&rest[open..=k]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

/// `0.0025` -> `0.0025`; rounds away float noise from a division.
pub fn tidy(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md_tables_split_on_blank_lines_and_drop_rules() {
        let t = md_tables("| A | B |\n| --- | --- |\n| 1 | 2 |\n\ntext\n\n| C |\n|---|\n| 3 |\n");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], vec![vec!["A", "B"], vec!["1", "2"]]);
        assert_eq!(t[1], vec![vec!["C"], vec!["3"]]);
    }

    #[test]
    fn plain_strips_links_ticks_and_breaks() {
        assert_eq!(
            plain("[GLM 5.3](/models/glm) `glm-5.3`<br>**Input** $1"),
            "GLM 5.3 glm-5.3 Input $1"
        );
    }

    #[test]
    fn dollars_reads_every_amount() {
        assert_eq!(
            dollars("Input $0.15 /M, cached $0.015, big $1,200"),
            vec![0.15, 0.015, 1200.0]
        );
        assert_eq!(first_dollar("free"), None);
    }

    #[test]
    fn html_lines_drop_scripts_and_decode_entities() {
        let l = html_lines("<div>A &amp; B</div><script>var x = 1;</script><p>$2 <b>per</b> M</p>");
        assert_eq!(l, vec!["A & B", "$2 per M"]);
    }

    #[test]
    fn embedded_json_finds_the_balanced_value() {
        let v = embedded_json(r#"x __M__={"a":[1,{"b":"}"}],"c":2};y"#, "__M__=").unwrap();
        assert_eq!(v["a"][1]["b"], "}");
        assert_eq!(v["c"], 2);
    }
}
