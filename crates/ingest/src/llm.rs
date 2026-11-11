//! LLM enrichment of news items through Liquid Inference, an OpenAI-compatible
//! exchange: a topic, a relevance grade, a review verdict with a confidence,
//! a one-sentence summary, and organizations of the directory the item is
//! about.
//!
//! The model picks organizations from a closed list of slugs, so it can never
//! invent one. Its output only suggests: items are promoted (approved) only on
//! a confident `approve` verdict for core news from a source that is about one
//! organization; everything else stays in the review queue with the
//! suggestion shown to the editor.
//!
//! Spend is bounded three ways: a cap per call sent to the exchange, a budget
//! per scheduler pass, and a budget per day counted from stored costs.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Bump when the prompt or schema changes, so items get enriched again.
const PROMPT_VERSION: &str = "v1";
const MIN_CONFIDENCE_TO_APPROVE: f32 = 0.7;
const TOPICS: [&str; 10] = [
    "pricing",
    "funding",
    "launch",
    "model_release",
    "partnership",
    "infrastructure",
    "research",
    "policy",
    "people",
    "other",
];

/// Where and how to buy enrichment.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// OpenAI-compatible origin, such as `https://router.inference.ai.exchange`.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// The most one call may cost, sent to the exchange.
    pub cap_usd_per_call: f64,
    /// The most one scheduler pass may spend.
    pub max_usd_per_run: f64,
    /// The most all enrichment may spend in 24 hours.
    pub max_usd_per_day: f64,
    /// Items per pass.
    pub batch: i64,
}

impl LlmConfig {
    /// Reads `LIQUID_BASE_URL` and `LIQUID_API_KEY`; `None` when either is unset,
    /// which turns enrichment off.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("LIQUID_BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let api_key = std::env::var("LIQUID_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())?;
        let num = |k: &str, d: f64| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v: &f64| v.is_finite() && *v >= 0.0)
                .unwrap_or(d)
        };
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key,
            model: std::env::var("LIQUID_MODEL").unwrap_or_else(|_| "liquid.auto".into()),
            cap_usd_per_call: num("LIQUID_CAP_USD_PER_CALL", 0.01),
            max_usd_per_run: num("MAX_ENRICH_USD_PER_RUN", 0.25),
            max_usd_per_day: num("MAX_ENRICH_USD_PER_DAY", 1.0),
            batch: num("ENRICH_BATCH", 20.0) as i64,
        })
    }
}

/// What the model returns, validated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Enrichment {
    pub topic: String,
    pub relevance: String,
    pub verdict: String,
    pub confidence: f32,
    pub summary: String,
    pub organizations: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum EnrichError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the exchange answered HTTP {0}: {1}")]
    Status(u16, String),
    #[error("unexpected answer: {0}")]
    Format(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct EnrichReport {
    pub enriched: usize,
    pub failed: usize,
    pub approved: usize,
    pub organizations_added: usize,
    pub cost_usd: f64,
    /// The pass stopped early because a budget ran out.
    pub budget_exhausted: bool,
}

#[derive(sqlx::FromRow)]
struct Candidate {
    id: Uuid,
    title: String,
    summary: Option<String>,
    publisher: Option<String>,
    /// Slugs the keyword tagger already linked.
    tagged: Vec<String>,
    /// The source is about one organization (a company blog), not a shared outlet.
    own_source: bool,
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["topic", "relevance", "verdict", "confidence", "summary", "organizations"],
        "properties": {
            "topic": { "type": "string", "enum": TOPICS },
            "relevance": { "type": "string", "enum": ["core", "adjacent", "off_topic"] },
            "verdict": { "type": "string", "enum": ["approve", "reject"] },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "summary": { "type": "string" },
            "organizations": { "type": "array", "items": { "type": "string" } }
        }
    })
}

