//! Vultr: cloud GPU (`vcg`) and bare-metal plans from its public plan API.

use serde::Deserialize;

use super::{
    BoxFuture, DAILY, GpuListing, NeoCloudAdapter, adapter_info, gpu,
    paging::{Items, Paging, fetch_all},
    round6,
    simple::{json, non_empty},
};
use crate::adapters::AdapterError;

const ID: &str = "vultr";

pub struct VultrAdapter {
    origin: String,
}

impl VultrAdapter {
    pub fn new(origin: &str) -> Self {
        Self {
            origin: origin.trim_end_matches('/').to_owned(),
        }
    }
}

adapter_info!(VultrAdapter, ID, "Vultr", "vultr", DAILY);

#[derive(Deserialize)]
struct Plans {
    #[serde(alias = "plans_metal")]
    plans: Vec<Plan>,
}

#[derive(Deserialize)]
struct Plan {
    id: String,
    hourly_cost: f64,
    gpu_type: Option<String>,
    gpu_count: Option<i16>,
    #[serde(default)]
    locations: Vec<String>,
}

impl NeoCloudAdapter for VultrAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            // Plans page with an opaque `cursor` in `meta.links.next`.
            let fetch = |path: &'static str, key: &'static [&'static str]| {
                let url = format!("{}{path}", self.origin);
                async move {
                    let paging = Paging::Cursor {
                        param: "cursor",
                        cursor_field: &["meta", "links", "next"],
                    };
                    let (items, _) = fetch_all(client, ID, &url, Items::At(key), &paging).await?;
                    Ok::<_, AdapterError>(serde_json::json!({ "plans": items }).to_string())
                }
            };
            let cloud = fetch("/v2/plans?type=vcg&per_page=100", &["plans"]).await?;
            let metal = fetch("/v2/plans-metal?per_page=100", &["plans_metal"]).await?;
            let mut out = parse_plans(&cloud, "cloud GPU")?;
            out.extend(parse_plans(&metal, "bare metal")?);
            non_empty(ID, out)
        })
    }
}

fn parse_plans(body: &str, offer: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let plans: Plans = json(ID, body)?;
    Ok(plans
        .plans
        .into_iter()
        .filter_map(|p| {
            let raw = p.gpu_type?;
            let count = p.gpu_count.filter(|c| *c > 0)?;
            if !p.hourly_cost.is_finite() || p.hourly_cost <= 0.0 {
                return None;
            }
            let gpu_type = gpu::normalize(&raw)?;
            Some(GpuListing {
                // The plan id is the configuration: two plans can share GPU and count.
                external_id: raw,
                name: format!("{count}× {gpu_type} ({offer})"),
                gpu_type,
                gpu_count: count,
                price_per_hour: round6(p.hourly_cost),
                region: Some(p.id.clone()),
                source_url: Some("https://www.vultr.com/pricing/#cloud-gpu".into()),
                extra: serde_json::json!({ "plan": p.id, "offer": offer, "locations": p.locations.len() }),
            })
        })
        .collect())
}

#[cfg(test)]
pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    non_empty(ID, parse_plans(body, "cloud GPU")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cloud_and_metal_plans() {
        let l = parse(r#"{"plans":[{"id":"vcg-l40s-16c-180g-48vram","hourly_cost":1.671,"gpu_type":"NVIDIA_L40S","gpu_count":1},
                                   {"id":"vc2-1c-1gb","hourly_cost":0.007}]}"#).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("L40S", 1.671)
        );
        let m = parse_plans(r#"{"plans_metal":[{"id":"vbm-112c-2048gb-8-h100-gpu","hourly_cost":23.92,"gpu_type":"NVIDIA_H100","gpu_count":8}]}"#, "bare metal").unwrap();
        assert_eq!((m[0].gpu_type.as_str(), m[0].gpu_count), ("H100 SXM", 8));
        assert_eq!(m[0].region.as_deref(), Some("vbm-112c-2048gb-8-h100-gpu"));
    }
}
