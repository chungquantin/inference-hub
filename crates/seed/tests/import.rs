//! Import tests against a real Postgres (see crates/api/tests/api.rs for setup).

use seed::{DataSet, Summary, data::OrgSeed};
use sqlx::PgPool;

fn file(yaml: &str) -> (String, OrgSeed) {
    let org: OrgSeed = serde_saphyr::from_str(yaml).unwrap();
    (format!("{}.yaml", org.slug), org)
}

fn data(files: Vec<(String, OrgSeed)>) -> DataSet {
    DataSet::from_organizations(files)
}

const ORG_V1: &str = r#"
slug: acme
name: Acme
kinds: [model_provider]
categories: [investors]
tagging: { aliases: [Acme AI], exclude: [acme corp], match_case: true }
feeds:
  - { slug: acme-blog, name: Acme Blog, url: "https://acme.example/feed.xml" }
products:
  - slug: acme-1
    name: Acme 1
    type: model
    model: { parameters_b: 7, open_weights: true }
    offerings:
      - { provider: acme, input: 1, output: 2, observed_on: 2025-01-01 }
"#;

const ORG_V2: &str = r#"
slug: acme
name: Acme Labs
kinds: [gpu_cloud]
feeds:
  - { slug: acme-blog, name: Acme Blog, url: "https://acme.example/new-feed.xml", auto_approve: false }
products:
  - slug: acme-1
    name: Acme 1
    type: oss_project
    offerings:
      - { provider: acme, input: 1, output: 2, observed_on: 2025-01-01 }
      - { provider: acme, input: 0.5, output: 1, observed_on: 2025-02-01 }
"#;

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn import_is_idempotent(pool: PgPool) {
    let files = data(vec![file(ORG_V1)]);
    let first = seed::import(&pool, &files).await.unwrap();
    assert_eq!(
        first,
        Summary {
            organizations: 1,
            products: 1,
            offerings_added: 1,
            sources: 1,
            funding_rounds: 0,
        }
    );
    let second = seed::import(&pool, &files).await.unwrap();
    assert_eq!(second.offerings_added, 0);

    let offerings: i64 = sqlx::query_scalar("SELECT count(*) FROM offerings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(offerings, 1);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn reimport_updates_fields_categories_and_model(pool: PgPool) {
    seed::import(&pool, &data(vec![file(ORG_V1)]))
        .await
        .unwrap();
    let summary = seed::import(&pool, &data(vec![file(ORG_V2)]))
        .await
        .unwrap();
    assert_eq!(
        summary.offerings_added, 1,
        "only the new observation is added"
    );

    let name: String = sqlx::query_scalar("SELECT name FROM organizations WHERE slug = 'acme'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Acme Labs");

    let categories: Vec<String> = sqlx::query_scalar(
        "SELECT c.slug FROM categories c
         JOIN organization_categories oc ON oc.category_id = c.id
         JOIN organizations o ON o.id = oc.organization_id
         WHERE o.slug = 'acme' ORDER BY c.slug",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(categories, ["gpu-clouds"], "categories follow the YAML");

    let models: i64 = sqlx::query_scalar("SELECT count(*) FROM models")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        models, 0,
        "model row is removed when the product is no longer a model"
    );
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn invalid_data_is_rejected_without_writing(pool: PgPool) {
    let mut bad = file(ORG_V1);
    bad.1.products[0].offerings[0].provider = "nobody".to_owned();
    bad.1.categories.push("not-a-category".to_owned());

    let err = seed::import(&pool, &data(vec![bad]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("2 problem(s)"), "{err}");
    assert!(err.contains("unknown category `not-a-category`"), "{err}");

    let acme: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations WHERE slug = 'acme'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(acme, 0);
}

#[test]
fn bundled_data_parses() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let set = seed::load(&dir).expect("data/ parses");
    assert!(
        set.organizations.len() >= 20,
        "expected the curated data set, got {}",
        set.organizations.len()
    );
    assert!(!set.sources.feeds.is_empty(), "expected data/sources.yaml");
    let categories = [
        "model-providers",
        "inference-providers",
        "llm-gateways",
        "open-source",
        "gpu-clouds",
        "hyperscalers",
        "hardware",
        "investors",
    ]
    .map(str::to_owned)
    .into();
    let issues = set.validate(&categories);
    assert!(issues.is_empty(), "{issues:#?}");
}

const STARTUP: &str = r#"
slug: startup
name: Startup
kinds: [inference_provider]
products:
  - { slug: startup-engine, name: Engine, type: oss_project, github: startup/engine }
funding:
  - round: series_a
    announced_on: 2025-03-01
    amount: 50000000
    lead: [fund-a]
    investors: [fund-b]
"#;

const FUNDS: [&str; 2] = [
    "slug: fund-a\nname: Fund A\nkinds: [investor]\n",
    "slug: fund-b\nname: Fund B\nkinds: [investor]\n",
];

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn funding_rounds_and_repos_are_upserted(pool: PgPool) {
    let set = |startup: &str| data(vec![file(startup), file(FUNDS[0]), file(FUNDS[1])]);
    let first = seed::import(&pool, &set(STARTUP)).await.unwrap();
    assert_eq!(first.funding_rounds, 1);

    let investors = || async {
        sqlx::query_as::<_, (String, bool, Option<i64>)>(
            "SELECT o.slug, fi.is_lead, fr.amount FROM funding_round_investors fi
             JOIN organizations o ON o.id = fi.investor_id
             JOIN funding_rounds fr ON fr.id = fi.funding_round_id ORDER BY o.slug",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
    };
    assert_eq!(
        investors().await,
        [
            ("fund-a".to_owned(), true, Some(50_000_000)),
            ("fund-b".to_owned(), false, Some(50_000_000)),
        ]
    );

    // Same round with a corrected amount and investor list: updated, not duplicated.
    let changed = STARTUP
        .replace("amount: 50000000", "amount: 55000000")
        .replace("    investors: [fund-b]\n", "");
    seed::import(&pool, &set(&changed)).await.unwrap();
    let rounds: i64 = sqlx::query_scalar("SELECT count(*) FROM funding_rounds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rounds, 1);
    assert_eq!(
        investors().await,
        [("fund-a".to_owned(), true, Some(55_000_000))]
    );

    let repo: Option<String> =
        sqlx::query_scalar("SELECT github_repo FROM products WHERE slug = 'startup-engine'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(repo.as_deref(), Some("startup/engine"));
}