const SYSTEM: &str = "You triage news for a directory of the AI inference market: model API \
providers, LLM gateways, GPU clouds, inference hardware and open-source serving software. \
Classify the item. relevance: core if it is about that market (prices, capacity, launches, \
funding or deals of those companies, serving software); adjacent if AI news that touches it; \
off_topic otherwise. verdict: approve if an editor of that directory should publish it. \
summary: one plain sentence of at most 30 words, facts from the item only. organizations: \
slugs from the given list that the item is substantially about; never a slug outside the list. \
Answer only with the JSON object.";

fn prompt(c: &Candidate, slugs: &[String]) -> String {
    format!(
        "Organizations (slugs): {}\n\nTitle: {}\nPublisher: {}\nAlready tagged: {}\nSummary from the feed: {}",
        slugs.join(", "),
        c.title,
        c.publisher.as_deref().unwrap_or("unknown"),
        if c.tagged.is_empty() {
            "none".into()
        } else {
            c.tagged.join(", ")
        },
        c.summary
            .as_deref()
            .map(|s| s.chars().take(2000).collect::<String>())
            .unwrap_or_else(|| "none".into()),
    )
}

/// A short stable hash of the prompt version and input, for re-enrichment checks.
fn prompt_hash(text: &str) -> String {
    // FNV-1a: stable across builds, no extra dependency; collisions only cause a skipped re-run.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in PROMPT_VERSION.bytes().chain(text.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{PROMPT_VERSION}:{h:016x}")
}

/// Checks the model's answer: known enums, a confidence in range, and only
/// slugs from the list. Unknown slugs are dropped, not trusted.
pub fn validate(raw: &str, slugs: &[String]) -> Result<Enrichment, EnrichError> {
    let mut e: Enrichment = serde_json::from_str(raw.trim())
        .map_err(|err| EnrichError::Format(format!("{err}: {raw}")))?;
    if !TOPICS.contains(&e.topic.as_str()) {
        e.topic = "other".into();
    }
    if !matches!(e.relevance.as_str(), "core" | "adjacent" | "off_topic") {
        return Err(EnrichError::Format(format!("relevance `{}`", e.relevance)));
    }
    if !matches!(e.verdict.as_str(), "approve" | "reject") {
        return Err(EnrichError::Format(format!("verdict `{}`", e.verdict)));
    }
    if !e.confidence.is_finite() {
        return Err(EnrichError::Format("confidence is not a number".into()));
    }
    e.confidence = e.confidence.clamp(0.0, 1.0);
    e.summary = e.summary.trim().chars().take(400).collect();
    e.organizations.retain(|s| slugs.contains(s));
    e.organizations.sort();
    e.organizations.dedup();
    Ok(e)
}

/// Whether an enrichment is strong enough to publish the item without an editor.
pub fn auto_approve(e: &Enrichment, own_source: bool) -> bool {
    own_source
        && e.verdict == "approve"
        && e.relevance == "core"
        && e.confidence >= MIN_CONFIDENCE_TO_APPROVE
}

#[derive(Deserialize)]
struct ChatResponse {
    model: Option<String>,
    choices: Vec<Choice>,
    exchange: Option<Exchange>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Exchange {
    charge: Option<Charge>,
}

#[derive(Deserialize)]
struct Charge {
    total_usd: Option<f64>,
}

/// One call to the exchange. Returns the raw answer, the model that served it and its cost.
async fn complete(
    http: &reqwest::Client,
    cfg: &LlmConfig,
    user: &str,
) -> Result<(String, String, f64), EnrichError> {
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            { "role": "system", "content": SYSTEM },
            { "role": "user", "content": user }
        ],
        "max_tokens": 800,
        "temperature": 0,
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "news_triage", "strict": true, "schema": schema() }
        }
    });
    let response = http
        .post(format!("{}/v1/chat/completions", cfg.base_url))
        .bearer_auth(&cfg.api_key)
        .header("x-liquid-cap-usd", cfg.cap_usd_per_call.to_string())
        .header("x-liquid-tag", "news-enrichment")
        .timeout(Duration::from_secs(90))
        .json(&body)
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(EnrichError::Status(
            status.as_u16(),
            text.chars().take(500).collect(),
        ));
    }
    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|e| EnrichError::Format(e.to_string()))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .filter(|c| !c.trim().is_empty())
        .ok_or_else(|| EnrichError::Format("empty answer".into()))?;
    let cost = parsed
        .exchange
        .and_then(|x| x.charge)
        .and_then(|c| c.total_usd)
        .unwrap_or(cfg.cap_usd_per_call);
    Ok((
        content,
        parsed.model.unwrap_or_else(|| cfg.model.clone()),
        cost,
    ))
}

