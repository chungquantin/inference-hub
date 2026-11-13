//! OVHcloud: hourly prices of Public Cloud GPU instances from its public order catalog.
//!
//! The catalog (`/order/catalog/public/cloud`) is one large JSON document. GPU instances
//! are the addons of the `publiccloud-instance` product whose commercial brick is `gpu`;
//! their consumption pricing is per hour, in 1e-8 of the subsidiary's currency (EUR for
//! `FR`), before tax. Prices are converted to USD with the ECB rate of the day.

use serde_json::Value;

use super::{
    AdapterError, BoxFuture, DAILY, GpuListing, NeoCloudAdapter, adapter_info,
    clouds::{Fx, fx_rate},
    get_text, gpu, round6,
    simple::{json, non_empty},
};

const ID: &str = "ovhcloud";
const SUBSIDIARY: &str = "FR";
/// The catalog lists a dozen GPU plans.
const MIN_EXPECTED: usize = 4;

pub struct OvhCloudAdapter {
    origin: String,
    fx_origin: String,
}

impl OvhCloudAdapter {
    pub fn new(origin: &str, fx_origin: &str) -> Self {
        Self {
            origin: origin.trim_end_matches('/').to_owned(),
            fx_origin: fx_origin.trim_end_matches('/').to_owned(),
        }
    }
}

adapter_info!(OvhCloudAdapter, ID, "OVHcloud", "ovhcloud", DAILY);

impl NeoCloudAdapter for OvhCloudAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            let fx = fx_rate(client, ID, &self.fx_origin, "EUR").await?;
            let url = format!(
                "{}/1.0/order/catalog/public/cloud?ovhSubsidiary={SUBSIDIARY}",
                self.origin
            );
            parse(&get_text(client, ID, &url).await?, &fx)
        })
    }
}

pub(crate) fn parse(body: &str, fx: &Fx) -> Result<Vec<GpuListing>, AdapterError> {
    let root: Value = json(ID, body)?;
    let addons = root["addons"]
        .as_array()
        .ok_or_else(|| AdapterError::Format(ID, "the catalog has no addons".into()))?;
    let mut out: Vec<GpuListing> = Vec::new();
    for a in addons {
        if a["product"] != "publiccloud-instance" || a["blobs"]["commercial"]["brick"] != "gpu" {
            continue;
        }
        let code = a["planCode"].as_str().unwrap_or_default();
        let Some(plan) = code.strip_suffix(".consumption") else {
            continue;
        };
        // Windows images and the like are separate plan codes; Linux on-demand carries the name alone.
        if plan.contains("win") || plan.contains("flex") {
            continue;
        }
        let gpu_info = &a["blobs"]["technical"]["gpu"];
        let (Some(model), Some(count)) = (gpu_info["model"].as_str(), gpu_info["number"].as_i64())
        else {
            continue;
        };
        let Some(hourly) = a["pricings"].as_array().and_then(|ps| {
            ps.iter().find(|p| {
                p["intervalUnit"] == "hour"
                    && p["phase"] == 0
                    && p["capacities"]
                        .as_array()
                        .is_some_and(|c| c.iter().any(|c| c == "consumption"))
            })
        }) else {
            continue;
        };
        let Some(raw) = hourly["price"].as_f64() else {
            continue;
        };
        let native = raw / 1e8;
        if !(native.is_finite() && native > 0.0) || count < 1 {
            continue;
        }
        let memory = &gpu_info["memory"];
        let tech = &a["blobs"]["technical"];
        out.push(GpuListing {
            external_id: plan.to_owned(),
            name: format!("{plan} ({count}× {model})"),
            gpu_type: gpu::normalize(model).unwrap_or_else(|| model.to_owned()),
            gpu_count: count as i16,
            price_per_hour: fx.to_usd(native),
            region: None,
            source_url: Some("https://www.ovhcloud.com/en/public-cloud/prices/".into()),
            extra: serde_json::json!({
                "gpu_model": model,
                "gpu_memory_gb": memory["size"],
                "vcpus": tech["cpu"]["cores"],
                "ram_gb": tech["memory"]["size"],
                "subsidiary": SUBSIDIARY,
                "price_excludes_tax": true,
                "original": fx.original(round6(native)),
            }),
        });
    }
    if out.len() < MIN_EXPECTED {
        return Err(AdapterError::Format(
            ID,
            format!(
                "only {} GPU plans in the catalog; its layout probably changed",
                out.len()
            ),
        ));
    }
    non_empty(ID, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addon(plan: &str, model: &str, n: i64, price: i64) -> String {
        format!(
            r#"{{"planCode":"{plan}.consumption","product":"publiccloud-instance",
                "pricings":[{{"phase":0,"capacities":["consumption"],"intervalUnit":"hour","price":{price}}}],
                "blobs":{{"commercial":{{"brick":"gpu"}},
                         "technical":{{"cpu":{{"cores":30}},"memory":{{"size":380}},
                                       "gpu":{{"model":"{model}","number":{n},"memory":{{"size":80}}}}}}}}}}"#
        )
    }

    fn fx() -> Fx {
        Fx {
            currency: "EUR",
            rate: 1.1,
            date: "2026-11-12".into(),
        }
    }

    #[test]
    fn reads_hourly_gpu_plans_and_converts_to_usd() {
        let body = format!(
            r#"{{"addons":[{},{},{},{},
              {{"planCode":"b3-8.consumption","product":"publiccloud-instance","blobs":{{"commercial":{{"brick":"cpu"}}}}}}]}}"#,
            addon("h100-380", "H100", 1, 280_000_000),
            addon("h100-760", "H100", 2, 560_000_000),
            addon("l40s-90", "L40S", 1, 150_000_000),
            addon("a100-180", "A100", 1, 300_000_000),
        );
        let l = parse(&body, &fx()).unwrap();
        assert_eq!(l.len(), 4, "the CPU plan is left out");
        let h100 = l.iter().find(|x| x.external_id == "h100-760").unwrap();
        assert_eq!((h100.gpu_type.as_str(), h100.gpu_count), ("H100 SXM", 2));
        assert_eq!(h100.price_per_hour, 6.16, "5.60 EUR at 1.1");
        assert_eq!(h100.extra["original"]["currency"], "EUR");
        assert_eq!(h100.extra["original"]["price_per_hour"], 5.6);
    }

    #[test]
    fn a_catalog_without_gpu_plans_is_an_error() {
        assert!(parse(r#"{"addons":[]}"#, &fx()).is_err());
        assert!(parse(r#"{"plans":[]}"#, &fx()).is_err());
    }
}
