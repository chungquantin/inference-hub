//! Site-wide search across organizations, products, market models and GPUs,
//! and news.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// One search result.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SearchHit {
    /// `organization`, `product`, `model`, `media`, `gpu` or `news`.
    pub kind: String,
    /// The site path of the result, such as `/organizations/openai`.
    pub href: String,
    pub title: String,
    /// A short second line: maker, kind, price or source.
    pub subtitle: Option<String>,
    pub logo_url: Option<String>,
    /// Models: the maker as providers name it (`deepseek-ai`), for its logo.
    pub maker: Option<String>,
    /// Higher is better; exact and prefix matches score above substrings.
    pub score: f32,
    pub date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// The text to search for (at least 2 characters).
    pub q: String,
    /// Only these kinds, comma-separated, such as `model,gpu`.
    pub kinds: Option<String>,
    /// Results per kind. Default 6, at most 50.
    pub per_kind: Option<i64>,
}

/// Results grouped by kind, best first, with how many of each matched.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SearchResults {
    pub q: String,
    pub hits: Vec<SearchHit>,
    pub counts: Vec<SearchCount>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SearchCount {
    pub kind: String,
    pub count: i64,
}
