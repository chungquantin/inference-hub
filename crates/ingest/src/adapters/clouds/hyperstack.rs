//! Hyperstack: on-demand GPU prices from its public pricing page. The page lists one
//! row per GPU model (VRAM, max pCPUs and RAM per GPU, price per hour); the price is per
//! GPU, so each row is a one-GPU configuration. Reservation ("starting from") and spot
//! prices of the same model go in `extra`.

use std::collections::HashMap;

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-hyperstack",
    name: "Hyperstack",
    organization: "hyperstack",
    origin: "https://www.hyperstack.cloud",
    paths: &["/gpu-pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.hyperstack.cloud/gpu-pricing";

fn number(line: &str) -> Option<f64> {
    line.split_whitespace()
        .next()?
        .replace(',', "")
        .parse()
        .ok()
}

fn price(line: &str) -> Option<f64> {
    line.trim()
        .starts_with('$')
        .then(|| text::first_dollar(line))?
}

/// The page names its PCIe cards without the form factor (`NVIDIA H100`, `NVIDIA A100`; its
/// spot table calls the first `H100 PCIe`) and the A100 NVLink is a PCIe card with a bridge.
fn hint(name: &str) -> String {
    match name {
        "NVIDIA H100" | "NVIDIA A100" | "NVIDIA A100 NVLink" => {
            format!("{} PCIe", name.trim_end_matches(" NVLink"))
        }
        _ => name.to_owned(),
    }
}

/// `NVIDIA X` / `$p` pairs from `from` on, until the pairs stop.
fn pairs(lines: &[String], from: usize, skip: &str) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let mut i = from;
    while i + 1 < lines.len() && lines[i].starts_with("NVIDIA") {
        let Some(p) = price(&lines[i + 1]) else { break };
        out.insert(lines[i].clone(), p);
        i += 2;
        if lines.get(i).is_some_and(|l| l == skip) {
            i += 1;
        }
    }
    out
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let html = pages.first().ok_or("no page")?;
    let lines = text::html_lines(html);
    let head = lines
        .iter()
        .position(|l| l == "Pricing Per Hour")
        .ok_or("no on-demand table header")?;
    let after_head = |marker: &str, from: usize| {
        lines[from..]
            .iter()
            .position(|l| l == marker)
            .map(|p| p + from + 1)
    };
    let reserve = after_head("Starting from", head)
        .map(|i| {
            pairs(
                &lines,
                i + usize::from(lines[i] == "Reserve"),
                "Reserve here",
            )
        })
        .unwrap_or_default();
    let spot = lines[head..]
        .windows(2)
        .position(|w| w[0] == "GPU Model" && w[1] == "Spot VM")
        .map(|p| pairs(&lines, head + p + 2, ""))
        .unwrap_or_default();

    let mut out = Vec::new();
    let mut i = head + 1;
    while i + 4 < lines.len() && lines[i].starts_with("NVIDIA") {
        let name = &lines[i];
        let (vram, pcpu, ram) = (
            number(&lines[i + 1]),
            number(&lines[i + 2]),
            number(&lines[i + 3]),
        );
        let usd = price(&lines[i + 4])
            .ok_or_else(|| format!("no price for {name}: {:?}", lines[i + 4]))?;
        if vram.is_none() || pcpu.is_none() || ram.is_none() {
            return Err(format!("unreadable specs for {name}"));
        }
        let per_hour = fx.to_usd(usd);
        out.push(GpuListing {
            external_id: format!("{name}-1x"),
            name: name.clone(),
            gpu_type: gpu::normalize(&hint(name)).unwrap_or_else(|| name.clone()),
            gpu_count: 1,
            price_per_hour: per_hour,
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "vram_gb": vram,
                "max_vcpus_per_gpu": pcpu,
                "max_ram_gb_per_gpu": ram,
                "price_basis": "per GPU",
                "reserved_from_per_gpu_hour": reserve.get(name),
                "spot_per_gpu_hour": spot.get(name),
                "original": fx.original(usd),
            }),
        });
        i += 5;
    }
    if out.is_empty() {
        return Err("no GPU rows under the on-demand header".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
<h2>On-Demand GPU<br> Pricing</h2>
<div class="row"><div>GPU Model </div><div>VRAM (GB)</div><div>Max pCPUs per GPU</div><div>Max RAM (GB) per GPU</div><div>Pricing Per Hour</div></div>
<div class="row"><div><span>NVIDIA H200 SXM</span></div><div>141</div><div>22</div><div>225</div><div>$3.99</div></div>
<div class="row"><div><span>NVIDIA H100 SXM</span></div><div>80</div><div>24</div><div>240</div><div>$3.20</div></div>
<div class="row"><div><span>NVIDIA H100 NVLink</span></div><div>80</div><div>31</div><div>180</div><div>$2.60</div></div>
<div class="row"><div><span>NVIDIA A100</span></div><div>80</div><div>28</div><div>120</div><div>&nbsp;$1.35</div></div>
<h2>Reservation<br> Pricing</h2>
<div>GPU Model </div><div>Starting from&nbsp;</div><div>Reserve</div>
<div>NVIDIA H200 SXM</div><div>&nbsp;$2.79</div><div>Reserve here</div>
<div>NVIDIA H100 SXM</div><div>&nbsp;$2.72</div><div>Reserve here</div>
<h2>Spot VM<br> Pricing</h2>
<div>GPU Model </div><div>Spot VM</div>
<div>NVIDIA A100</div><div>$1.08</div>
<h2>On-Demand CPU Pricing</h2>
"#;

    #[test]
    fn parses_gpu_rows() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4);
        let h100 = l.iter().find(|x| x.name == "NVIDIA H100 SXM").unwrap();
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 3.20);
        assert_eq!(h100.extra["reserved_from_per_gpu_hour"], 2.72);
        assert_eq!(h100.extra["max_vcpus_per_gpu"], 24.0);
        let a100 = l.iter().find(|x| x.name == "NVIDIA A100").unwrap();
        assert_eq!(a100.price_per_hour, 1.35);
        assert_eq!(a100.gpu_type, "A100 PCIe");
        assert_eq!(a100.extra["spot_per_gpu_hour"], 1.08);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_without_the_table() {
        assert!(parse(&["<p>hi</p>".into()], &Fx::usd()).is_err());
    }

    #[test]
    fn errors_on_unreadable_price() {
        let bad = FIXTURE.replace("$3.99", "Contact us");
        assert!(parse(&[bad], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_HYPERSTACK").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
