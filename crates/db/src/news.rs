use std::collections::HashMap;

use chrono::{DateTime, Utc};
use ih_core::{NewsItem, NewsMatch, NewsMention, NewsStatus, Page, PageParams, SourceStatus};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Filters for listing news. All filters combine with AND.
#[derive(Debug, Clone, Default)]
pub struct NewsFilter {
    /// Defaults to approved items only.
    pub status: Option<NewsStatus>,
    /// Keep items about this organization.
    pub organization_slug: Option<String>,
    /// Keep items from this source.
    pub source_slug: Option<String>,
    /// Free text over title and summary.
    pub query: Option<String>,
}

#[derive(FromRow)]
struct NewsRow {
    id: Uuid,
    url: String,
    title: String,
    publisher: Option<String>,
    summary: Option<String>,
    image_url: Option<String>,
    author: Option<String>,
    published_at: DateTime<Utc>,
    status: NewsStatus,
}

macro_rules! filter_where {
    () => {
        " WHERE n.status = $1
            AND ($2::text IS NULL OR EXISTS (
                  SELECT 1 FROM news_item_organizations no
                  JOIN organizations o ON o.id = no.organization_id
                  WHERE no.news_item_id = n.id AND o.slug = $2))
            AND ($3::text IS NULL OR EXISTS (
                  SELECT 1 FROM sources s WHERE s.id = n.source_id AND s.slug = $3))
            AND ($4::text IS NULL OR n.title ILIKE '%' || $4 || '%'
                 OR n.summary ILIKE '%' || $4 || '%') "
    };
}

/// Lists news, newest first.
pub async fn list(
    pool: &PgPool,
    filter: &NewsFilter,
    page: PageParams,
) -> sqlx::Result<Page<NewsItem>> {
    let status = filter.status.unwrap_or(NewsStatus::Approved);
    let query = filter
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(escape_like);

    let rows: Vec<NewsRow> = sqlx::query_as(concat!(
        "SELECT n.id, n.url, n.title, n.publisher, n.summary, n.image_url, n.author,
                n.published_at, n.status
         FROM news_items n",
        filter_where!(),
        "ORDER BY n.published_at DESC, n.id LIMIT $5 OFFSET $6"
    ))
    .bind(status)
    .bind(filter.organization_slug.as_deref())
    .bind(filter.source_slug.as_deref())
    .bind(query.as_deref())
    .bind(page.limit())
    .bind(page.offset())
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(concat!(
        "SELECT count(*) FROM news_items n",
        filter_where!()
    ))
    .bind(status)
    .bind(filter.organization_slug.as_deref())
    .bind(filter.source_slug.as_deref())
    .bind(query.as_deref())
    .fetch_one(pool)
    .await?;

    Ok(Page {
        items: with_mentions(pool, rows).await?,
        total,
        limit: page.limit(),
        offset: page.offset(),
    })
}

pub async fn get(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<NewsItem>> {
    let row: Option<NewsRow> = sqlx::query_as(
        "SELECT id, url, title, publisher, summary, image_url, author, published_at, status
         FROM news_items WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some(row) => with_mentions(pool, vec![row]).await?.pop(),
        None => None,
    })
}

/// Loads the organizations of each item in one query, keeping the item order.
async fn with_mentions(pool: &PgPool, rows: Vec<NewsRow>) -> sqlx::Result<Vec<NewsItem>> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mentions: Vec<(Uuid, String, String, NewsMatch)> = sqlx::query_as(
        "SELECT no.news_item_id, o.slug, o.name, no.matched_by
         FROM news_item_organizations no
         JOIN organizations o ON o.id = no.organization_id
         WHERE no.news_item_id = ANY ($1)
         ORDER BY no.matched_by, o.name",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;

    let mut by_item: HashMap<Uuid, Vec<NewsMention>> = HashMap::new();
    for (item, slug, name, matched_by) in mentions {
        by_item.entry(item).or_default().push(NewsMention {
            slug,
            name,
            matched_by,
        });
    }

    Ok(rows
        .into_iter()
        .map(|r| NewsItem {
            organizations: by_item.remove(&r.id).unwrap_or_default(),
            id: r.id,
            url: r.url,
            title: r.title,
            publisher: r.publisher,
            summary: r.summary,
            image_url: r.image_url,
            author: r.author,
            published_at: r.published_at,
            status: r.status,
        })
        .collect())
}

/// Sets the review status of items. Returns how many changed.
pub async fn set_status(pool: &PgPool, ids: &[Uuid], status: NewsStatus) -> sqlx::Result<u64> {
    let result = sqlx::query("UPDATE news_items SET status = $2 WHERE id = ANY ($1)")
        .bind(ids)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Replaces the organizations of an item with an editor's choice.
/// Unknown slugs are ignored. Returns `false` when the item does not exist.
pub async fn set_organizations(pool: &PgPool, id: Uuid, slugs: &[String]) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM news_items WHERE id = $1)")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Ok(false);
    }
    // Keep how existing links were found, so an editor confirming a tag does not erase that.
    sqlx::query(
        "DELETE FROM news_item_organizations no USING organizations o
         WHERE no.organization_id = o.id AND no.news_item_id = $1 AND o.slug <> ALL ($2)",
    )
    .bind(id)
    .bind(slugs)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO news_item_organizations (news_item_id, organization_id, matched_by)
         SELECT $1, id, 'manual' FROM organizations WHERE slug = ANY ($2)
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(slugs)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Feed sources with their fetch health and item counts, failing ones first.
pub async fn list_sources(pool: &PgPool) -> sqlx::Result<Vec<SourceStatus>> {
    sqlx::query_as(
        "SELECT s.id, s.slug, s.name, s.url, s.enabled, s.auto_approve, s.fetch_interval_minutes,
                o.slug AS organization_slug, o.name AS organization_name,
                s.last_run_at, s.last_success_at, s.last_error, s.consecutive_failures,
                count(n.id) AS item_count,
                count(n.id) FILTER (WHERE n.status = 'pending') AS pending_count
         FROM sources s
         LEFT JOIN organizations o ON o.id = s.organization_id
         LEFT JOIN news_items n ON n.source_id = s.id
         GROUP BY s.id, o.slug, o.name
         ORDER BY s.consecutive_failures DESC, s.name",
    )
    .fetch_all(pool)
    .await
}

/// Escapes `%`, `_` and `\` so user text matches literally in ILIKE.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_like_makes_wildcards_literal() {
        assert_eq!(escape_like("50%_off\\"), "50\\%\\_off\\\\");
    }
}
