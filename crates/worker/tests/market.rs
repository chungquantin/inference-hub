//! Market adapters end to end: fake provider APIs, the real worker job, and
//! the store's history rules.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use axum::{Router, routing::get, routing::post};
use ih_ingest::adapters::{Endpoints, market_adapters};
use sqlx::PgPool;

/// Fake APIs of all five providers. The test edits the bodies between runs.
#[derive(Clone)]
struct FakeMarket {
    openrouter: Arc<Mutex<String>>,
    liquid: Arc<Mutex<String>>,
    runpod: Arc<Mutex<String>>,
    hyperbolic: Arc<Mutex<String>>,
    coreweave: Arc<Mutex<String>>,
    runpod_hits: Arc<AtomicUsize>,
}

fn coreweave_card(name: &str, count: &str, price: &str) -> String {
    format!(
        "<div><h3>{name}</h3><span>{count}</span><span>80</span><span>128</span><span>2,048</span>\
         <span>61.44</span><span>{price}</span></div><div><p>{name}</p><p>On-Demand Price:</p>\
         <p>{price}</p><p>/ Hour</p><p>Spot Price:</p><p>N/A</p><p>/ Hour</p></div>"
    )
}

fn coreweave_page(h100: &str) -> String {
    let mut html = String::from("<html>");
    html.push_str(&coreweave_card("NVIDIA HGX H100", "8", h100));
    for (name, price) in [
        ("NVIDIA L40S", "$18.00"),
        ("NVIDIA A100", "$21.60"),
        ("NVIDIA GH200", "$6.50"),
    ] {
        html.push_str(&coreweave_card(
            name,
            if name.contains("GH200") { "1" } else { "8" },
            price,
        ));
    }
    html.push_str("</html>");
    html
}

fn runpod_body(h100_price: f64) -> String {
    format!(
        r#"{{"data":{{"gpuTypes":[
            {{"id":"NVIDIA H100 80GB HBM3","displayName":"H100 SXM","memoryInGb":80,"securePrice":{h100_price},"communityPrice":2.69}},
            {{"id":"NVIDIA L40S","displayName":"L40S","memoryInGb":48,"securePrice":1.09,"communityPrice":0.79}},
            {{"id":"NVIDIA A40","displayName":"A40","memoryInGb":48,"securePrice":0.59,"communityPrice":0.35}},
            {{"id":"NVIDIA L4","displayName":"L4","memoryInGb":24,"securePrice":0.59,"communityPrice":0.44}}
        ]}}}}"#
    )
}

impl FakeMarket {
    fn new() -> Self {
        Self {
            openrouter: Arc::new(Mutex::new(
                r#"{"data":[
                    {"id":"openai/gpt-oss-120b","name":"OpenAI: gpt-oss-120b","context_length":131072,
                     "pricing":{"prompt":"0.00000015","completion":"0.0000006"}},
                    {"id":"unmapped/model","name":"Unmapped","context_length":8192,
                     "pricing":{"prompt":"0.000001","completion":"0.000002"}}
                ]}"#
                .into(),
            )),
            liquid: Arc::new(Mutex::new(
                r#"[{"served_model":"gpt-oss-120b","creator":"openai","provider_slug":"akash",
                     "billing_schema":{"rate_uncached_input":0.037,"rate_generation":0.187},
                     "input_length_band":{"max":131072},"unverified":false}]"#
                    .into(),
            )),
            runpod: Arc::new(Mutex::new(runpod_body(3.99))),
            hyperbolic: Arc::new(Mutex::new(
                r#"[{"enabled":true,"costPerHourCents":349,"region":"us-east-1","gpuCount":1,"gpuType":"h100","gpuFormFactor":"sxm5"},
                    {"enabled":true,"costPerHourCents":3592,"region":"us-southeast-2","gpuCount":8,"gpuType":"h100","gpuFormFactor":"sxm5"}]"#
                    .into(),
            )),
            coreweave: Arc::new(Mutex::new(coreweave_page("$49.24"))),
            runpod_hits: Arc::default(),
        }
    }

    async fn start(&self) -> Endpoints {
        let s = self.clone();
        let body = |m: &Arc<Mutex<String>>| {
            let m = m.clone();
            move || {
                let m = m.clone();
                async move { m.lock().unwrap().clone() }
            }
        };
        let hits = s.runpod_hits.clone();
        let runpod = s.runpod.clone();
        let app = Router::new()
            .route("/api/v1/models", get(body(&s.openrouter)))
            .route("/v1/exchange/book", get(body(&s.liquid)))
            .route(
                "/graphql",
                post(move || {
                    let (runpod, hits) = (runpod.clone(), hits.clone());
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        runpod.lock().unwrap().clone()
                    }
                }),
            )
            .route("/v2/on-demand/rental-options", get(body(&s.hyperbolic)))
            .route("/pricing", get(body(&s.coreweave)));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Endpoints::all(base)
    }
}

