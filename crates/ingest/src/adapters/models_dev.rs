//! Providers that models.dev lists with prices, as one adapter each.
//!
//! models.dev (github.com/anomalyco/models.dev, MIT) is a community-maintained database: for each
//! provider it lists the models and their per-million-token prices. These providers publish no
//! keyless catalog of their own, so their prices come from there. Every adapter of this family reads
//! the same `api.json`, downloaded once and shared for a while, and keeps only its own provider's
//! entries. The data is copied by hand by the community, so each listing says so (`extra.via`) and
//! the adapter's name ends in "via models.dev".
//!
//! Only the providers below are read: each was matched to an organization in the directory by hand,
//! and none has a first-hand adapter of its own.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::Client;
use serde_json::Value;
use tokio::sync::Mutex;

use super::{
    Adapter, AdapterError, BoxFuture, DAILY, Endpoints, GatewayAdapter, MediaKind, ModelListing,
    caps, get_text, round6,
};

/// models.dev provider id, adapter id, adapter name, directory organization slug.
pub struct Provider {
    pub provider: &'static str,
    pub id: &'static str,
    pub name: &'static str,
    pub organization: &'static str,
}

macro_rules! providers {
    ($(($p:expr, $name:expr, $org:expr)),* $(,)?) => {
        &[$(Provider { provider: $p, id: concat!("mdev-", $p), name: concat!($name, " via models.dev"), organization: $org }),*]
    };
}

