//! The news adapter for RSS, Atom and JSON Feed sources: one instance per feed.
//!
//! The worker's scheduled ingestion (`ingest::fetch_source`) does conditional
//! requests and stores state per source; this adapter is the plain fetch-and-
//! parse step behind it, usable on its own (for previews and tests).

use reqwest::Client;

use super::{Adapter, AdapterError, BoxFuture, NewsAdapter, get_text};
use crate::feed::{self, FeedEntry};

pub struct RssNewsAdapter {
    pub url: String,
}

impl Adapter for RssNewsAdapter {
    fn id(&self) -> &'static str {
        "rss"
    }
    fn name(&self) -> &'static str {
        "RSS / Atom feed"
    }
    /// Feeds belong to the organization configured on their source, not to the adapter.
    fn organization_slug(&self) -> &'static str {
        ""
    }
}

impl NewsAdapter for RssNewsAdapter {
    fn fetch_entries<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<FeedEntry>, AdapterError>> {
        Box::pin(async move {
            let body = get_text(client, "rss", &self.url).await?;
            let parsed = feed::parse(body.as_bytes(), &self.url)
                .map_err(|e| AdapterError::Format("rss", e.to_string()))?;
            Ok(parsed.entries)
        })
    }
}
