//! Cross-provider comparisons, price history, funding and repository stats.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ih_core::{
    FundingRound, GpuPrice, GpuPriceQuery, ModelPrice, ModelPriceQuery, ModelPriceSort,
    OrganizationRef, PortfolioCompany, PortfolioRound, PricePoint, ProductType, RepoLeader,
    RepoStats, RoundInvestor,
};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Weight of input tokens in the blended price: 3 input tokens per output token.
pub const BLEND_INPUT_WEIGHT: f64 = 3.0;

/// Latest price of every model at every provider, filtered and sorted.
pub async fn model_prices(pool: &PgPool, q: &ModelPriceQuery) -> sqlx::Result<Vec<ModelPrice>> {
    let sort = match q.sort.unwrap_or_default() {
        ModelPriceSort::Blended => "blended",
        ModelPriceSort::Input => "input",
        ModelPriceSort::Output => "output",
        ModelPriceSort::ContextWindow => "context",
    };
    sqlx::query_as(
        "SELECT * FROM (
             SELECT p.slug AS product_slug, p.name AS product_name,
                    mk.slug AS maker_slug, mk.name AS maker_name, mk.logo_url AS maker_logo,
                    pr.slug AS organization_slug, pr.name AS organization_name, pr.logo_url AS organization_logo,
                    m.open_weights, m.context_window, m.parameters_b::float8 AS parameters_b,
                    f.price_input_per_mtok::float8 AS price_input_per_mtok,
                    f.price_output_per_mtok::float8 AS price_output_per_mtok,
                    -- numeric math, rounded, so 0.175 does not become 0.17500000000000002
                    round((f.price_input_per_mtok * $5::numeric + f.price_output_per_mtok)
                          / ($5::numeric + 1), 6)::float8 AS blended_per_mtok,
                    f.currency::text AS currency, f.source_url,
                    coalesce(f.confirmed_at, f.observed_at) AS observed_at
             FROM latest_offerings f
             JOIN products p ON p.id = f.product_id AND p.product_type = 'model'
             JOIN models m ON m.product_id = p.id
             JOIN organizations mk ON mk.id = p.organization_id
             JOIN organizations pr ON pr.id = f.provider_id
             WHERE f.price_per_hour IS NULL
               AND ($1::boolean IS NULL OR m.open_weights = $1)
               AND ($2::text IS NULL OR mk.slug = $2)
               AND ($3::text IS NULL OR pr.slug = $3)
               AND ($4::integer IS NULL OR m.context_window >= $4)
         ) rows
         ORDER BY
             CASE $6 WHEN 'blended' THEN blended_per_mtok
                     WHEN 'input' THEN price_input_per_mtok
                     WHEN 'output' THEN price_output_per_mtok END ASC NULLS LAST,
             CASE WHEN $6 = 'context' THEN context_window END DESC NULLS LAST,
             blended_per_mtok ASC NULLS LAST, product_name, organization_name",
    )
    .bind(q.open_weights)
    .bind(q.maker.as_deref())
    .bind(q.provider.as_deref())
    .bind(q.min_context)
    .bind(BLEND_INPUT_WEIGHT)
    .bind(sort)
    .fetch_all(pool)
    .await
}

/// Latest price of every GPU configuration at every provider, cheapest per GPU-hour first.
pub async fn gpu_prices(pool: &PgPool, q: &GpuPriceQuery) -> sqlx::Result<Vec<GpuPrice>> {
    sqlx::query_as(
        "SELECT p.slug AS product_slug, p.name AS product_name,
                pr.slug AS organization_slug, pr.name AS organization_name, pr.logo_url AS organization_logo,
                f.gpu_type, f.gpu_count, f.price_per_hour::float8 AS price_per_hour,
                (f.price_per_hour / f.gpu_count)::float8 AS price_per_gpu_hour,
                f.region, f.currency::text AS currency, f.source_url,
                    coalesce(f.confirmed_at, f.observed_at) AS observed_at
         FROM latest_offerings f
         JOIN products p ON p.id = f.product_id
         JOIN organizations pr ON pr.id = f.provider_id
         WHERE f.price_per_hour IS NOT NULL AND f.gpu_type IS NOT NULL
           AND coalesce(f.gpu_count, 0) > 0
           AND ($1::text IS NULL OR lower(f.gpu_type) = lower($1))
           AND ($2::text IS NULL OR pr.slug = $2)
         ORDER BY price_per_gpu_hour, f.gpu_type, pr.name",
    )
    .bind(q.gpu.as_deref())
    .bind(q.provider.as_deref())
    .fetch_all(pool)
    .await
}

