//! Paperspace (DigitalOcean): on-demand GPU prices from its pricing page. The "On-Demand
//! Compute / Dedicated GPUs" blocks give a price per hour for one GPU and the multi-GPU
//! sizes offered ("Multi-GPU types: 2x 4x"; the page says to multiply price and specs by
//! the GPU count), so each size is one listing.
//!
//! Webflow keeps stale prices in elements hidden with the `hide` class; only the visible
//! price counts. A price marked `*` or `**` is a 3-year commitment price, not on-demand: it
//! goes in `extra`, and the on-demand price is taken from the footnote when it states one
//! ("On-demand pricing for H100 is $5.95/hour"), otherwise the GPU is left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-paperspace",
    name: "Paperspace",
    organization: "paperspace",
    origin: "https://www.paperspace.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.paperspace.com/pricing";

/// The text inside the first element whose opening tag contains `class_attr`, searching
/// from `from`; with the byte offset after its closing tag.
fn element_text<'a>(html: &'a str, class_attr: &str) -> Vec<(bool, &'a str)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = html[at..].find(class_attr) {
        let pos = at + i;
        let tag_start = html[..pos].rfind('<').unwrap_or(0);
        let tag_end = html[pos..].find('>').map_or(html.len(), |e| pos + e);
        let tag = &html[tag_start..tag_end];
        let hidden = tag
            .split_whitespace()
            .any(|w| w.trim_matches('"') == "hide");
        let inner_start = (tag_end + 1).min(html.len());
        let inner_end = html[inner_start..]
            .find('<')
            .map_or(html.len(), |e| inner_start + e);
        out.push((hidden, html[inner_start..inner_end].trim()));
        at = tag_end;
    }
    out
}

/// Text of the `<li>` spec items in the visible spec list of a block.
fn visible_specs(block: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for list in block.split("<ul ").skip(1) {
        let (open, rest) = list.split_once('>').unwrap_or((list, ""));
        if open
            .split_whitespace()
            .any(|w| w.trim_matches('"') == "hide")
        {
            continue;
        }
        let rest = &rest[..rest.find("</ul>").unwrap_or(rest.len())];
        specs.extend(
            element_text(rest, "instance-specs-2")
                .into_iter()
                .map(|(_, t)| t.to_owned()),
        );
        if !specs.is_empty() {
            break;
        }
    }
    specs
}

/// The on-demand price a footnote states for `name`: "On-demand pricing for H100 is $5.95/hour".
fn footnote_price(page: &str, name: &str) -> Option<f64> {
    let key = format!("On-demand pricing for {name} is");
    let i = page.find(&key)?;
    text::first_dollar(&page[i + key.len()..][..40])
}

/// The price text of the first `instance-price-wrap` not hidden with the `hide` class.
fn visible_price(block: &str) -> Option<&str> {
    block.split("instance-price-wrap").skip(1).find_map(|wrap| {
        let (open, rest) = wrap.split_once('>')?;
        if open
            .split_whitespace()
            .any(|w| w.trim_matches('"') == "hide")
        {
            return None;
        }
        element_text(rest, "instance-price-2")
            .first()
            .map(|(_, t)| *t)
    })
}

