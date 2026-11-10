//! Imports the curated YAML data in `data/` into Postgres.
//!
//! The import is idempotent: organizations, products, models and feed sources
//! are upserted by slug, and an offering is added only when the same price
//! observation is not already stored. Nothing is deleted, so removing a file
//! does not remove rows. Fetch state of sources (validators, failures) is kept.

pub mod data;

use std::{collections::HashSet, path::Path};

use anyhow::{Context, bail};
use chrono::{NaiveTime, TimeZone, Utc};
use data::{FeedSeed, FundingSeed, OrgSeed, SourcesFile, categories_for, validate, validate_feeds};
use ih_db::{PgPool, sqlx};
use sqlx::{Postgres, Transaction};

/// Counts of what one import wrote.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub organizations: usize,
    pub products: usize,
    pub offerings_added: usize,
    pub sources: usize,
    pub funding_rounds: usize,
}

/// Everything in the data directory.
#[derive(Debug, Clone, Default)]
pub struct DataSet {
    /// `organizations/<slug>.yaml` files, as (file name, content), sorted by name.
    pub organizations: Vec<(String, OrgSeed)>,
    /// `sources.yaml`, if present.
    pub sources: SourcesFile,
}

impl DataSet {
    pub fn from_organizations(organizations: Vec<(String, OrgSeed)>) -> Self {
        Self {
            organizations,
            sources: SourcesFile::default(),
        }
    }

    /// Every problem in the data set.
    pub fn validate(&self, known_categories: &HashSet<String>) -> Vec<data::Issue> {
        let mut issues = validate(&self.organizations, known_categories);
        issues.extend(validate_feeds(&self.organizations, &self.sources));
        issues
    }
}

/// Reads the data directory: `organizations/*.yaml` and the optional `sources.yaml`.
pub fn load(dir: &Path) -> anyhow::Result<DataSet> {
    let sources_path = dir.join("sources.yaml");
    let sources = if sources_path.exists() {
        let text = std::fs::read_to_string(&sources_path)
            .with_context(|| format!("reading {}", sources_path.display()))?;
        serde_saphyr::from_str(&text)
            .with_context(|| format!("parsing {}", sources_path.display()))?
    } else {
        SourcesFile::default()
    };
    Ok(DataSet {
        organizations: load_organizations(dir)?,
        sources,
    })
}

fn load_organizations(dir: &Path) -> anyhow::Result<Vec<(String, OrgSeed)>> {
    let org_dir = dir.join("organizations");
    let mut paths: Vec<_> = std::fs::read_dir(&org_dir)
        .with_context(|| format!("reading {}", org_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let org: OrgSeed = serde_saphyr::from_str(&text)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok((name, org))
        })
        .collect()
}

/// Slugs of the categories in the database.
pub async fn known_categories(pool: &PgPool) -> sqlx::Result<HashSet<String>> {
    let slugs: Vec<String> = sqlx::query_scalar("SELECT slug FROM categories")
        .fetch_all(pool)
        .await?;
    Ok(slugs.into_iter().collect())
}

/// Validates the data against itself and the database, then writes it in one transaction.
pub async fn import(pool: &PgPool, data: &DataSet) -> anyhow::Result<Summary> {
    let files = &data.organizations;
    let issues = data.validate(&known_categories(pool).await?);
    if !issues.is_empty() {
        let list: Vec<String> = issues.iter().map(|i| format!("  - {i}")).collect();
        bail!(
            "the data has {} problem(s):\n{}",
            issues.len(),
            list.join("\n")
        );
    }

    let mut tx = pool.begin().await?;
    let mut summary = Summary::default();

    // Organizations first, so offerings can refer to any of them.
    for (_, org) in files {
        upsert_organization(&mut tx, org).await?;
        summary.organizations += 1;
    }
    for (_, org) in files {
        for product in &org.products {
            summary.offerings_added += upsert_product(&mut tx, &org.slug, product).await?;
            summary.products += 1;
        }
        for feed in &org.feeds {
            // An organization's own feed is trusted by default.
            upsert_feed(
                &mut tx,
                feed,
                Some(&org.slug),
                feed.auto_approve.unwrap_or(true),
            )
            .await?;
            summary.sources += 1;
        }
    }
    for feed in &data.sources.feeds {
        upsert_feed(&mut tx, feed, None, feed.auto_approve.unwrap_or(false)).await?;
        summary.sources += 1;
    }
    // Funding last: investors may be any organization of the data set.
    for (_, org) in files {
        for round in &org.funding {
            upsert_funding_round(&mut tx, &org.slug, round).await?;
            summary.funding_rounds += 1;
        }
    }

    tx.commit().await?;
    Ok(summary)
}

