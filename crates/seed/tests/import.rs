//! Import tests against a real Postgres (see crates/api/tests/api.rs for setup).

use seed::{Summary, data::OrgSeed};
use sqlx::PgPool;

fn file(yaml: &str) -> (String, OrgSeed) {
    let org: OrgSeed = serde_saphyr::from_str(yaml).unwrap();
    (format!("{}.yaml", org.slug), org)
}

const ORG_V1: &str = r#"
slug: acme
name: Acme
kinds: [model_provider]
categories: [investors]
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
    let files = vec![file(ORG_V1)];
    let first = seed::import(&pool, &files).await.unwrap();
    assert_eq!(
        first,
        Summary {
            organizations: 1,
            products: 1,
            offerings_added: 1
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
    seed::import(&pool, &[file(ORG_V1)]).await.unwrap();
    let summary = seed::import(&pool, &[file(ORG_V2)]).await.unwrap();
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

    let err = seed::import(&pool, &[bad]).await.unwrap_err().to_string();
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
    let files = seed::load(&dir).expect("data/ parses");
    assert!(
        files.len() >= 20,
        "expected the curated data set, got {}",
        files.len()
    );
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
    let issues = seed::data::validate(&files, &categories);
    assert!(issues.is_empty(), "{issues:#?}");
}
