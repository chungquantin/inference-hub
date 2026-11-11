//! Shadeform: a GPU marketplace that resells instances from many clouds
//! (Lambda, Crusoe, Nebius, Hyperstack, Massed Compute, Voltage Park, ...)
//! and publishes all of them, with prices, in one public API.
//!
//! Each listing is one cloud's instance type, with the cloud in `region` so
//! the same GPU from two clouds stays two configurations. Shadeform's price is
//! what a buyer pays through Shadeform; the cloud is in `extra.cloud`.

use serde::Deserialize;

use super::{
    DAILY, GpuListing, gpu, round6,
    simple::{json, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "shadeform";

simple_gpu_adapter!(
    ShadeformAdapter,
    ID,
    "Shadeform",
    "shadeform",
    DAILY,
    "/v1/instances/types"
);

#[derive(Deserialize)]
struct Response {
    instance_types: Vec<InstanceType>,
}

#[derive(Deserialize)]
struct InstanceType {
    cloud: String,
    cloud_instance_type: String,
    gpu_type: String,
    num_gpus: i16,
    /// US cents per hour.
    hourly_price: i64,
    #[serde(default)]
    interconnect: Option<String>,
    #[serde(default)]
    deployment_type: Option<String>,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let listings = response
        .instance_types
        .into_iter()
        .filter(|t| t.num_gpus > 0 && t.hourly_price > 0)
        .filter_map(|t| {
            let hint = match t.interconnect.as_deref() {
                Some("pcie") => " PCIe",
                _ => "",
            };
            let gpu_type = gpu::normalize(&format!("{}{hint}", t.gpu_type))
                .or_else(|| gpu::normalize(&t.gpu_type))?;
            Some(GpuListing {
                external_id: t.gpu_type.clone(),
                name: format!("{}× {gpu_type} on {}", t.num_gpus, t.cloud),
                gpu_type,
                gpu_count: t.num_gpus,
                price_per_hour: round6(t.hourly_price as f64 / 100.0),
                region: Some(format!("{}:{}", t.cloud, t.cloud_instance_type)),
                source_url: Some("https://www.shadeform.ai/instances".into()),
                extra: serde_json::json!({
                    "cloud": t.cloud,
                    "instance_type": t.cloud_instance_type,
                    "deployment": t.deployment_type,
                }),
            })
        })
        .collect();
    non_empty(ID, listings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_listing_per_cloud_instance() {
        let l = parse(r#"{"instance_types":[
            {"cloud":"lambdalabs","cloud_instance_type":"gpu_1x_h100_sxm5","gpu_type":"H100","num_gpus":1,"hourly_price":436,"interconnect":"sxm"},
            {"cloud":"hyperstack","cloud_instance_type":"n3-H100x1","gpu_type":"H100","num_gpus":1,"hourly_price":190,"interconnect":"pcie"},
            {"cloud":"x","cloud_instance_type":"cpu","gpu_type":"CPU","num_gpus":0,"hourly_price":10}
        ]}"#).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("H100 SXM", 4.36)
        );
        assert_eq!(l[1].gpu_type, "H100 PCIe");
        assert_eq!(l[1].region.as_deref(), Some("hyperstack:n3-H100x1"));
    }
}