/// GPU types with at least one current offering, for filters.
pub async fn gpu_types(pool: &PgPool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT DISTINCT gpu_type FROM latest_offerings
         WHERE gpu_type IS NOT NULL AND price_per_hour IS NOT NULL ORDER BY gpu_type",
    )
    .fetch_all(pool)
    .await
}

/// Every price observation of a product, oldest first. `None` when there is no such product.
pub async fn price_history(pool: &PgPool, slug: &str) -> sqlx::Result<Option<Vec<PricePoint>>> {
    let product: Option<Uuid> = sqlx::query_scalar("SELECT id FROM products WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    let Some(product) = product else {
        return Ok(None);
    };
    let points = sqlx::query_as(
        "SELECT o.slug AS organization_slug, o.name AS organization_name, o.logo_url AS organization_logo,
                nullif(concat_ws(' · ',
                    CASE WHEN f.gpu_type IS NOT NULL
                         THEN concat_ws('× ', f.gpu_count::text, f.gpu_type) END,
                    f.region), '') AS configuration,
                f.price_input_per_mtok::float8 AS price_input_per_mtok,
                f.price_output_per_mtok::float8 AS price_output_per_mtok,
                f.price_per_hour::float8 AS price_per_hour,
                f.currency::text AS currency, f.observed_at
         FROM offerings f JOIN organizations o ON o.id = f.provider_id
         WHERE f.product_id = $1
         ORDER BY f.observed_at, o.name, f.id",
    )
    .bind(product)
    .fetch_all(pool)
    .await?;
    Ok(Some(points))
}

#[derive(FromRow)]
struct RoundRow {
    id: Uuid,
    round: String,
    announced_on: NaiveDate,
    amount: Option<i64>,
    valuation: Option<i64>,
    currency: String,
    source_url: Option<String>,
    notes: Option<String>,
}

