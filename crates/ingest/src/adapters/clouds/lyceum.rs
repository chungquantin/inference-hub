//! Lyceum: on-demand GPU prices from its public pricing page. The "GPU compute" chart lists
//! each GPU with its on-demand and spot price in USD per GPU hour, one GPU per listing.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

const URL: &str = "https://lyceum.technology/resources/pricing/";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-lyceum",
    name: "Lyceum",
    organization: "lyceum",
    origin: "https://lyceum.technology",
    paths: &["/resources/pricing/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

/// The dollar amount right after `label` in `s`.
fn after(s: &str, label: &str) -> Option<f64> {
    let rest = &s[s.find(label)? + label.len()..];
    text::first_dollar(rest)
}

/// `NVIDIA H100 80 GB` -> (`H100`, 80).
fn gpu_heading(line: &str) -> Option<(String, u32)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let [name @ .., mem, "GB"] = words.as_slice() else {
        return None;
    };
    let mem = mem.parse().ok()?;
    let name = name.join(" ");
    name.starts_with("NVIDIA ").then_some((name, mem))
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = text::html_lines(page);
    let start = lines
        .iter()
        .position(|l| l.starts_with("USD per GPU hour"))
        .ok_or("no \"USD per GPU hour\" heading")?;
    let mut out = Vec::new();
    let mut i = start;
    while i < lines.len() {
        if let Some((raw, mem)) = gpu_heading(&lines[i]) {
            let Some(row) = lines.get(i + 1) else { break };
            // Rows without an on-demand price (reserved only) are skipped.
            if let Some(price) = after(row, "On demand") {
                let spot = after(row, "Spot");
                let gpu_type = gpu::normalize(&format!("{raw} {mem}GB")).unwrap_or(raw.clone());
                out.push(GpuListing {
                    external_id: format!("{}-1x", gpu_type.replace(' ', "-")),
                    name: format!("{raw} {mem} GB"),
                    gpu_type,
                    gpu_count: 1,
                    price_per_hour: price,
                    region: None,
                    source_url: Some(URL.into()),
                    extra: serde_json::json!({ "vram_gb": mem, "spot_price_per_hour": spot }),
                });
            }
        }
        i += 1;
    }
    if out.is_empty() {
        return Err("no GPU rows with an on-demand price".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<h2>GPU compute</h2><p>USD per GPU hour, billed per second with no subscription</p>
<div>GPU Memory</div><div>$0 $5 $10 $15 $20 On demand Spot Available now Reserved blocks</div>
<div>NVIDIA B300 288 GB</div>
<div>Spot $6.39 On demand $12.79 On demand $12.79 Spot $6.39 Available now Check dashboard ↗ Reserved</div>
<div>1024 · Central Europe · from Q4 2026 · 36 mo</div>
<div>NVIDIA H200 141 GB</div>
<div>On demand $7.09 Spot $3.59 On demand $7.09 Spot $3.59 Available now Check dashboard ↗ On request</div>
<div>NVIDIA H100 80 GB</div>
<div>On demand $5.39 Spot $2.69 On demand $5.39 Spot $2.69 Available now</div>
<div>NVIDIA L40S 48 GB</div>
<div>Reserved capacity on request</div>
<div>More ways to run</div>"#;

    #[test]
    fn parses_on_demand_and_spot() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3, "the L40S row has no on-demand price");
        let b300 = &l[0];
        assert_eq!(b300.gpu_type, "B300");
        assert_eq!(b300.gpu_count, 1);
        assert_eq!(b300.price_per_hour, 12.79);
        assert_eq!(b300.extra["spot_price_per_hour"], 6.39);
        let h100 = l.iter().find(|x| x.gpu_type == "H100 SXM").unwrap();
        assert_eq!(h100.price_per_hour, 5.39);
        assert_eq!(h100.extra["vram_gb"], 80);
        let mut ids: Vec<_> = l.iter().map(|x| &x.external_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn changed_page_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_LYCEUM").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!("{l:?}");
        }
    }
}
