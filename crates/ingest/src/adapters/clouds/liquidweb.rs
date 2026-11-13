//! Liquid Web: dedicated GPU servers from its GPU hosting page. Each card names the GPU
//! (`(x2)` prefix for the GPU count), the hourly list price of the whole server and a
//! promotional price for the first month, which goes into `extra`.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

const URL: &str = "https://www.liquidweb.com/gpu-hosting/";

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-liquidweb",
    name: "Liquid Web",
    organization: "liquid-web",
    origin: "https://www.liquidweb.com",
    paths: &["/gpu-hosting/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

fn is_price_line(l: &str) -> bool {
    l.starts_with('$') && l.contains("/hr")
}

/// `(x2) H100 94GB` -> (2, `H100 94GB`).
fn split_count(name: &str) -> (i16, &str) {
    if let Some(rest) = name.strip_prefix("(x")
        && let Some((n, tail)) = rest.split_once(')')
        && let Ok(n) = n.parse::<i16>()
    {
        return (n, tail.trim());
    }
    (1, name)
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = text::html_lines(page);
    let start = lines
        .iter()
        .position(|l| l.contains("cost-effective GPUs"))
        .ok_or("no GPU price section")?;
    let lines = &lines[start..];
    let mut out = Vec::new();
    for i in 1..lines.len() {
        if !is_price_line(&lines[i]) {
            continue;
        }
        // A promo price line follows its list price line; it is not a card of its own.
        if i >= 2 && is_price_line(&lines[i - 1]) {
            continue;
        }
        let name = &lines[i - 1];
        if name.starts_with('$') || name.len() > 40 {
            continue;
        }
        let list = text::first_dollar(&lines[i]).ok_or("unparsable price")?;
        let promo = lines[i].contains(" off ").then(|| {
            let label = lines[i]
                .split_once('/')
                .map(|(_, r)| r.trim())
                .unwrap_or("");
            let label = label.trim_start_matches("hr").trim();
            let price = lines
                .get(i + 1)
                .filter(|l| is_price_line(l))
                .and_then(|l| text::first_dollar(l));
            (label.to_owned(), price)
        });
        // Card details follow "Build now" until the next card.
        let rest = &lines[i + 1..];
        let details: Vec<&String> = rest
            .iter()
            .skip_while(|l| !l.starts_with("Build"))
            .skip(1)
            .take_while(|l| !is_price_line(l))
            .take(4)
            .collect();
        let (count, gpu_name) = split_count(name);
        let gpu_type = if gpu_name.starts_with("H100") && gpu_name.contains("94GB") {
            "H100 NVL".to_owned()
        } else {
            gpu::normalize(gpu_name).unwrap_or_else(|| gpu_name.to_owned())
        };
        let ram = details
            .iter()
            .find(|l| l.contains("DDR"))
            .map(|l| l.to_string());
        out.push(GpuListing {
            external_id: format!("{}-{count}x", gpu_type.replace(' ', "-")),
            name: name.clone(),
            gpu_type,
            gpu_count: count,
            price_per_hour: list,
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "cpu": details.first(),
                "cores": details.get(1),
                "ram": ram,
                "promo": promo.as_ref().map(|(label, price)| {
                    serde_json::json!({ "label": label, "price_per_hour": price })
                }),
            }),
        });
    }
    if out.is_empty() {
        return Err("no priced GPU server cards".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<h2>Powerful and cost-effective GPUs</h2>
<h3>L4 Ada 24GB</h3><p><s>$1.07/hr</s> <mark>25% off 1 month </mark></p><p>$0.80/hr</p><a>Build now</a>
<ul><li>(x2) EPYC 9124</li><li>32 cores / 64 threads @ 3.0 GHz</li><li>128 GB DDR5</li><li>1.92 TB NVMe RAID-1</li><li>∞ In / 10 TB Out bandwidth</li></ul>
<h3>H100 94GB</h3><p><s>$3.97/hr</s> <mark>25% off 1 month </mark></p><p>$2.98/hr</p><a>Build now</a>
<ul><li>(x2) EPYC 9124</li><li>48 cores / 96 threads @ 2.9 GHz</li><li>256 GB DDR5</li><li>3.84 TB NVMe RAID-1</li></ul>
<h3>(x2) H100 94GB</h3><p><s>$6.85/hr</s> <mark>25% off 1 month </mark></p><p>$5.14/hr</p><a>Build now</a>
<ul><li>(x2) EPYC 9254</li><li>48 cores / 96 threads @ 2.9 GHz</li><li>768 GB DDR5</li><li>7.68 TB NVMe RAID-1</li></ul>
<h3>Build smarter, faster, and at scale</h3>"#;

    #[test]
    fn parses_list_price_and_promo() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].gpu_type, "L4");
        assert_eq!((l[0].gpu_count, l[0].price_per_hour), (1, 1.07));
        assert_eq!(l[0].extra["promo"]["price_per_hour"], 0.80);
        assert_eq!(l[1].gpu_type, "H100 NVL");
        assert_eq!((l[1].gpu_count, l[1].price_per_hour), (1, 3.97));
        assert_eq!((l[2].gpu_count, l[2].price_per_hour), (2, 6.85));
        assert_eq!(l[2].extra["ram"], "768 GB DDR5");
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
        let path = std::env::var("CLOUD_FIXTURE_LIQUIDWEB").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!("{l:?}");
        }
    }
}
