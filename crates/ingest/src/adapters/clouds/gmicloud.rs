//! GMI Cloud: on-demand GPU prices from its public pricing page.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-gmicloud",
    name: "GMI Cloud",
    organization: "gmi-cloud",
    origin: "https://www.gmicloud.ai",
    paths: &["/en/pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // Stays off: the pricing page only shows "from" per-GPU prices, which are lower bounds.
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("not implemented".into())
}
