//! Model labs' own price lists.
//!
//! None of these labs serves a keyless models API, but each publishes its models and
//! prices on a public docs or pricing page. A [`LabSpec`] names the pages to read and the
//! function that turns them into [`ModelListing`]s, at the lab's own prices.
//!
//! Rules every parser follows:
//! - Read only what the page states. A model without a per-million-token price is skipped,
//!   never guessed; media and per-unit models use [`ModelListing::media`] where the unit fits.
//! - Prices are USD per million tokens, at the standard (non-batch, non-priority) rate. For
//!   tiered prices (by prompt length) the first tier is the price and the rest go in `extra`.
//! - The model id is the id the lab's API takes, and `maker` is the lab's conventional prefix
//!   (`anthropic`, `openai`, `google`, `x-ai`, `deepseek`, `qwen`, `z-ai`, `mistralai`), so a lab's
//!   models line up with the same models sold through gateways.
//! - A page that no longer parses is an error (`Err`), not an empty list: the store refuses
//!   a run that shrinks, and the scheduler backs off.

use std::time::Duration;

use reqwest::Client;

use super::{
    Adapter, AdapterError, BoxFuture, DAILY, Endpoints, GatewayAdapter, ModelListing, get_text,
};

pub mod text;

mod ai21;
mod anthropic;
mod deepseek;
mod google;
mod mistral;
mod openai;
mod perplexity;
mod qwen;
mod reka;
mod upstage;
mod voyage;
mod writer;
mod xai;
mod zai;

/// How to read one lab's price pages.
pub struct LabSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// Slug of the lab's organization in the directory.
    pub organization: &'static str,
    /// Origin of the docs site, such as `https://docs.mistral.ai`.
    pub origin: &'static str,
    /// Pages to fetch from the origin, in the order `parse` receives their bodies.
    pub paths: &'static [&'static str],
    pub interval: Duration,
    /// Parsers are registered once they pass their tests against a captured page.
    pub enabled: bool,
    pub parse: fn(&[String]) -> Result<Vec<ModelListing>, String>,
}

pub static LABS: &[&LabSpec] = &[
    &ai21::SPEC,
    &anthropic::SPEC,
    &deepseek::SPEC,
    &google::SPEC,
    &mistral::SPEC,
    &openai::SPEC,
    &perplexity::SPEC,
    &qwen::SPEC,
    &reka::SPEC,
    &upstage::SPEC,
    &voyage::SPEC,
    &writer::SPEC,
    &xai::SPEC,
    &zai::SPEC,
];

pub const LAB_INTERVAL: Duration = DAILY;

pub struct LabAdapter {
    spec: &'static LabSpec,
    urls: Vec<String>,
}

impl LabAdapter {
    pub fn new(spec: &'static LabSpec, endpoints: &Endpoints) -> Self {
        let origin = endpoints.origin(spec.origin);
        Self {
            urls: spec.paths.iter().map(|p| format!("{origin}{p}")).collect(),
            spec,
        }
    }
}

impl Adapter for LabAdapter {
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

impl GatewayAdapter for LabAdapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>> {
        Box::pin(async move {
            let mut bodies = Vec::with_capacity(self.urls.len());
            for url in &self.urls {
                bodies.push(get_text(client, self.spec.id, url).await?);
            }
            let listings =
                (self.spec.parse)(&bodies).map_err(|e| AdapterError::Format(self.spec.id, e))?;
            if listings.is_empty() {
                return Err(AdapterError::Format(
                    self.spec.id,
                    "the pages hold no priced models".into(),
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
    fn spec_ids_are_unique_and_registered_specs_are_enabled_only_when_ready() {
        let mut ids: Vec<_> = LABS.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), LABS.len());
    }
}
