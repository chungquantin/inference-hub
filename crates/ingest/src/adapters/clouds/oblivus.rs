//! Oblivus: on-demand GPU prices from its pricing page. Each GPU has a server-rendered
//! modal listing its configurations (`8x GPU - 192 vCPU - 1800GB RAM - 32100GB NVMe`) and
//! one "On-Demand: $X/hr" price that is per GPU (the page says to multiply the hourly price
//! by the number of GPUs), so every configuration is one listing priced per-GPU x count.
//! The reservation price of the same modal goes in `extra`. The loose cards below the
//! modals ("Reservations starting from") are not read.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-oblivus",
    name: "Oblivus",
    organization: "oblivus",
    origin: "https://oblivus.com",
    paths: &["/pricing/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://oblivus.com/pricing/";

/// The text pieces of an HTML fragment, tags removed, trimmed, empties dropped.
fn pieces(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                out.push(std::mem::take(&mut cur));
                in_tag = true;
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => cur.push(c),
            _ => {}
        }
    }
    out.push(cur);
    out.into_iter()
        .map(|s| {
            s.replace("&amp;", "&")
                .replace("&nbsp;", " ")
                .trim()
                .to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// `$4.31/hr` after `label` in `s`, strictly parsed.
fn labelled_price(s: &str, label: &str) -> Result<Option<f64>, String> {
    let Some(rest) = s.strip_prefix(label) else {
        return Ok(None);
    };
    let num = rest
        .trim()
        .strip_prefix('$')
        .and_then(|r| r.strip_suffix("/hr"))
        .ok_or(format!("unreadable price {s:?}"))?;
    num.trim()
        .replace(',', "")
        .parse()
        .map(Some)
        .map_err(|_| format!("unreadable price {s:?}"))
}

/// `(count, vcpus, ram_gb, nvme_gb)` from `8x GPU - 192 vCPU - 1800GB RAM - 32100GB NVMe`.
fn config_line(s: &str) -> Option<(i16, f64, f64, f64)> {
    let mut parts = s.split(" - ");
    let count = parts.next()?.strip_suffix("x GPU")?.trim().parse().ok()?;
    let mut num = |suffix: &str| -> Option<f64> {
        parts
            .next()?
            .trim()
            .strip_suffix(suffix)?
            .trim()
            .parse()
            .ok()
    };
    Some((count, num(" vCPU")?, num("GB RAM")?, num("GB NVMe")?))
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let marker = "class=\"modal fade\" id=\"";
    let mut modals = page.split(marker).skip(1).peekable();
    if modals.peek().is_none() {
        return Err("no GPU modals on the page".into());
    }
    let mut out = Vec::new();
    for modal in modals {
        let id = modal.split('"').next().unwrap_or("");
        let t = pieces(modal);
        let Some(title) = t
            .iter()
            .skip_while(|s| *s != "Close")
            .nth(1)
            .filter(|s| s.as_str() != "Deploy")
        else {
            continue;
        };
        let Some(conf) = t.iter().position(|s| s == "Configurations") else {
            continue;
        };
        let stock: Option<i64> = t
            .iter()
            .position(|s| s == "Available Stock")
            .and_then(|i| t.get(i + 1))
            .and_then(|s| s.parse().ok());
        let vram: Option<f64> = t
            .iter()
            .position(|s| s == "GPU Memory")
            .and_then(|i| t.get(i + 1))
            .and_then(|s| s.split("GB").next()?.trim().parse().ok());
        let mut configs = Vec::new();
        let mut on_demand = None;
        let mut reserved = None;
        for (i, s) in t.iter().enumerate().skip(conf + 1) {
            if let Some(p) = labelled_price(s, "On-Demand:")? {
                on_demand = Some(p);
                reserved = t
                    .get(i + 1)
                    .map(|n| labelled_price(n, "Reservations:"))
                    .transpose()?
                    .flatten();
                break;
            }
            if let Some(c) = config_line(s) {
                configs.push(c);
            }
        }
        let Some(per_gpu) = on_demand else {
            continue;
        };
        if configs.is_empty() {
            return Err(format!("no configurations for {title}"));
        }
        let gpu_type = gpu::normalize(title).unwrap_or_else(|| title.clone());
        for (n, vcpus, ram, nvme) in configs {
            out.push(GpuListing {
                external_id: format!("{id}_x{n}"),
                name: format!("{n}x {title}"),
                gpu_type: gpu_type.clone(),
                gpu_count: n,
                price_per_hour: text::tidy(per_gpu * f64::from(n)),
                region: None,
                source_url: Some(URL.into()),
                extra: serde_json::json!({
                    "price_basis": "per GPU-hour, multiplied by the GPU count",
                    "per_gpu_hour": per_gpu,
                    "reserved_per_gpu_hour": reserved,
                    "vram_gb": vram,
                    "vcpus": vcpus,
                    "ram_gb": ram,
                    "nvme_gb": nvme,
                    "available_stock": stock,
                }),
            });
        }
    }
    if out.is_empty() {
        return Err("no priced GPU configurations on the page".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal(id: &str, title: &str, stock: u32, configs: &[&str], prices: &str) -> String {
        let confs: String = configs
            .iter()
            .map(|c| {
                let n = c.split('x').next().unwrap();
                format!(r#"<tr><td class="modal-table-item">{id}_x{n}</td><td class="modal-table-item">{c}</td></tr>"#)
            })
            .collect();
        format!(
            r#"<div class="modal fade" id="{id}" tabindex="-1"><div class="modal-body"><a class="btn" data-bs-dismiss="modal">Close</a><p class="modal-title mb-3">{title}</p><a href="https://console.oblivus.com/deploy/?term=OnDemand&location=MON1">Deploy</a><table><tbody><tr><td>GPU Memory</td><td>80GB HBM2e</td></tr><tr><td>Available Stock</td><td>{stock}</td></tr><tr><td>Configurations</td><td><table>{confs}</table></td></tr></tbody></table>{prices}</div></div>"#
        )
    }

    fn fixture() -> String {
        [
            modal(
                "H100_SXM5_80GB",
                "H100 80GB SXM5",
                0,
                &["8x GPU - 192 vCPU - 1800GB RAM - 32100GB NVMe"],
                "<p>On-Demand: $4.31/hr</p><p>Reservations: $2.92/hr</p>",
            ),
            modal(
                "A100_PCIE_80GB",
                "A100 80GB PCIe",
                0,
                &[
                    "1x GPU - 28 vCPU - 120GB RAM - 850GB NVMe",
                    "2x GPU - 60 vCPU - 240GB RAM - 1600GB NVMe",
                    "4x GPU - 124 vCPU - 480GB RAM - 3300GB NVMe",
                ],
                "<p>On-Demand: $1.47/hr</p><p>Reservations: $0.95/hr</p>",
            ),
            modal(
                "L40",
                "L40",
                1,
                &["1x GPU - 28 vCPU - 58GB RAM - 825GB NVMe"],
                "<p>Contact us</p>",
            ),
        ]
        .concat()
    }

    #[test]
    fn parses_modals() {
        let l = parse(&[fixture()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4, "the modal without a price is skipped");
        let h100 = l
            .iter()
            .find(|x| x.external_id == "H100_SXM5_80GB_x8")
            .unwrap();
        assert_eq!(h100.gpu_count, 8);
        assert_eq!(h100.price_per_hour, 34.48);
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.extra["reserved_per_gpu_hour"], 2.92);
        assert_eq!(h100.extra["vcpus"], 192.0);
        let a100 = l
            .iter()
            .find(|x| x.external_id == "A100_PCIE_80GB_x4")
            .unwrap();
        assert_eq!(a100.price_per_hour, 5.88);
        assert_eq!(a100.gpu_type, "A100 PCIe");
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_without_modals() {
        assert!(parse(&["<p>hi</p>".into()], &Fx::usd()).is_err());
    }

    #[test]
    fn errors_on_unreadable_price() {
        assert!(parse(&[fixture().replace("$4.31/hr", "$4.3x/hr")], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_OBLIVUS").expect("CLOUD_FIXTURE_OBLIVUS");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.name, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
