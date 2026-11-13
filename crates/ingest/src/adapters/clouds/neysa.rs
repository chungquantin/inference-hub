//! Neysa: on-demand GPU prices from its public pricing page.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-neysa",
    name: "Neysa",
    organization: "neysa",
    origin: "https://neysa.ai",
    paths: &["/pricing/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // Stays off: GPU prices are "starts at" per-GPU lower bounds (36-month reserved).
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("not implemented".into())
}