/// The number in front of `unit` in a spec such as `256 GB RAM` (spaces of any kind ignored).
fn number_before(specs: &[String], unit: &str) -> Option<f64> {
    specs.iter().find_map(|s| {
        let s: String = s
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let (num, _) = s.split_once(unit)?;
        num.parse().ok()
    })
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let from = page
        .find("On-Demand Compute")
        .ok_or("no On-Demand Compute section")?;
    let mut out = Vec::new();
    for block in page[from..].split("instance-type-holder-flex").skip(1) {
        // The dedicated-GPU blocks list their multi-GPU sizes; desktops and CPUs do not.
        let Some(multi) = block.find("Multi-GPU types:") else {
            continue;
        };
        let block = &block[..block
            .find("instance-blocks-flex")
            .unwrap_or(block.len())
            .max(multi)];
        let name = element_text(block, "instance-names")
            .first()
            .map(|(_, t)| t.to_string())
            .ok_or("a GPU block without a name")?;
        let raw = visible_price(block).ok_or(format!("no visible price for {name}"))?;
        let committed = raw.contains('*');
        let shown: f64 = raw
            .trim_end_matches('*')
            .parse()
            .map_err(|_| format!("unreadable price {raw:?} for {name}"))?;
        let (price, commitment) = if committed {
            match footnote_price(page, &name) {
                Some(p) => (p, Some(shown)),
                None => continue,
            }
        } else {
            (shown, None)
        };
        let specs = visible_specs(block);
        let gpu_label = specs
            .iter()
            .find(|s| s.to_ascii_uppercase().ends_with("GPU"))
            .cloned()
            .unwrap_or_default();
        let gpu_type = gpu::normalize(&gpu_label)
            .or_else(|| gpu::normalize(&name))
            .unwrap_or_else(|| name.clone());
        let multi_text = block[multi..].split("</div></div>").next().unwrap_or("");
        let mut counts = vec![1i16];
        counts.extend(
            text::html_lines(multi_text)
                .iter()
                .flat_map(|l| l.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
                .filter_map(|w| w.strip_suffix('x')?.parse::<i16>().ok())
                .filter(|n| *n > 1),
        );
        let vcpu = number_before(&specs, "vcpu");
        let ram = number_before(&specs, "gbram");
        for n in counts {
            out.push(GpuListing {
                external_id: format!("{name}-{n}x"),
                name: if n == 1 {
                    name.clone()
                } else {
                    format!("{name} x{n}")
                },
                gpu_type: gpu_type.clone(),
                gpu_count: n,
                price_per_hour: text::tidy(price * f64::from(n)),
                region: None,
                source_url: Some(URL.into()),
                extra: serde_json::json!({
                    "price_basis": "per GPU-hour, multiplied by the GPU count",
                    "vcpus": vcpu.map(|v| v * f64::from(n)),
                    "ram_gb": ram.map(|v| v * f64::from(n)),
                    "commitment_3yr_per_gpu_hour": commitment,
                }),
            });
        }
    }
    if out.is_empty() {
        return Err("no on-demand GPU prices on the page".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<h4>On-Demand Compute</h4><div class="pricing-heading-cuda-wrapper"><h5>Dedicated GPUs</h5></div><div class="instance-container-flex"><div class="instance-blocks-flex"><div class="instance-type-holder-flex"><h1 class="instance-names">H100</h1><div class="instance-price-wrap hide"><div class="text-block-13 hide">$</div><h1 class="instance-price-2">3.09</h1><div class="text-block-22">/ hour</div></div><ul role="list" class="instance-specs-list-2 hide"><li><p class="instance-specs-2">NVIDIA A100 GPU</p></li><li><p class="instance-specs-2">90GB RAM</p></li><li><p class="instance-specs-2">12 vCPU</p></li></ul><div class="instance-price-wrap"><div class="text-block-13">$</div><h1 class="instance-price-2">2.24*</h1><div class="text-block-22">/ hour</div></div><ul role="list" class="instance-specs-list-2"><li><p class="instance-specs-2">NVIDIA HGX H100 GPU</p></li><li><p class="instance-specs-2">256 GB RAM</p></li><li><p class="instance-specs-2">20 vCPU</p></li></ul><div class="pricing-multigpu-wrap"><div class="pricing-multigpu-title">Multi-GPU types:</div><div class="pricing-multigpu-info">8x</div></div></div><a href="https://console.paperspace.com/signup?core=true" class="button-2">Create</a></div><div class="instance-blocks-flex"><div class="instance-type-holder-flex"><h1 class="instance-names">A100-80G</h1><div class="instance-price-wrap"><div class="text-block-13">$</div><h1 class="instance-price-2">1.15**</h1><div class="text-block-22">/ hour</div></div><ul role="list" class="instance-specs-list-2"><li><p class="instance-specs-2">NVIDIA A100 GPU</p></li><li><p class="instance-specs-2">90GB RAM</p></li><li><p class="instance-specs-2">12 vCPU</p></li></ul><div class="pricing-multigpu-wrap"><div class="pricing-multigpu-title">Multi-GPU types:</div><div class="pricing-multigpu-info">8x</div></div></div><a href="x" class="button-2">Create</a></div><div class="instance-blocks-flex"><div class="instance-type-holder-flex"><h1 class="instance-names">A4000</h1><div class="instance-price-wrap"><div class="text-block-13">$</div><h1 class="instance-price-2">0.76</h1><div class="text-block-22">/ hour</div></div><ul role="list" class="instance-specs-list-2"><li><p class="instance-specs-2">NVIDIA A4000 GPU</p></li><li><p class="instance-specs-2">45GB RAM</p></li><li><p class="instance-specs-2">8 vCPU</p></li></ul><div class="pricing-multigpu-wrap"><div class="pricing-multigpu-title">Multi-GPU types:</div><div class="pricing-multigpu-info">2x 4x</div></div></div><a href="x" class="button-2">Create</a></div><div class="instance-blocks-flex"><div class="instance-type-holder-flex"><h1 class="instance-names">M4000</h1><div class="instance-price-wrap"><div class="text-block-13">$</div><h1 class="instance-price-2">0.45</h1><div class="text-block-22">/ hour</div></div><ul role="list" class="instance-specs-list-2"><li><p class="instance-specs-2">8 GB GPU</p></li><li><p class="instance-specs-2">30 GB RAM</p></li><li><p class="instance-specs-2">8 vCPU</p></li></ul><div class="pricing-multigpu-wrap"><div class="pricing-multigpu-title">Multi-GPU types:</div><div class="pricing-multigpu-info">none</div></div></div><a href="x" class="button-2">Create</a></div></div><p>*$2.24/hour pricing is for a 3-year commitment. On-demand pricing for H100 is $5.95/hour under our special promo price.**$1.15/hour pricing is for a 3-year commitment.</p>"#;

    #[test]
    fn parses_on_demand_blocks() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        let find = |id: &str| l.iter().find(|x| x.external_id == id).unwrap();
        // H100: the visible 2.24 is a commitment price; on-demand comes from the footnote.
        let h100 = find("H100-1x");
        assert_eq!(h100.price_per_hour, 5.95);
        assert_eq!(h100.extra["commitment_3yr_per_gpu_hour"], 2.24);
        assert_eq!(h100.extra["vcpus"], 20.0);
        let h100x8 = find("H100-8x");
        assert_eq!(h100x8.gpu_count, 8);
        assert_eq!(h100x8.price_per_hour, 47.6);
        assert!(h100x8.gpu_type.starts_with("H100"));
        // A100-80G has only a commitment price: left out.
        assert!(l.iter().all(|x| !x.external_id.starts_with("A100-80G")));
        let a4000x4 = find("A4000-4x");
        assert_eq!(a4000x4.price_per_hour, 3.04);
        assert_eq!(a4000x4.gpu_type, "RTX A4000");
        assert_eq!(l.iter().filter(|x| x.name.starts_with("M4000")).count(), 1);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_without_the_section() {
        assert!(parse(&["<p>hi</p>".into()], &Fx::usd()).is_err());
    }

    #[test]
    fn errors_on_unreadable_price() {
        let bad = FIXTURE.replace(">0.76<", ">tbd<");
        assert!(parse(&[bad], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_PAPERSPACE").expect("CLOUD_FIXTURE_PAPERSPACE");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.name, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
