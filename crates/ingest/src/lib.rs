//! Ingestion: news feeds (parsing, URL canonicalization, organization tagging)
//! and GitHub repository stats.

pub mod adapters;
pub mod article_date;
pub mod feed;
pub mod github;
pub mod ingest;
pub mod llm;
pub mod market;
pub mod metrics;
pub mod models_dev;
pub mod tagger;
pub mod terms;
pub mod url;

pub use ingest::{
    FetchReport, IngestError, fetch_source, http_client, load_tagger, redate_all, retag_all,
};