async fn upsert_organization(
    tx: &mut Transaction<'_, Postgres>,
    org: &OrgSeed,
) -> sqlx::Result<()> {
    let links = serde_json::to_value(&org.links).unwrap_or_default();
    let id: ih_db::sqlx::types::Uuid = sqlx::query_scalar(
        "INSERT INTO organizations
             (slug, name, kinds, tagline, description, website, logo_url,
              hq_country, founded_year, status, links, aliases, exclude_phrases, match_case)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, coalesce($10, 'active'::org_status), $11,
                 $12, $13, $14)
         ON CONFLICT (slug) DO UPDATE SET
             name = EXCLUDED.name, kinds = EXCLUDED.kinds, tagline = EXCLUDED.tagline,
             description = EXCLUDED.description, website = EXCLUDED.website,
             logo_url = EXCLUDED.logo_url, hq_country = EXCLUDED.hq_country,
             founded_year = EXCLUDED.founded_year, status = EXCLUDED.status,
             links = EXCLUDED.links, aliases = EXCLUDED.aliases,
             exclude_phrases = EXCLUDED.exclude_phrases, match_case = EXCLUDED.match_case
         RETURNING id",
    )
    .bind(&org.slug)
    .bind(&org.name)
    .bind(&org.kinds)
    .bind(&org.tagline)
    .bind(&org.description)
    .bind(&org.website)
    .bind(&org.logo_url)
    .bind(&org.hq_country)
    .bind(org.founded_year)
    .bind(org.status)
    .bind(links)
    .bind(&org.tagging.aliases)
    .bind(&org.tagging.exclude)
    .bind(org.tagging.match_case)
    .fetch_one(&mut **tx)
    .await?;

    // The YAML is the source of truth for an organization's categories.
    sqlx::query("DELETE FROM organization_categories WHERE organization_id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO organization_categories (organization_id, category_id)
         SELECT $1, id FROM categories WHERE slug = ANY ($2)",
    )
    .bind(id)
    .bind(categories_for(org))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Upserts a product, its model attributes and its offerings. Returns the offerings added.
async fn upsert_product(
    tx: &mut Transaction<'_, Postgres>,
    org_slug: &str,
    product: &data::ProductSeed,
) -> sqlx::Result<usize> {
    let metadata = serde_json::to_value(&product.metadata).unwrap_or_default();
    let product_id: ih_db::sqlx::types::Uuid = sqlx::query_scalar(
        "INSERT INTO products
             (organization_id, slug, name, product_type, tagline, description, url, launched_at,
              metadata, github_repo)
         SELECT o.id, $2, $3, $4, $5, $6, $7, $8, $9, $10 FROM organizations o WHERE o.slug = $1
         ON CONFLICT (slug) DO UPDATE SET
             organization_id = EXCLUDED.organization_id, name = EXCLUDED.name,
             product_type = EXCLUDED.product_type, tagline = EXCLUDED.tagline,
             description = EXCLUDED.description, url = EXCLUDED.url,
             launched_at = EXCLUDED.launched_at, metadata = EXCLUDED.metadata,
             github_repo = EXCLUDED.github_repo
         RETURNING id",
    )
    .bind(org_slug)
    .bind(&product.slug)
    .bind(&product.name)
    .bind(product.product_type)
    .bind(&product.tagline)
    .bind(&product.description)
    .bind(&product.url)
    .bind(product.launched_at)
    .bind(metadata)
    .bind(&product.github)
    .fetch_one(&mut **tx)
    .await?;

    // The YAML is the source of truth for which listings map to this product.
    sqlx::query("DELETE FROM product_listing_ids WHERE product_id = $1")
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    for (adapter, ids) in &product.listings {
        for id in ids {
            sqlx::query(
                "INSERT INTO product_listing_ids (product_id, adapter, external_id)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (adapter, external_id) DO UPDATE SET product_id = EXCLUDED.product_id",
            )
            .bind(product_id)
            .bind(adapter)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        }
    }

    match &product.model {
        Some(model) => {
            sqlx::query(
                "INSERT INTO models
                     (product_id, family, parameters_b, active_params_b, context_window,
                      modalities, license, open_weights)
                 VALUES ($1, $2, $3::float8::numeric, $4::float8::numeric, $5, $6, $7, $8)
                 ON CONFLICT (product_id) DO UPDATE SET
                     family = EXCLUDED.family, parameters_b = EXCLUDED.parameters_b,
                     active_params_b = EXCLUDED.active_params_b,
                     context_window = EXCLUDED.context_window,
                     modalities = EXCLUDED.modalities, license = EXCLUDED.license,
                     open_weights = EXCLUDED.open_weights",
            )
            .bind(product_id)
            .bind(&model.family)
            .bind(model.parameters_b)
            .bind(model.active_params_b)
            .bind(model.context_window)
            .bind(&model.modalities)
            .bind(&model.license)
            .bind(model.open_weights)
            .execute(&mut **tx)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM models WHERE product_id = $1")
                .bind(product_id)
                .execute(&mut **tx)
                .await?;
        }
    }

    let mut added = 0;
    for offering in &product.offerings {
        let observed_at = Utc.from_utc_datetime(&offering.observed_on.and_time(NaiveTime::MIN));
        // Offerings are append-only price history: add a row only for a new observation.
        let result = sqlx::query(
            "INSERT INTO offerings
                 (product_id, provider_id, price_input_per_mtok, price_output_per_mtok,
                  gpu_type, gpu_count, price_per_hour, region, currency, source_url, observed_at)
             SELECT $1, o.id, $3::float8::numeric, $4::float8::numeric, $5, $6,
                    $7::float8::numeric, $8, $9, $10, $11
             FROM organizations o
             WHERE o.slug = $2
               AND NOT EXISTS (
                   SELECT 1 FROM offerings f
                   WHERE f.product_id = $1 AND f.provider_id = o.id
                     AND f.gpu_type IS NOT DISTINCT FROM $5
                     AND f.gpu_count IS NOT DISTINCT FROM $6
                     AND f.region IS NOT DISTINCT FROM $8
                     AND f.observed_at = $11)",
        )
        .bind(product_id)
        .bind(&offering.provider)
        .bind(offering.input)
        .bind(offering.output)
        .bind(&offering.gpu_type)
        .bind(offering.gpu_count)
        .bind(offering.per_hour)
        .bind(&offering.region)
        .bind(&offering.currency)
        .bind(&offering.source_url)
        .bind(observed_at)
        .execute(&mut **tx)
        .await?;
        added += result.rows_affected() as usize;
    }
    Ok(added)
}

