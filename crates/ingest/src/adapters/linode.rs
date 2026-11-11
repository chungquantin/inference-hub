//! Akamai (Linode): GPU plans from the public Linode types API.

use serde::Deserialize;

use super::{
    BoxFuture, DAILY, GpuListing, NeoCloudAdapter, adapter_info, gpu,
    paging::{Items, Paging, fetch_all},
    round6,
    simple::{json, non_empty},
};
use crate::adapters::AdapterError;

const ID: &str = "linode";

pub struct LinodeAdapter {
    url: String,
}

impl LinodeAdapter {
    pub fn new(origin: &str) -> Self {
        Self {
            url: format!("{}/v4/linode/types", origin.trim_end_matches('/')),
        }
    }
}

adapter_info!(LinodeAdapter, ID, "Akamai Cloud (Linode)", "akamai", DAILY);

impl NeoCloudAdapter for LinodeAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            // The types API pages (`page`, `pages`); walk every page.
            let paging = Paging::Page {
                param: "page",
                first: 1,
                pages_field: Some(&["pages"]),
            };
            let (items, _) =
                fetch_all(client, ID, &self.url, Items::At(&["data"]), &paging).await?;
            parse(&serde_json::json!({ "data": items }).to_string())
        })
    }
}

#[derive(Deserialize)]
struct Types {
    data: Vec<Type>,
}

#[derive(Deserialize)]
struct Type {
    id: String,
    label: String,
    class: String,
    gpus: i16,
    price: Price,
}

#[derive(Deserialize)]
struct Price {
    hourly: Option<f64>,
}

/// The GPU model from a plan label: `Dedicated 32GB + RTX6000 GPU x1`, `RTX4000 Ada x1 Small`.
fn gpu_of(label: &str) -> Option<String> {
    let l = label.to_ascii_uppercase();
    if l.contains("RTX4000 ADA") {
        Some("RTX 4000 Ada".into())
    } else if l.contains("RTX6000") {
        Some("RTX 6000".into())
    } else {
        l.split(['+', 'x', 'X'])
            .find_map(|part| gpu::normalize(part.trim().trim_end_matches(" GPU")))
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let types: Types = json(ID, body)?;
    let listings = types
        .data
        .into_iter()
        .filter(|t| t.class == "gpu" && t.gpus > 0)
        .filter_map(|t| {
            let hourly = t.price.hourly.filter(|h| h.is_finite() && *h > 0.0)?;
            let gpu_type = gpu_of(&t.label)?;
            Some(GpuListing {
                external_id: gpu_type.clone(),
                name: t.label.clone(),
                gpu_type,
                gpu_count: t.gpus,
                price_per_hour: round6(hourly),
                region: Some(t.id.clone()),
                source_url: Some("https://www.linode.com/pricing/#gpu".into()),
                extra: serde_json::json!({ "plan": t.id }),
            })
        })
        .collect();
    non_empty(ID, listings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_gpu_plans() {
        let l = parse(r#"{"data":[
            {"id":"g1-gpu-rtx6000-2","label":"Dedicated 64GB + RTX6000 GPU x2","class":"gpu","gpus":2,"price":{"hourly":3.0}},
            {"id":"g2-gpu-rtx4000a1-s","label":"RTX4000 Ada x1 Small","class":"gpu","gpus":1,"price":{"hourly":0.52}},
            {"id":"g6-nanode-1","label":"Nanode 1GB","class":"nanode","gpus":0,"price":{"hourly":0.0075}}
        ]}"#).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].gpu_count, l[0].price_per_hour),
            ("RTX 6000", 2, 3.0)
        );
        assert_eq!(l[1].gpu_type, "RTX 4000 Ada");
    }
}
