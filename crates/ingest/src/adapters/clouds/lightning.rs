//! Lightning AI: the GPU panels of the pricing page's machine price card. One panel per
//! GPUs-per-machine count (1, 2, 4, 8); each row is a GPU with its VRAM and a price per GPU
//! hour (the same GPU costs the same in every panel except where the page shows another
//! provider's rate), so the configuration price is that times the count. CPU, TPU and
//! cluster panels are left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

const URL: &str = "https://lightning.ai/pricing";
const COUNTS: [i16; 4] = [1, 2, 4, 8];

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-lightning",
    name: "Lightning AI",
    organization: "lightning-ai",
    origin: "https://lightning.ai",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

/// Inner text of the first `<span class="{class}"...>` in `s`, nested tags removed.
fn span(s: &str, class: &str) -> Option<String> {
    let key = format!("<span class=\"{class}\"");
    let rest = &s[s.find(&key)?..];
    let rest = &rest[rest.find('>')? + 1..];
    let mut depth = 1;
    let mut out = String::new();
    let mut in_tag = false;
    let mut i = 0;
    let b = rest.as_bytes();
    while i < b.len() {
        if rest[i..].starts_with("</span>") {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if rest[i..].starts_with("<span") {
            depth += 1;
        }
        match b[i] {
            b'<' => in_tag = true,
            b'>' => in_tag = false,
            _ if !in_tag => out.push(b[i] as char),
            _ => {}
        }
        i += 1;
    }
    Some(out.trim().to_owned())
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let mut out = Vec::new();
    let mut panels = 0;
    for panel in page.split("<ul class=\"panel\"").skip(1) {
        let head = &panel[..panel.find('>').unwrap_or(0)];
        if !head.contains("data-panel-kind=\"gpu\"") {
            continue;
        }
        let Some(&count) = COUNTS.get(panels) else {
            return Err("more GPU panels than machine sizes".into());
        };
        panels += 1;
        let body = &panel[..panel.find("</ul>").unwrap_or(panel.len())];
        for li in body.split("<li").skip(1) {
            let (Some(raw), Some(spec), Some(price)) =
                (span(li, "name"), span(li, "spec"), span(li, "price"))
            else {
                continue;
            };
            let Some(per_gpu) = price
                .strip_prefix('$')
                .and_then(|_| text::first_dollar(&price))
            else {
                continue;
            };
            let vram = spec.trim_end_matches(" GB").trim().parse::<u32>().ok();
            let lookup = match raw.as_str() {
                "RTXP 6000" => "RTX PRO 6000".to_owned(),
                _ => format!("{raw} {}", spec.replace(' ', "")),
            };
            let gpu_type = gpu::normalize(&lookup).unwrap_or_else(|| raw.clone());
            out.push(GpuListing {
                external_id: format!("{}-{count}x", gpu_type.replace(' ', "-")),
                name: format!("{count}× {raw}"),
                gpu_type,
                gpu_count: count,
                price_per_hour: super::super::round6(per_gpu * count as f64),
                region: None,
                source_url: Some(URL.into()),
                extra: serde_json::json!({ "vram_gb": vram, "per_gpu_hour": per_gpu }),
            });
        }
    }
    if panels == 0 || out.is_empty() {
        return Err("no GPU panels with prices".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn li(name: &str, spec: &str, price: &str) -> String {
        format!(
            r#"<li><span class="name">{name}</span><span class="spec">{spec}</span><span class="price">{price}<span class="unit">/hr</span></span></li>"#
        )
    }

    fn fixture() -> String {
        format!(
            r#"<ul class="panel" data-panel="0" data-panel-kind="cpu" hidden>{}</ul>
<ul class="panel" data-panel="1" data-panel-kind="gpu">{}{}{}</ul>
<ul class="panel" data-panel="2" data-panel-kind="gpu" hidden>{}{}</ul>
<ul class="panel" data-panel="5" data-panel-kind="tpu" hidden>{}</ul>"#,
            li("Large (CPU)", "32 GB", "$0.51"),
            li("T4", "16 GB", "$0.55"),
            li("H100", "80 GB", "$5.45"),
            li("RTXP 6000", "96 GB", "Free"),
            li("A100", "40 GB", "$2.19"),
            li("H100", "80 GB", "$5.45"),
            li("1 TPU", "32 GB", "$3.05"),
        )
    }

    #[test]
    fn parses_gpu_panels_only() {
        let l = parse(&[fixture()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4, "CPU, TPU and unpriced rows are skipped");
        assert_eq!(l[0].gpu_type, "T4");
        assert_eq!((l[0].gpu_count, l[0].price_per_hour), (1, 0.55));
        let two = l
            .iter()
            .find(|x| x.gpu_type == "H100 SXM" && x.gpu_count == 2)
            .unwrap();
        assert_eq!(two.price_per_hour, 10.9);
        let a100 = l.iter().find(|x| x.gpu_type.starts_with("A100")).unwrap();
        assert!(a100.gpu_type.contains("40GB"));
        let mut ids: Vec<_> = l.iter().map(|x| &x.external_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn changed_page_is_an_error() {
        assert!(parse(&["<p>x</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_LIGHTNING").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!("{l:?}");
        }
    }
}
