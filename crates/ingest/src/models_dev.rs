//! models.dev: an open-source database of AI models (MIT, github.com/anomalyco/models.dev).
//!
//! `https://models.dev/api.json` lists every provider with its models. The same model appears
//! under many providers; entries that share a `canonical_model_id` are one model, and are folded
//! into one set of facts. Facts are what is true of the model itself, not of one provider's offer:
//! release date, knowledge cutoff, open weights, the largest context and output limits any
//! provider states, modalities, and the capabilities most providers report.
//!
//! Prices in the file are not read: they are one provider's list price copied by hand, and the
//! market's own adapters read them at the source.

use std::collections::{BTreeSet, HashMap};

use chrono::NaiveDate;
use reqwest::Client;
use serde_json::Value;
use sqlx::PgPool;

use crate::adapters::{AdapterError, get_text};

const ID: &str = "models-dev";

/// What is known of one model, folded from every entry that names it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Fact {
    /// The canonical id (or, without one, the provider's id): normalized to a market key on store.
    pub raw_id: String,
    pub name: String,
    pub family: Option<String>,
    pub description: Option<String>,
    pub released_on: Option<NaiveDate>,
    pub knowledge_cutoff: Option<String>,
    pub open_weights: Option<bool>,
    pub context_window: Option<i32>,
    pub max_output: Option<i32>,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub reasoning: Option<bool>,
    pub tool_call: Option<bool>,
    pub structured_output: Option<bool>,
    pub attachment: Option<bool>,
    pub deprecated: bool,
    pub providers_listing: i32,
}

impl Fact {
    /// How much the entry says: the store keeps the fullest when two raw ids normalize to one key.
    fn score(&self) -> i32 {
        [
            self.family.is_some(),
            self.description.is_some(),
            self.released_on.is_some(),
            self.knowledge_cutoff.is_some(),
            self.open_weights.is_some(),
            self.context_window.is_some(),
            self.max_output.is_some(),
            !self.input_modalities.is_empty(),
            self.reasoning.is_some(),
            self.tool_call.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count() as i32
            * 10
            + self.providers_listing
    }
}

/// `2026-06-15`, `2026-06` (first of the month) or `2026`: the dates the file carries.
fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d").ok())
        .or_else(|| NaiveDate::parse_from_str(&format!("{s}-01-01"), "%Y-%m-%d").ok())
}

fn most_common<T: Clone + Eq + std::hash::Hash + Ord>(items: impl Iterator<Item = T>) -> Option<T> {
    let mut counts: HashMap<T, usize> = HashMap::new();
    for i in items {
        *counts.entry(i).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(v, _)| v)
}

/// True when at least half of the entries that state the flag say true.
fn majority(items: &[Option<bool>]) -> Option<bool> {
    let stated: Vec<bool> = items.iter().flatten().copied().collect();
    if stated.is_empty() {
        return None;
    }
    Some(stated.iter().filter(|b| **b).count() * 2 >= stated.len())
}

struct Entry<'a> {
    provider: &'a str,
    v: &'a Value,
}

fn int(v: &Value, path: &[&str]) -> Option<i32> {
    path.iter()
        .try_fold(v, |v, k| v.get(*k))
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .and_then(|n| i32::try_from(n).ok())
}

fn strs(v: &Value, path: &[&str]) -> Vec<String> {
    path.iter()
        .try_fold(v, |v, k| v.get(*k))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn flag(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(Value::as_bool)
}

fn fold(raw_id: &str, entries: &[Entry<'_>]) -> Fact {
    let providers: BTreeSet<&str> = entries.iter().map(|e| e.provider).collect();
    // The lab's own entry (provider `openai` for `openai/...`) is the best source of a description.
    let maker = raw_id.split('/').next().unwrap_or_default();
    let first_party = entries
        .iter()
        .find(|e| e.provider == maker || e.provider.starts_with(maker));
    let desc = |e: &Entry<'_>| {
        e.v.get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|d| d.len() >= 20)
            .map(str::to_owned)
    };
    let modalities = |dir: &str| -> Vec<String> {
        // What most providers say, so one wrapper that adds image input does not change the model.
        most_common(entries.iter().map(|e| strs(e.v, &["modalities", dir]))).unwrap_or_default()
    };
    let flags = |k: &str| majority(&entries.iter().map(|e| flag(e.v, k)).collect::<Vec<_>>());
    Fact {
        raw_id: raw_id.to_owned(),
        name: most_common(
            entries
                .iter()
                .filter_map(|e| e.v.get("name").and_then(Value::as_str))
                .map(str::to_owned),
        )
        .unwrap_or_else(|| raw_id.to_owned()),
        family: most_common(
            entries
                .iter()
                .filter_map(|e| e.v.get("family").and_then(Value::as_str))
                .map(str::to_owned),
        ),
        description: first_party
            .and_then(desc)
            .or_else(|| most_common(entries.iter().filter_map(desc))),
        released_on: entries
            .iter()
            .filter_map(|e| {
                e.v.get("release_date")
                    .and_then(Value::as_str)
                    .and_then(parse_date)
            })
            .min(),
        knowledge_cutoff: most_common(
            entries
                .iter()
                .filter_map(|e| e.v.get("knowledge").and_then(Value::as_str))
                .filter(|k| k.len() >= 4)
                .map(str::to_owned),
        ),
        open_weights: flags("open_weights"),
        context_window: entries
            .iter()
            .filter_map(|e| int(e.v, &["limit", "context"]))
            .max(),
        max_output: entries
            .iter()
            .filter_map(|e| int(e.v, &["limit", "output"]))
            .max(),
        input_modalities: modalities("input"),
        output_modalities: modalities("output"),
        reasoning: flags("reasoning"),
        tool_call: flags("tool_call"),
        structured_output: flags("structured_output"),
        attachment: flags("attachment"),
        deprecated: entries
            .iter()
            .all(|e| e.v.get("status").and_then(Value::as_str) == Some("deprecated")),
        providers_listing: providers.len() as i32,
    }
}

