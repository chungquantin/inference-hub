//! Reads for the landscape map: every organization and its funding rounds.

use std::collections::HashMap;

use chrono::NaiveDate;
use ih_core::{
    OrgKind, OrgStatus,
    landscape::{Landscape, LandscapeItem, LandscapeRound},
};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct OrgRow {
    id: Uuid,
    slug: String,
    name: String,
    kinds: Vec<OrgKind>,
    logo_url: Option<String>,
    summary: Option<String>,
    website: Option<String>,
    status: OrgStatus,
    hq_country: Option<String>,
    founded_year: Option<i16>,
    models_sold: i64,
    gpu_configs: i64,
}

#[derive(sqlx::FromRow)]
struct RoundRow {
    organization_id: Uuid,
    round: String,
    announced_on: NaiveDate,
    amount: Option<i64>,
    valuation: Option<i64>,
    currency: String,
    source_url: Option<String>,
    leads: Vec<String>,
    investors: Vec<String>,
}

pub async fn landscape(pool: &PgPool) -> sqlx::Result<Landscape> {
    let orgs: Vec<OrgRow> = sqlx::query_as(
        "SELECT o.id, o.slug, o.name, o.kinds, o.logo_url,
                left(coalesce(o.tagline, o.description), 220) AS summary,
                o.website, o.status, o.hq_country::text AS hq_country, o.founded_year,
                coalesce(mp.models, 0)::int8 AS models_sold,
                coalesce(mp.gpu_configurations, 0)::int8 AS gpu_configs
         FROM organizations o
         LEFT JOIN market_providers mp ON mp.organization_id = o.id
         ORDER BY o.name",
    )
    .fetch_all(pool)
    .await?;
    let rounds: Vec<RoundRow> = sqlx::query_as(
        "SELECT f.organization_id, f.round, f.announced_on, f.amount, f.valuation,
                f.currency::text AS currency, f.source_url,
                coalesce(array_agg(i.name ORDER BY i.name)
                         FILTER (WHERE fi.is_lead), '{}') AS leads,
                coalesce(array_agg(i.name ORDER BY i.name)
                         FILTER (WHERE NOT fi.is_lead), '{}') AS investors
         FROM funding_rounds f
         LEFT JOIN funding_round_investors fi ON fi.funding_round_id = f.id
         LEFT JOIN organizations i ON i.id = fi.investor_id
         GROUP BY f.id
         ORDER BY f.announced_on DESC",
    )
    .fetch_all(pool)
    .await?;
    let mut by_org: HashMap<Uuid, Vec<LandscapeRound>> = HashMap::new();
    for r in rounds {
        by_org
            .entry(r.organization_id)
            .or_default()
            .push(LandscapeRound {
                round: r.round,
                announced_on: r.announced_on,
                amount: r.amount,
                valuation: r.valuation,
                currency: r.currency,
                source_url: r.source_url,
                leads: r.leads,
                investors: r.investors,
            });
    }
    Ok(Landscape {
        items: orgs
            .into_iter()
            .map(|o| LandscapeItem {
                rounds: by_org.remove(&o.id).unwrap_or_default(),
                slug: o.slug,
                name: o.name,
                kinds: o.kinds,
                logo_url: o.logo_url,
                summary: o.summary,
                website: o.website,
                status: o.status,
                hq_country: o.hq_country,
                founded_year: o.founded_year,
                models_sold: o.models_sold,
                gpu_configs: o.gpu_configs,
            })
            .collect(),
    })
}
