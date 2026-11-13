//! The landscape: every organization with what a map of the market needs, in one response.

use chrono::NaiveDate;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{OrgKind, OrgStatus};

/// One funding round, as the landscape shows it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LandscapeRound {
    pub round: String,
    pub announced_on: NaiveDate,
    /// Amount raised, in `currency`.
    pub amount: Option<i64>,
    /// Post-money valuation, in `currency`.
    pub valuation: Option<i64>,
    pub currency: String,
    pub source_url: Option<String>,
    /// Names of the investors that led the round.
    pub leads: Vec<String>,
    /// Names of the other investors in the directory.
    pub investors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LandscapeItem {
    pub slug: String,
    pub name: String,
    pub kinds: Vec<OrgKind>,
    /// Path of the logo, such as `/logos/openai.svg`.
    pub logo_url: Option<String>,
    /// The tagline, or the start of the description when there is no tagline.
    pub summary: Option<String>,
    pub website: Option<String>,
    pub status: OrgStatus,
    pub hq_country: Option<String>,
    pub founded_year: Option<i16>,
    /// Distinct chat models the organization sells at a price in the market, 0 if it sells none.
    pub models_sold: i64,
    /// GPU configurations it rents at a price in the market.
    pub gpu_configs: i64,
    /// Rounds raised, newest first. Empty when none is on record.
    pub rounds: Vec<LandscapeRound>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Landscape {
    pub items: Vec<LandscapeItem>,
}
