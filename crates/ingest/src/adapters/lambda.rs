//! Lambda: on-demand instance prices from its pricing page. The page has one
//! table per instance size (8×, 4×, 2×, 1×) whose rows carry the GPU name in
//! `data-plan` and the price per GPU hour; cluster tables (2 weeks and longer)
//! are left out.
//!
//! When the page announces new prices with a future start date, it shows the
//! new ones; the date is kept in `extra.note`.

use super::{
    DAILY, GpuListing, gpu, parse_price, round6,
    simple::{non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "lambda";
/// The on-demand page lists a dozen or more configurations.
const MIN_EXPECTED: usize = 5;

simple_gpu_adapter!(LambdaAdapter, ID, "Lambda", "lambda", DAILY, "/pricing");

/// Text between the first `>` after `from` and the next `<`.
fn inner_text(s: &str) -> &str {
    let start = s.find('>').map(|i| i + 1).unwrap_or(0);
    let end = s[start..].find('<').map(|i| start + i).unwrap_or(s.len());
    s[start..end].trim()
}

pub(crate) fn parse(html: &str) -> Result<Vec<GpuListing>, AdapterError> {
    parse_on(html, chrono::Utc::now().date_naive())
}

/// Parses the page as of `today`. When the page announces prices that take
/// effect later, the run fails: the directory never shows a price before it
/// applies, and the adapter recovers by itself once the date passes.
pub(crate) fn parse_on(
    html: &str,
    today: chrono::NaiveDate,
) -> Result<Vec<GpuListing>, AdapterError> {
    let note = html.find("These prices take effect").map(|i| {
        let rest = &html[i..];
        rest[..rest.find('<').unwrap_or(rest.len())]
            .trim()
            .to_owned()
    });
    if let Some(starts) = note.as_deref().and_then(effective_date)
        && starts > today
    {
        return Err(AdapterError::NotYet(ID, starts));
    }
    // The size tabs (`8x`, `4x`, ...) in page order; the on-demand tables follow in the same order.
    let mut sizes = tab_sizes(html).into_iter();
    let mut out = Vec::new();
    for table in html.split("<table").skip(1) {
        let table = &table[..table.find("</table>").unwrap_or(table.len())];
        let header = &table[..table.find("</thead>").unwrap_or(0)];
        // On-demand tables have a VRAM column; cluster tables have DURATION.
        if !header.contains("VRAM/GPU") || header.contains("DURATION") {
            continue;
        }
        let Some(count) = sizes.next() else {
            return Err(AdapterError::Format(
                ID,
                "more price tables than size tabs".into(),
            ));
        };
        for row in table.split("<tr").skip(1) {
            let Some(plan) = attr(row, "data-plan") else {
                continue;
            };
            let Some(price_cell) = row.find("data-label=\"PRICE/GPU/HR") else {
                continue;
            };
            let text = inner_text(&row[price_cell..]);
            let Some(price) = text.strip_prefix('$') else {
                continue;
            };
            let per_gpu = parse_price(ID, price)?;
            let Some(gpu_type) = gpu::normalize(&plan) else {
                continue;
            };
            out.push(GpuListing {
                external_id: plan.clone(),
                name: format!("{count}× {plan}"),
                gpu_type,
                gpu_count: count,
                price_per_hour: round6(per_gpu * count as f64),
                region: None,
                source_url: Some("https://lambda.ai/pricing".into()),
                extra: serde_json::json!({ "per_gpu_hour": per_gpu, "note": note }),
            });
        }
    }
    if out.len() < MIN_EXPECTED {
        return Err(AdapterError::Format(
            ID,
            format!(
                "found {} on-demand configurations; the page layout changed",
                out.len()
            ),
        ));
    }
    non_empty(ID, out)
}

/// The date in "These prices take effect November 16, 2026, at 00:00 UTC".
fn effective_date(note: &str) -> Option<chrono::NaiveDate> {
    let rest = note.split("take effect").nth(1)?.trim_start();
    let words: Vec<&str> = rest.split_whitespace().take(3).collect();
    let text = words.join(" ");
    let text = text.trim_end_matches([',', '.']);
    chrono::NaiveDate::parse_from_str(text, "%B %d, %Y").ok()
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].replace("&amp;", "&"))
}

