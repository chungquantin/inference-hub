//! Massed Compute: on-demand GPU prices from its public pricing page. Each GPU family
//! has one table (quantity, vCPU, RAM, storage, price per hour) under a heading such as
//! `H100 SXM5 (80GB)`; the price is for the whole instance of that quantity.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-massed",
    name: "Massed Compute",
    organization: "massed-compute",
    origin: "https://vm.massedcompute.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://vm.massedcompute.com/pricing";

/// Text of an HTML fragment: tags removed, entities of the common kind decoded.
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The family heading nearest before the end of `before`.
fn heading(before: &str) -> Option<String> {
    let at = before.rfind("leading-snug")?;
    let rest = &before[at..];
    let start = rest.find('>')? + 1;
    let end = start + rest[start..].find('<')?;
    Some(plain(&rest[start..end])).filter(|h| !h.is_empty())
}

/// The directory name for a heading such as `H100 (80GB)` or `RTX A6000 (48GB) [Premium]`.
/// A bare `H100 (80GB)` / `A100 (80GB)` is the PCIe card, which the page prices below its SXM
/// one; the shared normalizer would read it as SXM.
fn gpu_name(heading: &str) -> String {
    let mut base = heading.to_owned();
    while let (Some(a), Some(b)) = (base.find('['), base.find(']')) {
        base.replace_range(a..=b, "");
    }
    while let (Some(a), Some(b)) = (base.find('('), base.find(')')) {
        base.replace_range(a..=b, "");
    }
    let base = base.replace("DGX ", "").replace("NVLink", "");
    let base = base.split_whitespace().collect::<Vec<_>>().join(" ");
    let bare = ["H100 (", "A100 ("].iter().any(|p| heading.starts_with(p));
    let hinted = if bare { format!("{base} PCIe") } else { base };
    gpu::normalize(&hinted).unwrap_or_else(|| heading.to_owned())
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let html = pages.first().ok_or("no page")?;
    let mut out: Vec<GpuListing> = Vec::new();
    let mut tables = 0;
    let segments: Vec<&str> = html.split("<table").collect();
    for pair in segments.windows(2) {
        let Some(family) = heading(pair[0]) else {
            continue;
        };
        let table = pair[1].split("</table>").next().unwrap_or("");
        if !table.contains("Qty") {
            continue;
        }
        tables += 1;
        for row in table.split("<tr").skip(1) {
            let cells: Vec<String> = row
                .split("<td")
                .skip(1)
                .map(|c| plain(&format!("<td{c}")))
                .collect();
            if cells.len() < 5 {
                continue;
            }
            // A row without a stated price ("Contact us") is not a configuration to list.
            let Some(usd) = cells[4]
                .starts_with('$')
                .then(|| text::first_dollar(&cells[4]))
                .flatten()
            else {
                continue;
            };
            let count: i16 = cells[0]
                .trim_start_matches('×')
                .trim()
                .parse()
                .map_err(|_| format!("quantity of {family}: {:?}", cells[0]))?;
            let num = |s: &str| {
                s.split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<f64>().ok())
            };
            out.push(GpuListing {
                external_id: format!("{family}-{count}x"),
                name: format!("{count}× {family}"),
                gpu_type: gpu_name(&family),
                gpu_count: count,
                price_per_hour: fx.to_usd(usd),
                region: None,
                source_url: Some(URL.into()),
                extra: serde_json::json!({
                    "vcpus": num(&cells[1]),
                    "ram_gb": num(&cells[2]),
                    "storage_gb": num(&cells[3]),
                    "original": fx.original(usd),
                }),
            });
        }
    }
    if tables == 0 || out.is_empty() {
        return Err("no price tables with a Qty column".into());
    }
    let mut ids: Vec<_> = out.iter().map(|l| l.external_id.as_str()).collect();
    ids.sort();
    if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("duplicate configuration {}", w[0]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[(&str, &str, &str, &str, &str)]) -> String {
        let mut t = String::from(
            r#"<div class="hidden sm:block"><table class="w-full"><thead><tr><th>Qty</th><th>vCPU</th><th>RAM</th><th>Storage</th><th>Price</th><th></th></tr></thead><tbody>"#,
        );
        for (q, c, r, s, p) in rows {
            t.push_str(&format!(
                r#"<tr class="b"><td class="px-4"><span class="x">×<!-- -->{q}</span></td><td class="r">{c}</td><td class="r">{r} GB</td><td class="r">{s}</td><td class="r b">{p}<small class="ml-0.5">/hr</small></td><td><a href="/signup">Deploy →</a></td></tr>"#
            ));
        }
        t.push_str("</tbody></table></div>");
        t
    }

    fn fixture() -> String {
        let group = |name: &str, rows: &[(&str, &str, &str, &str, &str)]| {
            format!(
                r#"<div class="bone"><span class="font-pricing-display text-sm font-bold leading-snug sm:text-[15px]">{name}</span></div>{}"#,
                table(rows)
            )
        };
        [
            group("B200 SXM6", &[("8", "280", "2800", "6000", "$43.46")]),
            group(
                "H100 (80GB)",
                &[
                    ("1", "20", "128", "1250", "$2.73"),
                    ("2", "40", "256", "2500", "$5.46"),
                    ("4", "80", "512", "5000", "Contact us"),
                ],
            ),
            group("H100 SXM5 (80GB)", &[("1", "18", "128", "750", "$3.11")]),
            group(
                "RTX A6000 (48GB) [Premium]",
                &[("2", "14", "96", "512", "$1.24")],
            ),
        ]
        .concat()
    }

    #[test]
    fn parses_configurations_and_skips_unpriced_rows() {
        let l = parse(&[fixture()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 5);
        let by = |id: &str| l.iter().find(|x| x.external_id == id).unwrap();
        let b200 = by("B200 SXM6-8x");
        assert_eq!((b200.gpu_type.as_str(), b200.gpu_count), ("B200", 8));
        assert_eq!(b200.price_per_hour, 43.46);
        let h100 = by("H100 (80GB)-2x");
        assert_eq!((h100.gpu_type.as_str(), h100.gpu_count), ("H100 PCIe", 2));
        assert_eq!(h100.price_per_hour, 5.46);
        assert_eq!(h100.extra["vcpus"], 40.0);
        assert!(l.iter().all(|x| x.external_id != "H100 (80GB)-4x"));
        assert_eq!(by("H100 SXM5 (80GB)-1x").gpu_type, "H100 SXM");
        assert_eq!(by("RTX A6000 (48GB) [Premium]-2x").gpu_type, "RTX A6000");
        let mut ids: Vec<_> = l.iter().map(|x| x.external_id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn errors_without_tables() {
        assert!(parse(&["<p>nothing</p>".into()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_MASSED").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.external_id, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
