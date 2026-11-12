//! Metrics: numbers that describe players in the inference market over time,
//! beyond prices. Model downloads, serving speed, repository
//! stars. Each metric is a set of daily series, one per organization, model or project.

use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;

use crate::OrganizationRef;

/// What a metric measures. The list is fixed in code so sources and pages agree on units.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct MetricDef {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// `usd`, `count`, `tokens_per_second` or `ms`.
    pub unit: &'static str,
    /// The section it sits under: `Adoption`, `Serving` or `Open source`.
    pub group: &'static str,
    /// `organization`, `model` or `project`.
    pub subject_type: &'static str,
    /// Where the numbers come from.
    pub source: &'static str,
    pub source_url: &'static str,
    /// Whether a bigger number is the better one. `false` for latency.
    pub higher_is_better: bool,
    /// Whether the values add up to a meaningful total (market caps do, speeds do not).
    pub additive: bool,
}

pub const METRICS: &[MetricDef] = &[
    MetricDef {
        key: "hf_downloads_30d",
        label: "Model downloads (30d)",
        description: "Downloads in the last 30 days of each lab's most downloaded models on Hugging Face.",
        unit: "count",
        group: "Adoption",
        subject_type: "organization",
        source: "Hugging Face Hub",
        source_url: "https://huggingface.co/models",
        higher_is_better: true,
        additive: true,
    },
    MetricDef {
        key: "hf_likes",
        label: "Model likes",
        description: "Likes on each lab's most downloaded models on Hugging Face.",
        unit: "count",
        group: "Adoption",
        subject_type: "organization",
        source: "Hugging Face Hub",
        source_url: "https://huggingface.co/models",
        higher_is_better: true,
        additive: true,
    },
    MetricDef {
        key: "model_downloads_30d",
        label: "Downloads by model (30d)",
        description: "The most downloaded models on Hugging Face in the last 30 days.",
        unit: "count",
        group: "Adoption",
        subject_type: "model",
        source: "Hugging Face Hub",
        source_url: "https://huggingface.co/models",
        higher_is_better: true,
        additive: true,
    },
    MetricDef {
        key: "provider_throughput",
        label: "Serving speed",
        description: "Median output speed of each inference provider across the models it serves through Hugging Face Inference Providers.",
        unit: "tokens_per_second",
        group: "Serving",
        subject_type: "organization",
        source: "Hugging Face Inference Providers",
        source_url: "https://huggingface.co/docs/inference-providers",
        higher_is_better: true,
        additive: false,
    },
    MetricDef {
        key: "provider_first_token",
        label: "Time to first token",
        description: "Median wait before the first token, per inference provider, across the models it serves through Hugging Face Inference Providers.",
        unit: "ms",
        group: "Serving",
        subject_type: "organization",
        source: "Hugging Face Inference Providers",
        source_url: "https://huggingface.co/docs/inference-providers",
        higher_is_better: false,
        additive: false,
    },
    MetricDef {
        key: "model_throughput",
        label: "Fastest model speed",
        description: "Output speed of each model at the fastest provider that serves it.",
        unit: "tokens_per_second",
        group: "Serving",
        subject_type: "model",
        source: "Hugging Face Inference Providers",
        source_url: "https://huggingface.co/docs/inference-providers",
        higher_is_better: true,
        additive: false,
    },
    MetricDef {
        key: "github_stars",
        label: "GitHub stars",
        description: "Stars of the open-source inference projects in the directory.",
        unit: "count",
        group: "Open source",
        subject_type: "project",
        source: "GitHub",
        source_url: "https://github.com",
        higher_is_better: true,
        additive: true,
    },
    MetricDef {
        key: "github_forks",
        label: "GitHub forks",
        description: "Forks of the open-source inference projects in the directory.",
        unit: "count",
        group: "Open source",
        subject_type: "project",
        source: "GitHub",
        source_url: "https://github.com",
        higher_is_better: true,
        additive: true,
    },
];

pub fn metric_def(key: &str) -> Option<&'static MetricDef> {
    METRICS.iter().find(|m| m.key == key)
}

/// One subject in a ranking of a metric.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricRow {
    pub rank: i64,
    pub subject_key: String,
    pub name: String,
    /// The organization the subject belongs to, when the directory has it.
    pub organization: Option<OrganizationRef>,
    pub value: f64,
    /// Change over 7, 30 and 90 days, as a fraction (0.12 is +12%). Absent until the series is that old.
    pub change_7d: Option<f64>,
    pub change_30d: Option<f64>,
    pub change_90d: Option<f64>,
    /// Share of the metric's total, for metrics that add up.
    pub share: Option<f64>,
    /// The last 60 days, oldest first.
    pub spark: Vec<f64>,
}

/// A metric with its current leaders: one card of the terminal's overview.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricSummary {
    #[serde(flatten)]
    pub def: MetricDef,
    pub subjects: i64,
    pub as_of: Option<NaiveDate>,
    /// Sum of the latest values, for metrics that add up.
    pub total: Option<f64>,
    pub total_change_30d: Option<f64>,
    /// Days of history behind the oldest series.
    pub history_days: i64,
    pub leaders: Vec<MetricRow>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricCatalog {
    pub metrics: Vec<MetricSummary>,
    pub sources: Vec<MetricSourceStatus>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MetricSourceStatus {
    pub source: String,
    pub last_success_at: Option<chrono::DateTime<chrono::Utc>>,
    pub consecutive_failures: i32,
    pub last_error: Option<String>,
    pub series: i32,
}

/// One day's value of a series.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MetricPoint {
    pub day: NaiveDate,
    pub value: f64,
}

/// One line of the chart.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricLine {
    pub subject_key: String,
    pub name: String,
    pub organization: Option<OrganizationRef>,
    /// Oldest first.
    pub points: Vec<MetricPoint>,
}

/// A whole metric: its ranking and the history of its leaders.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricBoard {
    #[serde(flatten)]
    pub def: MetricDef,
    pub as_of: Option<NaiveDate>,
    pub total: Option<f64>,
    pub total_change_30d: Option<f64>,
    pub history_days: i64,
    pub rows: Vec<MetricRow>,
    /// The ten leaders' history over the last year.
    pub lines: Vec<MetricLine>,
}
