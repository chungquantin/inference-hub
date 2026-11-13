//! Northflank: on-demand GPU prices from its pricing page ("GPUs on Northflank Cloud"
//! table). A row gives the GPU, its VRAM, the GPU counts a container can take (`1× 2× 4× 8×`,
//! or only `8×` for the HGX parts) and a price "/ hour" that is per GPU (the page does not
//! say so; $2.74 for an H100 is only plausible per GPU, and the 1x-8x rows scale). Each
//! offered count is one listing priced per-GPU price x count. BYOC pricing (per GB VRAM
//! per month, on the customer's own cloud) is not a GPU rental price and is left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-northflank",
    name: "Northflank",
    organization: "northflank",
    origin: "https://northflank.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://northflank.com/pricing";

/// The text pieces of an HTML fragment, tags and comments removed, trimmed, empties dropped.
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

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let from = page
        .find("GPUs on Northflank Cloud")
        .ok_or("no GPUs on Northflank Cloud section")?;
    let section = &page[from..];
    let section = &section[..section
        .find("GPUs on Northflank BYOC")
        .unwrap_or(section.len())];
    let (head, body) = section
        .split_once("<tbody")
        .ok_or("no GPU price table body")?;
    let head = pieces(head).join(" ");
    if !head.contains("GPUs per container") || !head.contains("Per hour") {
        return Err("the GPU table lacks its GPUs-per-container / Per hour columns".into());
    }
    let mut out = Vec::new();
    for row in body.split("<tr").skip(1) {
        let row = &row[..row.find("</tr>").unwrap_or(row.len())];
        let t = pieces(row.split_once('>').map_or(row, |(_, r)| r));
        let Some(vendor) = t.iter().position(|s| s == "NVIDIA") else {
            continue;
        };
        let model = t.get(vendor + 1).ok_or("a GPU row without a model")?;
        let vram: Option<i64> = t.get(vendor + 2).and_then(|s| s.parse().ok());
        let counts: Vec<i16> = t
            .windows(2)
            .filter(|w| w[1] == "×")
            .filter_map(|w| w[0].parse().ok())
            .collect();
        let price_cell = t
            .iter()
            .find(|s| s.starts_with('$'))
            .ok_or(format!("no price for {model}"))?;
        let per_gpu: f64 = price_cell[1..]
            .replace(',', "")
            .parse()
            .map_err(|_| format!("unreadable price {price_cell:?} for {model}"))?;
        if counts.is_empty() {
            return Err(format!("no GPU counts for {model}"));
        }
        let label = match vram {
            Some(v) => format!("{model} {v}GB"),
            None => model.clone(),
        };
        let gpu_type = gpu::normalize(&format!("NVIDIA {label}"))
            .or_else(|| gpu::normalize(model))
            .unwrap_or_else(|| format!("NVIDIA {model}"));
        for n in counts {
            out.push(GpuListing {
                external_id: format!("{}-{n}x", label.replace(' ', "-")),
                name: format!("NVIDIA {label} x{n}"),
                gpu_type: gpu_type.clone(),
                gpu_count: n,
                price_per_hour: text::tidy(per_gpu * f64::from(n)),
                region: None,
                source_url: Some(URL.into()),
                extra: serde_json::json!({
                    "price_basis": "per GPU-hour, multiplied by the GPU count",
                    "per_gpu_hour": per_gpu,
                    "vram_gb": vram,
                }),
            });
        }
    }
    if out.is_empty() {
        return Err("the GPU table holds no priced configurations".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(model: &str, vram: u32, counts: &[u32], price: &str) -> String {
        let spans: String = counts
            .iter()
            .map(|c| format!("<span>{c}<!-- -->×</span>"))
            .collect();
        format!(
            r#"<tr><th scope="row"><span class="Cf"><span class="If" data-vendor="nvidia"><div class="tt"><svg viewBox="0 0 24 24"><path d="M8 8"></path></svg></div>NVIDIA</span>{model}</span><span class="Df">{vram}<!-- --> GB VRAM</span></th><td class="Af">{vram}<!-- --> GB</td><td class="Af"><span class="Yf">{spans}</span></td><td class="Yr Xf">{price}<span class="uf">/ hour</span></td></tr>"#
        )
    }

    fn fixture() -> String {
        let rows = [
            row("L4", 24, &[1, 2, 4, 8], "$0.80"),
            row("A100", 80, &[1, 2, 4, 8], "$1.76"),
            row("H100", 80, &[8], "$2.74"),
            row("B200", 180, &[8], "Contact us"),
        ]
        .concat();
        format!(
            r#"<div><h3>GPUs on Northflank Cloud</h3><p>On demand, billed by the second.</p><table class="d"><thead><tr><th scope="col">GPU</th><th scope="col">VRAM</th><th scope="col">GPUs per container</th><th scope="col">Per hour</th></tr></thead><tbody>{rows}</tbody></table></div><div><h3>GPUs on Northflank BYOC</h3><p>$1.00<span>/ GB VRAM / month</span></p></div>"#
        )
    }

    #[test]
    fn parses_gpu_rows() {
        let page = fixture().replace("Contact us", "$5.87");
        let l = parse(&[page], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4 + 4 + 1 + 1);
        let a100x4 = l.iter().find(|x| x.external_id == "A100-80GB-4x").unwrap();
        assert_eq!(a100x4.gpu_count, 4);
        assert_eq!(a100x4.price_per_hour, 7.04);
        assert!(a100x4.gpu_type.starts_with("A100"));
        let h100 = l.iter().find(|x| x.external_id == "H100-80GB-8x").unwrap();
        assert_eq!(h100.price_per_hour, 21.92);
        assert!(h100.gpu_type.starts_with("H100"));
        assert_eq!(h100.extra["vram_gb"], 80);
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
        assert!(parse(&[fixture().replace("$0.80", "$0.8x")], &Fx::usd()).is_err());
        assert!(
            parse(&[fixture()], &Fx::usd()).is_err(),
            "a row without a price"
        );
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_NORTHFLANK").expect("CLOUD_FIXTURE_NORTHFLANK");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.name, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