async fn upsert_feed(
    tx: &mut Transaction<'_, Postgres>,
    feed: &FeedSeed,
    org_slug: Option<&str>,
    auto_approve: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO sources
             (slug, name, source_type, url, organization_id, auto_approve,
              fetch_interval_minutes, enabled)
         VALUES ($1, $2, 'rss', $3, (SELECT id FROM organizations WHERE slug = $4), $5, $6, $7)
         ON CONFLICT (slug) DO UPDATE SET
             name = EXCLUDED.name, url = EXCLUDED.url,
             organization_id = EXCLUDED.organization_id,
             auto_approve = EXCLUDED.auto_approve,
             fetch_interval_minutes = EXCLUDED.fetch_interval_minutes,
             enabled = EXCLUDED.enabled,
             -- A new URL starts fresh: old validators belong to the old URL.
             etag = CASE WHEN sources.url IS DISTINCT FROM EXCLUDED.url THEN NULL ELSE sources.etag END,
             last_modified = CASE WHEN sources.url IS DISTINCT FROM EXCLUDED.url
                                  THEN NULL ELSE sources.last_modified END",
    )
    .bind(&feed.slug)
    .bind(&feed.name)
    .bind(&feed.url)
    .bind(org_slug)
    .bind(auto_approve)
    .bind(feed.interval_minutes)
    .bind(feed.enabled)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_funding_round(
    tx: &mut Transaction<'_, Postgres>,
    org_slug: &str,
    round: &FundingSeed,
) -> sqlx::Result<()> {
    let id: ih_db::sqlx::types::Uuid = sqlx::query_scalar(
        "INSERT INTO funding_rounds
             (organization_id, round, announced_on, amount, valuation, currency, source_url, notes)
         SELECT id, $2, $3, $4, $5, $6, $7, $8 FROM organizations WHERE slug = $1
         ON CONFLICT (organization_id, round, announced_on) DO UPDATE SET
             amount = EXCLUDED.amount, valuation = EXCLUDED.valuation,
             currency = EXCLUDED.currency, source_url = EXCLUDED.source_url,
             notes = EXCLUDED.notes
         RETURNING id",
    )
    .bind(org_slug)
    .bind(&round.round)
    .bind(round.announced_on)
    .bind(round.amount)
    .bind(round.valuation)
    .bind(&round.currency)
    .bind(&round.source_url)
    .bind(&round.notes)
    .fetch_one(&mut **tx)
    .await?;

    // The YAML is the source of truth for who invested.
    sqlx::query("DELETE FROM funding_round_investors WHERE funding_round_id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO funding_round_investors (funding_round_id, investor_id, is_lead)
         SELECT $1, o.id, o.slug = ANY ($2) FROM organizations o
         WHERE o.slug = ANY ($2) OR o.slug = ANY ($3)",
    )
    .bind(id)
    .bind(&round.lead)
    .bind(&round.investors)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
