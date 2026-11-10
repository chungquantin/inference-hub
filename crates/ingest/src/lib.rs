//! News ingestion: feed parsing, URL canonicalization, organization tagging
//! and storing entries as news items.

pub mod feed;
pub mod ingest;
pub mod tagger;
pub mod url;

pub use ingest::{FetchReport, IngestError, fetch_source, http_client, load_tagger, retag_all};