/// Spend of the last 24 hours.
async fn spent_today(pool: &PgPool) -> sqlx::Result<f64> {
    sqlx::query_scalar(
        "SELECT coalesce(sum(cost_usd), 0)::float8 FROM news_enrichment
         WHERE enriched_at > now() - interval '24 hours'",
    )
    .fetch_one(pool)
    .await
}

/// Enriches up to one batch of pending, not yet enriched items, newest first,
/// within the budgets.
pub async fn enrich_pending(
    pool: &PgPool,
    http: &reqwest::Client,
    cfg: &LlmConfig,
) -> Result<EnrichReport, EnrichError> {
    let mut report = EnrichReport::default();
    let mut day_left = cfg.max_usd_per_day - spent_today(pool).await?;
    let mut run_left = cfg.max_usd_per_run;
    if day_left < cfg.cap_usd_per_call || run_left < cfg.cap_usd_per_call {
        report.budget_exhausted = true;
        return Ok(report);
    }

    let slugs: Vec<String> = sqlx::query_scalar(
        "SELECT slug FROM organizations WHERE NOT (kinds = ARRAY['investor']::org_kind[])
         ORDER BY slug",
    )
    .fetch_all(pool)
    .await?;
    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT n.id, n.title, n.summary, n.publisher,
                coalesce(array_agg(o.slug) FILTER (WHERE o.slug IS NOT NULL), '{}') AS tagged,
                coalesce(bool_or(s.organization_id IS NOT NULL), false) AS own_source
         FROM news_items n
         LEFT JOIN sources s ON s.id = n.source_id
         LEFT JOIN news_item_organizations nio ON nio.news_item_id = n.id
         LEFT JOIN organizations o ON o.id = nio.organization_id
         WHERE n.status = 'pending'
           AND NOT EXISTS (SELECT 1 FROM news_enrichment e WHERE e.news_item_id = n.id)
         GROUP BY n.id
         ORDER BY n.published_at DESC
         LIMIT $1",
    )
    .bind(cfg.batch.clamp(1, 200))
    .fetch_all(pool)
    .await?;

    for c in candidates {
        if day_left < cfg.cap_usd_per_call || run_left < cfg.cap_usd_per_call {
            report.budget_exhausted = true;
            break;
        }
        let user = prompt(&c, &slugs);
        let (raw, model, cost) = match complete(http, cfg, &user).await {
            Ok(answer) => answer,
            Err(err) => {
                tracing::warn!(item = %c.id, error = %err, "enrichment call failed");
                report.failed += 1;
                // An unreachable exchange fails every item; stop the pass.
                if matches!(err, EnrichError::Http(_))
                    || matches!(err, EnrichError::Status(s, _) if s == 402 || s == 401 || s >= 500)
                {
                    break;
                }
                continue;
            }
        };
        day_left -= cost;
        run_left -= cost;
        report.cost_usd += cost;
        let enrichment = match validate(&raw, &slugs) {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(item = %c.id, error = %err, "enrichment answer rejected");
                report.failed += 1;
                continue;
            }
        };
        let (added, approved) = store(
            pool,
            &c,
            &enrichment,
            &model,
            cost,
            &prompt_hash(&user),
            &raw,
        )
        .await?;
        report.enriched += 1;
        report.organizations_added += added;
        report.approved += usize::from(approved);
    }
    Ok(report)
}

