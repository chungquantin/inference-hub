//! Packet.ai: the on-demand prices in the page's JSON-LD `Service` offers. "Dedicated" offers
//! give whole cards and are priced for the whole configuration (`2 x NVIDIA B200 - Dedicated`).
//! "Dynamic" offers are a share of a pooled card, not a GPU, so they go in the dedicated
//! listing's `extra` when there is one and are otherwise left out; monthly plans are left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text, round6};

const URL: &str = "https://packet.ai/pricing";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-packet-ai",
    name: "Packet.ai",
    organization: "packet-ai",
    origin: "https://packet.ai",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

/// `NVIDIA B200 180GB`, `2 x NVIDIA B200` -> (count, gpu name, VRAM in GB).
fn split_gpu(raw: &str) -> (i16, String, Option<u32>) {
    let (count, rest) = match raw.split_once(" x ") {
        Some((n, rest)) if n.trim().parse::<i16>().is_ok() => (n.trim().parse().unwrap_or(1), rest),
        _ => (1, raw),
    };
    let mut words: Vec<&str> = rest.split_whitespace().collect();
    let vram = words
        .last()
        .and_then(|w| w.strip_suffix("GB"))
        .and_then(|n| n.parse().ok());
    if vram.is_some() {
        words.pop();
    }
    (count, words.join(" "), vram)
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("packet.ai: no page")?;
    let offers = ld_offers(page).ok_or("packet.ai: no Service offers in the page's JSON-LD")?;
    let mut dedicated: Vec<GpuListing> = Vec::new();
    // (gpu name without count, dynamic price)
    let mut dynamic: Vec<(String, f64)> = Vec::new();
    for offer in offers {
        let name = offer["name"].as_str().unwrap_or_default();
        let price = match &offer["price"] {
            serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
            v => v.as_f64(),
        };
        if offer["priceCurrency"].as_str() != Some("USD") {
            continue;
        }
        let Some((gpu_part, plan)) = name.rsplit_once(" - ") else {
            continue;
        };
        let price = price
            .filter(|p| *p > 0.0)
            .ok_or_else(|| format!("packet.ai: price of `{name}` does not parse"))?;
        let (count, gpu_name, vram) = split_gpu(gpu_part);
        match plan.trim() {
            "Dedicated" => {
                let gpu_type = gpu::normalize(&gpu_name).unwrap_or_else(|| gpu_name.clone());
                dedicated.push(GpuListing {
                    external_id: format!("{}-{count}x", gpu_name.replace(' ', "-")),
                    name: name.to_owned(),
                    gpu_type,
                    gpu_count: count,
                    price_per_hour: round6(price),
                    region: None,
                    source_url: Some(URL.into()),
                    extra: serde_json::json!({
                        "vram_gb": vram,
                        "plan": "Dedicated",
                        "per_gpu_hour": round6(price / f64::from(count)),
                    }),
                });
            }
            "Dynamic" => dynamic.push((gpu_name, price)),
            _ => {}
        }
    }
    for l in &mut dedicated {
        let own = l.name.split(" - ").next().unwrap_or_default();
        let (_, gpu_name, _) = split_gpu(own);
        if let Some((_, p)) = dynamic.iter().find(|(g, _)| *g == gpu_name) {
            l.extra["dynamic_per_gpu_hour"] = serde_json::json!(p);
        }
    }
    if dedicated.is_empty() {
        return Err("packet.ai: the page lists no dedicated GPU offers".into());
    }
    Ok(dedicated)
}

/// The `offers` of the `Service` in the page's JSON-LD blocks.
fn ld_offers(page: &str) -> Option<Vec<serde_json::Value>> {
    page.split("application/ld+json").skip(1).find_map(|block| {
        let v = text::embedded_json(block, ">")?;
        let nodes = v["@graph"].as_array().cloned().unwrap_or_else(|| vec![v]);
        nodes
            .into_iter()
            .find(|n| n["@type"] == "Service")
            .and_then(|n| n["offers"].as_array().cloned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"Service","name":"GPU Cloud Pricing","offers":[
{"@type":"Offer","name":"NVIDIA B200 180GB - Dynamic","price":"3.75","priceCurrency":"USD"},
{"@type":"Offer","name":"NVIDIA B200 180GB - Dedicated","price":"6.99","priceCurrency":"USD"},
{"@type":"Offer","name":"2 x NVIDIA B200 - Dedicated","price":"13.98","priceCurrency":"USD"},
{"@type":"Offer","name":"8 x NVIDIA B200 - Dedicated","price":"55.92","priceCurrency":"USD"},
{"@type":"Offer","name":"2 x NVIDIA B200 - Dedicated, monthly","price":"10074","priceCurrency":"USD"},
{"@type":"Offer","name":"NVIDIA RTX PRO 6000 Blackwell 96GB - Dynamic","price":"0.76","priceCurrency":"USD"},
{"@type":"Offer","name":"NVIDIA RTX 4090 24GB - Dedicated","price":"0.39","priceCurrency":"USD"}]},
{"@type":"FAQPage","mainEntity":[]}]}</script>"#;

    #[test]
    fn reads_dedicated_offers_and_skips_the_rest() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].gpu_count, l[0].price_per_hour),
            ("B200", 1, 6.99)
        );
        assert_eq!((l[1].gpu_count, l[1].price_per_hour), (2, 13.98));
        assert_eq!((l[2].gpu_count, l[2].price_per_hour), (8, 55.92));
        assert_eq!(l[3].gpu_type, "RTX 4090");
        assert_eq!(l[0].extra["dynamic_per_gpu_hour"], 3.75);
        assert_eq!(l[0].extra["vram_gb"], 180);
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn missing_json_ld_is_an_error() {
        assert!(parse(&["<html></html>".into()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_PACKET").expect("CLOUD_FIXTURE_PACKET");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
