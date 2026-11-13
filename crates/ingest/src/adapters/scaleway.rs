//! Scaleway: GPU instance prices from its public product catalog.
//!
//! `/product-catalog/v2alpha1/public-catalog/products?product_types=instance` lists every
//! instance type in every zone, 100 per page. GPU types carry `properties.hardware.gpu`.
//! The price is in EUR per minute or per hour; the unit is stated in the description
//! ("(0.047775€ per minute)"), so it is read from there and a description without a unit
//! fails the run. Prices are converted to USD with the ECB rate of the day.

use serde_json::Value;

use super::{
    AdapterError, BoxFuture, DAILY, GpuListing, NeoCloudAdapter, adapter_info,
    clouds::{Fx, fx_rate},
    gpu,
    paging::{Items, Paging, fetch_all},
    round6,
    simple::non_empty,
};

const ID: &str = "scaleway";
/// The catalog lists a few dozen GPU offers across zones.
const MIN_EXPECTED: usize = 8;

pub struct ScalewayAdapter {
    url: String,
    fx_origin: String,
}

impl ScalewayAdapter {
    pub fn new(origin: &str, fx_origin: &str) -> Self {
        Self {
            url: format!(
                "{}/product-catalog/v2alpha1/public-catalog/products?product_types=instance&page_size=100",
                origin.trim_end_matches('/')
            ),
            fx_origin: fx_origin.trim_end_matches('/').to_owned(),
        }
    }
}

adapter_info!(ScalewayAdapter, ID, "Scaleway", "scaleway", DAILY);

impl NeoCloudAdapter for ScalewayAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            let fx = fx_rate(client, ID, &self.fx_origin, "EUR").await?;
            let (items, _) = fetch_all(
                client,
                ID,
                &self.url,
                Items::At(&["products"]),
                &Paging::Page {
                    param: "page",
                    first: 1,
                    pages_field: None,
                },
            )
            .await?;
            parse(&items, &fx)
        })
    }
}

/// Hours in the unit the description states: `(… per minute)` is 1/60 of an hour.
fn unit_hours(description: &str) -> Option<f64> {
    if description.contains("per minute") {
        Some(1.0 / 60.0)
    } else if description.contains("per hour") {
        Some(1.0)
    } else {
        None
    }
}

pub(crate) fn parse(items: &[Value], fx: &Fx) -> Result<Vec<GpuListing>, AdapterError> {
    let fail = |m: String| AdapterError::Format(ID, m);
    let mut out: Vec<GpuListing> = Vec::new();
    for p in items {
        let gpu_info = &p["properties"]["hardware"]["gpu"];
        let Some(count) = gpu_info["count"].as_i64().filter(|c| *c > 0) else {
            continue;
        };
        let product = p["product"].as_str().unwrap_or_default();
        let zone = p["locality"]["zone"].as_str().unwrap_or_default();
        let price = &p["price"]["retail_price"];
        if price["currency_code"] != "EUR" {
            return Err(fail(format!(
                "{product} is priced in {}",
                price["currency_code"]
            )));
        }
        let native =
            price["units"].as_f64().unwrap_or(0.0) + price["nanos"].as_f64().unwrap_or(0.0) / 1e9;
        if !(native.is_finite() && native > 0.0) {
            continue;
        }
        let description = p["description"].as_str().unwrap_or_default();
        let hours = unit_hours(description).ok_or_else(|| {
            fail(format!(
                "`{description}` states no price unit (per minute or per hour)"
            ))
        })?;
        let hourly = native / hours;
        let raw_type = gpu_info["type"].as_str().unwrap_or(product);
        out.push(GpuListing {
            external_id: product.to_owned(),
            name: format!("{product} ({count}× {raw_type})"),
            gpu_type: gpu::normalize(raw_type).unwrap_or_else(|| raw_type.to_owned()),
            gpu_count: count as i16,
            price_per_hour: fx.to_usd(hourly),
            region: Some(zone.to_owned()).filter(|z| !z.is_empty()),
            source_url: Some("https://www.scaleway.com/en/pricing/gpu/".into()),
            extra: serde_json::json!({
                "gpu_type": raw_type,
                "vcpus": p["properties"]["hardware"]["cpu"]["virtual"]["count"],
                "ram_bytes": p["properties"]["hardware"]["ram"]["size"],
                "original": fx.original(round6(hourly)),
            }),
        });
    }
    if out.len() < MIN_EXPECTED {
        return Err(fail(format!(
            "only {} GPU offers in the catalog; its layout probably changed",
            out.len()
        )));
    }
    non_empty(ID, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(
        product: &str,
        zone: &str,
        gpu: &str,
        n: i64,
        units: i64,
        nanos: i64,
        unit: &str,
    ) -> Value {
        serde_json::json!({
            "product": product,
            "description": format!("{product} - {zone} (price€ per {unit})"),
            "locality": {"zone": zone},
            "price": {"retail_price": {"currency_code": "EUR", "units": units, "nanos": nanos}},
            "properties": {"hardware": {"gpu": {"count": n, "type": gpu},
                                         "cpu": {"virtual": {"count": 24}}, "ram": {"size": 1}}}
        })
    }

    fn fx() -> Fx {
        Fx {
            currency: "EUR",
            rate: 1.2,
            date: "2026-11-12".into(),
        }
    }

    fn catalog() -> Vec<Value> {
        let mut v = vec![serde_json::json!({"product": "DEV1-S", "properties": {"hardware": {}}})];
        for z in ["fr-par-1", "fr-par-2"] {
            for (i, g) in ["H100-PCIe", "L40S", "L4", "H100-SXM"].iter().enumerate() {
                v.push(item(
                    &format!("GPU-{i}"),
                    z,
                    g,
                    1 + i as i64,
                    0,
                    60_000_000,
                    "minute",
                ));
            }
        }
        v
    }

    #[test]
    fn converts_per_minute_and_per_hour_prices() {
        let mut items = catalog();
        items.push(item(
            "RENDER-S",
            "fr-par-2",
            "P100",
            1,
            1,
            221_000_000,
            "hour",
        ));
        let l = parse(&items, &fx()).unwrap();
        assert_eq!(l.len(), 9, "the non-GPU type is skipped");
        let render = l.iter().find(|x| x.external_id == "RENDER-S").unwrap();
        assert_eq!(render.price_per_hour, 1.4652, "1.221 EUR per hour at 1.2");
        let first = &l[0];
        assert_eq!(
            first.price_per_hour, 4.32,
            "0.06 EUR per minute is 3.60 per hour, at 1.2"
        );
        assert_eq!(first.gpu_type, "H100 PCIe");
        assert_eq!(first.region.as_deref(), Some("fr-par-1"));
        assert_eq!(first.extra["original"]["price_per_hour"], 3.6);
    }

    #[test]
    fn a_price_without_a_stated_unit_fails_the_run() {
        let mut items = catalog();
        items[1]["description"] = Value::String("GPU-0 - fr-par-1".into());
        assert!(parse(&items, &fx()).is_err());
    }

    #[test]
    fn a_catalog_with_few_gpu_offers_is_an_error() {
        assert!(parse(&catalog()[..3], &fx()).is_err());
    }
}
