//! io.net: a decentralized GPU network. Its explorer's market snapshot gives
//! the hourly price per GPU model.

use serde::Deserialize;

use super::{
    GpuListing, HOURLY, gpu, round6,
    simple::{json, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "ionet";

simple_gpu_adapter!(
    IoNetAdapter,
    ID,
    "io.net",
    "io-net",
    HOURLY,
    "/v1/io-explorer/network/market-snapshot"
);

#[derive(Deserialize)]
struct Response {
    data: Vec<Hardware>,
}

#[derive(Deserialize)]
struct Hardware {
    brand_name: String,
    hardware_name: String,
    price: f64,
    #[serde(rename = "type")]
    kind: String,
    total: Option<i64>,
    hired: Option<i64>,
    baremetal_price: Option<f64>,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let listings = response
        .data
        .into_iter()
        .filter(|h| {
            h.kind == "GPU"
                && (h.brand_name.eq_ignore_ascii_case("nvidia")
                    || h.brand_name.eq_ignore_ascii_case("amd"))
        })
        .filter(|h| h.price.is_finite() && h.price > 0.0)
        .filter_map(|h| {
            let gpu_type = gpu::normalize(&h.hardware_name)?;
            Some(GpuListing {
                name: format!("1× {gpu_type}"),
                gpu_type,
                gpu_count: 1,
                price_per_hour: round6(h.price),
                region: None,
                source_url: Some("https://explorer.io.net/".into()),
                extra: serde_json::json!({
                    "gpus_total": h.total,
                    "gpus_hired": h.hired,
                    "baremetal_per_hour": h.baremetal_price,
                }),
                external_id: h.hardware_name,
            })
        })
        .collect();
    non_empty(ID, listings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpus_with_a_price() {
        let l = parse(r#"{"status":"succeeded","data":[
            {"brand_name":"NVIDIA","hardware_name":"H100 80GB HBM3","price":1.19,"type":"GPU","total":960,"hired":960},
            {"brand_name":"NVIDIA","hardware_name":"RTX PRO 6000 Blackwell Server Edition","price":0.0,"type":"GPU"},
            {"brand_name":"Apple","hardware_name":"M4","price":0.1,"type":"GPU"}
        ]}"#).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("H100 SXM", 1.19)
        );
    }
}
