//! Price curves: how the price of a GPU-hour moves with the term it is bought for, and
//! how the price of a GPU or a model has moved day by day.

use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;

use crate::OrganizationRef;

/// A GPU type or a model that has a curve page.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurveItem {
    pub key: String,
    pub name: String,
    /// Providers selling it.
    pub providers: i64,
    /// Distinct price tiers reported for it. A GPU with 1 has only an on-demand price.
    pub tiers: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurveIndex {
    pub gpus: Vec<CurveItem>,
    pub models: Vec<CurveItem>,
}

/// One provider's price of a GPU-hour at one tier.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TermPoint {
    pub organization: OrganizationRef,
    /// `spot`, `on_demand` or `committed`.
    pub tier: String,
    /// Months committed, 0 for spot and on demand.
    pub term_months: i32,
    pub per_gpu_hour: f64,
    pub source_url: Option<String>,
}

/// The market at one point of the term structure: the spread of the providers that report it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurveNode {
    pub tier: String,
    pub term_months: i32,
    pub providers: i64,
    pub min: f64,
    pub median: f64,
    pub max: f64,
}

/// One day of a price series.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SnapshotPoint {
    pub day: NaiveDate,
    pub providers: i64,
    pub min: f64,
    pub median: f64,
    /// Priciest provider; models do not carry it.
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GpuCurve {
    pub gpu_type: String,
    /// USD per GPU-hour.
    pub nodes: Vec<CurveNode>,
    pub points: Vec<TermPoint>,
    pub history: Vec<SnapshotPoint>,
}

/// One provider's price for a model: a rung of the ladder from cheapest to dearest.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LadderRung {
    pub organization: OrganizationRef,
    /// USD per million tokens, 3:1 input:output blend.
    pub blended_per_mtok: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelCurve {
    pub model_key: String,
    pub name: String,
    pub ladder: Vec<LadderRung>,
    pub history: Vec<SnapshotPoint>,
}
