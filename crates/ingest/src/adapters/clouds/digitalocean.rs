//! DigitalOcean: on-demand GPU Droplet prices from its public pricing page.
//!
//! The page has three card groups (12-month reserved, spot, on-demand). Every card reads
//! `<GPU name>`, `$X /GPU/hour`, a label (`On-Demand Price`, `12 Month Reserved Price*`,
//! `Current Spot Price*`), and, for on-demand cards, `GPUs per Droplet` (`Choose between 1 or
//! 8 GPU's per droplet`, or `1`), GPU memory, Droplet memory and vCPUs. The price is per
//! GPU-hour; a Droplet of N GPUs costs N times that. The Droplet memory and vCPUs shown are
//! those of the single-GPU Droplet, so they are only recorded on the 1-GPU listing.

use std::collections::HashMap;

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;
use crate::adapters::gpu;
use crate::adapters::labs::text::{first_dollar, html_lines};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-digitalocean",
    name: "DigitalOcean",
    organization: "digitalocean",
    origin: "https://www.digitalocean.com",
    paths: &["/pricing/gpu-droplets"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.digitalocean.com/pricing/gpu-droplets";

fn value<'a>(lines: &'a [String], key: &str) -> Option<&'a str> {
    lines
        .iter()
        .find_map(|l| l.strip_prefix(key))
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let mut reserved: HashMap<String, f64> = HashMap::new();
    let mut spot: HashMap<String, f64> = HashMap::new();
    let mut cards = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !line.ends_with("/GPU/hour") || !line.starts_with('$') || i == 0 {
            continue;
        }
        let price = first_dollar(line).ok_or_else(|| format!("unparseable price {line:?}"))?;
        let name = lines[i - 1].replace(['™', '®'], "");
        let label = lines.get(i + 1).map(String::as_str).unwrap_or("");
        if label.starts_with("12 Month Reserved") {
            reserved.insert(name, price);
        } else if label.starts_with("Current Spot") {
            spot.insert(name, price);
        } else if label.starts_with("On-Demand") {
            let end = lines[i..]
                .iter()
                .position(|l| l == "Get started")
                .map_or(lines.len(), |p| i + p);
            cards.push((name, price, &lines[i + 2..end]));
        }
    }
    if cards.is_empty() {
        return Err("no on-demand GPU Droplet cards found".into());
    }

    let mut out = Vec::new();
    for (name, price, body) in cards {
        // The count is on the label's line (`GPUs per Droplet 1`) or the line after it.
        let counts: Vec<i16> = body
            .iter()
            .position(|l| l.starts_with("GPUs per Droplet"))
            .map(|k| {
                let rest = body[k].trim_start_matches("GPUs per Droplet");
                let text = if rest.trim().is_empty() {
                    body.get(k + 1).map_or("", String::as_str)
                } else {
                    rest
                };
                text.split(|c: char| !c.is_ascii_digit())
                    .filter_map(|t| t.parse::<i16>().ok())
                    .filter(|c| *c > 0)
                    .collect()
            })
            .unwrap_or_default();
        if counts.is_empty() {
            return Err(format!("{name}: no GPUs per Droplet"));
        }
        let gpu_type = gpu::normalize(&name).unwrap_or_else(|| name.clone());
        let slug = gpu_type.replace(' ', "-");
        for count in counts {
            let single = count == 1;
            out.push(GpuListing {
                external_id: format!("{slug}-{count}x"),
                name: format!("{name} x{count}"),
                gpu_type: gpu_type.clone(),
                gpu_count: count,
                price_per_hour: fx.to_usd(price * f64::from(count)),
                region: None,
                source_url: Some(URL.into()),
                extra: serde_json::json!({
                    "per_gpu_hour": fx.to_usd(price),
                    "vram": value(body, "GPU Memory"),
                    "ram": single.then(|| value(body, "Droplet Memory")).flatten(),
                    "vcpu": single.then(|| value(body, "Droplet vCPUs")).flatten(),
                    "reserved_12_month_per_gpu_hour": reserved.get(&name),
                    "spot_per_gpu_hour": spot.get(&name),
                    "original": fx.original(price * f64::from(count)),
                }),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "<div>12 Month Reserved Plans</div>
<div>NVIDIA HGX H100</div><div>$3.26 /GPU/hour</div><div>12 Month Reserved Price*</div><div>Contact sales</div>
<div>NVIDIA HGX B300</div><div>$6.17 /GPU/hour</div><div>Current Spot Price*</div><div>Get started</div>
<div>On-Demand Plans</div>
<div>AMD Instinct™ MI300X</div><div>$2.59 /GPU/hour</div><div>On-Demand Price</div>
<div>GPUs per Droplet</div><div>Choose between 1 or 8 GPU's per droplet</div>
<div>GPU Memory 192 GB</div><div>Droplet Memory 240 GiB</div><div>Droplet vCPUs 20</div>
<div>Boot Disk 720 GiB NVMe</div><div>Get started</div>
<div>NVIDIA HGX H100</div><div>$4.41 /GPU/hour</div><div>On-Demand Price</div>
<div>GPUs per Droplet</div><div>Choose between 1 or 8 GPU's per droplet</div>
<div>GPU Memory 80 GB</div><div>Droplet Memory 240 GiB</div><div>Droplet vCPUs 20</div>
<div>Get started</div>
<div>NVIDIA L40S</div><div>$1.57 /GPU/hour</div><div>On-Demand Price</div>
<div>GPUs per Droplet 1</div><div>GPU Memory 48 GB</div><div>Droplet Memory 64 GiB</div>
<div>Droplet vCPUs 8</div><div>Get started</div>
<div>NVIDIA HGX B300</div><div>Contact sales</div>";

    #[test]
    fn parses_on_demand_cards_and_skips_others() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 5, "MI300X 1x/8x, H100 1x/8x, L40S 1x");
        let h8 = l.iter().find(|x| x.external_id == "H100-SXM-8x").unwrap();
        assert_eq!(h8.gpu_type, "H100 SXM");
        assert_eq!(h8.gpu_count, 8);
        assert_eq!(h8.price_per_hour, 35.28);
        assert_eq!(h8.extra["reserved_12_month_per_gpu_hour"], 3.26);
        assert!(h8.extra["vcpu"].is_null());
        let h1 = l.iter().find(|x| x.external_id == "H100-SXM-1x").unwrap();
        assert_eq!(h1.price_per_hour, 4.41);
        assert_eq!(h1.extra["vcpu"], "20");
        let l40 = l.iter().find(|x| x.gpu_type == "L40S").unwrap();
        assert_eq!((l40.gpu_count, l40.price_per_hour), (1, 1.57));
        let mi = l
            .iter()
            .find(|x| x.external_id.ends_with("MI300X-1x"))
            .unwrap();
        assert_eq!(mi.gpu_type, "AMD MI300X");
        let mut ids: Vec<_> = l.iter().map(|x| &x.external_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn page_without_cards_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_DIGITALOCEAN").expect("CLOUD_FIXTURE_DIGITALOCEAN");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} | {} x {} | ${} | {}",
                l.external_id, l.name, l.gpu_count, l.gpu_type, l.price_per_hour, l.extra
            );
        }
    }
}
