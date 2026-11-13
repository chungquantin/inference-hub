//! Denvr Dataworks: on-demand GPU node prices from its public pricing page.
//!
//! The first table has one row per node type, eight cells each: platform, GPUs per node,
//! GPU memory, vCPUs, memory, local storage, interconnect and `Price/Hour`. The price is
//! quoted per GPU (`$2.45 / GPU`), so a node costs the GPU count times that. `Reserved only`
//! rows have no on-demand price and are left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;
use crate::adapters::gpu;
use crate::adapters::labs::text::{first_dollar, html_lines};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-denvr",
    name: "Denvr Dataworks",
    organization: "denvr-dataworks",
    origin: "https://www.denvr.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.denvr.com/pricing";
const ROW: usize = 8;

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let head = lines
        .windows(2)
        .position(|w| w[0] == "Interconnect" && w[1] == "Price/Hour")
        .ok_or("no GPU table with an Interconnect and Price/Hour column")?;

    let mut out: Vec<GpuListing> = Vec::new();
    for row in lines[head + 2..].chunks(ROW) {
        let [
            platform,
            gpus,
            vram,
            vcpus,
            memory,
            storage,
            interconnect,
            price,
        ] = row
        else {
            break;
        };
        let reserved_only = price == "Reserved only";
        if !reserved_only && !price.starts_with('$') {
            break; // past the table
        }
        if reserved_only {
            continue;
        }
        let count: i16 = gpus
            .parse()
            .ok()
            .filter(|c| *c > 0)
            .ok_or_else(|| format!("{platform}: GPU count {gpus:?}"))?;
        let per_gpu = first_dollar(price).ok_or_else(|| format!("{platform}: price {price:?}"))?;
        let gpu_type = if platform.contains("MIG") {
            platform.clone() // a slice of an A100, not comparable to a whole one
        } else {
            gpu::normalize(&format!("{platform} {vram}")).unwrap_or_else(|| platform.clone())
        };
        let external_id = format!(
            "{}-{count}x-{}",
            platform.replace(' ', "-"),
            vram.replace(' ', "")
        );
        if out.iter().any(|l| l.external_id == external_id) {
            continue;
        }
        out.push(GpuListing {
            external_id,
            name: format!("{platform} x{count} ({vram})"),
            gpu_type,
            gpu_count: count,
            price_per_hour: fx.to_usd(per_gpu * f64::from(count)),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "per_gpu_hour": fx.to_usd(per_gpu),
                "vram": vram,
                "vcpu": vcpus,
                "ram": memory,
                "storage": storage,
                "interconnect": interconnect,
                "original": fx.original(per_gpu * f64::from(count)),
            }),
        });
    }
    if out.is_empty() {
        return Err("no node type with an on-demand price found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "<div>Platform</div><div>GPUs</div><div>GPU VRAM</div><div>vCPUs</div>
<div>Memory</div><div>Local Storage</div><div>Interconnect</div><div>Price/Hour</div>
<div>NVIDIA H200 SXM</div><div>8</div><div>141 GB</div><div>208</div><div>2048 GB</div>
<div>6x 3.8TB NVMe</div><div>RoCE 3200G</div><div>Reserved only</div>
<div>NVIDIA GH200</div><div>1</div><div>96GB</div><div>72</div><div>480 GB</div>
<div>2x 3.8TB NVMe</div><div>IB 400G</div><div>$3.87 / GPU</div>
<div>NVIDIA H100 SXM</div><div>8</div><div>80 GB</div><div>208</div><div>1024 GB</div>
<div>6x 3.8TB NVMe</div><div>IB 3200G</div><div>$2.45 / GPU</div>
<div>NVIDIA A100 SXM</div><div>8</div><div>40 GB</div><div>128</div><div>1024 GB</div>
<div>4x 3.8TB NVMe</div><div>IB 800G</div><div>$1.15 / GPU</div>
<div>NVIDIA A100 PCIe</div><div>4</div><div>40 GB</div><div>64</div><div>512 GB</div>
<div>2x 3.8TB NVMe</div><div>-</div><div>$1.15 / GPU</div>
<div>NVIDIA A100 MIG</div><div>1</div><div>20 GB</div><div>5</div><div>55 GB</div>
<div>-</div><div>-</div><div>$0.58 / GPU</div>
<div>Intel Gaudi 2</div><div>8</div><div>96 GB</div><div>160</div><div>1024 GB</div>
<div>4x 7.6TB NVMe</div><div>-</div><div>$1.10 / GPU</div>
<div>GPU Instances</div><div>Platform</div>";

    #[test]
    fn parses_nodes_and_skips_reserved_only() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 6, "the reserved-only H200 row is left out");
        let h100 = l.iter().find(|x| x.gpu_type == "H100 SXM").unwrap();
        assert_eq!(h100.gpu_count, 8);
        assert_eq!(h100.price_per_hour, 19.6);
        assert_eq!(h100.extra["per_gpu_hour"], 2.45);
        assert_eq!(h100.external_id, "NVIDIA-H100-SXM-8x-80GB");
        let a40 = l.iter().find(|x| x.gpu_type == "A100 SXM 40GB").unwrap();
        assert_eq!(a40.price_per_hour, 9.2);
        let pcie = l.iter().find(|x| x.gpu_type == "A100 PCIe 40GB").unwrap();
        assert_eq!((pcie.gpu_count, pcie.price_per_hour), (4, 4.6));
        assert_eq!(
            l.iter()
                .find(|x| x.gpu_type == "GH200")
                .unwrap()
                .price_per_hour,
            3.87
        );
        assert!(l.iter().any(|x| x.gpu_type == "Intel Gaudi 2"));
        let mut ids: Vec<_> = l.iter().map(|x| &x.external_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn missing_table_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_DENVR").expect("CLOUD_FIXTURE_DENVR");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} | {} x {} | ${} | {}",
                l.external_id, l.name, l.gpu_count, l.gpu_type, l.price_per_hour, l.extra
            );
        }
    }
}
