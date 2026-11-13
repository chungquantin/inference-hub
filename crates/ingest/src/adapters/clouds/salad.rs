//! Salad: a marketplace of consumer GPUs, priced per GPU-hour by priority tier. The
//! standard (High priority) price is the listing's price; the other tiers go in `extra`.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, round6};

const URL: &str = "https://salad.com/pricing/";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-salad",
    name: "Salad",
    organization: "salad",
    origin: "https://salad.com",
    paths: &["/pricing/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

/// The text of each `<td>` of a row; the page leaves `<td>` and `<tr>` unclosed.
fn cells(row: &str) -> Vec<String> {
    row.split("<td")
        .skip(1)
        .map(|c| {
            let c = &c[c.find('>').map_or(0, |i| i + 1)..];
            let mut text = String::new();
            let mut in_tag = false;
            for ch in c.chars() {
                match ch {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    _ if !in_tag => text.push(ch),
                    _ => {}
                }
            }
            text.trim().to_owned()
        })
        .collect()
}

fn usd(cell: &str) -> Result<f64, String> {
    cell.trim()
        .strip_prefix('$')
        .and_then(|n| n.replace(',', "").parse::<f64>().ok())
        .filter(|p| *p > 0.0)
        .ok_or_else(|| format!("salad: price `{cell}` does not parse"))
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("salad: no page")?;
    let start = page
        .find("data-gpu-table")
        .ok_or("salad: the GPU price table is missing")?;
    let table = &page[start..];
    let table = &table[..table.find("</table>").unwrap_or(table.len())];
    let mut out: Vec<GpuListing> = Vec::new();
    for row in table.split("<tr").skip(1) {
        let c = cells(row);
        if c.len() < 6 {
            continue;
        }
        let name = c[0].clone();
        let vram: Option<u32> = c[1].trim_end_matches(" GB").parse().ok();
        let (high, medium, low, lowest) = (usd(&c[2])?, usd(&c[3])?, usd(&c[4])?, usd(&c[5])?);
        if out.iter().any(|l| l.external_id == name) {
            continue;
        }
        out.push(GpuListing {
            external_id: name.clone(),
            name: name.clone(),
            gpu_type: gpu::normalize(&name).unwrap_or_else(|| name.clone()),
            gpu_count: 1,
            price_per_hour: round6(high),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "vram_gb": vram,
                "priority": "High",
                "priority_prices": { "high": high, "medium": medium, "low": low, "lowest": lowest },
            }),
        });
    }
    if out.is_empty() {
        return Err("salad: the GPU price table has no rows".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<table class=table data-gpu-table><caption>USD per GPU-hour by priority tier</caption><thead><tr><th>GPU<th class=num>VRAM<th class=num>High<th class=num>Medium<th class=num>Low<th class=num>Lowest<th class=num>$/mo (Lowest)<tbody><tr><td><b>RTX 5090</b><td class=num>32 GB<td class=num>$0.600<td class=num>$0.483<td class=num>$0.367<td class="num hl">$0.250<td class=num>$182.50<tr><td><b>RTX 4090</b><td class=num>24 GB<td class=num>$0.440<td class=num>$0.347<td class=num>$0.253<td class="num hl">$0.160<td class=num>$116.80<tr><td><b>RTX 3090</b><td class=num>24 GB<td class=num>$0.200<td class=num>$0.163<td class=num>$0.127<td class="num hl">$0.090<td class=num>$65.70</table>"#;

    #[test]
    fn reads_the_high_priority_price() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!((l[0].gpu_type.as_str(), l[0].gpu_count), ("RTX 5090", 1));
        assert_eq!(l[0].price_per_hour, 0.6);
        assert_eq!(l[1].price_per_hour, 0.44);
        assert_eq!(l[0].extra["priority_prices"]["lowest"], 0.25);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.as_str()).collect();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn missing_table_is_an_error() {
        assert!(parse(&["<html></html>".into()], &Fx::usd()).is_err());
        assert!(
            parse(
                &["<table data-gpu-table><tbody></table>".into()],
                &Fx::usd()
            )
            .is_err()
        );
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_SALAD").expect("CLOUD_FIXTURE_SALAD");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