/// Rounds an organization raised, newest first, each with its investors (leads first).
pub async fn funding_rounds(
    pool: &PgPool,
    organization_id: Uuid,
) -> sqlx::Result<Vec<FundingRound>> {
    let rounds: Vec<RoundRow> = sqlx::query_as(
        "SELECT id, round, announced_on, amount, valuation, currency::text AS currency,
                source_url, notes
         FROM funding_rounds WHERE organization_id = $1
         ORDER BY announced_on DESC, round",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await?;
    let ids: Vec<Uuid> = rounds.iter().map(|r| r.id).collect();
    let investors: Vec<(Uuid, String, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT fi.funding_round_id, o.slug, o.name, o.logo_url, fi.is_lead
         FROM funding_round_investors fi JOIN organizations o ON o.id = fi.investor_id
         WHERE fi.funding_round_id = ANY ($1)
         ORDER BY fi.is_lead DESC, o.name",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;

    let mut by_round: BTreeMap<Uuid, Vec<RoundInvestor>> = BTreeMap::new();
    for (round, slug, name, logo_url, lead) in investors {
        by_round.entry(round).or_default().push(RoundInvestor {
            slug,
            name,
            logo_url,
            lead,
        });
    }
    Ok(rounds
        .into_iter()
        .map(|r| FundingRound {
            investors: by_round.remove(&r.id).unwrap_or_default(),
            round: r.round,
            announced_on: r.announced_on,
            amount: r.amount,
            valuation: r.valuation,
            currency: r.currency,
            source_url: r.source_url,
            notes: r.notes,
        })
        .collect())
}

#[derive(FromRow)]
struct PortfolioRow {
    slug: String,
    name: String,
    logo_url: Option<String>,
    round: String,
    announced_on: NaiveDate,
    amount: Option<i64>,
    currency: String,
    lead: bool,
}

/// Companies an investor backed, most recent investment first.
pub async fn portfolio(pool: &PgPool, investor_id: Uuid) -> sqlx::Result<Vec<PortfolioCompany>> {
    let rows: Vec<PortfolioRow> = sqlx::query_as(
        "SELECT o.slug, o.name, o.logo_url, fr.round, fr.announced_on, fr.amount,
                fr.currency::text AS currency, fi.is_lead AS lead
         FROM funding_round_investors fi
         JOIN funding_rounds fr ON fr.id = fi.funding_round_id
         JOIN organizations o ON o.id = fr.organization_id
         WHERE fi.investor_id = $1
         ORDER BY fr.announced_on DESC, o.name",
    )
    .bind(investor_id)
    .fetch_all(pool)
    .await?;

    // Keep companies in order of their most recent round.
    let mut companies: Vec<PortfolioCompany> = Vec::new();
    for row in rows {
        let entry = PortfolioRound {
            round: row.round,
            announced_on: row.announced_on,
            amount: row.amount,
            currency: row.currency,
            lead: row.lead,
        };
        match companies
            .iter_mut()
            .find(|c| c.organization.slug == row.slug)
        {
            Some(company) => company.rounds.push(entry),
            None => companies.push(PortfolioCompany {
                organization: OrganizationRef {
                    slug: row.slug,
                    name: row.name,
                    logo_url: row.logo_url,
                },
                rounds: vec![entry],
            }),
        }
    }
    Ok(companies)
}

macro_rules! repo_stats_columns {
    () => {
        "p.github_repo AS repo, s.stars, s.forks, s.open_issues, s.license, s.archived,
         s.pushed_at, s.latest_release, s.released_at, s.synced_at,
         (s.stars - (SELECT h.stars FROM repo_star_history h
                     WHERE h.product_id = p.id AND h.day >= current_date - 30
                     ORDER BY h.day LIMIT 1)) AS stars_30d"
    };
}

/// Latest GitHub stats of a product, when it has a synced repository.
pub async fn repo_stats(pool: &PgPool, product_id: Uuid) -> sqlx::Result<Option<RepoStats>> {
    sqlx::query_as(concat!(
        "SELECT ",
        repo_stats_columns!(),
        " FROM products p JOIN repo_stats s ON s.product_id = p.id
          WHERE p.id = $1 AND s.stars IS NOT NULL"
    ))
    .bind(product_id)
    .fetch_optional(pool)
    .await
}

#[derive(FromRow)]
struct LeaderRow {
    product_slug: String,
    product_name: String,
    product_type: ProductType,
    organization_slug: String,
    organization_name: String,
    organization_logo: Option<String>,
    #[sqlx(flatten)]
    stats: RepoStats,
}

/// Products with GitHub stats, most stars first. Archived repositories come last.
pub async fn repo_leaderboard(pool: &PgPool) -> sqlx::Result<Vec<RepoLeader>> {
    let rows: Vec<LeaderRow> = sqlx::query_as(concat!(
        "SELECT p.slug AS product_slug, p.name AS product_name, p.product_type,
                o.slug AS organization_slug, o.name AS organization_name, o.logo_url AS organization_logo, ",
        repo_stats_columns!(),
        " FROM products p
          JOIN repo_stats s ON s.product_id = p.id
          JOIN organizations o ON o.id = p.organization_id
          WHERE s.stars IS NOT NULL
          ORDER BY s.archived, s.stars DESC, p.name"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| RepoLeader {
            product_slug: r.product_slug,
            product_name: r.product_name,
            product_type: r.product_type,
            organization: OrganizationRef {
                slug: r.organization_slug,
                name: r.organization_name,
                logo_url: r.organization_logo,
            },
            stats: r.stats,
        })
        .collect())
}
