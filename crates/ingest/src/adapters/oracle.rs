//! Oracle Cloud Infrastructure: the public price list, filtered to
//! pay-as-you-go GPU parts billed per GPU hour.

use serde::Deserialize;

use super::{
    DAILY, GpuListing, gpu, round6,
    simple::{json, non_empty, simple_gpu_adapter},
};
use crate::adapters::AdapterError;

const ID: &str = "oracle";

simple_gpu_adapter!(
    OracleAdapter,
    ID,
    "Oracle Cloud",
    "oracle",
    DAILY,
    "/pls/apex/cetools/api/v1/products/?currencyCode=USD"
);

#[derive(Deserialize)]
struct Response {
    items: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    part_number: String,
    display_name: String,
    metric_name: Option<String>,
    service_category: Option<String>,
    #[serde(default)]
    currency_code_localizations: Vec<Localization>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Localization {
    currency_code: String,
    #[serde(default)]
    prices: Vec<Price>,
}

#[derive(Deserialize)]
struct Price {
    model: String,
    value: f64,
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let mut out = Vec::new();
    for item in response.items {
        let name = &item.display_name;
        // Only the public cloud's own GPU shapes, not Cloud@Customer or VMware.
        if item.metric_name.as_deref() != Some("GPU Per Hour")
            || !name.contains("Compute - GPU")
            || name.contains("Customer")
            || item
                .service_category
                .as_deref()
                .is_some_and(|c| c.contains("VMware"))
        {
            continue;
        }
        let Some(price) = item
            .currency_code_localizations
            .iter()
            .filter(|l| l.currency_code == "USD")
            .flat_map(|l| &l.prices)
            .find(|p| p.model == "PAY_AS_YOU_GO")
            .map(|p| p.value)
            .filter(|p| p.is_finite() && *p > 0.0)
        else {
            continue;
        };
        let model = name.rsplit(" - ").next().unwrap_or("").trim();
        let Some(gpu_type) = gpu::normalize(model) else {
            continue;
        };
        out.push(GpuListing {
            external_id: model.to_owned(),
            name: name.clone(),
            gpu_type,
            gpu_count: 1,
            price_per_hour: round6(price),
            region: None,
            source_url: Some("https://www.oracle.com/cloud/compute/gpu/pricing/".into()),
            extra: serde_json::json!({ "part_number": item.part_number }),
        });
    }
    // The list can carry two parts for one GPU (old and new SKUs); keep the cheapest.
    out.sort_by(|a, b| {
        a.external_id
            .cmp(&b.external_id)
            .then(a.price_per_hour.total_cmp(&b.price_per_hour))
    });
    out.dedup_by(|a, b| a.external_id == b.external_id);
    non_empty(ID, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_gpu_hour_parts_of_the_public_cloud() {
        let usd = |v: f64| {
            format!(
                r#"[{{"currencyCode":"USD","prices":[{{"model":"PAY_AS_YOU_GO","value":{v}}}]}}]"#
            )
        };
        let body = format!(
            r#"{{"items":[
              {{"partNumber":"B1","displayName":"OCI - Compute - GPU - H200","metricName":"GPU Per Hour","serviceCategory":"Compute - GPU","currencyCodeLocalizations":{}}},
              {{"partNumber":"B2","displayName":"OCI - Compute - GPU - MI300X","metricName":"GPU Per Hour","serviceCategory":"Compute - GPU","currencyCodeLocalizations":{}}},
              {{"partNumber":"B3","displayName":"Oracle Compute Cloud@Customer - Compute - GPU.L40S","metricName":"GPU Per Hour","currencyCodeLocalizations":{}}},
              {{"partNumber":"B4","displayName":"Oracle Cloud VMware Solution - BM.GPU.A10.64","metricName":"Node Per Hour","serviceCategory":"Compute - VMware","currencyCodeLocalizations":{}}}
            ]}}"#,
            usd(10.0),
            usd(6.0),
            usd(3.5),
            usd(16.0)
        );
        let l = parse(&body).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].price_per_hour),
            ("H200 SXM", 10.0)
        );
        assert_eq!(l[1].gpu_type, "AMD MI300X");
    }
}
