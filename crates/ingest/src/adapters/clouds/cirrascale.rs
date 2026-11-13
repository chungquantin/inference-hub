//! Cirrascale: on-demand GPU prices from its public pricing page.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-cirrascale",
    name: "Cirrascale",
    organization: "cirrascale",
    origin: "https://www.cirrascale.com",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // Stays off: the pricing page only shows "as low as" per-GPU prices, which are lower bounds.
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("not implemented".into())
}
