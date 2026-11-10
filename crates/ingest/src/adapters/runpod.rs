//! RunPod: GPU pods by the hour. Its public GraphQL API lists every GPU type
//! with the per-GPU price in Secure Cloud (data centers) and Community Cloud.
//!
//! The directory records the Secure Cloud on-demand price for one GPU, the
//! like-for-like comparison with other clouds. Community prices go in `extra`.

use reqwest::Client;
use serde::Deserialize;

use super::{Adapter, AdapterError, BoxFuture, GpuListing, NeoCloudAdapter, gpu, round6};

const ID: &str = "runpod";
const QUERY: &str = "query { gpuTypes { id displayName memoryInGb securePrice communityPrice } }";

pub struct RunPodAdapter {
    base_url: String,
}

impl RunPodAdapter {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }
}

impl Adapter for RunPodAdapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn name(&self) -> &'static str {
        "RunPod"
    }
    fn organization_slug(&self) -> &'static str {
        "runpod"
    }
}

#[derive(Deserialize)]
struct Response {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    gpu_types: Vec<GpuType>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuType {
    id: String,
    display_name: String,
    memory_in_gb: Option<i32>,
    /// USD per GPU-hour; 0 when not offered in Secure Cloud.
    secure_price: Option<f64>,
    community_price: Option<f64>,
}

impl NeoCloudAdapter for RunPodAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            let response = client
                .post(format!("{}/graphql", self.base_url))
                .json(&serde_json::json!({ "query": QUERY }))
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(AdapterError::Status(ID, response.status()));
            }
            parse(&response.text().await?)
        })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    if !response.errors.is_empty() {
        return Err(AdapterError::Format(
            ID,
            format!("GraphQL errors: {:?}", response.errors),
        ));
    }
    let data = response
        .data
        .ok_or_else(|| AdapterError::Format(ID, "no data".into()))?;

    Ok(data
        .gpu_types
        .into_iter()
        .filter_map(|g| {
            let price = g.secure_price.filter(|p| p.is_finite() && *p > 0.0)?;
            let gpu_type = gpu::normalize(&g.id)?;
            Some(GpuListing {
                name: g.display_name,
                gpu_type,
                gpu_count: 1,
                price_per_hour: round6(price),
                region: None,
                source_url: Some("https://www.runpod.io/pricing".into()),
                extra: serde_json::json!({
                    "memory_gb": g.memory_in_gb,
                    "community_price_per_hour": g.community_price.filter(|p| *p > 0.0),
                }),
                external_id: g.id,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_secure_cloud_prices_of_known_gpus() {
        let body = r#"{"data":{"gpuTypes":[
            {"id":"NVIDIA H100 80GB HBM3","displayName":"H100 SXM","memoryInGb":80,"securePrice":3.99,"communityPrice":2.69},
            {"id":"NVIDIA A100-SXM4-40GB","displayName":"A100 SXM 40GB","memoryInGb":40,"securePrice":0,"communityPrice":1},
            {"id":"Mystery Accelerator","displayName":"Mystery","memoryInGb":1,"securePrice":9.0,"communityPrice":0}
        ]}}"#;
        let listings = parse(body).unwrap();
        assert_eq!(
            listings.len(),
            1,
            "no secure price, or unknown GPU: left out"
        );
        let h100 = &listings[0];
        assert_eq!(h100.gpu_type, "H100 SXM");
        assert_eq!(h100.gpu_count, 1);
        assert_eq!(h100.price_per_hour, 3.99);
        assert_eq!(h100.extra["community_price_per_hour"], 2.69);
    }

    #[test]
    fn graphql_errors_fail_the_run() {
        assert!(parse(r#"{"data":null,"errors":[{"message":"rate limited"}]}"#).is_err());
    }
}