pub static PROVIDERS: &[Provider] = providers![
    ("groq", "Groq", "groq"),
    ("cerebras", "Cerebras", "cerebras"),
    ("fireworks-ai", "Fireworks AI", "fireworks-ai"),
    ("baseten", "Baseten", "baseten"),
    ("cohere", "Cohere", "cohere"),
    ("nano-gpt", "NanoGPT", "nano-gpt"),
    ("llmgateway-providers", "LLM Gateway", "llm-gateway"),
    ("kilo", "Kilo Gateway", "kilo-code"),
    ("edenai", "Eden AI", "eden-ai"),
    ("aimlapi", "AI/ML API", "aimlapi"),
    ("amazon-bedrock", "Amazon Bedrock", "aws"),
    ("302ai", "302.AI", "302-ai"),
    ("aihubmix", "AIHubMix", "aihubmix"),
    ("orcarouter", "OrcaRouter", "orcarouter"),
    ("cortecs", "Cortecs", "cortecs"),
    ("zenmux", "ZenMux", "zenmux"),
    ("helicone", "Helicone", "helicone"),
    (
        "cloudflare-workers-ai",
        "Cloudflare Workers AI",
        "cloudflare"
    ),
    ("opencode", "OpenCode Zen", "opencode"),
    ("prime", "Prime Inference", "prime-intellect"),
    ("commandcode", "CommandCode", "command-code"),
    ("abacus", "Abacus", "abacus-ai"),
    ("nearai", "NEAR AI Cloud", "near-ai"),
    ("siliconflow", "SiliconFlow", "siliconflow"),
    ("jiekou", "Jiekou.AI", "jiekou-ai"),
    ("standardcompute", "Standard Compute", "standard-compute"),
    ("thinkingmachines", "Thinking Machines", "thinking-machines"),
    ("fastrouter", "FastRouter", "fastrouter"),
    ("databricks", "Databricks", "databricks"),
    ("ollama-cloud", "Ollama Cloud", "ollama"),
    ("neuralwatt", "Neuralwatt", "neuralwatt"),
    ("gmicloud", "GMI Cloud", "gmi-cloud"),
    ("evroc", "evroc", "evroc"),
    ("tensorx", "TensorX", "tensorx"),
    ("morph", "Morph", "morph"),
    ("volcengine", "Volcengine Ark", "volcengine"),
    ("arcee", "Arcee", "arcee-ai"),
    ("ambient", "Ambient", "ambient"),
    ("modal", "Modal", "modal"),
    ("moark", "Moark", "gitee-ai"),
    ("infomaniak", "Infomaniak", "infomaniak"),
    ("meta", "Meta Llama API", "meta"),
    ("stackit", "STACKIT", "stackit"),
    ("inceptron", "Inceptron", "inceptron"),
    ("moonshotai", "Moonshot AI", "moonshot-ai"),
    ("unorouter", "UnoRouter", "unorouter"),
    ("friendli", "Friendli", "friendli"),
    ("watsonx", "watsonx.ai", "ibm"),
    ("berget", "Berget.AI", "berget-ai"),
    ("ai-router", "AI-ROUTER", "ai-router"),
    ("stepfun", "StepFun", "stepfun"),
    ("modular", "Modular", "modular"),
    ("sakana", "Sakana AI", "sakana"),
    ("minimax", "MiniMax", "minimax"),
    ("longcat", "LongCat", "meituan-longcat"),
    ("wafer.ai", "Wafer", "wafer"),
    ("sarvam", "Sarvam AI", "sarvam-ai"),
    ("merge-gateway", "Merge Gateway", "merge-gateway"),
    ("tempr", "Tempr Gateway", "tempr-gateway"),
    ("crossmodel", "CrossModel", "crossmodel"),
    ("impossibl", "Impossibl", "impossibl"),
    ("empiriolabs", "EmpirioLabs AI", "empiriolabs-ai"),
    ("frogbot", "FrogBot", "frogbot"),
    ("kenari", "Kenari", "kenari"),
    ("lowrouter", "LowRouter", "lowrouter"),
    ("greenpt", "GreenPT", "greenpt"),
    ("vivgrid", "Vivgrid", "vivgrid"),
    ("llmtr", "LLMTR", "llmtr"),
    ("meganova", "Meganova", "meganova"),
    ("boundless", "Boundless", "boundless"),
    ("auriko", "Auriko", "auriko"),
    ("byteplus", "BytePlus ModelArk", "byteplus-modelark"),
    ("tokengo", "TokenGo", "tokengo"),
    ("aixy", "Aixy", "aixy"),
    ("regolo-ai", "Regolo AI", "regolo-ai"),
    ("jalapeno", "Jalapeno Cloud", "jalapeno-cloud"),
    ("xpersona", "Xpersona", "xpersona"),
    ("engy", "engy", "engy"),
    ("freemodel", "FreeModel", "freemodel"),
    ("privatemode-ai", "Privatemode AI", "privatemode-ai"),
    ("hpc-ai", "HPC-AI", "hpc-ai"),
    ("above", "above.dev", "above-dev"),
    ("iteracompute", "IteraCompute", "iteracompute"),
    ("daoxe", "DaoXE", "daoxe"),
    ("umans-ai", "Umans AI", "umans-ai"),
    ("melious", "Melious", "melious"),
    ("synthetic", "Synthetic", "synthetic"),
    ("vancine", "Vancine", "vancine"),
    ("aki-io", "AKI.IO", "aki-io"),
    ("bothub", "Bothub", "bothub"),
    ("munito", "Munito AI", "munito-ai"),
    ("runinfra", "RunInfra", "runinfra"),
    ("qiniu-ai", "Qiniu", "qiniu"),
    ("mixlayer", "Mixlayer", "mixlayer"),
    (
        "cloudferro-sherlock",
        "CloudFerro Sherlock",
        "cloudferro-sherlock"
    ),
];

/// How long a downloaded `api.json` is reused: the adapters of one scheduler pass share one download.
const SHARE_FOR: Duration = Duration::from_secs(15 * 60);

static SHARED: Mutex<Option<(Instant, Arc<Value>)>> = Mutex::const_new(None);

async fn api_json(
    client: &Client,
    adapter: &'static str,
    origin: &str,
) -> Result<Arc<Value>, AdapterError> {
    let mut slot = SHARED.lock().await;
    if let Some((at, doc)) = slot.as_ref()
        && at.elapsed() < SHARE_FOR
    {
        return Ok(doc.clone());
    }
    let body = get_text(client, adapter, &format!("{origin}/api.json")).await?;
    let doc: Value = serde_json::from_str(&body)
        .map_err(|e| AdapterError::Format(adapter, format!("api.json: {e}")))?;
    let doc = Arc::new(doc);
    *slot = Some((Instant::now(), doc.clone()));
    Ok(doc)
}

pub struct ModelsDevAdapter {
    spec: &'static Provider,
    origin: String,
}

impl ModelsDevAdapter {
    pub fn new(spec: &'static Provider, endpoints: &Endpoints) -> Self {
        Self {
            spec,
            origin: endpoints.origin("https://models.dev"),
        }
    }
}