/// The organizations the adapters report for, and two mapped products.
async fn seed(pool: &PgPool) {
    let yaml = [
        "slug: openrouter\nname: OpenRouter\nkinds: [llm_gateway]\n",
        "slug: liquid-inference\nname: Liquid Inference\nkinds: [llm_gateway]\n",
        "slug: runpod\nname: RunPod\nkinds: [gpu_cloud]\n",
        "slug: hyperbolic\nname: Hyperbolic\nkinds: [gpu_cloud]\n",
        "slug: coreweave\nname: CoreWeave\nkinds: [gpu_cloud]\n",
        r#"
slug: openai
name: OpenAI
kinds: [model_provider]
products:
  - slug: gpt-oss-120b
    name: gpt-oss-120b
    type: model
    listings: { openrouter: [openai/gpt-oss-120b], liquid: [gpt-oss-120b] }
"#,
        r#"
slug: nvidia
name: NVIDIA
kinds: [hardware]
products:
  - slug: nvidia-h100
    name: NVIDIA H100
    type: hardware
    listings:
      runpod: [NVIDIA H100 80GB HBM3]
      hyperbolic: [h100 sxm5]
      coreweave: [NVIDIA HGX H100]
"#,
    ];
    let files = yaml
        .iter()
        .map(|y| {
            let org: seed::data::OrgSeed = serde_saphyr::from_str(y).unwrap();
            (format!("{}.yaml", org.slug), org)
        })
        .collect();
    seed::import(pool, &seed::DataSet::from_organizations(files))
        .await
        .unwrap();
}

fn context(endpoints: &Endpoints) -> worker::Context {
    worker::Context::with_endpoints(
        ih_ingest::http_client().unwrap(),
        worker::GitHub {
            api_url: "http://127.0.0.1:9".into(),
            token: None,
        },
        endpoints,
    )
    .only_adapters(&TESTED)
}

/// The adapters this suite fakes; their parsers have unit tests of their own.
const TESTED: [&str; 5] = ["openrouter", "liquid", "runpod", "hyperbolic", "coreweave"];

/// Runs every due adapter job through the real worker.
async fn run_adapters(pool: &PgPool, ctx: &worker::Context) -> usize {
    worker::schedule_due_adapters(pool, ctx).await.unwrap();
    worker::drain(pool, ctx, "t").await.unwrap()
}

async fn make_due(pool: &PgPool) {
    sqlx::query("UPDATE adapter_state SET last_run_at = now() - interval '1 day'")
        .execute(pool)
        .await
        .unwrap();
}

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct OfferingRow {
    provider: String,
    gpu_count: Option<i16>,
    region: Option<String>,
    input: Option<f64>,
    output: Option<f64>,
    per_hour: Option<f64>,
    ended: bool,
}

