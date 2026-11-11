//! Vast.ai: a marketplace where hosts list their own machines. Prices move by
//! the hour and differ per host, so the directory records the median on-demand
//! price per GPU and count among verified, rentable offers, with the range and
//! the number of offers in `extra`.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{
    GpuListing, HOURLY, gpu, round6,
    simple::{json, median, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "vastai";

// Rentable, on-demand offers, as many as the API returns in one page.
simple_gpu_adapter!(
    VastAiAdapter,
    ID,
    "Vast.ai",
    "vast-ai",
    HOURLY,
    "/api/v0/bundles/?q=%7B%22rentable%22%3A%7B%22eq%22%3Atrue%7D%2C%22type%22%3A%22on-demand%22%2C%22limit%22%3A3000%7D",
    volatile
);

#[derive(Deserialize)]
struct Response {
    offers: Vec<Offer>,
}

#[derive(Deserialize)]
struct Offer {
    gpu_name: String,
    num_gpus: i16,
    /// USD per hour for the whole offer, storage included.
    dph_total: f64,
    #[serde(default)]
    verification: Option<String>,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let mut groups: BTreeMap<(String, String, i16), Vec<f64>> = BTreeMap::new();
    let mut fallback: BTreeMap<(String, String, i16), Vec<f64>> = BTreeMap::new();
    for o in response.offers {
        if o.num_gpus <= 0 || !o.dph_total.is_finite() || o.dph_total <= 0.0 {
            continue;
        }
        let Some(gpu_type) = gpu::normalize(&o.gpu_name) else {
            continue;
        };
        let key = (o.gpu_name, gpu_type, o.num_gpus);
        // Prefer verified hosts; use the rest only for GPUs no verified host offers.
        if o.verification.as_deref() == Some("verified") {
            groups.entry(key).or_default().push(o.dph_total);
        } else {
            fallback.entry(key).or_default().push(o.dph_total);
        }
    }
    for (k, v) in fallback {
        groups.entry(k).or_insert(v);
    }
    let listings = groups
        .into_iter()
        .map(|((raw, gpu_type, count), prices)| {
            let (min, max) = prices
                .iter()
                .fold((f64::MAX, 0.0_f64), |(a, b), p| (a.min(*p), b.max(*p)));
            GpuListing {
                name: format!("{count}× {gpu_type}"),
                gpu_count: count,
                price_per_hour: round6(median(prices.clone())),
                region: None,
                source_url: Some("https://cloud.vast.ai/create/".into()),
                extra: serde_json::json!({
                    "price": "median of host offers",
                    "offers": prices.len(),
                    "min_per_hour": round6(min),
                    "max_per_hour": round6(max),
                }),
                external_id: raw,
                gpu_type,
            }
        })
        .collect();
    non_empty(ID, listings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_per_gpu_and_count_preferring_verified_hosts() {
        let body = r#"{"offers":[
            {"gpu_name":"RTX 5090","num_gpus":1,"dph_total":0.30,"verification":"verified"},
            {"gpu_name":"RTX 5090","num_gpus":1,"dph_total":0.50,"verification":"verified"},
            {"gpu_name":"RTX 5090","num_gpus":1,"dph_total":0.40,"verification":"verified"},
            {"gpu_name":"RTX 5090","num_gpus":1,"dph_total":0.05,"verification":"deverified"},
            {"gpu_name":"RTX 5090","num_gpus":2,"dph_total":0.90,"verification":"verified"},
            {"gpu_name":"Tesla V100","num_gpus":1,"dph_total":0.12,"verification":"unverified"},
            {"gpu_name":"CMP 170HX","num_gpus":1,"dph_total":0.1,"verification":"verified"}
        ]}"#;
        let l = parse(body).unwrap();
        assert_eq!(l.len(), 3, "unknown GPUs are left out: {l:?}");
        let one = l
            .iter()
            .find(|l| l.gpu_type == "RTX 5090" && l.gpu_count == 1)
            .unwrap();
        assert_eq!(one.price_per_hour, 0.4, "the deverified outlier is ignored");
        assert_eq!(one.extra["offers"], 3);
        let v100 = l.iter().find(|l| l.gpu_type == "V100").unwrap();
        assert_eq!(v100.price_per_hour, 0.12, "unverified hosts fill gaps");
    }

    #[test]
    fn rejects_empty_or_changed() {
        assert!(parse(r#"{"offers":[]}"#).is_err());
        assert!(parse(r#"{"bundles":[]}"#).is_err());
    }
}
