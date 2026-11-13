//! Runcrate: per-GPU hourly prices from the pricing page's GPU table.
//!
//! Left disabled: the column is headed "From" with the note "the lowest live rate for each",
//! so the prices are lower bounds across a marketplace of suppliers, which the cloud
//! contract leaves out. Prepaid-term (1 to 12 month) prices are not on-demand and are not read.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, round6};

const URL: &str = "https://www.runcrate.ai/pricing";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-runcrate",
    name: "Runcrate",
    organization: "runcrate",
    origin: "https://www.runcrate.ai",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // "From" prices (lowest live rate): see the module note.
    enabled: false,
    parse,
};

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&#x27;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("runcrate: no page")?;
    let marker = page
        .find("Against the dearest GPU")
        .ok_or("runcrate: the GPU price table is missing")?;
    let table = &page[marker..];
    let table = &table[..table.find("</table>").unwrap_or(table.len())];
    let mut out: Vec<GpuListing> = Vec::new();
    for row in table.split("<tr").skip(1) {
        let cells: Vec<String> = row
            .split("<td")
            .skip(1)
            .map(|c| format!("<td{c}"))
            .collect();
        if cells.len() < 3 {
            continue;
        }
        // The first span of the first cell is the GPU name; the rest is architecture and notes.
        let Some(name) = cells[0]
            .split("<span")
            .nth(1)
            .map(|s| strip_tags(&format!("<span{s}")))
        else {
            continue;
        };
        let vram: Option<u32> = strip_tags(&cells[1]).trim_end_matches("GB").parse().ok();
        let price_cell = &cells[2][..cells[2].find("<a ").unwrap_or(cells[2].len())];
        let price_text = strip_tags(price_cell);
        let Some(price) = price_text
            .strip_prefix('$')
            .and_then(|p| p.strip_suffix("/hr"))
            .and_then(|p| p.replace(',', "").parse::<f64>().ok())
        else {
            // Rows without a number (sold out, contact us) carry no price.
            if price_text.contains('$') {
                return Err(format!(
                    "runcrate: price `{price_text}` of {name} does not parse"
                ));
            }
            continue;
        };
        if price <= 0.0 || name.is_empty() || out.iter().any(|l| l.external_id == name) {
            continue;
        }
        out.push(GpuListing {
            external_id: name.clone(),
            name: name.clone(),
            gpu_type: gpu::normalize(&name).unwrap_or_else(|| name.clone()),
            gpu_count: 1,
            price_per_hour: round6(price),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({ "vram_gb": vram, "note": "lowest live rate, per GPU" }),
        });
    }
    if out.is_empty() {
        return Err("runcrate: the GPU price table has no priced rows".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, arch: &str, vram: &str, price: &str) -> String {
        format!(
            r#"<tr><td class="h-10"><span class="block text-body-lg font-medium text-ink">{name}</span><span class="mt-0.5 block text-body-sm text-ink-mid">{arch}</span></td><td class="h-10 u-num text-right">{vram}</td><td class="h-10 text-right"><span class="u-num inline-block"><span class="text-ink">{price}</span><span class="text-ink-mid">/hr</span></span><a aria-label="Deploy">Deploy</a></td><td></td></tr>"#
        )
    }

    fn fixture() -> String {
        format!(
            r#"<table><thead><tr><th>GPU</th><th>Memory</th><th>From</th><th>Against the dearest GPU</th><th>Deploy</th></tr></thead><tbody>{}{}{}{}</tbody></table>"#,
            row("H200", "Hopper", "141GB", "$5.25"),
            row("H100", "Hopper", "80GB", "$2.85"),
            row("RTX 4090", "Ada Lovelace", "24GB", "$0.43"),
            r#"<tr><td><span>GAUDI2</span></td><td>—</td><td>Sold out</td></tr>"#,
        )
    }

    #[test]
    fn reads_per_gpu_prices_and_skips_rows_without_one() {
        let l = parse(&[fixture()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("H200 SXM", 5.25)
        );
        assert_eq!(
            (l[1].gpu_type.as_str(), l[1].gpu_count, l[1].price_per_hour),
            ("H100 SXM", 1, 2.85)
        );
        assert_eq!(l[2].gpu_type, "RTX 4090");
        assert_eq!(l[1].extra["vram_gb"], 80);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn missing_table_is_an_error() {
        assert!(parse(&["<html></html>".into()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_RUNCRATE").expect("CLOUD_FIXTURE_RUNCRATE");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
