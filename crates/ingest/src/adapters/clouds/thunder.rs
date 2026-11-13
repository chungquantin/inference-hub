//! Thunder Compute: on-demand GPU prices from its public pricing page. The "GPU instance
//! prices" table lists one row per GPU model with the price of a one-GPU instance (the page
//! selector defaults to 1× GPU); sandbox rates are a different product and are left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-thunder",
    name: "Thunder Compute",
    organization: "thunder-compute",
    origin: "https://www.thundercompute.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.thundercompute.com/pricing";

fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace('\u{2060}', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The text of the cell labelled `label` in a row.
fn cell(row: &str, label: &str) -> Option<String> {
    let key = format!("data-label=\"{label}\"");
    let at = row.find(&key)?;
    let rest = &row[at..];
    let start = rest.find('>')? + 1;
    let end = start + rest[start..].find("</td>")?;
    Some(plain(&rest[start..end]))
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let html = pages.first().ok_or("no page")?;
    let at = html
        .find("GPU instance prices")
        .ok_or("no GPU instance price table")?;
    let table = &html[at..];
    let table = &table[..table.find("</table>").unwrap_or(table.len())];
    let mut out: Vec<GpuListing> = Vec::new();
    for row in table.split("<tr").skip(1) {
        let (Some(a), Some(b)) = (row.find("<th scope=\"row\""), row.find("</th>")) else {
            continue;
        };
        let name = plain(&row[a..b]);
        // Add-on rows (extra vCPU) share the table but carry no GPU specs.
        let Some(vram) = cell(row, "VRAM").filter(|v| !v.is_empty()) else {
            continue;
        };
        let price_at = row
            .find("amount\">")
            .ok_or_else(|| format!("no price cell for {name}"))?;
        let price_text = plain(&row[price_at..]);
        let Some(usd) = text::first_dollar(&price_text) else {
            // No stated price: not a configuration to list.
            continue;
        };
        out.push(GpuListing {
            external_id: format!("{name}-1x"),
            name: name.clone(),
            gpu_type: gpu::normalize(&name).unwrap_or_else(|| name.clone()),
            gpu_count: 1,
            price_per_hour: fx.to_usd(usd),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "vram": vram,
                "vcpus": cell(row, "vCPUs"),
                "ram": cell(row, "RAM"),
                "disk": cell(row, "Disk"),
                "price_basis": "one-GPU instance, billed per minute",
                "original": fx.original(usd),
            }),
        });
    }
    if out.is_empty() {
        return Err("no priced rows in the GPU instance table".into());
    }
    let mut ids: Vec<_> = out.iter().map(|l| l.external_id.as_str()).collect();
    ids.sort();
    if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("duplicate configuration {}", w[0]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The real page puts U+2060 (word joiner) inside its ranges (`48–⁠64 GB`); keep it in the fixture.
    #[allow(clippy::invisible_characters)]
    const FIXTURE: &str = r##"<div><table class="PriceTable"><caption class="sr-only">GPU instance prices</caption><thead><tr><th scope="col">GPU</th><th scope="col">VRAM</th><th scope="col">vCPUs</th><th scope="col">RAM</th><th scope="col">Disk</th><th scope="col" class="p">Price</th><th scope="col"><span class="sr-only">Action</span></th></tr></thead><tbody><tr><th scope="row" class="PriceTable-name">RTX A6000</th><td data-label="VRAM" class="c">48 GB</td><td data-label="vCPUs" class="c">6 included · 8 max</td><td data-label="RAM" class="c">48–⁠64 GB</td><td data-label="Disk" class="c">100–⁠500 GB</td><td class="PriceTable-price"><span class="PriceTable-amount">$0.35</span><span class="u">/hr</span></td><td class="a"><a href="#">Launch A6000<!-- --> →</a></td></tr><tr><th scope="row" class="PriceTable-name">A100 80GB</th><td data-label="VRAM" class="c">80 GB</td><td data-label="vCPUs" class="c">8 included · 16 max</td><td data-label="RAM" class="c">64–⁠128 GB</td><td data-label="Disk" class="c">100–⁠500 GB</td><td class="PriceTable-price"><span class="PriceTable-amount">$1.09</span><span class="u">/hr</span></td><td class="a"><a href="#">Launch A100<!-- --> →</a></td></tr><tr><th scope="row" class="PriceTable-name">H100 SXM</th><td data-label="VRAM" class="c">80 GB</td><td data-label="vCPUs" class="c">4 included · 16 max</td><td data-label="RAM" class="c">32–⁠128 GB</td><td data-label="Disk" class="c">100–⁠500 GB</td><td class="PriceTable-price"><span class="PriceTable-amount">$3.20</span><span class="u">/hr</span></td><td class="a"><a href="#">Launch H100<!-- --> →</a></td></tr><tr><th scope="row" class="PriceTable-name">B200</th><td data-label="VRAM" class="c">180 GB</td><td data-label="vCPUs" class="c">-</td><td data-label="RAM" class="c">-</td><td data-label="Disk" class="c">-</td><td class="PriceTable-price"><span class="PriceTable-amount">Contact us</span></td><td class="a"></td></tr><tr><th scope="row" class="PriceTable-name">Extra vCPU</th><td data-label="VRAM" class="c"></td><td data-label="vCPUs" class="x">Above the included vCPUs</td><td class="PriceTable-price"><span class="PriceTable-amount">$0.04</span><span class="u">/vCPU-hr</span></td></tr></tbody></table></div>"##;

    #[test]
    fn parses_rows_and_skips_unpriced() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3);
        let h100 = l.iter().find(|x| x.name == "H100 SXM").unwrap();
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 3.20);
        assert_eq!(h100.extra["vcpus"], "4 included · 16 max");
        let a100 = l.iter().find(|x| x.name == "A100 80GB").unwrap();
        assert_eq!(a100.gpu_type, "A100 SXM");
        assert_eq!(a100.price_per_hour, 1.09);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_without_the_table() {
        assert!(parse(&["<p>hi</p>".into()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_THUNDER").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
