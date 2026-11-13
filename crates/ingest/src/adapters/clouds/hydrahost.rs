//! Hydra Host: on-demand GPU prices from its public pricing page.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-hydrahost",
    name: "Hydra Host",
    organization: "hydra-host",
    origin: "https://hydrahost.com",
    paths: &["/"],
    currency: "USD",
    interval: CLOUD_INTERVAL,
    // Stays off: the homepage only shows marketplace price ranges per GPU, not on-demand list prices.
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("not implemented".into())
}
