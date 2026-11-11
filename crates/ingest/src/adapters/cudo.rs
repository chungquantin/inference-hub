//! Cudo Compute: per-GPU prices of each machine type and data center from its
//! public API. vCPU and memory are billed on top; the listing carries the GPU
//! price only, and says so in `extra`.

use serde::Deserialize;

use super::{
    DAILY, GpuListing, gpu, parse_price, round6,
    simple::{json, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "cudo";

simple_gpu_adapter!(
    CudoAdapter,
    ID,
    "Cudo Compute",
    "cudo-compute",
    DAILY,
    "/v1/vms/machine-types"
);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    machine_types: Vec<MachineType>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachineType {
    data_center_id: String,
    machine_type: String,
    gpu_model: String,
    gpu_price_hr: Value,
    vcpu_price_hr: Value,
    memory_gib_price_hr: Value,
}

#[derive(Deserialize)]
struct Value {
    value: String,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let mut out = Vec::new();
    for m in response.machine_types {
        let price = parse_price(ID, &m.gpu_price_hr.value)?;
        if price <= 0.0 {
            continue;
        }
        let raw = m.gpu_model.replace("(graphics mode)", "");
        let Some(gpu_type) = gpu::normalize(raw.trim()) else {
            continue;
        };
        out.push(GpuListing {
            external_id: raw.trim().to_owned(),
            name: format!("1× {gpu_type} ({})", m.machine_type),
            gpu_type,
            gpu_count: 1,
            price_per_hour: round6(price),
            region: Some(m.data_center_id),
            source_url: Some("https://www.cudocompute.com/pricing".into()),
            extra: serde_json::json!({
                "price": "GPU only; vCPU and memory are billed separately",
                "machine_type": m.machine_type,
                "vcpu_per_hour": parse_price(ID, &m.vcpu_price_hr.value)?,
                "memory_gib_per_hour": parse_price(ID, &m.memory_gib_price_hr.value)?,
            }),
        });
    }
    non_empty(ID, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_price_per_data_center() {
        let l = parse(r#"{"machineTypes":[{"dataCenterId":"no-kristiansand-1","machineType":"epyc-genoa-l40s-graphics",
            "gpuModel":"L40S (graphics mode)","gpuPriceHr":{"value":"0.870000"},"vcpuPriceHr":{"value":"0.0049"},
            "memoryGibPriceHr":{"value":"0.0049"}}]}"#).unwrap();
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("L40S", 0.87)
        );
        assert_eq!(l[0].region.as_deref(), Some("no-kristiansand-1"));
    }
}
