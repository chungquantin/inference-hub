//! The YAML format of the curated data and the checks run before importing it.
//!
//! One file per organization lives in `data/organizations/<slug>.yaml`. It holds
//! the organization, the products it makes and, under each product, the offers
//! of the providers that sell it.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::NaiveDate;
use ih_core::{OrgKind, OrgStatus, ProductType};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrgSeed {
    pub slug: String,
    pub name: String,
    pub kinds: Vec<OrgKind>,
    pub tagline: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub logo_url: Option<String>,
    /// ISO 3166-1 alpha-2, upper case.
    pub hq_country: Option<String>,
    pub founded_year: Option<i16>,
    pub status: Option<OrgStatus>,
    #[serde(default)]
    pub links: BTreeMap<String, String>,
    /// Categories on top of the ones implied by `kinds`.
    #[serde(default)]
    pub categories: Vec<String>,
    /// How the news tagger recognizes the organization.
    #[serde(default)]
    pub tagging: TaggingSeed,
    /// The organization's own feeds, such as its blog. Their items are tagged with it.
    #[serde(default)]
    pub feeds: Vec<FeedSeed>,
    #[serde(default)]
    pub products: Vec<ProductSeed>,
    /// Funding rounds the organization raised.
    #[serde(default)]
    pub funding: Vec<FundingSeed>,
}

/// One funding round. Rounds are keyed by (round, announced_on) within an organization.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingSeed {
    /// Such as `seed`, `series_a`, `series_b`, `strategic`, `debt`.
    pub round: String,
    pub announced_on: NaiveDate,
    /// Amount raised, in `currency`, as announced.
    pub amount: Option<i64>,
    /// Post-money valuation, in `currency`.
    pub valuation: Option<i64>,
    /// ISO 4217 code of the amounts. Default USD.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Slugs of the investors that led the round.
    #[serde(default)]
    pub lead: Vec<String>,
    /// Slugs of the other investors.
    #[serde(default)]
    pub investors: Vec<String>,
    pub source_url: Option<String>,
    /// Context the investor lists cannot hold, such as investors not in the directory.
    pub notes: Option<String>,
}

/// News tagging rules. The organization's name is always a match term.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaggingSeed {
    /// Other names to match, such as "AWS".
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Phrases that contain a name but mean something else, such as "AWS Lambda".
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Match case-sensitively, for names that are also common words.
    #[serde(default)]
    pub match_case: bool,
    /// Match the name itself. False for names that are ordinary words or belong
    /// to other things too ("Jan", "Modal"): then only the aliases match.
    #[serde(default = "default_true")]
    pub match_name: bool,
}

impl Default for TaggingSeed {
    fn default() -> Self {
        Self {
            aliases: Vec::new(),
            exclude: Vec::new(),
            match_case: false,
            match_name: true,
        }
    }
}

/// A feed to ingest news from.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedSeed {
    pub slug: String,
    pub name: String,
    pub url: String,
    /// Publish items without review. Defaults to true for an organization's own feeds.
    pub auto_approve: Option<bool>,
    #[serde(default = "default_interval")]
    pub interval_minutes: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_interval() -> i32 {
    60
}

fn default_true() -> bool {
    true
}

