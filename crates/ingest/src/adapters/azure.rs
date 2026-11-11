//! Microsoft Azure: pay-as-you-go Linux prices of GPU virtual machines from
//! the public Retail Prices API, in one reference region (East US).
//!
//! VM size names do not state their GPU count, so [`SIZES`] maps each size to
//! its GPUs as Microsoft's size documentation lists them. Sizes not in the
//! table are left out rather than guessed; add a row (with its doc page) to
//! cover a new series.

use serde::Deserialize;

use super::{
    BoxFuture, DAILY, GpuListing, NeoCloudAdapter, adapter_info, get_text, round6,
    simple::{json, non_empty},
};
use crate::adapters::AdapterError;

const ID: &str = "azure";
const REGION: &str = "eastus";

/// VM size → (directory GPU name, GPUs). Sources, under
/// https://learn.microsoft.com/en-us/azure/virtual-machines/ :
/// nd-h100-v5-series, ncads-h100-v5, nc-a100-v4-series, ndm-a100-v4-series,
/// nda100-v4-series, nct4-v3-series, nva10v5-series (fractional A10 sizes left out).
pub const SIZES: &[(&str, &str, i16)] = &[
    ("Standard_ND96isr_H100_v5", "H100 SXM", 8),
    ("Standard_NC40ads_H100_v5", "H100 NVL", 1),
    ("Standard_NC80adis_H100_v5", "H100 NVL", 2),
    ("Standard_NC24ads_A100_v4", "A100 PCIe", 1),
    ("Standard_NC48ads_A100_v4", "A100 PCIe", 2),
    ("Standard_NC96ads_A100_v4", "A100 PCIe", 4),
    ("Standard_ND96amsr_A100_v4", "A100 SXM", 8),
    ("Standard_ND96asr_v4", "A100 SXM 40GB", 8),
    ("Standard_NC4as_T4_v3", "T4", 1),
    ("Standard_NC8as_T4_v3", "T4", 1),
    ("Standard_NC16as_T4_v3", "T4", 1),
    ("Standard_NC64as_T4_v3", "T4", 4),
    ("Standard_NV36ads_A10_v5", "A10", 1),
    ("Standard_NV36adms_A10_v5", "A10", 1),
    ("Standard_NV72ads_A10_v5", "A10", 2),
];

pub struct AzureAdapter {
    origin: String,
}

impl AzureAdapter {
    pub fn new(origin: &str) -> Self {
        Self {
            origin: origin.trim_end_matches('/').to_owned(),
        }
    }

    fn url(&self) -> String {
        let skus = SIZES
            .iter()
            .map(|(s, _, _)| format!("armSkuName eq '{s}'"))
            .collect::<Vec<_>>()
            .join(" or ");
        let filter = format!(
            "serviceName eq 'Virtual Machines' and armRegionName eq '{REGION}' \
             and priceType eq 'Consumption' and ({skus})"
        );
        let mut url =
            reqwest::Url::parse(&format!("{}/api/retail/prices", self.origin)).expect("static URL");
        url.query_pairs_mut().append_pair("$filter", &filter);
        url.into()
    }
}

adapter_info!(AzureAdapter, ID, "Microsoft Azure", "microsoft", DAILY);

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Response {
    items: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    arm_sku_name: String,
    retail_price: f64,
    unit_of_measure: String,
    sku_name: String,
    product_name: String,
    #[serde(rename = "type")]
    kind: String,
}

impl NeoCloudAdapter for AzureAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move { parse(&get_text(client, ID, &self.url()).await?) })
    }
}

pub(crate) fn parse(body: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let response: Response = json(ID, body)?;
    let mut out: Vec<GpuListing> = Vec::new();
    for item in response.items {
        // Linux on-demand only: no Windows licences, Spot or Low Priority.
        if item.kind != "Consumption"
            || item.unit_of_measure != "1 Hour"
            || item.product_name.contains("Windows")
            || item.sku_name.contains("Spot")
            || item.sku_name.contains("Low Priority")
            || !item.retail_price.is_finite()
            || item.retail_price <= 0.0
        {
            continue;
        }
        let Some((sku, gpu_type, count)) = SIZES.iter().find(|(s, _, _)| *s == item.arm_sku_name)
        else {
            continue;
        };
        if out.iter().any(|l| l.region.as_deref() == Some(*sku)) {
            continue;
        }
        out.push(GpuListing {
            external_id: (*gpu_type).to_owned(),
            name: format!("{sku} ({count}× {gpu_type})"),
            gpu_type: (*gpu_type).to_owned(),
            gpu_count: *count,
            price_per_hour: round6(item.retail_price),
            // The size is the configuration; the region is fixed.
            region: Some((*sku).to_owned()),
            source_url: Some(
                "https://azure.microsoft.com/en-us/pricing/details/virtual-machines/linux/".into(),
            ),
            extra: serde_json::json!({ "vm_size": sku, "azure_region": REGION }),
        });
    }
    non_empty(ID, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_on_demand_prices_of_known_sizes() {
        let item = |sku: &str, sku_name: &str, product: &str, price: f64| {
            format!(
                r#"{{"armSkuName":"{sku}","retailPrice":{price},"unitOfMeasure":"1 Hour","skuName":"{sku_name}",
                    "productName":"{product}","type":"Consumption"}}"#
            )
        };
        let body = format!(
            r#"{{"BillingCurrency":"USD","Items":[{},{},{},{},{}]}}"#,
            item(
                "Standard_ND96isr_H100_v5",
                "ND96isr H100 v5",
                "Virtual Machines NDsr H100 v5 Series",
                98.32
            ),
            item(
                "Standard_ND96isr_H100_v5",
                "ND96isr H100 v5 Spot",
                "Virtual Machines NDsr H100 v5 Series",
                30.0
            ),
            item(
                "Standard_ND96isr_H100_v5",
                "ND96isr H100 v5",
                "Virtual Machines NDsr H100 v5 Series Windows",
                110.0
            ),
            item(
                "Standard_NC24ads_A100_v4",
                "NC24ads A100 v4",
                "NCads A100 v4 Series Linux",
                3.673
            ),
            item(
                "Standard_NV6ads_A10_v5",
                "NV6ads A10 v5",
                "NVads A10 v5 Series",
                0.45
            ),
        );
        let l = parse(&body).unwrap();
        assert_eq!(l.len(), 2, "{l:?}");
        assert_eq!(
            (l[0].gpu_type.as_str(), l[0].gpu_count, l[0].price_per_hour),
            ("H100 SXM", 8, 98.32)
        );
        assert_eq!((l[1].gpu_type.as_str(), l[1].gpu_count), ("A100 PCIe", 1));
    }

    #[test]
    fn the_query_names_every_size() {
        let url = AzureAdapter::new("https://prices.azure.com").url();
        assert!(url.contains("Standard_ND96isr_H100_v5"));
        assert!(url.starts_with("https://prices.azure.com/api/retail/prices?%24filter="));
    }
}
