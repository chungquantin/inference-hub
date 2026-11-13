//! E2E Networks: on-demand GPU prices from its public pricing page.
//!
//! The NVIDIA GPU table (USD) has one row per GPU: name, vRAM (GB), vCPUs, RAM (GB),
//! `Hourly/On-Demand` and `Monthly/Annual` (`Contact Sales`). The page has 1x/2x/4x/8x
//! tabs but only the 1x rows are in the HTML, and it states the prices are per GPU-hour, so
//! each listing is a single GPU.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;
use crate::adapters::gpu;
use crate::adapters::labs::text::{first_dollar, html_lines};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-e2e",
    name: "E2E Networks",
    organization: "e2e-networks",
    origin: "https://www.e2enetworks.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.e2enetworks.com/pricing";
const ROW: usize = 6;

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let head = lines
        .windows(3)
        .position(|w| w[0] == "RAM, GB" && w[1] == "Hourly/On-Demand" && w[2] == "Monthly/Annual")
        .ok_or("no GPU table with a Hourly/On-Demand column")?;

    let mut out: Vec<GpuListing> = Vec::new();
    for row in lines[head + 3..].chunks(ROW) {
        let [name, vram, vcpus, ram, price, _monthly] = row else {
            break;
        };
        if !name.starts_with("NVIDIA ") || !price.starts_with('$') {
            break;
        }
        let per_gpu = first_dollar(price).ok_or_else(|| format!("{name}: price {price:?}"))?;
        let gpu_type = gpu::normalize(&format!("{name} {vram}")).unwrap_or_else(|| name.clone());
        let external_id = format!(
            "{}-{vram}GB-1x",
            name.trim_start_matches("NVIDIA ").replace(' ', "-")
        );
        if out.iter().any(|l| l.external_id == external_id) {
            return Err(format!("duplicate configuration {external_id}"));
        }
        out.push(GpuListing {
            external_id,
            name: format!("{name} {vram}GB"),
            gpu_type,
            gpu_count: 1,
            price_per_hour: fx.to_usd(per_gpu),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "billing": "per GPU-hour",
                "vram": format!("{vram} GB"),
                "vcpu": vcpus,
                "ram": format!("{ram} GB"),
                "original": fx.original(per_gpu),
            }),
        });
    }
    if out.is_empty() {
        return Err("no GPU row with an on-demand price found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "<h2>NVIDIA GPU</h2><button>1x</button><button>2x</button>
<table><thead><tr><th>GPU</th><th>vRAM</th><th>vCPUs</th><th>RAM, GB</th><th>Hourly/On-Demand</th><th>Monthly/Annual</th></tr></thead><tbody>
<tr><td><span>NVIDIA B200</span></td><td>180</td><td>32</td><td>400</td><td><span>$7.99</span></td><td><a>Contact Sales</a></td></tr>
<tr><td><span>NVIDIA H100</span></td><td>80</td><td>26</td><td>250</td><td><span>$3.99</span></td><td><a>Contact Sales</a></td></tr>
<tr><td><span>NVIDIA RTX PRO 6000</span></td><td>96</td><td>32</td><td>170</td><td><span>$2.49</span></td><td><a>Contact Sales</a></td></tr>
<tr><td><span>NVIDIA A100</span></td><td>80</td><td>16</td><td>115</td><td><span>$2.10</span></td><td><a>Contact Sales</a></td></tr>
<tr><td><span>NVIDIA A100</span></td><td>40</td><td>16</td><td>115</td><td><span>$1.98</span></td><td><a>Contact Sales</a></td></tr>
<tr><td><span>NVIDIA L4</span></td><td>24</td><td>25</td><td>110</td><td><span>$0.57</span></td><td><a>Contact Sales</a></td></tr>
</tbody></table>
<p>On-demand prices are per GPU-hour and do not include taxes.</p>
<h2>CPU instances</h2><table><thead><tr><th>Plan</th></tr></thead></table>";

    #[test]
    fn parses_gpu_rows() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 6);
        let by = |id: &str| l.iter().find(|x| x.external_id == id).unwrap();
        let h = by("H100-80GB-1x");
        assert_eq!(
            (h.gpu_type.as_str(), h.gpu_count, h.price_per_hour),
            ("H100 SXM", 1, 3.99)
        );
        assert_eq!(by("B200-180GB-1x").price_per_hour, 7.99);
        assert_eq!(by("RTX-PRO-6000-96GB-1x").gpu_type, "RTX PRO 6000");
        assert_eq!(by("A100-80GB-1x").gpu_type, "A100 SXM");
        assert_eq!(by("A100-40GB-1x").gpu_type, "A100 SXM 40GB");
        assert_eq!(by("L4-24GB-1x").price_per_hour, 0.57);
        assert_eq!(h.extra["vcpu"], "26");
    }

    #[test]
    fn stops_at_the_end_of_the_gpu_table() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert!(l.iter().all(|x| x.price_per_hour >= 0.5));
    }

    #[test]
    fn page_without_table_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_E2E").expect("CLOUD_FIXTURE_E2E");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} | {} x {} | ${} | {}",
                l.external_id, l.name, l.gpu_count, l.gpu_type, l.price_per_hour, l.extra
            );
        }
    }
}