/// `data/sources.yaml`: feeds not owned by one organization, such as trade press.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcesFile {
    #[serde(default)]
    pub feeds: Vec<FeedSeed>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductSeed {
    pub slug: String,
    pub name: String,
    #[serde(rename = "type")]
    pub product_type: ProductType,
    pub tagline: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub launched_at: Option<NaiveDate>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    /// GitHub repository as `owner/name`; the worker syncs its stars and releases.
    pub github: Option<String>,
    /// The ids market adapters use for this product, by adapter: their prices
    /// become offerings of this product. Such as `openrouter: [openai/gpt-oss-120b]`.
    #[serde(default)]
    pub listings: BTreeMap<String, Vec<String>>,
    pub model: Option<ModelSeed>,
    #[serde(default)]
    pub offerings: Vec<OfferingSeed>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSeed {
    pub family: Option<String>,
    pub parameters_b: Option<f64>,
    pub active_params_b: Option<f64>,
    pub context_window: Option<i32>,
    #[serde(default)]
    pub modalities: Vec<String>,
    pub license: Option<String>,
    #[serde(default)]
    pub open_weights: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferingSeed {
    /// Slug of the organization that sells it.
    pub provider: String,
    /// USD per million input tokens.
    pub input: Option<f64>,
    /// USD per million output tokens.
    pub output: Option<f64>,
    pub gpu_type: Option<String>,
    pub gpu_count: Option<i16>,
    /// Price per instance-hour.
    pub per_hour: Option<f64>,
    pub region: Option<String>,
    #[serde(default = "default_currency")]
    pub currency: String,
    pub source_url: Option<String>,
    /// When the price was checked.
    pub observed_on: NaiveDate,
}

fn default_currency() -> String {
    "USD".to_owned()
}

/// A problem found in the data, with the file it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub file: String,
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A path under `/logos/` with an image extension, without traversal.
pub fn is_logo_path(s: &str) -> bool {
    s.strip_prefix("/logos/").is_some_and(|name| {
        !name.is_empty()
            && !name.contains('/')
            && !name.contains("..")
            && [".svg", ".png", ".jpg", ".webp"]
                .iter()
                .any(|ext| name.ends_with(ext))
    })
}

/// `owner/name` with GitHub's allowed characters.
pub fn is_github_repo(s: &str) -> bool {
    let valid = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    matches!(s.split_once('/'), Some((owner, name)) if valid(owner) && valid(name) && !name.contains('/'))
}

fn is_round_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn is_http_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// Checks every feed: slugs are unique across the data set, URLs are http(s)
/// and intervals are at least 5 minutes.
pub fn validate_feeds(files: &[(String, OrgSeed)], sources: &SourcesFile) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut seen: HashMap<&str, &str> = HashMap::new();
    let all = files
        .iter()
        .flat_map(|(file, org)| org.feeds.iter().map(move |f| (file.as_str(), f)))
        .chain(sources.feeds.iter().map(|f| ("sources.yaml", f)));
    for (file, feed) in all {
        let mut issue = |message: String| {
            issues.push(Issue {
                file: file.to_owned(),
                message: format!("feed `{}`: {message}", feed.slug),
            })
        };
        if !is_slug(&feed.slug) {
            issue("not a valid slug".to_owned());
        }
        if let Some(first) = seen.insert(&feed.slug, file) {
            issue(format!("also defined in {first}"));
        }
        if !is_http_url(&feed.url) {
            issue(format!("url `{}` must be an http(s) URL", feed.url));
        }
        if feed.interval_minutes < 5 {
            issue("interval_minutes must be at least 5".to_owned());
        }
    }
    issues
}

/// Ids of the market adapters that listings may refer to.
pub const ADAPTERS: &[&str] = &[
    // Model APIs
    "openrouter",
    "liquid",
    "huggingface",
    "deepinfra",
    "novita",
    "chutes",
    "sambanova",
    "atlascloud",
    "inferencenet",
    "featherless",
    "venice",
    "ovhcloud",
    "scaleway",
    // Providers read from models.dev's community-maintained price database.
    "mdev-groq",
    "mdev-cerebras",
    "mdev-fireworks-ai",
    "mdev-baseten",
    "mdev-cohere",
    "mdev-nano-gpt",
    "mdev-llmgateway-providers",
    "mdev-kilo",
    "mdev-edenai",
    "mdev-aimlapi",
    "mdev-amazon-bedrock",
    "mdev-302ai",
    "mdev-aihubmix",
    "mdev-orcarouter",
    "mdev-cortecs",
    "mdev-zenmux",
    "mdev-helicone",
    "mdev-cloudflare-workers-ai",
    "mdev-opencode",
    "mdev-prime",
    "mdev-commandcode",
    "mdev-abacus",
    "mdev-nearai",
    "mdev-siliconflow",
    "mdev-jiekou",
    "mdev-standardcompute",
    "mdev-thinkingmachines",
    "mdev-fastrouter",
    "mdev-databricks",
    "mdev-ollama-cloud",
    "mdev-neuralwatt",
    "mdev-gmicloud",
    "mdev-evroc",
    "mdev-tensorx",
    "mdev-morph",
    "mdev-volcengine",
    "mdev-arcee",
    "mdev-ambient",
    "mdev-modal",
    "mdev-moark",
    "mdev-infomaniak",
    "mdev-meta",
    "mdev-stackit",
    "mdev-inceptron",
    "mdev-moonshotai",
    "mdev-unorouter",
    "mdev-friendli",
    "mdev-watsonx",
    "mdev-berget",
    "mdev-ai-router",
    "mdev-stepfun",
    "mdev-modular",
    "mdev-sakana",
    "mdev-minimax",
    "mdev-longcat",
    "mdev-wafer.ai",
    "mdev-sarvam",
    "mdev-merge-gateway",
    "mdev-tempr",
    "mdev-crossmodel",
    "mdev-impossibl",
    "mdev-empiriolabs",
    "mdev-frogbot",
    "mdev-kenari",
    "mdev-lowrouter",
    "mdev-greenpt",
    "mdev-vivgrid",
    "mdev-llmtr",
    "mdev-meganova",
    "mdev-boundless",
    "mdev-auriko",
    "mdev-byteplus",
    "mdev-tokengo",
    "mdev-aixy",
    "mdev-regolo-ai",
    "mdev-jalapeno",
    "mdev-xpersona",
    "mdev-engy",
    "mdev-freemodel",
    "mdev-privatemode-ai",
    "mdev-hpc-ai",
    "mdev-above",
    "mdev-iteracompute",
    "mdev-daoxe",
    "mdev-umans-ai",
    "mdev-melious",
    "mdev-synthetic",
    "mdev-vancine",
    "mdev-aki-io",
    "mdev-bothub",
    "mdev-munito",
    "mdev-runinfra",
    "mdev-qiniu-ai",
    "mdev-mixlayer",
    "mdev-cloudferro-sherlock",
    // Gateways, exchanges and privacy routers that compete with Liquid Inference.
    "trustedrouter",
    "minirouter",
    "wiro",
    "gatewayz",
    "plaingateway",
    "loopxxi",
    "inferall",
    "infercom",
    "ofox",
    "eurouter",
    "routerbase",
    "blockrun",
    "cometapi",
    "bitrouter",
    "saygm",
    "aigateway",
    "tinfoil",
    "opper",
    // GPU clouds read from their pricing pages.
    "cloud-civo",
    "cloud-crusoe",
    "cloud-denvr",
    "cloud-digitalocean",
    "cloud-e2e",
    "cloud-gcore",
    "cloud-hyperstack",
    "cloud-jarvislabs",
    "cloud-lightning",
    "cloud-liquidweb",
    "cloud-lyceum",
    "cloud-massed",
    "cloud-nebius",
    "cloud-northflank",
    "cloud-oblivus",
    "cloud-packet-ai",
    "cloud-paperspace",
    "cloud-salad",
    "cloud-saturn",
    "cloud-thunder",
    "cloud-together",
    // Keyless catalogs of smaller APIs and resellers.
    "inception",
    "jina",
    "nous",
    // Labs' own price pages.
    "lab-ai21",
    "lab-anthropic",
    "lab-deepseek",
    "lab-google",
    "lab-mistral",
    "lab-openai",
    "lab-perplexity",
    "lab-qwen",
    "lab-reka",
    "lab-upstage",
    "lab-voyage",
    "lab-writer",
    "lab-xai",
    "lab-zai",
    "vercel",
    "requesty",
    // GPU clouds
    "runpod",
    "hyperbolic",
    "coreweave",
    "vastai",
    "vultr",
    "linode",
    "verda",
    "akash",
    "ionet",
    "cudo",
    "shadeform",
    "oracle",
    "lambda",
    "azure",
];

/// Checks the whole data set: each file on its own and the references between files.
/// `files` pairs a file name (`<slug>.yaml`) with its parsed content.
pub fn validate(files: &[(String, OrgSeed)], known_categories: &HashSet<String>) -> Vec<Issue> {
    let mut issues = Vec::new();
    let org_slugs: HashSet<&str> = files.iter().map(|(_, o)| o.slug.as_str()).collect();
    let mut seen_orgs: HashMap<&str, &str> = HashMap::new();
    let mut seen_products: HashMap<&str, &str> = HashMap::new();
    let mut seen_repos: HashMap<String, String> = HashMap::new();
    let mut seen_listings: HashMap<(String, String), String> = HashMap::new();

    for (file, org) in files {
        let mut issue = |message: String| {
            issues.push(Issue {
                file: file.clone(),
                message,
            })
        };

        if !is_slug(&org.slug) {
            issue(format!(
                "organization slug `{}` is not a valid slug",
                org.slug
            ));
        }
        if file.strip_suffix(".yaml") != Some(org.slug.as_str()) {
            issue(format!("file name must be `{}.yaml`", org.slug));
        }
        if let Some(first) = seen_orgs.insert(&org.slug, file) {
            issue(format!(
                "organization `{}` is also defined in {first}",
                org.slug
            ));
        }
        if org.name.trim().is_empty() {
            issue("name is empty".to_owned());
        }
        if org.kinds.is_empty() {
            issue("kinds must list at least one kind".to_owned());
        }
        if let Some(country) = &org.hq_country
            && !(country.len() == 2 && country.bytes().all(|b| b.is_ascii_uppercase()))
        {
            issue(format!(
                "hq_country `{country}` must be two upper-case letters"
            ));
        }
        for (name, url) in org
            .website
            .iter()
            .map(|u| ("website", u))
            .chain(org.links.iter().map(|(k, v)| (k.as_str(), v)))
        {
            if !is_http_url(url) {
                issue(format!("{name} `{url}` must be an http(s) URL"));
            }
        }
        // Logos are served by the web app from `apps/web/public/logos/`.
        if let Some(logo) = &org.logo_url
            && !is_logo_path(logo)
        {
            issue(format!(
                "logo_url `{logo}` must be a file under /logos/, such as /logos/{}.svg",
                org.slug
            ));
        }
        for category in &org.categories {
            if !known_categories.contains(category) {
                issue(format!("unknown category `{category}`"));
            }
        }
        let mut seen_rounds = HashSet::new();
        for round in &org.funding {
            let at = format!("funding `{}` on {}", round.round, round.announced_on);
            if !is_round_name(&round.round) {
                issue(format!(
                    "{at}: round must be lower_snake_case, such as `series_a`"
                ));
            }
            if !seen_rounds.insert((round.round.as_str(), round.announced_on)) {
                issue(format!("{at}: listed twice"));
            }
            if !(round.currency.len() == 3
                && round.currency.bytes().all(|b| b.is_ascii_uppercase()))
            {
                issue(format!(
                    "{at}: currency must be an ISO 4217 code such as USD"
                ));
            }
            for (name, value) in [("amount", round.amount), ("valuation", round.valuation)] {
                if value.is_some_and(|v| v <= 0) {
                    issue(format!("{at}: {name} must be positive"));
                }
            }
            let mut investors = HashSet::new();
            for investor in round.lead.iter().chain(&round.investors) {
                if !org_slugs.contains(investor.as_str()) {
                    issue(format!(
                        "{at}: investor `{investor}` is not a known organization"
                    ));
                }
                if investor == &org.slug {
                    issue(format!("{at}: an organization cannot invest in itself"));
                }
                if !investors.insert(investor.as_str()) {
                    issue(format!("{at}: investor `{investor}` is listed twice"));
                }
            }
            if let Some(url) = &round.source_url
                && !is_http_url(url)
            {
                issue(format!("{at}: source_url `{url}` must be an http(s) URL"));
            }
        }
        for term in org.tagging.aliases.iter().chain(&org.tagging.exclude) {
            if term.trim().len() < 2 {
                issue(format!("tagging term `{term}` is too short"));
            }
        }

        for product in &org.products {
            let at = format!("product `{}`", product.slug);
            if !is_slug(&product.slug) {
                issue(format!("{at}: not a valid slug"));
            }
            if let Some(first) = seen_products.insert(&product.slug, file) {
                issue(format!("{at}: also defined in {first}"));
            }
            if let Some(repo) = &product.github
                && !is_github_repo(repo)
            {
                issue(format!("{at}: github `{repo}` must be `owner/name`"));
            }
            if let Some(repo) = &product.github
                && let Some(first) =
                    seen_repos.insert(repo.to_ascii_lowercase(), product.slug.clone())
            {
                issue(format!(
                    "{at}: github `{repo}` is also used by product `{first}`"
                ));
            }
            for (adapter, ids) in &product.listings {
                if !ADAPTERS.contains(&adapter.as_str()) {
                    issue(format!("{at}: unknown adapter `{adapter}` in listings"));
                }
                for id in ids {
                    if id.trim().is_empty() {
                        issue(format!("{at}: empty {adapter} listing id"));
                    } else if let Some(first) =
                        seen_listings.insert((adapter.clone(), id.clone()), product.slug.clone())
                    {
                        issue(format!(
                            "{at}: {adapter} listing `{id}` is also mapped to product `{first}`"
                        ));
                    }
                }
            }
            if product.model.is_some() && product.product_type != ProductType::Model {
                issue(format!(
                    "{at}: `model` is only allowed when type is `model`"
                ));
            }
            if let Some(url) = &product.url
                && !is_http_url(url)
            {
                issue(format!("{at}: url `{url}` must be an http(s) URL"));
            }
            if let Some(model) = &product.model {
                if let (Some(total), Some(active)) = (model.parameters_b, model.active_params_b)
                    && active > total
                {
                    issue(format!("{at}: active_params_b is larger than parameters_b"));
                }
                if model.context_window.is_some_and(|c| c <= 0) {
                    issue(format!("{at}: context_window must be positive"));
                }
            }
            for offering in &product.offerings {
                let at = format!("{at}, offering by `{}`", offering.provider);
                if !org_slugs.contains(offering.provider.as_str()) {
                    issue(format!("{at}: provider is not a known organization"));
                }
                let token_priced = offering.input.is_some() || offering.output.is_some();
                let time_priced = offering.per_hour.is_some();
                if token_priced == time_priced {
                    issue(format!(
                        "{at}: give either token prices (input/output) or per_hour, not both or neither"
                    ));
                }
                let prices = [offering.input, offering.output, offering.per_hour];
                if prices.iter().flatten().any(|p| !p.is_finite() || *p < 0.0) {
                    issue(format!("{at}: prices must be zero or positive"));
                }
                if offering.currency.len() != 3 {
                    issue(format!("{at}: currency must be an ISO 4217 code"));
                }
            }
        }
    }
    issues
}

/// The categories an organization belongs to: those implied by its kinds plus the explicit ones.
pub fn categories_for(org: &OrgSeed) -> Vec<String> {
    let mut slugs: Vec<String> = org
        .kinds
        .iter()
        .filter_map(|k| k.category_slug())
        .map(str::to_owned)
        .chain(org.categories.iter().cloned())
        .collect();
    slugs.sort();
    slugs.dedup();
    slugs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> OrgSeed {
        serde_saphyr::from_str(yaml).expect("valid seed YAML")
    }

    fn categories() -> HashSet<String> {
        ["model-providers", "inference-providers", "gpu-clouds"]
            .map(str::to_owned)
            .into()
    }

    const LAB: &str = r#"
slug: lab
name: Lab
kinds: [model_provider]
hq_country: US
products:
  - slug: lab-1
    name: Lab 1
    type: model
    launched_at: 2025-01-02
    model:
      parameters_b: 70
      context_window: 131072
      open_weights: true
    offerings:
      - provider: cloud
        input: 0.5
        output: 1.5
        observed_on: 2026-11-01
"#;

    const CLOUD: &str = r#"
slug: cloud
name: Cloud
kinds: [inference_provider, gpu_cloud]
"#;

    #[test]
    fn valid_data_has_no_issues() {
        let files = vec![
            ("lab.yaml".to_owned(), parse(LAB)),
            ("cloud.yaml".to_owned(), parse(CLOUD)),
        ];
        assert_eq!(validate(&files, &categories()), vec![]);
        let product = &files[0].1.products[0];
        assert_eq!(product.launched_at, NaiveDate::from_ymd_opt(2025, 1, 2));
        assert_eq!(product.offerings[0].currency, "USD");
    }

    #[test]
    fn reports_unknown_provider_bad_slug_and_wrong_file_name() {
        let mut lab = parse(LAB);
        lab.slug = "Lab!".to_owned();
        let files = vec![("lab.yaml".to_owned(), lab)];
        let messages: Vec<String> = validate(&files, &categories())
            .into_iter()
            .map(|i| i.message)
            .collect();
        assert!(messages.iter().any(|m| m.contains("not a valid slug")));
        assert!(messages.iter().any(|m| m.contains("file name must be")));
        assert!(
            messages
                .iter()
                .any(|m| m.contains("provider is not a known organization"))
        );
    }

    #[test]
    fn rejects_mixed_pricing_and_duplicates() {
        let mut lab = parse(LAB);
        lab.products[0].offerings[0].per_hour = Some(2.0);
        let dup = lab.clone();
        let files = vec![
            ("lab.yaml".to_owned(), lab),
            ("lab.yaml".to_owned(), dup),
            ("cloud.yaml".to_owned(), parse(CLOUD)),
        ];
        let messages: Vec<String> = validate(&files, &categories())
            .into_iter()
            .map(|i| i.message)
            .collect();
        assert!(messages.iter().any(|m| m.contains("not both or neither")));
        assert!(
            messages
                .iter()
                .any(|m| m.contains("organization `lab` is also defined"))
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("product `lab-1`: also defined"))
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err =
            serde_saphyr::from_str::<OrgSeed>("slug: a\nname: A\nkinds: []\nwebsite_url: x\n");
        assert!(err.is_err());
    }

    #[test]
    fn categories_come_from_kinds_and_extras() {
        let mut cloud = parse(CLOUD);
        cloud.categories.push("gpu-clouds".to_owned());
        cloud.categories.push("model-providers".to_owned());
        assert_eq!(
            categories_for(&cloud),
            ["gpu-clouds", "inference-providers", "model-providers"]
        );
    }

    #[test]
    fn parses_tagging_and_feeds_with_defaults() {
        let org = parse(
            r#"
slug: lab
name: Lab
kinds: [model_provider]
tagging: { aliases: [Lab AI], exclude: [lab coat], match_case: true }
feeds:
  - { slug: lab-blog, name: Lab Blog, url: "https://lab.example/feed.xml" }
"#,
        );
        assert_eq!(org.tagging.aliases, ["Lab AI"]);
        assert!(org.tagging.match_case);
        let feed = &org.feeds[0];
        assert_eq!(feed.interval_minutes, 60);
        assert!(feed.enabled);
        assert_eq!(feed.auto_approve, None);
    }

    #[test]
    fn feeds_must_be_unique_and_valid() {
        let mut lab = parse(LAB);
        lab.feeds.push(FeedSeed {
            slug: "dup".into(),
            name: "A".into(),
            url: "feed.xml".into(),
            auto_approve: None,
            interval_minutes: 1,
            enabled: true,
        });
        let sources = SourcesFile {
            feeds: vec![lab.feeds[0].clone()],
        };
        let messages: Vec<String> = validate_feeds(&[("lab.yaml".into(), lab)], &sources)
            .into_iter()
            .map(|i| i.to_string())
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m.contains("must be an http(s) URL"))
        );
        assert!(messages.iter().any(|m| m.contains("at least 5")));
        assert!(
            messages
                .iter()
                .any(|m| m.starts_with("sources.yaml: feed `dup`: also defined in lab.yaml"))
        );
    }

    #[test]
    fn validates_funding_rounds_and_repos() {
        let mut lab = parse(LAB);
        lab.products[0].github = Some("org/repo".into());
        lab.funding = serde_saphyr::from_str(
            r#"
- { round: series_a, announced_on: 2024-01-01, amount: 100, lead: [cloud] }
- { round: Series B, announced_on: 2024-06-01, amount: 0, currency: eur, investors: [nobody, lab, cloud, cloud] }
- { round: series_a, announced_on: 2024-01-01 }
"#,
        )
        .unwrap();
        let mut cloud = parse(CLOUD);
        cloud.products.push(ProductSeed {
            github: Some("ORG/repo".into()),
            ..lab.products[0].clone()
        });
        cloud.products[0].slug = "cloud-copy".into();
        cloud.products[0].offerings.clear();
        let files = vec![
            ("lab.yaml".to_owned(), lab),
            ("cloud.yaml".to_owned(), cloud),
        ];
        let messages: Vec<String> = validate(&files, &categories())
            .into_iter()
            .map(|i| i.message)
            .collect();
        let has = |needle: &str| messages.iter().any(|m| m.contains(needle));
        assert!(has("round must be lower_snake_case"), "{messages:#?}");
        assert!(has("amount must be positive"));
        assert!(has("currency must be an ISO 4217 code"));
        assert!(has("investor `nobody` is not a known organization"));
        assert!(has("cannot invest in itself"));
        assert!(has("investor `cloud` is listed twice"));
        assert!(has("funding `series_a` on 2024-01-01: listed twice"));
        assert!(has("github `ORG/repo` is also used by product `lab-1`"));
        assert!(!has("funding `series_a` on 2024-01-01: investor"));
    }

    #[test]
    fn listing_ids_must_name_a_known_adapter_and_map_once() {
        let mut lab = parse(LAB);
        lab.products[0].listings = [
            (
                "openrouter".to_owned(),
                vec!["lab/one".to_owned(), "lab/one".to_owned()],
            ),
            ("bogus".to_owned(), vec!["x".to_owned()]),
        ]
        .into();
        let files = vec![
            ("lab.yaml".to_owned(), lab),
            ("cloud.yaml".to_owned(), parse(CLOUD)),
        ];
        let messages: Vec<String> = validate(&files, &categories())
            .into_iter()
            .map(|i| i.message)
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m.contains("unknown adapter `bogus`")),
            "{messages:#?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m.contains("listing `lab/one` is also mapped"))
        );
    }

    #[test]
    fn logo_paths_stay_in_the_logos_folder() {
        for ok in ["/logos/openai.svg", "/logos/a16z.png"] {
            assert!(is_logo_path(ok), "{ok}");
        }
        for bad in [
            "https://x.com/logo.png",
            "/logos/../secret.svg",
            "/logos/a/b.svg",
            "/logos/x.gif",
            "/img/x.svg",
        ] {
            assert!(!is_logo_path(bad), "{bad}");
        }
    }

    #[test]
    fn github_repo_rules() {
        for ok in ["vllm-project/vllm", "ggml-org/llama.cpp", "a/b_c"] {
            assert!(is_github_repo(ok), "{ok}");
        }
        for bad in [
            "vllm",
            "a/b/c",
            "/b",
            "a/",
            "a/..",
            "a b/c",
            "https://github.com/a/b",
        ] {
            assert!(!is_github_repo(bad), "{bad}");
        }
    }

    #[test]
    fn slug_rules() {
        for ok in ["a", "gpt-4o", "llama-3-1-405b"] {
            assert!(is_slug(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "a--b", "A", "a_b", "a b"] {
            assert!(!is_slug(bad), "{bad}");
        }
    }
}