async fn offerings(pool: &PgPool, product: &str) -> Vec<OfferingRow> {
    sqlx::query_as(
        "SELECT o.slug AS provider, f.gpu_count, f.region,
                f.price_input_per_mtok::float8 AS input, f.price_output_per_mtok::float8 AS output,
                f.price_per_hour::float8 AS per_hour, f.ended_at IS NOT NULL AS ended
         FROM offerings f JOIN organizations o ON o.id = f.provider_id
         JOIN products p ON p.id = f.product_id
         WHERE p.slug = $1 ORDER BY o.slug, f.gpu_count NULLS FIRST, f.observed_at, f.id",
    )
    .bind(product)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn gpu(
    provider: &str,
    count: i16,
    region: Option<&str>,
    per_hour: f64,
    ended: bool,
) -> OfferingRow {
    OfferingRow {
        provider: provider.into(),
        gpu_count: Some(count),
        region: region.map(Into::into),
        input: None,
        output: None,
        per_hour: Some(per_hour),
        ended,
    }
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn every_adapter_records_listings_and_mapped_offerings(pool: PgPool) {
    seed(&pool).await;
    let market = FakeMarket::new();
    let ctx = context(&market.start().await);
    assert_eq!(run_adapters(&pool, &ctx).await, 5);

    let states: Vec<(String, i32, Option<String>)> = sqlx::query_as(
        "SELECT adapter, listings_seen, last_error FROM adapter_state ORDER BY adapter",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        states,
        [
            ("coreweave".into(), 4, None),
            ("hyperbolic".into(), 2, None),
            ("liquid".into(), 1, None),
            ("openrouter".into(), 2, None),
            ("runpod".into(), 4, None),
        ]
    );

    // Unmapped listings are kept for editors, but are not offerings.
    let unmapped: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM market_listings WHERE adapter = 'openrouter' AND external_id = 'unmapped/model'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unmapped, 1);

    let oss = offerings(&pool, "gpt-oss-120b").await;
    let prices: Vec<(&str, Option<f64>, Option<f64>)> = oss
        .iter()
        .map(|o| (o.provider.as_str(), o.input, o.output))
        .collect();
    assert_eq!(
        prices,
        [
            ("liquid-inference", Some(0.037), Some(0.187)),
            ("openrouter", Some(0.15), Some(0.6)),
        ]
    );

    assert_eq!(
        offerings(&pool, "nvidia-h100").await,
        [
            gpu("coreweave", 8, None, 49.24, false),
            gpu("hyperbolic", 1, Some("us-east-1"), 3.49, false),
            gpu("hyperbolic", 8, Some("us-southeast-2"), 35.92, false),
            gpu("runpod", 1, None, 3.99, false),
        ]
    );

    // The comparison API sees adapter offerings like curated ones.
    let per_gpu: Vec<(String, f64)> = sqlx::query_as(
        "SELECT o.slug, (f.price_per_hour / f.gpu_count)::float8 FROM latest_offerings f
         JOIN organizations o ON o.id = f.provider_id WHERE f.gpu_type = 'H100 SXM'
         ORDER BY 2, 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(per_gpu[0], ("hyperbolic".into(), 3.49));
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn history_appends_on_change_confirms_on_repeat_and_ends_on_delist(pool: PgPool) {
    seed(&pool).await;
    let market = FakeMarket::new();
    let ctx = context(&market.start().await);
    run_adapters(&pool, &ctx).await;

    // Same prices again: no new rows, confirmed_at moves.
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    let runpod_rows = || async {
        offerings(&pool, "nvidia-h100")
            .await
            .into_iter()
            .filter(|o| o.provider == "runpod")
            .collect::<Vec<_>>()
    };
    assert_eq!(runpod_rows().await, [gpu("runpod", 1, None, 3.99, false)]);
    let confirmed_later: bool = sqlx::query_scalar(
        "SELECT confirmed_at > observed_at FROM offerings f JOIN organizations o ON o.id = f.provider_id
         WHERE o.slug = 'runpod'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(confirmed_later);

    // RunPod cuts its H100 price: one new row, the old one stays as history.
    *market.runpod.lock().unwrap() = runpod_body(2.99);
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    assert_eq!(
        runpod_rows().await,
        [
            gpu("runpod", 1, None, 3.99, false),
            gpu("runpod", 1, None, 2.99, false)
        ]
    );

    // Hyperbolic stops offering the 8× configuration: its offering ends and
    // leaves the comparison, the 1× one stays.
    *market.hyperbolic.lock().unwrap() =
        r#"[{"enabled":true,"costPerHourCents":349,"region":"us-east-1","gpuCount":1,"gpuType":"h100","gpuFormFactor":"sxm5"}]"#
            .into();
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    let hyperbolic: Vec<(Option<i16>, bool)> = offerings(&pool, "nvidia-h100")
        .await
        .into_iter()
        .filter(|o| o.provider == "hyperbolic")
        .map(|o| (o.gpu_count, o.ended))
        .collect();
    assert_eq!(hyperbolic, [(Some(1), false), (Some(8), true)]);
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM latest_offerings f JOIN organizations o ON o.id = f.provider_id
         WHERE o.slug = 'hyperbolic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(live, 1);

    // It comes back at the same price: a fresh row, not a revival of the ended one.
    *market.hyperbolic.lock().unwrap() =
        r#"[{"enabled":true,"costPerHourCents":349,"region":"us-east-1","gpuCount":1,"gpuType":"h100","gpuFormFactor":"sxm5"},
            {"enabled":true,"costPerHourCents":3592,"region":"us-southeast-2","gpuCount":8,"gpuType":"h100","gpuFormFactor":"sxm5"}]"#
            .into();
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    let eight: Vec<bool> = offerings(&pool, "nvidia-h100")
        .await
        .into_iter()
        .filter(|o| o.provider == "hyperbolic" && o.gpu_count == Some(8))
        .map(|o| o.ended)
        .collect();
    assert_eq!(eight, [true, false]);
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn broken_providers_fail_without_touching_data(pool: PgPool) {
    seed(&pool).await;
    let market = FakeMarket::new();
    let ctx = context(&market.start().await);
    run_adapters(&pool, &ctx).await;
    let before = offerings(&pool, "nvidia-h100").await;

    // CoreWeave redesigns its page; RunPod returns one GPU instead of four.
    *market.coreweave.lock().unwrap() = "<html><h1>Talk to sales</h1></html>".into();
    *market.runpod.lock().unwrap() =
        r#"{"data":{"gpuTypes":[{"id":"NVIDIA L4","displayName":"L4","memoryInGb":24,"securePrice":0.59,"communityPrice":0.44}]}}"#
            .into();
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;

    let errors: Vec<(String, i32, String)> = sqlx::query_as(
        "SELECT adapter, consecutive_failures, last_error FROM adapter_state
         WHERE last_error IS NOT NULL ORDER BY adapter",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].2.contains("layout changed"), "{}", errors[0].2);
    assert!(
        errors[1].2.contains("saw 1 listings after 4"),
        "{}",
        errors[1].2
    );
    assert_eq!(
        offerings(&pool, "nvidia-h100").await,
        before,
        "nothing was ended or added"
    );

    // 7 hours later the healthy adapters are past their 6-hour interval; the two
    // failing ones have one failure each, so theirs doubled to 12 hours.
    sqlx::query("UPDATE adapter_state SET last_run_at = now() - interval '7 hours'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(worker::schedule_due_adapters(&pool, &ctx).await.unwrap(), 3);
}

#[test]
fn every_adapter_has_a_unique_id_and_an_organization_in_the_data() {
    let adapters = market_adapters(&Endpoints::default());
    let mut ids: Vec<&str> = adapters.iter().map(|a| a.info().id()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), adapters.len());
    assert_eq!(
        ids,
        seed::data::ADAPTERS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );

    let data =
        seed::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    for adapter in &adapters {
        let slug = adapter.info().organization_slug();
        assert!(
            data.organizations.iter().any(|(_, o)| o.slug == slug),
            "adapter {} reports for `{slug}`, which has no organization file",
            adapter.info().id()
        );
    }
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn delisting_only_ends_the_adapters_own_offerings(pool: PgPool) {
    seed(&pool).await;
    // A curated offering for the same configuration Hyperbolic will delist,
    // recorded by hand for Hyperbolic, and one for another provider.
    sqlx::query(
        "INSERT INTO offerings (product_id, provider_id, gpu_type, gpu_count, price_per_hour,
                                region, observed_at, source)
         SELECT p.id, o.id, 'H100 SXM', 8, 30, 'us-southeast-2', '2025-01-01', 'manual'
         FROM products p, organizations o
         WHERE p.slug = 'nvidia-h100' AND o.slug IN ('hyperbolic', 'runpod')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let market = FakeMarket::new();
    let ctx = context(&market.start().await);
    run_adapters(&pool, &ctx).await;

    *market.hyperbolic.lock().unwrap() =
        r#"[{"enabled":true,"costPerHourCents":349,"region":"us-east-1","gpuCount":1,"gpuType":"h100","gpuFormFactor":"sxm5"}]"#
            .into();
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;

    let manual_ended: Vec<(String, bool)> = sqlx::query_as(
        "SELECT o.slug, f.ended_at IS NOT NULL FROM offerings f
         JOIN organizations o ON o.id = f.provider_id
         WHERE f.source = 'manual' ORDER BY o.slug",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        manual_ended,
        [("hyperbolic".into(), false), ("runpod".into(), false)],
        "curated rows are the editor's to end, not the adapter's"
    );
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn implausible_prices_are_held_until_an_editor_accepts_them(pool: PgPool) {
    seed(&pool).await;
    let market = FakeMarket::new();
    let ctx = context(&market.start().await);
    run_adapters(&pool, &ctx).await;
    let h100 = || async {
        offerings(&pool, "nvidia-h100")
            .await
            .into_iter()
            .filter(|o| o.provider == "runpod")
            .collect::<Vec<_>>()
    };
    assert_eq!(h100().await, [gpu("runpod", 1, None, 3.99, false)]);

    // A tenfold jump (a unit mistake upstream) is held; the old price stays live.
    *market.runpod.lock().unwrap() = runpod_body(39.9);
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    assert_eq!(
        h100().await,
        [gpu("runpod", 1, None, 3.99, false)],
        "not published"
    );
    let holds: Vec<(i64, String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT id, reason, price_per_hour::float8, previous_per_hour::float8
         FROM price_holds WHERE resolution IS NULL",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(holds.len(), 1, "{holds:?}");
    assert_eq!(
        (holds[0].1.as_str(), holds[0].2, holds[0].3),
        ("jump", Some(39.9), Some(3.99))
    );
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM market_listings WHERE adapter = 'runpod' AND delisted_at IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(live, 4, "the held listing is not delisted");

    // Rejected: the same price stays held, silently.
    ih_db::market::resolve_hold(&pool, holds[0].0, ih_core::HoldResolution::Rejected)
        .await
        .unwrap();
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    assert!(ih_db::market::open_holds(&pool).await.unwrap().is_empty());
    assert_eq!(h100().await, [gpu("runpod", 1, None, 3.99, false)]);

    // A different implausible price reopens the review; accepting it publishes it.
    *market.runpod.lock().unwrap() = runpod_body(41.0);
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    let open = ih_db::market::open_holds(&pool).await.unwrap();
    assert_eq!(open.len(), 1);
    ih_db::market::resolve_hold(&pool, open[0].id, ih_core::HoldResolution::Accepted)
        .await
        .unwrap();
    make_due(&pool).await;
    run_adapters(&pool, &ctx).await;
    let prices: Vec<Option<f64>> = h100().await.iter().map(|o| o.per_hour).collect();
    assert_eq!(prices, [Some(3.99), Some(41.0)]);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM price_holds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0, "the accepted hold is settled once stored");
}

#[sqlx::test(migrator = "ih_db::MIGRATOR")]
async fn jobs_out_of_attempts_go_to_the_dead_letter_table(pool: PgPool) {
    seed(&pool).await;
    let market = FakeMarket::new();
    *market.coreweave.lock().unwrap() = "<html>changed</html>".into();
    let ctx = context(&market.start().await);
    run_adapters(&pool, &ctx).await;
    let failed = ih_db::market::failed_jobs(&pool, 10).await.unwrap();
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert_eq!(failed[0].kind, "run_adapter");
    assert_eq!(failed[0].payload["adapter"], "coreweave");
    assert!(
        failed[0]
            .last_error
            .as_deref()
            .unwrap()
            .contains("layout changed")
    );
}
