//! Ingestion: news feeds (parsing, URL canonicalization, organization tagging)
//! and GitHub repository stats.

pub mod adapters;
pub mod feed;
pub mod github;
pub mod ingest;
pub mod market;
pub mod tagger;
pub mod url;

pub use ingest::{FetchReport, IngestError, fetch_source, http_client, load_tagger, retag_all};