/// The labels of `role="tab"` buttons that name a size, such as `8x`, in page order.
fn tab_sizes(html: &str) -> Vec<i16> {
    html.split("role=\"tab\"")
        .skip(1)
        .filter_map(|t| inner_text(t).strip_suffix('x')?.parse::<i16>().ok())
        .filter(|n| *n > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[(&str, &str, &str)]) -> String {
        let mut t = String::from(
            r#"<table><thead><tr><th>Plan</th><th>VRAM/GPU</th><th>vCPUs</th><th>PRICE/GPU/HR*</th></tr></thead><tbody>"#,
        );
        for (plan, vcpus, price) in rows {
            t.push_str(&format!(
                r#"<tr data-plan="{plan}"><th>{plan}</th><td data-label="VRAM/GPU">80 GB</td><td data-label="vCPUs">{vcpus}</td><td data-label="PRICE/GPU/HR*">{price}</td></tr>"#
            ));
        }
        t.push_str("</tbody></table>");
        t
    }

    #[test]
    fn on_demand_tables_only_with_counts_from_vcpus() {
        let cluster = r#"<table><thead><tr><th>Plan</th><th>DURATION</th><th>PRICE/GPU/HR*</th></tr></thead>
            <tbody><tr data-plan="NVIDIA H100"><td data-label="PRICE/GPU/HR*">$6.16</td></tr></tbody></table>"#;
        let tabs = r#"<div role="tablist"><button role="tab" class="t">8x</button><button role="tab">2x</button><button role="tab">1x</button></div>"#;
        let html = format!(
            "<html>{cluster}{tabs}{}{}{}<p>These prices take effect November 16, 2026.</p></html>",
            table(&[
                ("NVIDIA HGX B200", "208", "$7.69"),
                ("NVIDIA HGX H100", "208", "$4.59")
            ]),
            table(&[
                ("NVIDIA H100 SXM", "104", "$4.69"),
                ("NVIDIA A6000", "56", "$1.09")
            ]),
            table(&[
                ("NVIDIA GH200", "64", "$2.59"),
                ("NVIDIA H100 PCIe", "26", "$3.79")
            ]),
        );
        let before = chrono::NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let err = parse_on(&html, before).unwrap_err();
        assert!(
            matches!(err, AdapterError::NotYet(_, d) if d.to_string() == "2026-11-16"),
            "{err}"
        );
        assert!(
            err.to_string().contains("take effect on 2026-11-16"),
            "{err}"
        );

        let after = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let l = parse_on(&html, after).unwrap();
        let h100x8 = l
            .iter()
            .find(|l| l.external_id == "NVIDIA HGX H100")
            .unwrap();
        assert_eq!(
            (
                h100x8.gpu_type.as_str(),
                h100x8.gpu_count,
                h100x8.price_per_hour
            ),
            ("H100 SXM", 8, 36.72)
        );
        let pcie = l
            .iter()
            .find(|l| l.external_id == "NVIDIA H100 PCIe")
            .unwrap();
        assert_eq!((pcie.gpu_count, pcie.price_per_hour), (1, 3.79));
        let gh200 = l.iter().find(|l| l.external_id == "NVIDIA GH200").unwrap();
        assert_eq!(
            gh200.gpu_count, 1,
            "the count comes from the tab, not the vCPUs"
        );
        assert!(
            l.iter().all(|l| l.price_per_hour != 6.16 * 8.0),
            "cluster prices are left out"
        );
        assert!(l[0].extra["note"].as_str().unwrap().contains("November 16"));
    }

    #[test]
    fn a_changed_layout_fails() {
        assert!(parse("<html><table></table></html>").is_err());
        assert_eq!(
            effective_date("These prices take effect November 16, 2026, at 00:00 UTC."),
            chrono::NaiveDate::from_ymd_opt(2026, 11, 16)
        );
    }
}
