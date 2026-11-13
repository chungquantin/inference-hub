//! Gcore: GPU prices in EUR (excluding VAT) from the AI pricing page: the 8-GPU bare-metal
//! nodes, one price per node and column. Columns marked "Contact sales" have no price and
//! are left out. The page's Everywhere Inference and Virtual GPU tables are not GPU nodes you
//! rent, so they are not read.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

const URL: &str = "https://gcore.com/pricing/ai";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-gcore",
    name: "Gcore",
    organization: "gcore",
    origin: "https://gcore.com",
    paths: &["/pricing/ai"],
    currency: "EUR",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

/// `€1,250.50` -> 1250.5
fn euros(s: &str) -> Option<f64> {
    s.trim()
        .strip_prefix('€')?
        .replace(',', "")
        .trim()
        .parse()
        .ok()
        .filter(|p: &f64| *p > 0.0)
}

fn number(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.replace(',', "").parse().ok()
}

/// `8×H100 SXM` -> (8, "H100 SXM")
fn config(line: &str) -> Option<(i16, String)> {
    let (n, gpu) = line.split_once('×')?;
    let n: i16 = n.trim().parse().ok().filter(|n| *n > 0)?;
    let gpu = gpu.trim();
    (!gpu.is_empty()).then(|| (n, gpu.to_owned()))
}