impl Adapter for ModelsDevAdapter {
    fn id(&self) -> &'static str {
        self.spec.id
    }
    fn name(&self) -> &'static str {
        self.spec.name
    }
    fn organization_slug(&self) -> &'static str {
        self.spec.organization
    }
    fn interval(&self) -> Duration {
        DAILY
    }
}

impl GatewayAdapter for ModelsDevAdapter {
    fn fetch_models<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<ModelListing>, AdapterError>> {
        Box::pin(async move {
            let doc = api_json(client, self.spec.id, &self.origin).await?;
            let listings = parse(&doc, self.spec.provider)
                .map_err(|e| AdapterError::Format(self.spec.id, e))?;
            if listings.is_empty() {
                return Err(AdapterError::Format(
                    self.spec.id,
                    "the provider has no priced models in models.dev".into(),
                ));
            }
            Ok(listings)
        })
    }
}

/// The maker prefix the market uses, where models.dev names the lab differently.
fn maker(canonical: &str) -> Option<String> {
    let lab = canonical.split_once('/')?.0.to_ascii_lowercase();
    Some(
        match lab.as_str() {
            "alibaba" => "qwen",
            "zhipuai" => "z-ai",
            "xai" => "x-ai",
            "meta" => "meta-llama",
            "mistral" => "mistralai",
            other => other,
        }
        .to_owned(),
    )
}

fn price(v: &Value, k: &str) -> Option<f64> {
    v.get("cost")?
        .get(k)?
        .as_f64()
        .filter(|p| p.is_finite() && *p >= 0.0)
}

