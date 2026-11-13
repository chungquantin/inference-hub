//! Latitude.sh: on-demand GPU prices from its public pricing page.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-latitude",
    name: "Latitude.sh",
    organization: "latitude-sh",
    origin: "https://www.latitude.sh",
    paths: &["/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // Left off: the page (and the per-plan pages, which render the same server-side HTML)
    // show only "Starting At" prices, the lowest across locations; per-location prices
    // load client-side and the plans API needs a key.
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("not implemented".into())
}
