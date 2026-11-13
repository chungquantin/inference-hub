//! GPU clouds that publish their on-demand prices on a public pricing page.
//!
//! A [`CloudSpec`] names the pages to read and the function that turns them into
//! [`GpuListing`]s. One parser per cloud, each in its own file with a test against a
//! captured excerpt of the real page.
//!
//! Rules every parser follows:
//! - Only prices the page states for renting that GPU configuration on demand. "From $X",
//!   "starting at", "as low as" and reserved-term prices are lower bounds, not list
//!   prices: leave them out (reserved and spot prices may go in `extra`).
//! - `price_per_hour` is for the whole configuration in USD, `gpu_count` its GPUs, and
//!   `gpu_type` is the name from [`super::gpu::normalize`] (an unrecognized GPU is kept
//!   with `gpu_type` set to the provider's own name, never guessed).
//! - Prices in another currency are converted with the [`Fx`] the adapter passes in (the
//!   European Central Bank's rate of the day); the original price, currency and rate go in
//!   `extra.original`.
//! - A page that no longer parses is an error, not an empty list.

use std::time::Duration;

use reqwest::Client;

use super::{
    Adapter, AdapterError, BoxFuture, DAILY, Endpoints, GpuListing, NeoCloudAdapter, get_text,
};

mod cirrascale;
mod civo;
mod crusoe;
mod denvr;
mod digitalocean;
mod e2e;
mod gcore;
mod gmicloud;
mod hydrahost;
mod hyperstack;
mod jarvislabs;
mod latitude;
mod lightning;
mod liquidweb;
mod lyceum;
mod massed;
mod mithril;
mod nebius;
mod neysa;
mod northerndata;
mod northflank;
mod oblivus;
mod packet;
mod paperspace;
mod runcrate;
mod sakura;
mod salad;
mod saturn;
mod thunder;
mod together;

/// An exchange rate to USD for the currency a page prices in.
#[derive(Debug, Clone)]
pub struct Fx {
    pub currency: &'static str,
    pub rate: f64,
    /// The date of the rate, `YYYY-MM-DD`.
    pub date: String,
}

impl Fx {
    /// USD itself: rate 1.
    pub fn usd() -> Self {
        Self {
            currency: "USD",
            rate: 1.0,
            date: String::new(),
        }
    }

    /// A price in the page's currency, in USD.
    pub fn to_usd(&self, native: f64) -> f64 {
        super::round6(native * self.rate)
    }

    /// `extra.original` for a converted price, `null` when the page prices in USD.
    pub fn original(&self, native: f64) -> serde_json::Value {
        if self.currency == "USD" {
            serde_json::Value::Null
        } else {
            serde_json::json!({
                "currency": self.currency,
                "price_per_hour": native,
                "rate_to_usd": self.rate,
                "rate_date": self.date,
            })
        }
    }
}

/// How to read one cloud's price pages.
pub struct CloudSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Slug of the cloud's organization in the directory.
    pub organization: &'static str,
    pub origin: &'static str,
    /// Pages to fetch from the origin, in the order `parse` receives their bodies.
    pub paths: &'static [&'static str],
    /// Currency the pages price in: `USD`, `EUR`, `JPY`...
    pub currency: &'static str,
    pub interval: Duration,
    /// Parsers are registered once they pass their tests against a captured page.
    pub enabled: bool,
    pub parse: fn(&[String], &Fx) -> Result<Vec<GpuListing>, String>,
}

pub static CLOUDS: &[&CloudSpec] = &[
    &cirrascale::SPEC,
    &civo::SPEC,
    &crusoe::SPEC,
    &denvr::SPEC,
    &digitalocean::SPEC,
    &e2e::SPEC,
    &gcore::SPEC,
    &gmicloud::SPEC,
    &hydrahost::SPEC,
    &hyperstack::SPEC,
    &jarvislabs::SPEC,
    &latitude::SPEC,
    &lightning::SPEC,
    &liquidweb::SPEC,
    &lyceum::SPEC,
    &massed::SPEC,
    &mithril::SPEC,
    &nebius::SPEC,
    &neysa::SPEC,
    &northerndata::SPEC,
    &northflank::SPEC,
    &oblivus::SPEC,
    &packet::SPEC,
    &paperspace::SPEC,
    &runcrate::SPEC,
    &sakura::SPEC,
    &salad::SPEC,
    &saturn::SPEC,
    &thunder::SPEC,
    &together::SPEC,
];

pub const CLOUD_INTERVAL: Duration = DAILY;

/// The ECB reference rate of `currency` to USD, from Frankfurter.
pub async fn fx_rate(
    client: &Client,
    adapter: &'static str,
    origin: &str,
    currency: &'static str,
) -> Result<Fx, AdapterError> {
    if currency == "USD" {
        return Ok(Fx::usd());
    }
    let url = format!("{origin}/v1/latest?base={currency}&symbols=USD");
    let body = get_text(client, adapter, &url).await?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AdapterError::Format(adapter, format!("exchange rate: {e}")))?;
    let rate = v["rates"]["USD"].as_f64().filter(|r| *r > 0.0);
    let date = v["date"].as_str();
    match (rate, date) {
        (Some(rate), Some(date)) => Ok(Fx {
            currency,
            rate,
            date: date.to_owned(),
        }),
        _ => Err(AdapterError::Format(
            adapter,
            format!("no {currency} to USD rate in the exchange-rate response"),
        )),
    }
}

pub struct CloudAdapter {
    spec: &'static CloudSpec,
    urls: Vec<String>,
    fx_origin: String,
}

impl CloudAdapter {
    pub fn new(spec: &'static CloudSpec, endpoints: &Endpoints) -> Self {
        let origin = endpoints.origin(spec.origin);
        Self {
            urls: spec.paths.iter().map(|p| format!("{origin}{p}")).collect(),
            fx_origin: endpoints.origin("https://api.frankfurter.dev"),
            spec,
        }
    }
}

impl Adapter for CloudAdapter {
    fn id(&self) -> &'static str {
        self.spec.id
    }
    fn name(&self) -> &'static str {
        self.spec.name
    }
    fn organization_slug(&self) -> &'static str {
        self.spec.organization
    }
    fn interval(&self) -> Duration {
        self.spec.interval
    }
}

impl NeoCloudAdapter for CloudAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            let fx = fx_rate(client, self.spec.id, &self.fx_origin, self.spec.currency).await?;
            let mut bodies = Vec::with_capacity(self.urls.len());
            for url in &self.urls {
                bodies.push(get_text(client, self.spec.id, url).await?);
            }
            let listings = (self.spec.parse)(&bodies, &fx)
                .map_err(|e| AdapterError::Format(self.spec.id, e))?;
            if listings.is_empty() {
                return Err(AdapterError::Format(
                    self.spec.id,
                    "the pages hold no priced GPU configurations".into(),
                ));
            }
            Ok(listings)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_ids_are_unique() {
        let mut ids: Vec<_> = CLOUDS.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CLOUDS.len());
    }

    #[test]
    fn fx_converts_and_records_the_original() {
        let fx = Fx {
            currency: "EUR",
            rate: 1.1,
            date: "2026-11-12".into(),
        };
        assert_eq!(fx.to_usd(2.0), 2.2);
        assert_eq!(fx.original(2.0)["currency"], "EUR");
        assert!(Fx::usd().original(2.0).is_null());
    }
}