#[allow(clippy::too_many_arguments)]
fn listing(
    fx: &Fx,
    offer: &str,
    slug: &str,
    count: i16,
    gpu_name: &str,
    native: f64,
    vram: Option<f64>,
    vcpu: Option<f64>,
    ram: Option<f64>,
) -> GpuListing {
    let model = gpu_name.split_whitespace().next().unwrap_or(gpu_name);
    GpuListing {
        external_id: format!("{slug}-{model}-{count}x"),
        name: format!("{offer} {count}×{gpu_name}"),
        gpu_type: gpu::normalize(gpu_name).unwrap_or_else(|| gpu_name.to_owned()),
        gpu_count: count,
        price_per_hour: fx.to_usd(native),
        region: None,
        source_url: Some(URL.into()),
        extra: serde_json::json!({
            "offer": offer,
            "vram_gb": vram,
            "vcpu": vcpu,
            "ram_gb": ram,
            "excludes_vat": true,
            "original": fx.original(native),
        }),
    }
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("gcore: no page")?;
    let lines = text::html_lines(&page.replace("&#160;", " ").replace('\u{a0}', " "));
    let find = |from: usize, s: &str| lines[from..].iter().position(|l| l == s).map(|p| p + from);
    let mut out = Vec::new();

    // Bare metal: a price row (one price per column, "Contact sales" for none), then a
    // configuration row and VRAM, RAM and vCPU rows, one value per column.
    let price = find(0, "Price").ok_or("gcore: the bare-metal price row is missing")?;
    let conf =
        find(price, "Configuration").ok_or("gcore: the bare-metal configuration is missing")?;
    let mut prices: Vec<Option<f64>> = Vec::new();
    for l in &lines[price + 1..conf] {
        if l.starts_with('€') && l.ends_with("/hr") {
            let native = euros(l.trim_end_matches("/hr"))
                .ok_or_else(|| format!("gcore: price `{l}` does not parse"))?;
            prices.push(Some(native));
        } else if l.eq_ignore_ascii_case("contact sales") {
            prices.push(None);
        }
    }
    let configs: Vec<(i16, String)> = lines[conf..]
        .iter()
        .skip_while(|l| config(l).is_none())
        .map_while(|l| config(l))
        .collect();
    let column = |head: &str| -> Vec<Option<f64>> {
        find(conf, head)
            .map(|i| {
                lines[i + 1..]
                    .iter()
                    .take(configs.len())
                    .map(|l| number(l))
                    .collect()
            })
            .unwrap_or_default()
    };
    let (vram, ram, vcpu) = (column("VRAM, GB"), column("RAM, GB"), column("vCPUs"));
    if prices.len() != configs.len() || configs.is_empty() {
        return Err(format!(
            "gcore: {} bare-metal prices for {} configurations",
            prices.len(),
            configs.len()
        ));
    }
    for (k, ((count, gpu_name), native)) in configs.iter().zip(&prices).enumerate() {
        let Some(native) = native else { continue };
        let at = |v: &[Option<f64>]| v.get(k).copied().flatten();
        out.push(listing(
            fx,
            "Bare Metal",
            "baremetal",
            *count,
            gpu_name,
            *native,
            at(&vram),
            at(&vcpu),
            at(&ram),
        ));
    }

    // The page also lists Everywhere Inference (managed inference by GPU count) and Virtual GPU
    // (shares of a card). Neither is a GPU node you rent, so they are not listed here.
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx() -> Fx {
        Fx {
            currency: "EUR",
            rate: 1.1,
            date: "2026-11-12".into(),
        }
    }

    const FIXTURE: &str = r#"<div><h2>GPU Cloud prices and configuration</h2>
<div class="n">H100</div><a>Get started</a><div class="n">A100</div><a>Get started</a><div class="n">GB300</div><a>Contact sales</a>
<div>Price</div>
<div><span>€21.82</span> <span>/hr</span></div><div>€2.73 per GPU</div>
<div><span>€10.91</span> <span>/hr</span></div><div>€1.36 per GPU</div>
<div>Contact sales</div>
<div>Configuration</div><div>GPU</div>
<div>8×H100 SXM</div><div>8×A100 SXM</div><div>72×GB300 NVL72</div>
<div>VRAM, GB</div><div>640</div><div>640</div><div>20,736</div>
<div>RAM, GB</div><div>2,048</div><div>2,048</div><div>17,280</div>
<div>vCPUs</div><div>224</div><div>192</div><div>2,592</div>
<p>Prices do not include VAT</p>
<h3>Everywhere Inference</h3><p>Billing is determined by the exact compute time you use.</p>
<div>GPU Configuration</div><div>GPU Memory</div><div>CPU/RAM</div><div>Cost per hour</div>
<div>1×L40S</div><div>48&#160;GB</div><div>16&#160;vCPU / 232&#160;GiB RAM</div><div>€1.00</div><a>Get started</a>
<div>2×L40S</div><div>96&#160;GB</div><div>32&#160;vCPU / 464&#160;GiB RAM</div><div>€2.00</div><a>Get started</a>
<h3>Virtual GPU</h3><p>Start with just one GPU.</p>
<div>1×H100</div><div>80&#160;GB</div><div>24&#160;vCPU / 232&#160;GB RAM</div><div>1250&#160;GB</div><div>€2.82</div><a>Get started</a>
<p>Prices do not include VAT.</p></div>"#;

    #[test]
    fn reads_only_the_bare_metal_nodes_in_usd() {
        let l = parse(&[FIXTURE.into()], &fx()).unwrap();
        assert_eq!(l.len(), 2, "{l:#?}");
        let by = |id: &str| l.iter().find(|x| x.external_id == id).unwrap();
        let h = by("baremetal-H100-8x");
        assert_eq!((h.gpu_type.as_str(), h.gpu_count), ("H100 SXM", 8));
        assert_eq!(h.price_per_hour, 24.002);
        assert_eq!(h.extra["original"]["price_per_hour"], 21.82);
        assert_eq!(h.extra["vram_gb"], 640.0);
        assert_eq!(h.extra["vcpu"], 224.0);
        assert_eq!(by("baremetal-A100-8x").price_per_hour, 12.001);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn missing_structure_is_an_error() {
        assert!(parse(&["<html></html>".into()], &fx()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_GCORE").expect("CLOUD_FIXTURE_GCORE");
        let page = std::fs::read_to_string(path).unwrap();
        let fx = Fx {
            currency: "EUR",
            rate: 1.15,
            date: "live".into(),
        };
        for l in parse(&[page], &fx).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
