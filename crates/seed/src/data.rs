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
    #[serde(default)]
    pub products: Vec<ProductSeed>,
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

fn is_http_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// Checks the whole data set: each file on its own and the references between files.
/// `files` pairs a file name (`<slug>.yaml`) with its parsed content.
pub fn validate(files: &[(String, OrgSeed)], known_categories: &HashSet<String>) -> Vec<Issue> {
    let mut issues = Vec::new();
    let org_slugs: HashSet<&str> = files.iter().map(|(_, o)| o.slug.as_str()).collect();
    let mut seen_orgs: HashMap<&str, &str> = HashMap::new();
    let mut seen_products: HashMap<&str, &str> = HashMap::new();

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
            .chain(org.logo_url.iter().map(|u| ("logo_url", u)))
            .chain(org.links.iter().map(|(k, v)| (k.as_str(), v)))
        {
            if !is_http_url(url) {
                issue(format!("{name} `{url}` must be an http(s) URL"));
            }
        }
        for category in &org.categories {
            if !known_categories.contains(category) {
                issue(format!("unknown category `{category}`"));
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
    fn slug_rules() {
        for ok in ["a", "gpt-4o", "llama-3-1-405b"] {
            assert!(is_slug(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "a--b", "A", "a_b", "a b"] {
            assert!(!is_slug(bad), "{bad}");
        }
    }
}
