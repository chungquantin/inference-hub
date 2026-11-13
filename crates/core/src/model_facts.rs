//! What is known about a model apart from its price, from models.dev.

use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ModelFacts {
    pub model_key: String,
    pub name: String,
    /// The model family, such as `claude-opus` or `gpt-oss`.
    pub family: Option<String>,
    pub description: Option<String>,
    pub released_on: Option<NaiveDate>,
    /// Month the training data ends, such as `2026-06`.
    pub knowledge_cutoff: Option<String>,
    pub open_weights: Option<bool>,
    /// The largest context any provider states, in tokens.
    pub context_window: Option<i32>,
    pub max_output: Option<i32>,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub reasoning: Option<bool>,
    pub tool_call: Option<bool>,
    pub structured_output: Option<bool>,
    pub attachment: Option<bool>,
    pub deprecated: bool,
    /// Providers that list the model at the source.
    pub providers_listing: i32,
    pub source: String,
}
