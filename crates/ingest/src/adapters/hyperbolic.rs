//! Hyperbolic: on-demand GPU instances. Its rental-options endpoint lists every
//! configuration (GPU, count, region) with its hourly price, without a key.

use reqwest::Client;
use serde::Deserialize;

use super::{Adapter, AdapterError, BoxFuture, GpuListing, NeoCloudAdapter, get_text, gpu, round6};

const ID: &str = "hyperbolic";

pub struct HyperbolicAdapter {
    base_url: String,
}

impl HyperbolicAdapter {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }
}

impl Adapter for HyperbolicAdapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn name(&self) -> &'static str {
        "Hyperbolic"
    }
    fn organization_slug(&self) -> &'static str {
        "hyperbolic"
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RentalOption {
    #[serde(default = "enabled_default")]
    enabled: bool,
    /// US cents per hour for the whole configuration.
    cost_per_hour_cents: i64,
    region: Option<String>,
    gpu_count: i16,
    gpu_type: String,
    gpu_form_factor: Option<String>,
    #[serde(default)]
    supports_spot: bool,
}

fn enabled_default() -> bool {
    true
}

impl NeoCloudAdapter for HyperbolicAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            let url = format!("{}/v2/on-demand/rental-options", self.base_url);
            let body = get_text(client, ID, &url).await?;
            parse(&body)
        })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let options: Vec<RentalOption> =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    Ok(options
        .into_iter()
        .filter(|o| o.enabled && o.cost_per_hour_cents > 0 && o.gpu_count > 0)
        .filter_map(|o| {
            let raw = format!(
                "{} {}",
                o.gpu_type,
                o.gpu_form_factor.as_deref().unwrap_or("")
            );
            let gpu_type = gpu::normalize(&raw)?;
            Some(GpuListing {
                external_id: raw.trim().to_owned(),
                name: format!("{}× {}", o.gpu_count, gpu_type),
                gpu_type,
                gpu_count: o.gpu_count,
                price_per_hour: round6(o.cost_per_hour_cents as f64 / 100.0),
                region: o.region,
                source_url: Some("https://app.hyperbolic.ai/gpus".into()),
                extra: serde_json::json!({ "supports_spot": o.supports_spot }),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_cents_and_keeps_each_configuration() {
        let body = r#"[
            {"enabled":true,"costPerHourCents":349,"region":"us-east-1","gpuCount":1,"gpuType":"h100","gpuFormFactor":"sxm5"},
            {"enabled":true,"costPerHourCents":3592,"region":"us-southeast-2","gpuCount":8,"gpuType":"h200","gpuFormFactor":"sxm5"},
            {"enabled":false,"costPerHourCents":100,"region":"x","gpuCount":1,"gpuType":"h100","gpuFormFactor":"sxm5"}
        ]"#;
        let listings = parse(body).unwrap();
        assert_eq!(listings.len(), 2, "disabled options are left out");
        assert_eq!(listings[0].gpu_type, "H100 SXM");
        assert_eq!(listings[0].price_per_hour, 3.49);
        assert_eq!(listings[1].gpu_count, 8);
        assert_eq!(listings[1].price_per_hour, 35.92);
        assert_eq!(listings[1].region.as_deref(), Some("us-southeast-2"));
        assert_eq!(listings[1].external_id, "h200 sxm5");
    }
}
