//! Crusoe Cloud: on-demand GPU prices from its public pricing page.
//!
//! The "GPU instances pricing" table is three columns laid out one after another in the page
//! text: the GPU models (name, memory, optional form factor), the current on-demand price per
//! GPU-hour (`Contact sales` where unpublished), and the new price that starts on a later
//! date (`No change`, `Contact sales` or a price). Prices are per GPU-hour, so each listing is
//! one GPU. Rack-scale NVL72 parts and anything without a number are left out.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;
use crate::adapters::gpu;
use crate::adapters::labs::text::{first_dollar, html_lines};

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-crusoe",
    name: "Crusoe",
    organization: "crusoe",
    origin: "https://www.crusoe.ai",
    paths: &["/cloud/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    enabled: true,
    parse,
};

const URL: &str = "https://www.crusoe.ai/cloud/pricing";

struct Model {
    name: String,
    vram: String,
    form: String,
}

#[derive(Debug, PartialEq)]
enum Cell {
    ContactSales,
    NoChange,
    Price(f64),
}

fn is_model_start(line: &str) -> bool {
    line.starts_with("NVIDIA ") || line.starts_with("AMD ")
}

fn cell(line: &str) -> Option<Cell> {
    match line {
        "Contact sales" => Some(Cell::ContactSales),
        "No change" => Some(Cell::NoChange),
        _ if line.starts_with('$') => first_dollar(line).map(Cell::Price),
        _ => None,
    }
}

fn parse(pages: &[String], fx: &Fx) -> Result<Vec<GpuListing>, String> {
    let page = pages.first().ok_or("no page")?;
    let lines = html_lines(page);
    let head = lines
        .iter()
        .position(|l| l == "Current on-demand")
        .ok_or("no \"Current on-demand\" column in the GPU instances table")?;

    // Column 1: the models, up to the first price cell.
    let mut models: Vec<Model> = Vec::new();
    let mut i = head + 1;
    while i < lines.len() && cell(&lines[i]).is_none() {
        let l = &lines[i];
        if is_model_start(l) {
            models.push(Model {
                name: l.clone(),
                vram: String::new(),
                form: String::new(),
            });
        } else if let Some(m) = models.last_mut() {
            if m.vram.is_empty() && l.ends_with("GB") {
                m.vram = l.clone();
            } else if m.form.is_empty() && l.len() <= 8 && !l.contains(' ') {
                m.form = l.clone();
            }
        }
        i += 1;
    }
    if models.is_empty() {
        return Err("no GPU models in the GPU instances table".into());
    }

    // Columns 2 and 3: one cell per model each; the `/` and `GPU-hr` lines are skipped.
    let cells: Vec<Cell> = lines[i..].iter().filter_map(|l| cell(l)).collect();
    if cells.len() < models.len() {
        return Err(format!(
            "{} models but only {} price cells",
            models.len(),
            cells.len()
        ));
    }
    let (current, rest) = cells.split_at(models.len());
    let next = rest.get(..models.len());
    if !current.iter().any(|c| matches!(c, Cell::Price(_))) {
        return Err("the current on-demand column holds no prices".into());
    }

    let mut out = Vec::new();
    for (k, (m, c)) in models.iter().zip(current).enumerate() {
        let Cell::Price(price) = c else { continue };
        let family = m.name.trim_start_matches("NVIDIA ").trim();
        let raw = format!("{} {} {}", m.name, m.form, m.vram);
        let Some(gpu_type) = gpu::normalize(&raw).or_else(|| Some(family.to_owned())) else {
            continue;
        };
        if m.form == "NVL72" {
            continue;
        }
        let mut id = family.replace(' ', "-");
        for part in [&m.form, &m.vram] {
            if !part.is_empty() {
                id.push('-');
                id.push_str(part);
            }
        }
        let new_price = match next.and_then(|n| n.get(k)) {
            Some(Cell::Price(p)) => Some(fx.to_usd(*p)),
            _ => None,
        };
        out.push(GpuListing {
            external_id: id,
            name: [m.name.as_str(), m.form.as_str(), m.vram.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" "),
            gpu_type,
            gpu_count: 1,
            price_per_hour: fx.to_usd(*price),
            region: None,
            source_url: Some(URL.into()),
            extra: serde_json::json!({
                "billing": "per GPU-hour",
                "vram": m.vram,
                "form_factor": m.form,
                "new_price_per_hour": new_price,
                "new_price_from": new_price.map(|_| "2026-12-01"),
                "original": fx.original(*price),
            }),
        });
    }
    if out.is_empty() {
        return Err("no GPU model with an on-demand price found".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "GPU instances pricing
GPU model
Current on-demand
New on-demand
starting Dec 1, 2026
NVIDIA GB300
288GB
NVL72
NVIDIA B300
288GB
HGX
NVIDIA H200
141GB
HGX
NVIDIA H100
80GB
HGX
NVIDIA A100
80GB
PCIe
NVIDIA L40S
48GB
AMD MI355X
288GB
AMD MI300X
192GB
Contact sales
Contact sales
$4.29
/
GPU-hr
$3.90
/
GPU-hr
$2.00
/
GPU-hr
$1.50
/
GPU-hr
Contact sales
$3.45
/
GPU-hr
Contact sales
$9.90
/
GPU-hr
$5.40
/
GPU-hr
$4.50
/
GPU-hr
No change
No change
No change
No change
CPU instances pricing
";

    #[test]
    fn parses_priced_models_and_skips_contact_sales() {
        let l = parse(&[FIXTURE.to_owned()], &Fx::usd()).unwrap();
        let ids: Vec<_> = l.iter().map(|x| x.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "H200-HGX-141GB",
                "H100-HGX-80GB",
                "A100-PCIe-80GB",
                "L40S-48GB",
                "AMD-MI300X-192GB"
            ]
        );
        let h100 = &l[1];
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 3.90);
        assert_eq!(h100.extra["new_price_per_hour"], 4.5);
        assert_eq!(l[2].extra["new_price_per_hour"], serde_json::Value::Null);
        assert_eq!(l[0].price_per_hour, 4.29);
        assert_eq!(l[0].extra["new_price_per_hour"], 5.4);
        assert_eq!(l[2].gpu_type, "A100 PCIe");
        assert_eq!(l[4].gpu_type, "AMD MI300X");
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }

    #[test]
    fn missing_table_is_an_error() {
        assert!(parse(&["<p>nothing</p>".to_owned()], &Fx::usd()).is_err());
    }

    #[test]
    #[ignore]
    fn live() {
        let path = std::env::var("CLOUD_FIXTURE_CRUSOE").expect("CLOUD_FIXTURE_CRUSOE");
        let page = std::fs::read_to_string(path).unwrap();
        for l in parse(&[page], &Fx::usd()).unwrap() {
            println!(
                "{} | {} | {} x {} | ${} | {}",
                l.external_id, l.name, l.gpu_count, l.gpu_type, l.price_per_hour, l.extra
            );
        }
    }
}
