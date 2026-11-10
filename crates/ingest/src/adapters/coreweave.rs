//! CoreWeave: GPU instances sold per node. It has no public price API, so this
//! adapter reads the pricing page. The page repeats each price card for every
//! region tab; the first card of each GPU is the default (US) price.
//!
//! Scraping is fragile by nature: when the page no longer yields the GPUs it
//! used to, the run fails instead of reporting that CoreWeave stopped selling them.

use std::collections::BTreeMap;

use reqwest::Client;

use super::{Adapter, AdapterError, BoxFuture, GpuListing, NeoCloudAdapter, get_text, gpu, round6};

const ID: &str = "coreweave";
/// A real page lists about a dozen GPUs. Far fewer means the layout changed.
const MIN_EXPECTED: usize = 4;

pub struct CoreWeaveAdapter {
    base_url: String,
}

impl CoreWeaveAdapter {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }
}

impl Adapter for CoreWeaveAdapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn name(&self) -> &'static str {
        "CoreWeave"
    }
    fn organization_slug(&self) -> &'static str {
        "coreweave"
    }
}

impl NeoCloudAdapter for CoreWeaveAdapter {
    fn fetch_gpus<'a>(
        &'a self,
        client: &'a Client,
    ) -> BoxFuture<'a, Result<Vec<GpuListing>, AdapterError>> {
        Box::pin(async move {
            let url = format!("{}/pricing", self.base_url);
            let body = get_text(client, ID, &url).await?;
            parse(&body)
        })
    }
}

/// The visible text of the page, one non-empty line per text node.
fn text_lines(html: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut text = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let after = &rest[start..];
        // Skip whole <script> and <style> elements.
        let skip_to = ["script", "style"].iter().find_map(|tag| {
            let opening = format!("<{tag}");
            after
                .get(..opening.len())
                .filter(|p| p.eq_ignore_ascii_case(&opening))
                .and_then(|_| after.find(&format!("</{tag}>")).map(|e| e + tag.len() + 3))
        });
        let end = skip_to
            .or_else(|| after.find('>').map(|e| e + 1))
            .unwrap_or(after.len());
        lines.push(std::mem::take(&mut text));
        rest = &after[end..];
    }
    lines.push(rest.to_owned());
    lines
        .into_iter()
        .map(|l| crate::feed::clean_text(&l))
        .filter(|l| !l.is_empty())
        .collect()
}

fn dollars(s: &str) -> Option<f64> {
    s.trim()
        .strip_prefix('$')?
        .replace(',', "")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite() && *v > 0.0)
}

pub(crate) fn parse(html: &str) -> Result<Vec<GpuListing>, AdapterError> {
    let lines = text_lines(html);
    // A card reads: <name>, <GPU count>, <VRAM>, <vCPUs>, <RAM>, <storage>,
    // <price or "Contact sales">, <name>, "On-Demand Price:", ..., "Spot Price:", <spot>.
    let mut cards: BTreeMap<String, GpuListing> = BTreeMap::new();
    let mut seen = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if line != "On-Demand Price:" || i < 8 {
            continue;
        }
        seen += 1;
        let name = &lines[i - 1];
        if &lines[i - 8] != name {
            continue; // not a card in the expected layout
        }
        let count: i16 = match lines[i - 7].parse() {
            Ok(c) if c > 0 => c,
            _ => continue, // rack-scale systems such as `4^1`
        };
        let Some(price) = dollars(&lines[i + 1]) else {
            continue; // "Contact sales"
        };
        let Some(gpu_type) = gpu::normalize(name) else {
            continue;
        };
        let spot = lines
            .iter()
            .skip(i)
            .take(8)
            .position(|l| l == "Spot Price:")
            .and_then(|p| lines.get(i + p + 1))
            .and_then(|s| dollars(s));
        // First card of each GPU is the default region; later ones repeat it.
        cards.entry(name.clone()).or_insert_with(|| GpuListing {
            external_id: name.clone(),
            name: name.clone(),
            gpu_type,
            gpu_count: count,
            price_per_hour: round6(price),
            region: None,
            source_url: Some("https://www.coreweave.com/pricing".into()),
            extra: serde_json::json!({ "spot_price_per_hour": spot, "vram_gb": lines[i - 6] }),
        });
    }
    if seen < MIN_EXPECTED {
        return Err(AdapterError::Format(
            ID,
            format!(
                "found {seen} price cards, expected at least {MIN_EXPECTED}: the page layout changed"
            ),
        ));
    }
    Ok(cards.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, count: &str, price: &str, spot: &str) -> String {
        format!(
            "<div><h3>{name}</h3><span>{count}</span><span>80</span><span>128</span><span>2,048</span>\
             <span>61.44</span><span>{price}</span></div><div><p>{name}</p><p>On-Demand Price:</p>\
             <p>{price}</p><p>/ Hour</p><p>Spot Price:</p><p>{spot}</p><p>/ Hour</p></div>"
        )
    }

    #[test]
    fn reads_the_first_card_of_each_gpu() {
        let html = format!(
            "<html><script>var x = '<p>On-Demand Price:</p>';</script>{}{}{}{}{}</html>",
            card("NVIDIA HGX H100", "8", "$49.24", "$19.71"),
            card("NVIDIA GB200 NVL72", "4^1", "$42.00", "N/A"),
            card("NVIDIA HGX B300", "8", "Contact sales", "$35.84"),
            card("NVIDIA L40S", "8", "$18.00", "N/A"),
            // The same GPU again, in another region tab.
            card("NVIDIA HGX H100", "8", "$55.00", "$20.00"),
        );
        let listings = parse(&html).unwrap();
        let summary: Vec<(String, i16, f64)> = listings
            .iter()
            .map(|l| (l.gpu_type.clone(), l.gpu_count, l.price_per_hour))
            .collect();
        assert_eq!(
            summary,
            [
                ("H100 SXM".to_owned(), 8, 49.24),
                ("L40S".to_owned(), 8, 18.0)
            ],
            "rack-scale and contact-sales cards are left out; the first H100 card wins"
        );
        assert_eq!(listings[0].extra["spot_price_per_hour"], 19.71);
    }

    #[test]
    fn a_changed_layout_fails_instead_of_returning_nothing() {
        let err = parse("<html><h1>Pricing</h1><p>Talk to us</p></html>").unwrap_err();
        assert!(err.to_string().contains("layout changed"), "{err}");
    }
}
