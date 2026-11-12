//! Site-wide search. One query per kind, each ranked by how well the text
//! matches: exact names first, then prefixes, then trigram similarity. Every
//! query uses a trigram index; nothing scans the raw listings.

use ih_core::{SearchCount, SearchHit, SearchResults};
use sqlx::PgPool;

pub const KINDS: [&str; 6] = ["organization", "product", "model", "media", "gpu", "news"];

/// Searches every kind (or the given ones) and returns up to `per_kind` hits of each.
pub async fn search(
    pool: &PgPool,
    q: &str,
    kinds: &[&str],
    per_kind: i64,
) -> sqlx::Result<SearchResults> {
    let q = q.trim();
    let pattern = crate::sql::words_like(q).unwrap_or_else(|| "%%".to_owned());
    let want = |k: &str| kinds.is_empty() || kinds.contains(&k);
    // Score: 3 exact, 2 prefix, else similarity (0..1), on lower-cased text.
    const SCORE: &str = "(CASE WHEN lower({f}) = lower($1) THEN 3
                               WHEN lower({f}) LIKE lower($1) || '%' THEN 2
                               ELSE similarity(lower({f}), lower($1)) END)::float4";
    let score = |field: &str| SCORE.replace("{f}", field);

    let mut parts: Vec<String> = Vec::new();
    if want("organization") {
        parts.push(format!(
            "(SELECT 'organization' AS kind, '/organizations/' || slug AS href, name AS title,
                     tagline AS subtitle, logo_url, NULL AS maker, {} AS score, NULL::timestamptz AS date
              FROM organizations
              WHERE name ILIKE $2 OR slug ILIKE $2 OR $1 ILIKE ANY (aliases)
              ORDER BY score DESC, name LIMIT $3)",
            score("name")
        ));
    }
    if want("product") {
        parts.push(format!(
            "(SELECT 'product' AS kind, '/products/' || p.slug AS href, p.name AS title,
                     o.name || ' · ' || replace(p.product_type::text, '_', ' ') AS subtitle,
                     o.logo_url, NULL AS maker, {} AS score, NULL::timestamptz AS date
              FROM products p JOIN organizations o ON o.id = p.organization_id
              WHERE p.name ILIKE $2 OR p.slug ILIKE $2
              ORDER BY score DESC, p.name LIMIT $3)",
            score("p.name")
        ));
    }
    if want("model") {
        parts.push(format!(
            "(SELECT 'model' AS kind, '/market/models/' || m.model_key AS href, m.name AS title,
                     concat_ws(' · ', m.maker, m.providers || ' providers',
                               'from $' || round(m.min_blended_per_mtok::numeric, 4) || ' / 1M') AS subtitle,
                     NULL AS logo_url, m.maker AS maker, ({} + ln(1 + m.providers))::float4 AS score,
                     NULL::timestamptz AS date
              FROM market_models m
              WHERE m.kind = 'model' AND (m.model_key ILIKE $2 OR m.name ILIKE $2)
              ORDER BY score DESC, m.providers DESC LIMIT $3)",
            score("m.model_key")
        ));
    }
    if want("media") {
        parts.push(format!(
            "(SELECT 'media' AS kind, '/market/models/' || m.model_key AS href, m.name AS title,
                     concat_ws(' · ', m.kind, m.maker, m.providers || ' providers') AS subtitle,
                     NULL AS logo_url, m.maker AS maker, ({} + ln(1 + m.providers))::float4 AS score,
                     NULL::timestamptz AS date
              FROM market_models m
              WHERE m.kind <> 'model' AND (m.model_key ILIKE $2 OR m.name ILIKE $2)
              ORDER BY score DESC, m.providers DESC LIMIT $3)",
            score("m.model_key")
        ));
    }
    if want("gpu") {
        parts.push(format!(
            "(SELECT 'gpu' AS kind, '/market/gpus/' || g.gpu_type AS href, g.gpu_type AS title,
                     g.providers || ' clouds · from $' || round(g.min_per_gpu_hour::numeric, 2)
                         || ' / GPU-hour' AS subtitle,
                     NULL AS logo_url, NULL AS maker, {} AS score, NULL::timestamptz AS date
              FROM market_gpus g WHERE g.gpu_type ILIKE $2
              ORDER BY score DESC, g.providers DESC LIMIT $3)",
            score("g.gpu_type")
        ));
    }
    if want("news") {
        parts.push(format!(
            "(SELECT 'news' AS kind, n.url AS href, n.title AS title, n.publisher AS subtitle,
                     NULL AS logo_url, NULL AS maker, ({} * 0.9)::float4 AS score, n.published_at AS date
              FROM news_items n
              WHERE n.status = 'approved' AND n.title ILIKE $2
              ORDER BY n.published_at DESC LIMIT $3)",
            score("n.title")
        ));
    }
    if parts.is_empty() || q.chars().count() < 2 {
        return Ok(SearchResults {
            q: q.to_owned(),
            hits: vec![],
            counts: vec![],
        });
    }
    let hits: Vec<SearchHit> = sqlx::query_as(sqlx::AssertSqlSafe(parts.join(" UNION ALL ")))
        .bind(q)
        .bind(&pattern)
        .bind(per_kind)
        .fetch_all(pool)
        .await?;

    // How many matched in total, per kind, for "see all" links.
    let counts: Vec<SearchCount> = sqlx::query_as(
        "SELECT kind, count FROM (
             SELECT 'organization' AS kind, count(*) FROM organizations
              WHERE name ILIKE $1 OR slug ILIKE $1 OR $2 ILIKE ANY (aliases)
             UNION ALL SELECT 'product', count(*) FROM products WHERE name ILIKE $1 OR slug ILIKE $1
             UNION ALL SELECT 'model', count(*) FROM market_models
              WHERE kind = 'model' AND (model_key ILIKE $1 OR name ILIKE $1)
             UNION ALL SELECT 'media', count(*) FROM market_models
              WHERE kind <> 'model' AND (model_key ILIKE $1 OR name ILIKE $1)
             UNION ALL SELECT 'gpu', count(*) FROM market_gpus WHERE gpu_type ILIKE $1
             UNION ALL SELECT 'news', count(*) FROM news_items
              WHERE status = 'approved' AND title ILIKE $1
         ) c WHERE count > 0",
    )
    .bind(&pattern)
    .bind(q)
    .fetch_all(pool)
    .await?;
    let counts = counts.into_iter().filter(|c| want(&c.kind)).collect();

    let mut hits = hits;
    // Groups in a fixed order, best match first within each.
    hits.sort_by(|a, b| {
        let order = |k: &str| KINDS.iter().position(|x| *x == k).unwrap_or(99);
        order(&a.kind)
            .cmp(&order(&b.kind))
            .then(b.score.total_cmp(&a.score))
    });
    Ok(SearchResults {
        q: q.to_owned(),
        hits,
        counts,
    })
}
