//! Northern Data (Taiga Cloud): on-demand GPU prices from its public pricing page.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-northerndata",
    name: "Northern Data (Taiga Cloud)",
    organization: "northern-data",
    origin: "https://northerndata.de",
    paths: &["/taiga-cloud-ai-pricing"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // Stays off: GPU prices are "from" lower bounds; the only exact hourly prices are for CPU-only vCPU plans.
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("not implemented".into())
}