/// Folds the whole file into one [`Fact`] per canonical model.
pub fn parse(body: &str) -> Result<Vec<Fact>, AdapterError> {
    let root: Value =
        serde_json::from_str(body).map_err(|e| AdapterError::Format(ID, e.to_string()))?;
    let providers = root
        .as_object()
        .ok_or_else(|| AdapterError::Format(ID, "the file is not an object of providers".into()))?;
    let mut groups: HashMap<String, Vec<Entry<'_>>> = HashMap::new();
    for (provider, p) in providers {
        let Some(models) = p.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (id, m) in models {
            let raw = m
                .get("canonical_model_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(id)
                .to_owned();
            groups
                .entry(raw)
                .or_default()
                .push(Entry { provider, v: m });
        }
    }
    if groups.len() < 100 {
        return Err(AdapterError::Format(
            ID,
            format!(
                "only {} models in the file; its layout probably changed",
                groups.len()
            ),
        ));
    }
    let mut facts: Vec<Fact> = groups.iter().map(|(raw, es)| fold(raw, es)).collect();
    facts.sort_by(|a, b| a.raw_id.cmp(&b.raw_id));
    Ok(facts)
}

pub async fn fetch(client: &Client, origin: &str) -> Result<Vec<Fact>, AdapterError> {
    let url = format!("{}/api.json", origin.trim_end_matches('/'));
    parse(&get_text(client, ID, &url).await?)
}

/// Replaces the stored facts. Two raw ids that normalize to one market key keep the fuller one.
pub async fn store(pool: &PgPool, facts: &[Fact]) -> sqlx::Result<usize> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "CREATE TEMP TABLE facts_in (
             raw_id text, score int, name text, family text, description text, released_on date,
             knowledge_cutoff text, open_weights boolean, context_window int, max_output int,
             input_modalities text[], output_modalities text[], reasoning boolean, tool_call boolean,
             structured_output boolean, attachment boolean, deprecated boolean, providers_listing int
         ) ON COMMIT DROP",
    )
    .execute(&mut *tx)
    .await?;
    for chunk in facts.chunks(500) {
        let mut q = sqlx::QueryBuilder::new(
            "INSERT INTO facts_in (raw_id, score, name, family, description, released_on, knowledge_cutoff,
                 open_weights, context_window, max_output, input_modalities, output_modalities, reasoning,
                 tool_call, structured_output, attachment, deprecated, providers_listing) ",
        );
        q.push_values(chunk, |mut b, f| {
            b.push_bind(&f.raw_id)
                .push_bind(f.score())
                .push_bind(&f.name)
                .push_bind(&f.family)
                .push_bind(&f.description)
                .push_bind(f.released_on)
                .push_bind(&f.knowledge_cutoff)
                .push_bind(f.open_weights)
                .push_bind(f.context_window)
                .push_bind(f.max_output)
                .push_bind(&f.input_modalities)
                .push_bind(&f.output_modalities)
                .push_bind(f.reasoning)
                .push_bind(f.tool_call)
                .push_bind(f.structured_output)
                .push_bind(f.attachment)
                .push_bind(f.deprecated)
                .push_bind(f.providers_listing);
        });
        q.build().execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM model_facts WHERE source = 'models.dev'")
        .execute(&mut *tx)
        .await?;
    let stored = sqlx::query(
        "INSERT INTO model_facts (model_key, name, family, description, released_on, knowledge_cutoff,
             open_weights, context_window, max_output, input_modalities, output_modalities, reasoning,
             tool_call, structured_output, attachment, deprecated, providers_listing)
         SELECT DISTINCT ON (k) k, name, family, description, released_on, knowledge_cutoff,
                open_weights, context_window, max_output, input_modalities, output_modalities,
                reasoning, tool_call, structured_output, attachment, deprecated, providers_listing
         FROM (SELECT market_model_key(raw_id) AS k, * FROM facts_in) f
         WHERE k <> ''
         ORDER BY k, score DESC, raw_id
         ON CONFLICT (model_key) DO NOTHING",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected() as usize;
    tx.commit().await?;
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three providers list the same model; a fourth lists a model with no canonical id.
    fn file() -> String {
        let mut providers = serde_json::Map::new();
        let entry = |name: &str,
                     ctx: i64,
                     out: i64,
                     release: &str,
                     open: bool,
                     tools: bool,
                     status: Option<&str>| {
            let mut m = serde_json::json!({
                "id": "x", "name": name, "family": "gpt-oss", "canonical_model_id": "openai/gpt-oss-120b",
                "description": "OpenAI's open-weight reasoning model for agentic tasks.",
                "release_date": release, "knowledge": "2024-06", "open_weights": open, "tool_call": tools,
                "reasoning": true, "modalities": {"input": ["text"], "output": ["text"]},
                "limit": {"context": ctx, "output": out}
            });
            if let Some(s) = status {
                m["status"] = s.into();
            }
            m
        };
        let mut add = |p: &str, models: Vec<(&str, Value)>| {
            providers.insert(
                p.into(),
                serde_json::json!({"id": p, "models": models.into_iter().map(|(k, v)| (k.to_owned(), v)).collect::<serde_json::Map<_, _>>()}),
            );
        };
        add(
            "openai",
            vec![(
                "gpt-oss-120b",
                entry(
                    "GPT OSS 120B",
                    131072,
                    32768,
                    "2025-08-05",
                    true,
                    true,
                    None,
                ),
            )],
        );
        add(
            "groq",
            vec![(
                "openai/gpt-oss-120b",
                entry("GPT OSS 120B", 65536, 16384, "2025-08-06", true, true, None),
            )],
        );
        add(
            "cerebras",
            vec![(
                "gpt-oss-120b",
                entry(
                    "gpt-oss-120b",
                    131072,
                    40000,
                    "2025-08-05",
                    true,
                    false,
                    None,
                ),
            )],
        );
        // Filler so the file looks like the real one: >= 100 distinct models.
        for i in 0..110 {
            add(
                &format!("p{i}"),
                vec![(
                    "m",
                    serde_json::json!({"id": "m", "name": format!("M{i}"), "canonical_model_id": format!("lab/m-{i}"),
                       "limit": {"context": 8192}, "modalities": {"input": ["text"], "output": ["text"]}}),
                )],
            );
        }
        add(
            "solo",
            vec![(
                "only-here",
                serde_json::json!({"id": "only-here", "name": "Only Here", "status": "deprecated",
              "release_date": "2024-02", "modalities": {"input": ["text", "image"], "output": ["text"]}}),
            )],
        );
        Value::Object(providers).to_string()
    }

    #[test]
    fn folds_entries_of_one_canonical_model_into_one_set_of_facts() {
        let facts = parse(&file()).unwrap();
        let f = facts
            .iter()
            .find(|f| f.raw_id == "openai/gpt-oss-120b")
            .unwrap();
        assert_eq!(f.providers_listing, 3);
        assert_eq!(f.name, "GPT OSS 120B", "the most common name");
        assert_eq!(
            f.context_window,
            Some(131_072),
            "the largest limit any provider states"
        );
        assert_eq!(f.max_output, Some(40_000));
        assert_eq!(
            f.released_on,
            NaiveDate::from_ymd_opt(2025, 8, 5),
            "the earliest release date"
        );
        assert_eq!(f.open_weights, Some(true));
        assert_eq!(f.tool_call, Some(true), "two of three providers say so");
        assert_eq!(f.knowledge_cutoff.as_deref(), Some("2024-06"));
        assert!(f.description.as_deref().unwrap().starts_with("OpenAI's"));
        assert!(!f.deprecated);
    }

    #[test]
    fn entries_without_a_canonical_id_stand_alone_and_deprecation_needs_every_entry() {
        let facts = parse(&file()).unwrap();
        let f = facts.iter().find(|f| f.raw_id == "only-here").unwrap();
        assert!(f.deprecated);
        assert_eq!(
            f.released_on,
            NaiveDate::from_ymd_opt(2024, 2, 1),
            "a month alone is the first of it"
        );
        assert_eq!(f.input_modalities, vec!["text", "image"]);
        assert_eq!(f.open_weights, None, "not stated is not false");
    }

    #[test]
    fn a_file_with_too_few_models_is_an_error() {
        assert!(parse(r#"{"a":{"models":{"m":{"name":"M"}}}}"#).is_err());
        assert!(parse("[]").is_err());
    }
}
