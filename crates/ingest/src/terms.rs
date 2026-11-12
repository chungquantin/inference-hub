//! The GPU term structure: what one GPU-hour costs as spot, on demand, or committed
//! for a term, per provider and GPU type.
//!
//! Two inputs:
//! - Azure's public Retail Prices API lists spot, 1, 3 and 5 year reservations of its
//!   GPU virtual machines. A reservation is priced for the whole term, so the hourly
//!   price is the term price over the hours in the term (365 days a year).
//! - The listings the market adapters already collect carry on-demand prices and, for
//!   a few providers (CoreWeave, Verda), spot prices of the same configuration.
//!
//! Only prices a provider states are stored. A missing tier stays missing.

use std::collections::BTreeMap;

use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;

use crate::adapters::{AdapterError, azure::SIZES, get_text};

const HOURS_PER_YEAR: f64 = 8760.0;
const AZURE_REGION: &str = "eastus";
const AZURE_SOURCE: &str = "azure-retail-prices";
const LISTINGS_SOURCE: &str = "listings";

/// One price of a GPU-hour at a provider.
#[derive(Debug, Clone, PartialEq)]
pub struct TermPrice {
    pub organization_slug: &'static str,
    pub gpu_type: String,
    pub tier: &'static str,
    pub term_months: i32,
    pub per_gpu_hour: f64,
    pub source: &'static str,
    pub source_url: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Page {
    items: Vec<Item>,
    next_page_link: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    arm_sku_name: String,
    retail_price: f64,
    unit_of_measure: String,
    sku_name: String,
    product_name: String,
    #[serde(rename = "type")]
    kind: String,
    reservation_term: Option<String>,
}

fn azure_url(origin: &str) -> String {
    let skus = SIZES
        .iter()
        .map(|(s, _, _)| format!("armSkuName eq '{s}'"))
        .collect::<Vec<_>>()
        .join(" or ");
    let filter = format!(
        "serviceName eq 'Virtual Machines' and armRegionName eq '{AZURE_REGION}' and ({skus})"
    );
    let mut url = reqwest::Url::parse(&format!(
        "{}/api/retail/prices",
        origin.trim_end_matches('/')
    ))
    .expect("static URL");
    url.query_pairs_mut().append_pair("$filter", &filter);
    url.into()
}

/// Azure's spot and committed prices of its GPU sizes, cheapest size per GPU type.
pub async fn azure(client: &Client, origin: &str) -> Result<Vec<TermPrice>, AdapterError> {
    let mut items = Vec::new();
    let mut next = Some(azure_url(origin));
    // The API pages at 100 rows; the sizes we ask for come to a few hundred.
    for _ in 0..8 {
        let Some(url) = next.take() else { break };
        let page: Page = serde_json::from_str(&get_text(client, AZURE_SOURCE_ID, &url).await?)
            .map_err(|e| AdapterError::Format(AZURE_SOURCE_ID, e.to_string()))?;
        items.extend(page.items);
        next = page.next_page_link;
    }
    Ok(azure_prices(items))
}

const AZURE_SOURCE_ID: &str = "azure-terms";

fn term_months(term: &str) -> Option<i32> {
    let n: i32 = term.split_whitespace().next()?.parse().ok()?;
    match term.split_whitespace().nth(1)? {
        "Year" | "Years" => Some(n * 12),
        _ => None,
    }
}

fn azure_prices(items: Vec<Item>) -> Vec<TermPrice> {
    // (gpu type, tier, term) -> cheapest per GPU-hour over the sizes of that type.
    let mut best: BTreeMap<(String, &'static str, i32), f64> = BTreeMap::new();
    for item in items {
        if !item.retail_price.is_finite()
            || item.retail_price <= 0.0
            || item.product_name.contains("Windows")
            || item.sku_name.contains("Low Priority")
        {
            continue;
        }
        let Some((_, gpu_type, count)) = SIZES.iter().find(|(s, _, _)| *s == item.arm_sku_name)
        else {
            continue;
        };
        let (tier, months, hours) = match item.kind.as_str() {
            "Consumption" if item.sku_name.contains("Spot") && item.unit_of_measure == "1 Hour" => {
                ("spot", 0, 1.0)
            }
            "Reservation" => {
                let Some(months) = item.reservation_term.as_deref().and_then(term_months) else {
                    continue;
                };
                (
                    "committed",
                    months,
                    f64::from(months) / 12.0 * HOURS_PER_YEAR,
                )
            }
            _ => continue,
        };
        let per_gpu = item.retail_price / hours / f64::from(*count);
        let slot = best
            .entry(((*gpu_type).to_owned(), tier, months))
            .or_insert(per_gpu);
        if per_gpu < *slot {
            *slot = per_gpu;
        }
    }
    best.into_iter()
        .map(|((gpu_type, tier, term_months), per_gpu)| TermPrice {
            organization_slug: "microsoft",
            gpu_type,
            tier,
            term_months,
            per_gpu_hour: (per_gpu * 1e4).round() / 1e4,
            source: AZURE_SOURCE,
            source_url: "https://azure.microsoft.com/en-us/pricing/details/virtual-machines/linux/",
        })
        .collect()
}

/// Replaces the stored term prices of the Azure source, and refreshes the listing-derived
/// on-demand and spot prices. Returns how many rows are stored.
pub async fn store_azure(pool: &PgPool, prices: &[TermPrice]) -> sqlx::Result<usize> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM gpu_term_prices WHERE source = $1")
        .bind(AZURE_SOURCE)
        .execute(&mut *tx)
        .await?;
    let mut stored = 0;
    for p in prices {
        stored += sqlx::query(
            "INSERT INTO gpu_term_prices
                 (organization_id, gpu_type, tier, term_months, per_gpu_hour, source, source_url)
             SELECT id, $2, $3, $4, $5, $6, $7 FROM organizations WHERE slug = $1
             ON CONFLICT (organization_id, gpu_type, tier, term_months) DO UPDATE SET
                 per_gpu_hour = EXCLUDED.per_gpu_hour, source = EXCLUDED.source,
                 source_url = EXCLUDED.source_url, observed_at = now()",
        )
        .bind(p.organization_slug)
        .bind(&p.gpu_type)
        .bind(p.tier)
        .bind(p.term_months)
        .bind(p.per_gpu_hour)
        .bind(p.source)
        .bind(p.source_url)
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;
    }
    tx.commit().await?;
    Ok(stored)
}

/// On-demand prices from every live GPU listing, and spot prices from the adapters
/// that report them, as the cheapest per-GPU price of each provider and GPU type.
pub async fn store_from_listings(pool: &PgPool) -> sqlx::Result<usize> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM gpu_term_prices WHERE source = $1")
        .bind(LISTINGS_SOURCE)
        .execute(&mut *tx)
        .await?;
    // A config's spot price covers the whole config, like its on-demand price.
    let rows = sqlx::query(
        "WITH base AS (
             SELECT a.organization_id, l.gpu_type, l.gpu_count,
                    l.price_per_hour::float8 AS hourly,
                    coalesce((l.extra ->> 'spot_per_hour')::float8,
                             (l.extra ->> 'spot_price_per_hour')::float8) AS spot
             FROM market_listings l JOIN adapter_state a ON a.adapter = l.adapter
             WHERE l.kind = 'gpu' AND l.delisted_at IS NULL AND l.gpu_type IS NOT NULL
               AND l.gpu_count > 0 AND l.price_per_hour > 0
         ), tiers AS (
             SELECT organization_id, gpu_type, 'on_demand' AS tier,
                    min(hourly / gpu_count) AS per_gpu
             FROM base GROUP BY organization_id, gpu_type
             UNION ALL
             SELECT organization_id, gpu_type, 'spot', min(spot / gpu_count)
             FROM base WHERE spot > 0 GROUP BY organization_id, gpu_type
         )
         INSERT INTO gpu_term_prices
             (organization_id, gpu_type, tier, term_months, per_gpu_hour, source)
         SELECT organization_id, gpu_type, tier, 0, round(per_gpu::numeric, 4)::float8, $1
         FROM tiers
         WHERE per_gpu > 0
         ON CONFLICT (organization_id, gpu_type, tier, term_months) DO NOTHING",
    )
    .bind(LISTINGS_SOURCE)
    .execute(&mut *tx)
    .await?
    .rows_affected() as usize;
    tx.commit().await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(sku: &str, sku_name: &str, kind: &str, price: f64, term: Option<&str>) -> Item {
        Item {
            arm_sku_name: sku.into(),
            retail_price: price,
            unit_of_measure: "1 Hour".into(),
            sku_name: sku_name.into(),
            product_name: "Virtual Machines NDsr H100 v5 Series".into(),
            kind: kind.into(),
            reservation_term: term.map(Into::into),
        }
    }

    #[test]
    fn reservations_become_hourly_prices_per_gpu() {
        let h100 = "Standard_ND96isr_H100_v5";
        let prices = azure_prices(vec![
            item(h100, "ND96isrH100v5", "Consumption", 98.32, None),
            item(h100, "ND96isrH100v5 Spot", "Consumption", 18.169536, None),
            item(
                h100,
                "ND96isrH100v5",
                "Reservation",
                551_221.0,
                Some("1 Year"),
            ),
            item(
                h100,
                "ND96isrH100v5",
                "Reservation",
                1_134_310.0,
                Some("3 Years"),
            ),
            item(
                h100,
                "ND96isrH100v5 Low Priority",
                "Consumption",
                19.664,
                None,
            ),
        ]);
        let get = |tier: &str, months: i32| {
            prices
                .iter()
                .find(|p| p.tier == tier && p.term_months == months)
                .map(|p| p.per_gpu_hour)
        };
        // 551,221 over 8,760 hours and 8 GPUs; on-demand is not taken from here.
        assert_eq!(get("committed", 12), Some(7.8656));
        assert_eq!(get("committed", 36), Some(5.3953));
        assert_eq!(get("spot", 0), Some(2.2712));
        assert_eq!(get("on_demand", 0), None);
        assert_eq!(prices.len(), 3, "low priority and on-demand are left out");
    }

    #[test]
    fn the_cheapest_size_of_a_gpu_type_wins() {
        let prices = azure_prices(vec![
            item(
                "Standard_NC24ads_A100_v4",
                "NC24ads A100 v4 Spot",
                "Consumption",
                0.9,
                None,
            ),
            item(
                "Standard_NC96ads_A100_v4",
                "NC96ads A100 v4 Spot",
                "Consumption",
                2.4,
                None,
            ),
        ]);
        assert_eq!(prices.len(), 1);
        assert_eq!(
            prices[0].per_gpu_hour, 0.6,
            "4 GPUs at 2.4 beat 1 GPU at 0.9"
        );
    }

    #[test]
    fn terms_parse_in_years() {
        assert_eq!(term_months("1 Year"), Some(12));
        assert_eq!(term_months("5 Years"), Some(60));
        assert_eq!(term_months("Monthly"), None);
    }
}