fn strs(v: &Value, path: [&str; 2]) -> Vec<String> {
    v.get(path[0])
        .and_then(|m| m.get(path[1]))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// One provider's priced chat and embedding models. Deprecated models, free ones and models that
/// write something other than text (image, audio, video) are left out.
pub fn parse(doc: &Value, provider: &str) -> Result<Vec<ModelListing>, String> {
    let p = doc
        .get(provider)
        .ok_or_else(|| format!("provider `{provider}` is not in models.dev"))?;
    let models = p
        .get("models")
        .and_then(Value::as_object)
        .ok_or("the provider has no model list")?;
    let page = p
        .get("doc")
        .and_then(Value::as_str)
        .unwrap_or("https://models.dev");
    let mut out = Vec::new();
    for (id, m) in models {
        if m.get("status").and_then(Value::as_str) == Some("deprecated") {
            continue;
        }
        let name = m.get("name").and_then(Value::as_str).map(str::to_owned);
        let canonical = m.get("canonical_model_id").and_then(Value::as_str);
        let maker = canonical.and_then(maker).or_else(|| super::maker_of(id));
        let source = Some(page.to_owned());
        let extra = serde_json::json!({
            "via": "models.dev",
            "cache_read_per_mtok": m.get("cost").and_then(|c| c.get("cache_read")),
            "cache_write_per_mtok": m.get("cost").and_then(|c| c.get("cache_write")),
        });
        if m.get("type").and_then(Value::as_str) == Some("embedding") {
            let Some(input) = price(m, "input").filter(|p| *p > 0.0) else {
                continue;
            };
            let mut l = ModelListing::media(
                id.clone(),
                name.unwrap_or_else(|| id.clone()),
                maker,
                MediaKind::Embedding,
                round6(input),
                source,
                extra,
            );
            l.context_window = m
                .pointer("/limit/context")
                .and_then(Value::as_i64)
                .and_then(|c| i32::try_from(c).ok());
            out.push(l);
            continue;
        }
        let outputs = strs(m, ["modalities", "output"]);
        if !outputs.is_empty() && !outputs.iter().any(|o| o == "text") {
            continue;
        }
        let (Some(input), Some(output)) = (price(m, "input"), price(m, "output")) else {
            continue;
        };
        if input <= 0.0 || output <= 0.0 {
            continue; // free: not a price
        }
        let mut c: Vec<String> = Vec::new();
        for (flag, cap) in [
            ("reasoning", "reasoning"),
            ("tool_call", "tools"),
            ("structured_output", "json"),
        ] {
            if m.get(flag).and_then(Value::as_bool) == Some(true) {
                c.push(cap.to_owned());
            }
        }
        for (dir, label) in [("input", "in"), ("output", "out")] {
            for modality in strs(m, ["modalities", dir]) {
                if let Some(x) = caps::modality(label, &modality) {
                    c.push(x);
                }
            }
        }
        out.push(ModelListing {
            external_id: id.clone(),
            name: name.unwrap_or_else(|| id.clone()),
            maker,
            context_window: m
                .pointer("/limit/context")
                .and_then(Value::as_i64)
                .and_then(|c| i32::try_from(c).ok()),
            input_per_mtok: round6(input),
            output_per_mtok: round6(output),
            variant: None,
            source_url: source,
            extra,
            media: None,
            capabilities: caps::normalize(c),
        });
    }
    out.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Value {
        serde_json::json!({
          "groq": {"doc": "https://console.groq.com/docs/models", "models": {
            "openai/gpt-oss-120b": {"name": "GPT OSS 120B", "canonical_model_id": "openai/gpt-oss-120b",
               "reasoning": true, "tool_call": true, "structured_output": true,
               "modalities": {"input": ["text"], "output": ["text"]},
               "limit": {"context": 131072, "output": 65536},
               "cost": {"input": 0.15, "output": 0.6, "cache_read": 0.075}},
            "llama-3.3-70b-versatile": {"name": "Llama 3.3 70B", "canonical_model_id": "meta/llama-3.3-70b-instruct",
               "modalities": {"input": ["text"], "output": ["text"]}, "limit": {"context": 131072},
               "cost": {"input": 0.59, "output": 0.79}},
            "whisper-large-v3": {"name": "Whisper", "modalities": {"input": ["audio"], "output": ["text"]},
               "cost": {"input": 0.111, "output": 0}},
            "playai-tts": {"name": "TTS", "modalities": {"input": ["text"], "output": ["audio"]},
               "cost": {"input": 50, "output": 50}},
            "old-model": {"name": "Old", "status": "deprecated", "modalities": {"input": ["text"], "output": ["text"]},
               "cost": {"input": 1, "output": 1}},
            "free-model": {"name": "Free", "modalities": {"input": ["text"], "output": ["text"]}, "cost": {"input": 0, "output": 0}},
            "embed-1": {"name": "Embed", "type": "embedding", "limit": {"context": 8192}, "cost": {"input": 0.02}}
          }}
        })
    }

    #[test]
    fn reads_one_providers_priced_chat_and_embedding_models() {
        let l = parse(&doc(), "groq").unwrap();
        let ids: Vec<_> = l.iter().map(|m| m.external_id.as_str()).collect();
        assert_eq!(
            ids,
            ["embed-1", "llama-3.3-70b-versatile", "openai/gpt-oss-120b"],
            "tts, deprecated, free and the audio-in model are skipped"
        );
        let gpt = l
            .iter()
            .find(|m| m.external_id == "openai/gpt-oss-120b")
            .unwrap();
        assert_eq!((gpt.input_per_mtok, gpt.output_per_mtok), (0.15, 0.6));
        assert_eq!(gpt.maker.as_deref(), Some("openai"));
        assert_eq!(gpt.context_window, Some(131_072));
        assert_eq!(gpt.extra["via"], "models.dev");
        assert!(
            gpt.capabilities.contains(&"reasoning".to_owned())
                && gpt.capabilities.contains(&"tools".to_owned())
        );
        assert_eq!(
            gpt.source_url.as_deref(),
            Some("https://console.groq.com/docs/models")
        );
        let llama = l
            .iter()
            .find(|m| m.external_id.starts_with("llama"))
            .unwrap();
        assert_eq!(
            llama.maker.as_deref(),
            Some("meta-llama"),
            "the market's name for the lab"
        );
        let e = l.iter().find(|m| m.external_id == "embed-1").unwrap();
        assert_eq!(e.media.unwrap().kind, MediaKind::Embedding);
        assert_eq!(e.media.unwrap().price, 0.02);
    }

    #[test]
    fn a_provider_that_is_missing_or_empty_is_an_error() {
        assert!(parse(&doc(), "nope").is_err());
        assert!(parse(&serde_json::json!({"x": {"id": "x"}}), "x").is_err());
    }

    #[test]
    fn provider_ids_and_adapter_ids_are_unique() {
        let mut p: Vec<_> = PROVIDERS.iter().map(|p| p.provider).collect();
        p.sort();
        p.dedup();
        assert_eq!(p.len(), PROVIDERS.len());
        assert!(
            PROVIDERS
                .iter()
                .all(|p| p.id == format!("mdev-{}", p.provider))
        );
    }
}
