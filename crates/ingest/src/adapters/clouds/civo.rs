//! Civo: on-demand GPU prices from its public pricing page.
//!
//! The page has one table per GPU. Each row is a size name, `N x NVIDIA <gpu> - <vram>`, RAM,
//! vCPUs, storage, data transfer, then five hourly/monthly price pairs: on-demand and the
//! 6, 12, 24 and 36 month commitments. `N/A` marks a term that is not offered. The hourly
//! price is for the whole N-GPU configuration.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;
use crate::adapters::gpu;
use crate::adapters::labs::text::{first_dollar, html_lines};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-civo",
    name: "Civo",
    organization: "civo",
    origin: "https://www.civo.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.civo.com/pricing";
const TERMS: [&str; 4] = ["6", "12", "24", "36"];

/// `8 x NVIDIA H100 - 80GB` -> (8, "H100", "80GB"); the memory is empty when not stated.
fn config(line: &str) -> Option<(i16, String, String)> {
    let (count, rest) = line.split_once(" x ")?;
    let count: i16 = count.trim().parse().ok().filter(|c| *c > 0)?;
    let rest = rest.strip_prefix("NVIDIA ")?;
    let (gpu, mem) = rest.split_once(" - ").unwrap_or((rest, ""));
    Some((count, gpu.trim().to_owned(), mem.trim().to_owned()))
}

/// The hourly price of a `$X per hour` line, `None` for `N/A per hour`.
fn hourly(line: &str) -> Result<Option<f64>, String> {
    if !line.ends_with("per hour") {
        return Err(format!("expected an hourly price, found {line:?}"));
    }
    if line.starts_with("N/A") {
        return Ok(None);
    }
    first_dollar(line)
        .map(Some)
        .ok_or_else(|| format!("unparseable price {line:?}"))
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let mut out: Vec<GpuListing> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some((count, gpu_name, mem)) = config(line) else {
            continue;
        };
        let size = i.checked_sub(1).map(|k| lines[k].as_str()).unwrap_or("");
        // RAM, vCPU, storage, data transfer; then 5 x (hourly, monthly).
        let row = lines
            .get(i + 1..i + 15)
            .ok_or("the table ends inside a row")?;
        let (ram, vcpu, storage) = (&row[0], &row[1], &row[2]);
        let mut prices = Vec::new();
        for pair in row[4..].chunks(2) {
            prices.push(hourly(&pair[0])?);
        }
        let Some(on_demand) = prices[0] else {
            continue;
        };
        let mut commitment = serde_json::Map::new();
        for (term, price) in TERMS.iter().zip(&prices[1..]) {
            if let Some(p) = price {
                commitment.insert(format!("{term}_months"), serde_json::json!(p));
            }
        }
        let external_id = if mem.is_empty() {
            format!("{gpu_name}-{count}x")
        } else {
            format!("{gpu_name}-{mem}-{count}x")
        };
        if out.iter().any(|l| l.external_id == external_id) {
            continue;
        }
        let gpu_type = gpu::normalize(&format!("{gpu_name} {mem}")).unwrap_or(gpu_name.clone());
        out.push(GpuListing {
            external_id,
            name: format!("{size} {line}").trim().to_owned(),
            gpu_type,
            gpu_count: count,
            price_per_hour: fx.to_usd(on_demand),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "vram": mem,
                "ram": ram,
                "vcpu": vcpu,
                "storage": storage,
                "commitment_price_per_hour": commitment,
                "original": fx.original(on_demand),
            }),
        });
    }
    if out.is_empty() {
        return Err("no GPU configuration with an on-demand price found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "NVIDIA L40S 48GB GPU pricing
Size
RAM
vCPU
Storage
Data Transfer
On-demand
Commitment
Small
1 x NVIDIA L40S - 48GB
96 GB
12
200GB NVMe
FREE
$1.29 per hour
$941.70 per month
$1.19 per hour
$868.70 per month
$1.09 per hour
$795.70 per month
$0.99 per hour
$722.70 per month
$0.89 per hour
$649.70 per month
Medium
2 x NVIDIA L40S - 48GB
192 GB
24
400GB NVMe
FREE
$2.58 per hour
$1,883.40 per month
$2.38 per hour
$1,737.40 per month
$2.18 per hour
$1,591.40 per month
$1.98 per hour
$1,445.40 per month
$1.78 per hour
$1,299.40 per month
NVIDIA H100 SXM GPU pricing
Size
Small
1 x NVIDIA H100 - 80GB
184 GB
23
200GB NVMe
FREE
$2.99 per hour
$2,182.70 per month
$2.79 per hour
$2,036.70 per month
$2.69 per hour
$1,963.70 per month
$2.59 per hour
$1,890.70 per month
$2.49 per hour
$1,817.70 per month
Extra Large
8 x NVIDIA H100 - 80GB
1024 GB
184
1500GB NVMe
FREE
$23.92 per hour
$17,461.60 per month
$22.32 per hour
$16,293.60 per month
$21.52 per hour
$15,709.60 per month
$20.72 per hour
$15,125.60 per month
$19.92 per hour
$14,541.60 per month
NVIDIA B200 GPU pricing
Extra Large
8 x NVIDIA B200 - 180GB
2.5TB
220 cores
7000GB NVMe
FREE
N/A per hour
N/A per month
N/A per hour
N/A per month
$35.92 per hour
N/A per month
$31.92 per hour
$23,301.60 per month
$30.32 per hour
$22,133.60 per month
";

    #[test]
    fn parses_configurations_and_skips_unpriced() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4, "B200 has no on-demand price");
        let h8 = l.iter().find(|x| x.external_id == "H100-80GB-8x").unwrap();
        assert_eq!(h8.gpu_type, "H100 SXM");
        assert_eq!(h8.gpu_count, 8);
        assert_eq!(h8.price_per_hour, 23.92);
        assert_eq!(h8.extra["commitment_price_per_hour"]["12_months"], 21.52);
        let l2 = l.iter().find(|x| x.external_id == "L40S-48GB-2x").unwrap();
        assert_eq!((l2.gpu_type.as_str(), l2.gpu_count), ("L40S", 2));
        assert_eq!(l2.price_per_hour, 2.58);
        let mut ids: Vec<_> = l.iter().map(|x| &x.external_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn empty_page_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_CIVO").expect("CLOUD_FIXTURE_CIVO");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} | {} x {} | ${} | {}",
                l.external_id, l.name, l.gpu_count, l.gpu_type, l.price_per_hour, l.extra
            );
        }
    }
}
