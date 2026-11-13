//! Sakura Internet: left disabled. The service page (`/gpu/`) only has "from" prices
//! (`57.6円/時間〜`, `481円/時間〜`) in a comparison table, with no GPU count. The 高火力 DOK
//! page (`/gpu/koukaryoku-dok/`) lists JPY per second and hour for a "V100 plan", an "H100
//! plan" and an "8GPU 専有プラン" without saying how many GPUs the H100 plan has, and the
//! 8-GPU plan (2,988円/h) is cheaper than the single H100 plan (1,008円/h), so no price can
//! be tied to a stated GPU configuration.

use super::{CLOUD_INTERVAL, CloudSpec, Fx};
use crate::adapters::GpuListing;

pub static SPEC: CloudSpec = CloudSpec {
    id: "cloud-sakura",
    name: "Sakura Internet",
    organization: "sakura-internet",
    origin: "https://ai.sakura.ad.jp",
    paths: &["/gpu/"],
    currency: "JPY",
    interval: CLOUD_INTERVAL,
    // Turn on once `parse` passes its tests against a captured page.
    enabled: false,
    parse,
};

fn parse(_pages: &[String], _fx: &Fx) -> Result<Vec<GpuListing>, String> {
    Err("sakura: no hourly price is tied to a stated GPU configuration".into())
}
