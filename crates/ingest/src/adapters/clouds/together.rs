//! Together AI: GPU cluster prices from its pricing page. Together is an inference company;
//! only the "GPU Clusters" table counts here. Its prices are per GPU-hour with a preemptible,
//! an on-demand and three reserved-term columns, so each row is a one-GPU configuration.
//! Preemptible and reserved prices go in `extra`; rows without an on-demand number (GB200
//! NVL72: "request capacity") are left out. The page's other table, dedicated inference
//! endpoints, is a different product and is not read.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-together",
    name: "Together AI",
    organization: "together-ai",
    origin: "https://www.together.ai",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.together.ai/pricing";

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

/// A price cell: `$3.99` plus possibly a capacity badge; `None` for dashes or links.
fn cell_price(cell: &str) -> Result<Option<f64>, String> {
    let text = pieces(cell);
    let Some(first) = text.first() else {
        return Ok(None);
    };
    let Some(rest) = first.strip_prefix('$') else {
        return Ok(None);
    };
    rest.replace(',', "")
        .parse()
        .map(Some)
        .map_err(|_| format!("unreadable price {first:?}"))
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let anchor = page
        .find("id=\"gpu-clusters\"")
        .ok_or("no GPU Clusters section")?;
    let rest = &page[anchor..];
    let table = &rest[rest.find("<table").ok_or("no GPU cluster table")?..];
    let table = &table[..table.find("</table>").ok_or("unterminated table")?];
    let (head, body) = table
        .split_once("<tbody")
        .ok_or("no GPU cluster table body")?;
    let head = pieces(head);
    let at = |name: &str| head.iter().position(|h| h.eq_ignore_ascii_case(name));
    match (at("Preemptible"), at("On-Demand")) {
        (Some(p), Some(o)) if p < o => {}
        _ => return Err("the cluster table lacks Preemptible / On-Demand columns".into()),
    }
    let mut out = Vec::new();
    for row in body.split("<tr").skip(1) {
        let cells: Vec<&str> = row
            .split("<td")
            .skip(1)
            .map(|c| c.split_once('>').map_or("", |(_, inner)| inner))
            .collect();
        if cells.len() < 3 {
            continue;
        }
        let name = pieces(cells[0]).join(" ");
        if name.is_empty() {
            continue;
        }
        let Some(on_demand) = cell_price(cells[2])? else {
            continue;
        };
        let reserved = |i: usize| cells.get(i).map(|c| cell_price(c)).transpose();
        let gpu_type = gpu::normalize(&name).unwrap_or_else(|| name.clone());
        let slug: String = name
            .replace("NVIDIA ", "")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        out.push(GpuListing {
            external_id: format!("{slug}-1x"),
            name,
            gpu_type,
            gpu_count: 1,
            price_per_hour: on_demand,
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "price_basis": "per GPU-hour",
                "preemptible_per_gpu_hour": cell_price(cells[1])?,
                "reserved_3_30_days_per_gpu_hour": reserved(3)?.flatten(),
                "reserved_31_90_days_per_gpu_hour": reserved(4)?.flatten(),
                "reserved_91_plus_days_per_gpu_hour": reserved(5)?.flatten(),
            }),
        });
    }
    if out.is_empty() {
        return Err("the GPU cluster table holds no on-demand prices".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, cells: &[&str]) -> String {
        let mut r = format!(
            r#"<div role="listitem"><tr fs-list-element="item"><td><p class="body-l"><span>NVIDIA</span><span> </span><span>{name}</span></p></td>"#
        );
        for c in cells {
            r.push_str(&format!(
                r#"<td class="is-right-border"><div class="opacity-70"><p data-batch="" class="body-m">{c}</p></div><div class="capacity_cell"><div class="capacity_badge"><p data-capacity-label="">Checking…</p></div></div></td>"#
            ));
        }
        r.push_str("</tr></div>");
        r
    }

    fn fixture() -> String {
        let head = r#"<div id="gpu-clusters" class="section-anchor"></div><h2>GPU Clusters</h2><table class="pricing_table is-gpu"><thead><tr><th rowspan="2"><p>Hardware</p></th><th><p>Preemptible</p></th><th><p>ON-Demand</p></th><th colspan="3"><p>Reserved</p></th></tr><tr><th><p>Pay as you go</p></th><th><p>Pay as you go</p></th><th><p>3-30 days</p></th><th><p>31-90 days</p></th><th><p>91+ days</p></th></tr></thead><tbody fs-list-element="list">"#;
        let rows = [
            row(
                "HGX H100",
                &["$1.99", "$3.99", "$3.69", "$3.45", "Request Capacity"],
            ),
            row(
                "HGX H200",
                &["$2.99", "$5.99", "$4.99", "$4.15", "Request Capacity"],
            ),
            row(
                "HGX B200",
                &["$4.65", "$9.29", "$8.69", "$7.99", "Request Capacity"],
            ),
            row("GB200 NVL72", &["—", "—", "Request Capacity"]),
        ]
        .concat();
        format!(
            "<html><body>{head}{rows}</tbody></table><p>Capacity is checked live.</p></body></html>"
        )
    }

    #[test]
    fn parses_cluster_rows() {
        let l = parse(&[fixture()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 3, "the row without a price is skipped");
        let h100 = l.iter().find(|x| x.name == "NVIDIA HGX H100").unwrap();
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 3.99);
        assert_eq!(h100.external_id, "HGX-H100-1x");
        assert_eq!(h100.extra["preemptible_per_gpu_hour"], 1.99);
        assert_eq!(h100.extra["reserved_31_90_days_per_gpu_hour"], 3.45);
        let b200 = l.iter().find(|x| x.name.contains("B200")).unwrap();
        assert_eq!(b200.price_per_hour, 9.29);
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
        let bad = fixture().replace("$3.99", "$3.9x");
        assert!(parse(&[bad], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_TOGETHER").expect("CLOUD_FIXTURE_TOGETHER");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.name, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
