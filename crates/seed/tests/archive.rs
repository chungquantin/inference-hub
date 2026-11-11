//! The archive round trip: export from one database, import into another.

use seed::{DataSet, archive, data::OrgSeed};
use sqlx::PgPool;

const ORG: &str = r#"
slug: acme
name: Acme
kinds: [inference_provider]
feeds:
  - { slug: acme-blog, name: Acme Blog, url: "https://acme.example/feed.xml" }
products:
  - { slug: acme-engine, name: Acme Engine, type: oss_project, github: acme/engine }
"#;

async fn curated(pool: &PgPool) {
    let org: OrgSeed = serde_saphyr::from_str(ORG).unwrap();
    seed::import(
        pool,
        &DataSet::from_organizations(vec![("acme.yaml".into(), org)]),
    )
    .await
    .unwrap();
}

async fn fill(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO market_listings (adapter, listing_key, external_id, kind, name,
            price_input_per_mtok, price_output_per_mtok, extra, capabilities)
        VALUES ('acme', 'm1', 'acme/m1', 'model', 'M1', 0.123456, 2.5,
                '{"note": "x"}', '{tools}');
        INSERT INTO adapter_state (adapter, listings_seen, organization_id)
        SELECT 'acme', 1, id FROM organizations WHERE slug = 'acme';
        INSERT INTO news_items (source_id, url, title, published_at, status)
        SELECT id, 'https://acme.example/post', 'Post', '2026-01-01T00:00:00Z', 'approved'
          FROM sources WHERE slug = 'acme-blog';
        INSERT INTO news_item_organizations (news_item_id, organization_id, matched_by)
        SELECT n.id, o.id, 'source' FROM news_items n, organizations o WHERE o.slug = 'acme';
        INSERT INTO repo_stats (product_id, stars) SELECT id, 42 FROM products;
        INSERT INTO repo_star_history (product_id, day, stars)
        SELECT id, '2026-01-01', 40 FROM products;
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn snapshot(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT concat_ws('|', l.name, l.price_input_per_mtok, l.extra::text, l.capabilities::text,
                (SELECT o.slug FROM adapter_state a JOIN organizations o ON o.id = a.organization_id),
                (SELECT s.slug FROM news_items n JOIN sources s ON s.id = n.source_id),
                (SELECT count(*) FROM news_item_organizations),
                (SELECT stars FROM repo_stats),
                (SELECT stars FROM repo_star_history))
           FROM market_listings l",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn archive_round_trips_by_natural_keys(pool: PgPool) {
    curated(&pool).await;
    fill(&pool).await;
    let before = snapshot(&pool).await;

    let dir = std::env::temp_dir().join(format!("ih-archive-{}", uuid_like()));
    archive::export(&pool, &dir).await.unwrap();

    // A clean database with the same curated data but new ids.
    sqlx::raw_sql(
        "TRUNCATE market_listings, adapter_state, news_items, repo_stats, repo_star_history CASCADE;
         DELETE FROM organizations; DELETE FROM sources;",
    )
    .execute(&pool)
    .await
    .unwrap();
    curated(&pool).await;

    archive::import(&pool, &dir).await.unwrap();
    // Twice: the import upserts.
    archive::import(&pool, &dir).await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
    assert_eq!(before.len(), 1);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn rows_of_unknown_organizations_are_skipped(pool: PgPool) {
    curated(&pool).await;
    fill(&pool).await;
    let dir = std::env::temp_dir().join(format!("ih-archive-{}", uuid_like()));
    archive::export(&pool, &dir).await.unwrap();

    sqlx::raw_sql(
        "TRUNCATE market_listings, adapter_state, news_items, repo_stats, repo_star_history CASCADE;
         DELETE FROM organizations;",
    )
    .execute(&pool)
    .await
    .unwrap();
    archive::import(&pool, &dir).await.unwrap();

    let tags: i64 = sqlx::query_scalar("SELECT count(*) FROM news_item_organizations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tags, 0);
    let news: i64 = sqlx::query_scalar("SELECT count(*) FROM news_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(news, 1, "news stays, without its source");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn token_shaped_rows_stay_out_of_the_archive(pool: PgPool) {
    curated(&pool).await;
    // Built at run time so no token-shaped string sits in the repository.
    let tokenish = format!("u/{}_{}", "hf", "a".repeat(36));
    sqlx::query(
        "INSERT INTO market_listings (adapter, listing_key, external_id, kind, name)
         VALUES ('acme', 'ok', 'acme/ok', 'model', 'Fine Model'),
                ('acme', 'tok', $1, 'model', $1),
                ('acme', 'short', 'acme/hf_notatoken', 'model', 'hf_notatoken')",
    )
    .bind(&tokenish)
    .execute(&pool)
    .await
    .unwrap();
    let dir = std::env::temp_dir().join(format!("ih-archive-{}", uuid_like()));
    let counts = archive::export(&pool, &dir).await.unwrap();
    let rows = counts
        .iter()
        .find(|c| c.table == "market_listings")
        .unwrap()
        .rows;
    assert_eq!(
        rows, 2,
        "the token-shaped row is dropped, a short hf_ name is kept"
    );
    let text = std::fs::read_to_string(dir.join("market_listings.jsonl")).unwrap();
    assert!(!text.contains(&tokenish));
    assert!(text.contains("hf_notatoken"));
    std::fs::remove_dir_all(&dir).unwrap();
}

fn uuid_like() -> String {
    format!(
        "{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
