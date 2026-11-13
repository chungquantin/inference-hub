//! Nebius: on-demand GPU prices from its public pricing page. The "NVIDIA GPU Instances"
//! table sits in the page's `__NEXT_DATA__`, inside a JSON string (the CMS content). Prices
//! are per GPU-hour, so each row is a one-GPU configuration; vCPUs and RAM are per GPU.
//! "from $X" and "Contact us" cells are not list prices and are left out. The preemptible
//! price goes in `extra`.

use serde_json::Value;

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::{GpuListing, gpu, labs::text};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-nebius",
    name: "Nebius",
    organization: "nebius",
    origin: "https://nebius.com",
    paths: &["/prices"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://nebius.com/prices";

/// Every JSON string in `v` that is itself a JSON document with CMS blocks, parsed.
fn collect_tables(v: &Value, out: &mut Vec<Vec<Vec<String>>>) {
    match v {
        Value::String(s) if s.starts_with("{\"blocks\"") => {
            if let Ok(inner) = serde_json::from_str::<Value>(s) {
                collect_tables(&inner, out);
            }
        }
        Value::Array(a) => {
            if let Some(table) = as_price_table(a) {
                out.push(table);
            } else {
                a.iter().for_each(|x| collect_tables(x, out));
            }
        }
        Value::Object(o) => o.values().for_each(|x| collect_tables(x, out)),
        _ => {}
    }
}

/// An array of rows of strings whose header names an on-demand column.
fn as_price_table(a: &[Value]) -> Option<Vec<Vec<String>>> {
    let rows: Option<Vec<Vec<String>>> = a
        .iter()
        .map(|r| {
            r.as_array()?
                .iter()
                .map(|c| c.as_str().map(str::to_owned))
                .collect()
        })
        .collect();
    let rows = rows?;
    let header = rows.first()?;
    header
        .iter()
        .any(|c| c.to_ascii_lowercase().starts_with("on-demand"))
        .then_some(rows)
}

/// `$4.50` -> 4.5; `None` for `from $1.55`, links, dashes.
fn list_price(cell: &str) -> Option<f64> {
    let rest = cell.trim().strip_prefix('$')?;
    rest.trim_end_matches('*').trim().parse().ok()
}

fn int(cell: &str) -> Option<i64> {
    cell.trim().parse().ok()
}

fn parse(pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let data = text::embedded_json(page, "__NEXT_DATA__").ok_or("no __NEXT_DATA__ in the page")?;
    let mut tables = Vec::new();
    collect_tables(&data, &mut tables);
    let table = tables
        .into_iter()
        .find(|t| t[0].iter().any(|c| c.starts_with("Item")))
        .ok_or("no GPU price table in the page data")?;
    let col = |name: &str| {
        table[0]
            .iter()
            .position(|c| c.to_ascii_lowercase().starts_with(name))
    };
    let on_demand = col("on-demand").ok_or("no on-demand column")?;
    let preempt = col("preemptible");
    let (vcpu, ram) = (col("vcpus"), col("ram"));

    let mut out = Vec::new();
    for row in &table[1..] {
        let Some(cell) = row.get(on_demand) else {
            continue;
        };
        let name = row[0].trim();
        let digits = cell.chars().any(|c| c.is_ascii_digit());
        let Some(price) = list_price(cell) else {
            if digits && !cell.trim_start().starts_with("from") {
                return Err(format!("unreadable on-demand price {cell:?} for {name}"));
            }
            continue;
        };
        let gpu_type = gpu::normalize(name).unwrap_or_else(|| name.to_owned());
        let slug: String = name
            .replace("NVIDIA ", "")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        out.push(GpuListing {
            external_id: format!("{slug}-1x"),
            name: name.to_owned(),
            gpu_type,
            gpu_count: 1,
            price_per_hour: price,
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "price_basis": "per GPU-hour",
                "vcpus_per_gpu": vcpu.and_then(|i| row.get(i)).and_then(|c| int(c)),
                "ram_gb_per_gpu": ram.and_then(|i| row.get(i)).and_then(|c| int(c)),
                "preemptible_from_per_gpu_hour": preempt
                    .and_then(|i| row.get(i))
                    .and_then(|c| text::first_dollar(c)),
            }),
        });
    }
    if out.is_empty() {
        return Err("the price table holds no on-demand prices".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r##"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"__APOLLO_STATE__":{"pages:{\"id\":\"99\"}":{"name":"prices","content":"{\"blocks\":[{\"type\":\"divider\"},{\"type\":\"highlight-table-block\",\"title\":\"NVIDIA GPU Instances\",\"table\":{\"content\":[[\"Item\",\"vCPUs\",\"RAM, GB\",\"Preemptible, GPU-hour *\",\"On-demand, GPU-hour\"],[\"NVIDIA GB300 NVL72\",\"112\",\"800\",\" - \",\"[Contact us](#ai-contact-form)\"],[\"NVIDIA HGX B200\",\"20\",\"224\",\"from $0.99\",\"$8.50\"],[\"NVIDIA HGX H200\",\"16\",\"200\",\"from $0.79\",\"$5.40\"],[\"NVIDIA HGX H100\",\"16\",\"200\",\"from $0.79\",\"$4.50\"],[\"NVIDIA RTX PRO 6000\",\"24\",\"218\",\"from $0.79\",\"$1.80 \"],[\"NVIDIA L40S with Intel CPU\",\"8-40\",\"32-160\",\"from $0.74**\",\"from $1.55\"]],\"customColumnWidth\":[3,2,2,2,2]}}]}"}}}}}</script></html>"##;

    #[test]
    fn parses_on_demand_rows() {
        let l = parse(&[FIXTURE.into()], &Fx::usd()).unwrap();
        assert_eq!(l.len(), 4, "contact-us and from-price rows are skipped");
        let h100 = l.iter().find(|x| x.name == "NVIDIA HGX H100").unwrap();
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 4.5);
        assert_eq!(h100.external_id, "HGX-H100-1x");
        assert_eq!(h100.extra["vcpus_per_gpu"], 16);
        assert_eq!(h100.extra["preemptible_from_per_gpu_hour"], 0.79);
        let rtx = l.iter().find(|x| x.name.contains("RTX PRO")).unwrap();
        assert_eq!(rtx.price_per_hour, 1.8);
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
        let bad = FIXTURE.replace("$4.50", "$4,5x0.0 per");
        assert!(parse(&[bad], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_NEBIUS").expect("CLOUD_FIXTURE_NEBIUS");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} x{} | ${} | {}",
                l.name, l.gpu_type, l.gpu_count, l.price_per_hour, l.extra
            );
        }
    }
}
