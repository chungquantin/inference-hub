//! Akash Network: a decentralized compute marketplace. Its console publishes
//! the median, min and max of current provider bids per GPU model.

use serde::Deserialize;

use super::{
    GpuListing, HOURLY, gpu, round6,
    simple::{json, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "akash";

simple_gpu_adapter!(
    AkashAdapter,
    ID,
    "Akash Network",
    "akash-network",
    HOURLY,
    "/v1/gpu-prices",
    volatile
);

#[derive(Deserialize)]
struct Response {
    models: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    vendor: String,
    model: String,
    ram: String,
    interface: String,
    availability: Availability,
    price: Option<Price>,
}

#[derive(Deserialize)]
struct Availability {
    total: i64,
    available: i64,
}

#[derive(Deserialize)]
struct Price {
    min: Option<f64>,
    max: Option<f64>,
    med: Option<f64>,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let listings = response
        .models
        .into_iter()
        .filter_map(|m| {
            let price = m.price?;
            let med = price.med.filter(|p| p.is_finite() && *p > 0.0)?;
            let raw = format!("{} {} {} {}", m.vendor, m.model, m.ram, m.interface);
            let gpu_type = gpu::normalize(&format!("{} {} {}", m.model, m.ram, m.interface))?;
            Some(GpuListing {
                name: format!("1× {gpu_type}"),
                gpu_type,
                gpu_count: 1,
                price_per_hour: round6(med),
                region: None,
                source_url: Some("https://akash.network/pricing/gpus/".into()),
                extra: serde_json::json!({
                    "price": "median of provider bids",
                    "min_per_hour": price.min,
                    "max_per_hour": price.max,
                    "gpus_total": m.availability.total,
                    "gpus_available": m.availability.available,
                }),
                external_id: raw,
            })
        })
        .collect();
    non_empty(ID, listings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_bid_per_model() {
        let l = parse(r#"{"availability":{"total":397,"available":103},"models":[
            {"vendor":"nvidia","model":"h100","ram":"80Gi","interface":"SXM5","availability":{"total":56,"available":8},
             "price":{"min":1.8,"max":3.1,"med":2.56}},
            {"vendor":"nvidia","model":"a100","ram":"40Gi","interface":"SXM4","availability":{"total":1,"available":1},
             "price":{"min":0.84,"max":0.84,"med":0.84}},
            {"vendor":"nvidia","model":"gtx1050ti","ram":"4Gi","interface":"PCIe","availability":{"total":1,"available":1},
             "price":{"min":null,"max":null,"med":null}}
        ]}"#).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("H100 SXM", 2.56)
        );
        assert_eq!(l[1].gpu_type, "A100 SXM 40GB");
        assert_eq!(l[0].extra["gpus_total"], 56);
    }
}