async fn store(
    pool: &PgPool,
    c: &Candidate,
    e: &Enrichment,
    model: &str,
    cost: f64,
    hash: &str,
    raw: &str,
) -> sqlx::Result<(usize, bool)> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO news_enrichment
             (news_item_id, topic, relevance, verdict, confidence, summary_ai, model, cost_usd,
              prompt_hash, raw)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::float8::numeric, $9, $10)
         ON CONFLICT (news_item_id) DO UPDATE SET
             topic = EXCLUDED.topic, relevance = EXCLUDED.relevance, verdict = EXCLUDED.verdict,
             confidence = EXCLUDED.confidence, summary_ai = EXCLUDED.summary_ai,
             model = EXCLUDED.model, cost_usd = EXCLUDED.cost_usd,
             prompt_hash = EXCLUDED.prompt_hash, raw = EXCLUDED.raw, enriched_at = now()",
    )
    .bind(c.id)
    .bind(&e.topic)
    .bind(&e.relevance)
    .bind(&e.verdict)
    .bind(e.confidence)
    .bind(Some(e.summary.as_str()).filter(|s| !s.is_empty()))
    .bind(model)
    .bind(cost)
    .bind(hash)
    .bind(serde_json::from_str::<serde_json::Value>(raw).unwrap_or(serde_json::Value::Null))
    .execute(&mut *tx)
    .await?;

    let added = sqlx::query(
        "INSERT INTO news_item_organizations (news_item_id, organization_id, matched_by)
         SELECT $1, id, 'llm' FROM organizations WHERE slug = ANY ($2)
         ON CONFLICT DO NOTHING",
    )
    .bind(c.id)
    .bind(&e.organizations)
    .execute(&mut *tx)
    .await?
    .rows_affected() as usize;

    let approve = auto_approve(e, c.own_source);
    if approve {
        sqlx::query(
            "UPDATE news_items SET status = 'approved' WHERE id = $1 AND status = 'pending'",
        )
        .bind(c.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok((added, approve))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slugs() -> Vec<String> {
        ["coreweave", "openai", "nvidia"].map(String::from).to_vec()
    }

    #[test]
    fn keeps_only_known_slugs_and_clamps() {
        let e = validate(
            r#"{"topic":"partnership","relevance":"core","verdict":"approve","confidence":1.4,
                "summary":"  CoreWeave signed a deal.  ","organizations":["openai","coreweave","made-up","openai"]}"#,
            &slugs(),
        )
        .unwrap();
        assert_eq!(e.organizations, ["coreweave", "openai"]);
        assert_eq!(e.confidence, 1.0);
        assert_eq!(e.summary, "CoreWeave signed a deal.");
    }

    #[test]
    fn unknown_topics_become_other_and_bad_enums_fail() {
        let ok = r#"{"topic":"gossip","relevance":"core","verdict":"reject","confidence":0.2,"summary":"","organizations":[]}"#;
        assert_eq!(validate(ok, &slugs()).unwrap().topic, "other");
        let bad = r#"{"topic":"other","relevance":"maybe","verdict":"reject","confidence":0.2,"summary":"","organizations":[]}"#;
        assert!(validate(bad, &slugs()).is_err());
        assert!(validate("not json", &slugs()).is_err());
    }

    #[test]
    fn approves_only_confident_core_news_from_an_own_source() {
        let e = Enrichment {
            topic: "launch".into(),
            relevance: "core".into(),
            verdict: "approve".into(),
            confidence: 0.8,
            summary: String::new(),
            organizations: vec![],
        };
        assert!(auto_approve(&e, true));
        assert!(
            !auto_approve(&e, false),
            "shared outlets always go to an editor"
        );
        assert!(!auto_approve(
            &Enrichment {
                confidence: 0.6,
                ..e.clone()
            },
            true
        ));
        assert!(!auto_approve(
            &Enrichment {
                relevance: "adjacent".into(),
                ..e.clone()
            },
            true
        ));
        assert!(!auto_approve(
            &Enrichment {
                verdict: "reject".into(),
                ..e
            },
            true
        ));
    }

    #[test]
    fn prompt_hash_is_stable_and_versioned() {
        assert_eq!(prompt_hash("a"), prompt_hash("a"));
        assert_ne!(prompt_hash("a"), prompt_hash("b"));
        assert!(prompt_hash("a").starts_with("v1:"));
    }
}
