//! Saturn Cloud: the reference price per GPU-hour from its plans page. Prices are per GPU,
//! one listing per GPU type; dedicated capacity is negotiated per deal, usually below these.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

const URL: &str = "https://saturncloud.io/plans/saturn_cloud_plans/";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-saturn",
    name: "Saturn Cloud",
    organization: "saturn-cloud",
    origin: "https://saturncloud.io",
    paths: &["/plans/saturn_cloud_plans/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

/// Text of the `<td data-label="{label}">` cell in `row`, tags removed.
fn cell(row: &str, label: &str) -> Option<String> {
    let key = format!("data-label=\"{label}\"");
    let rest = &row[row.find(&key)? + key.len()..];
    let rest = &rest[rest.find('>')? + 1..];
    let inner = &rest[..rest.find("</td>")?];
    let mut out = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    Some(out.trim().to_owned())
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let start = page
        .find("Reference price per GPU-hour")
        .ok_or("no reference price table")?;
    let table = &page[start..];
    let table = &table[..table.find("</table>").unwrap_or(table.len())];
    let mut out = Vec::new();
    for row in table.split("<tr").skip(1) {
        let (Some(raw), Some(price)) = (cell(row, "GPU"), cell(row, "Reference price")) else {
            continue;
        };
        // "from $1.10" is a lower bound and "Priced per deal" has no number.
        let Some(price) = price
            .strip_prefix('$')
            .and_then(|_| text::first_dollar(&price))
        else {
            continue;
        };
        let memory = cell(row, "Memory").unwrap_or_default();
        // Rack-scale NVL72 systems are priced per deal; per-GPU memory text marks them.
        if raw.contains("NVL72") {
            continue;
        }
        let gpu_type = gpu::normalize(&raw).unwrap_or_else(|| raw.clone());
        out.push(GpuListing {
            external_id: format!("{}-1x", gpu_type.replace(' ', "-")),
            name: raw,
            gpu_type,
            gpu_count: 1,
            price_per_hour: price,
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({ "memory": memory, "price_kind": "reference" }),
        });
    }
    if out.is_empty() {
        return Err("no priced GPU rows in the reference price table".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<table class="gpu-price-table"><thead><tr><th>GPU</th><th>Memory</th><th>Good for</th><th>Reference price per GPU-hour</th></tr></thead><tbody>
<tr><td data-label="GPU"><strong>NVIDIA L40S</strong></td><td data-label="Memory">48 GB</td><td data-label="Good for">Small models.</td><td data-label="Reference price">from $1.10</td></tr>
<tr><td data-label="GPU"><strong>NVIDIA RTX PRO 6000</strong></td><td data-label="Memory">96 GB</td><td data-label="Good for">Mid-size models.</td><td data-label="Reference price">$1.25</td></tr>
<tr><td data-label="GPU"><strong>NVIDIA H100</strong></td><td data-label="Memory">80 GB</td><td data-label="Good for">LLM serving.</td><td data-label="Reference price">$3.15</td></tr>
<tr><td data-label="GPU"><strong>NVIDIA B200</strong></td><td data-label="Memory">180 GB</td><td data-label="Good for">Large models.</td><td data-label="Reference price">$5.95</td></tr>
<tr><td data-label="GPU"><strong>NVIDIA GB300 NVL72</strong></td><td data-label="Memory">288 GB per GPU</td><td data-label="Good for">Very large.</td><td data-label="Reference price">Priced per deal</td></tr>
</tbody></table>"#;

    #[test]
    fn parses_reference_prices() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3, "from-price and per-deal rows are skipped");
        assert_eq!(l[0].gpu_type, "RTX PRO 6000");
        assert_eq!(l[0].price_per_hour, 1.25);
        let h100 = l.iter().find(|x| x.gpu_type == "H100 SXM").unwrap();
        assert_eq!((h100.gpu_count, h100.price_per_hour), (1, 3.15));
        assert_eq!(h100.extra["memory"], "80 GB");
        let mut ids: Vec<_> = l.iter().map(|x| &x.external_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn changed_page_is_an_error() {
        assert!(parse(&["<p>x</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_SATURN").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!("{l:?}");
        }
    }
}
