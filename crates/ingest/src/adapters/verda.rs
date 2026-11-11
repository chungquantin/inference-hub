//! Verda (formerly DataCrunch): instance types with on-demand and spot prices
//! from its public API.

use serde::Deserialize;

use super::{
    DAILY, GpuListing, gpu, parse_price, round6,
    simple::{json, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "verda";

simple_gpu_adapter!(
    VerdaAdapter,
    ID,
    "Verda (DataCrunch)",
    "verda",
    DAILY,
    "/v1/instance-types"
);

#[derive(Deserialize)]
struct InstanceType {
    instance_type: String,
    price_per_hour: String,
    spot_price: Option<String>,
    gpu: Gpu,
    model: Option<String>,
}

#[derive(Deserialize)]
struct Gpu {
    /// Such as `1x GB300 SXM6 288GB`.
    description: String,
    number_of_gpus: i16,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let types: Vec<InstanceType> = json(ID, body)?;
    let mut out = Vec::new();
    for t in types {
        if t.gpu.number_of_gpus <= 0 {
            continue; // CPU instances
        }
        let price = parse_price(ID, &t.price_per_hour)?;
        if price <= 0.0 {
            continue;
        }
        let raw = t
            .gpu
            .description
            .split_once(' ')
            .map(|(_, rest)| rest.to_owned())
            .unwrap_or(t.gpu.description.clone());
        let Some(gpu_type) =
            gpu::normalize(&raw).or_else(|| t.model.as_deref().and_then(gpu::normalize))
        else {
            continue;
        };
        let spot = t
            .spot_price
            .as_deref()
            .map(|s| parse_price(ID, s))
            .transpose()?;
        out.push(GpuListing {
            external_id: raw,
            name: t.gpu.description,
            gpu_type,
            gpu_count: t.gpu.number_of_gpus,
            price_per_hour: round6(price),
            region: Some(t.instance_type.clone()),
            source_url: Some("https://verda.com/products".into()),
            extra: serde_json::json!({ "instance_type": t.instance_type, "spot_per_hour": spot }),
        });
    }
    non_empty(ID, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_instance_types() {
        let l = parse(
            r#"[
            {"instance_type":"8H100.80S.176V","price_per_hour":"23.92","spot_price":"11.96",
             "gpu":{"description":"8x H100 SXM5 80GB","number_of_gpus":8},"model":"H100"},
            {"instance_type":"CPU.4V","price_per_hour":"0.1","spot_price":null,
             "gpu":{"description":"","number_of_gpus":0},"model":"CPU"}
        ]"#,
        )
        .unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].gpu_count, l[0].price_per_hour),
            ("H100 SXM", 8, 23.92)
        );
        assert_eq!(l[0].extra["spot_per_hour"], 11.96);
    }
}
