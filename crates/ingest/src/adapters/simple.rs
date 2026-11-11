//! Shared plumbing for the simple GPU clouds: one GET, one JSON document,
//! one parser that maps it to [`GpuListing`]s. Each cloud keeps its own
//! module with the parser and its tests; this macro writes the adapter type.

/// Defines a GPU-cloud adapter that GETs `$path` from its origin and hands
/// the body to `parse`.
macro_rules! simple_gpu_adapter {
    ($ty:ident, $id:expr, $name:expr, $org:expr, $interval:expr, $path:expr) => {
        pub struct $ty {
            url: String,
        }

        impl $ty {
            pub fn new(origin: &str) -> Self {
                Self {
                    url: format!("{}{}", origin.trim_end_matches('/'), $path),
                }
            }
        }

        $crate::adapters::adapter_info!($ty, $id, $name, $org, $interval);

        impl $crate::adapters::NeoCloudAdapter for $ty {
            fn fetch_gpus<'a>(
                &'a self,
                client: &'a reqwest::Client,
            ) -> $crate::adapters::BoxFuture<
                'a,
                Result<Vec<$crate::adapters::GpuListing>, $crate::adapters::AdapterError>,
            > {
                Box::pin(async move {
                    let body = $crate::adapters::get_text(client, $id, &self.url).await?;
                    parse(&body)
                })
            }
        }
    };
}
pub(crate) use simple_gpu_adapter;

use super::{AdapterError, GpuListing};

/// Parses JSON or fails with the adapter's format error.
pub(crate) fn json<T: serde::de::DeserializeOwned>(
    adapter: &'static str,
    body: &str,
) -> Result<T, AdapterError> {
    serde_json::from_str(body).map_err(|e| AdapterError::Format(adapter, e.to_string()))
}

/// Fails a run that found nothing: an empty list is a changed API far more
/// often than a cloud with no GPUs.
pub(crate) fn non_empty(
    adapter: &'static str,
    listings: Vec<GpuListing>,
) -> Result<Vec<GpuListing>, AdapterError> {
    if listings.is_empty() {
        return Err(AdapterError::Format(
            adapter,
            "no GPU configurations with a price".into(),
        ));
    }
    Ok(listings)
}

/// The median of a non-empty list.
pub(crate) fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.total_cmp(b));
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}
