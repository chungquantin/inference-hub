//! `cargo run -p ih-ingest --example discover_feeds -- <input.json> <output.json>`
//!
//! Finds the RSS, Atom or JSON feed of each site. The input is a JSON array of
//! `{ "slug": ..., "site": ... }`; the output has one entry per slug with the
//! feed that was accepted, or the reason none was.
//!
//! A candidate is accepted only when the worker's own parser reads it and it
//! holds at least one entry that has a link and a title, so whatever passes here
//! also ingests. Candidates, in order:
//!   1. feeds the page declares with `<link rel="alternate" type="application/(rss|atom)+xml">`
//!   2. a short list of common paths on the site's host and on its blog sections
//!
//! Feeds of other hosts are accepted only when the page itself declares them
//! (companies often host their blog on a subdomain or a platform).

use std::{collections::HashSet, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

#[derive(Deserialize)]
struct Input {
    slug: String,
    site: String,
}

#[derive(Serialize, Clone)]
struct Found {
    url: String,
    title: Option<String>,
    entries: usize,
    /// "declared" by the page, or "path" for a guessed common path.
    how: &'static str,
    latest: Option<String>,
}

#[derive(Serialize)]
struct Output {
    slug: String,
    site: String,
    feed: Option<Found>,
    /// Every candidate tried and why it failed, for the report.
    tried: Vec<String>,
}

const PATHS: &[&str] = &[
    "/feed",
    "/feed.xml",
    "/rss",
    "/rss.xml",
    "/atom.xml",
    "/index.xml",
    "/blog/feed",
    "/blog/feed.xml",
    "/blog/rss",
    "/blog/rss.xml",
    "/blog/index.xml",
    "/blog/atom.xml",
    "/news/feed",
    "/news/rss.xml",
    "/news/feed.xml",
    "/en/blog/rss.xml",
    "/changelog/rss.xml",
];

/// Declared feed links in a page: `<link ... rel="alternate" type="application/rss+xml" href="...">`.
fn declared(html: &str, base: &url::Url) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<link") {
        let start = from + i;
        let Some(len) = lower[start..].find('>') else {
            break;
        };
        let tag = &html[start..start + len];
        let tag_lower = &lower[start..start + len];
        from = start + len;
        let is_alt = tag_lower.contains("alternate");
        let is_feed = ["rss+xml", "atom+xml", "feed+json", "application/json"]
            .iter()
            .any(|t| tag_lower.contains(t));
        if !(is_alt && is_feed) {
            continue;
        }
        if let Some(href) = attr(tag, "href")
            && let Ok(u) = base.join(&href)
        {
            out.push(u.to_string());
        }
    }
    out
}

/// The value of an HTML attribute, quoted either way.
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = lower[..at]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace() || c == '"' || c == '\'');
        let rest = tag[at + name.len()..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let quote = rest.chars().next()?;
        if quote == '"' || quote == '\'' {
            let inner = &rest[1..];
            return inner.find(quote).map(|e| inner[..e].replace("&amp;", "&"));
        }
        return Some(
            rest.split(|c: char| c.is_whitespace() || c == '>')
                .next()?
                .to_owned(),
        );
    }
    None
}

async fn get(client: &reqwest::Client, url: &str) -> Result<(Vec<u8>, String), String> {
    let response = client
        .get(url)
        .header("accept", "application/rss+xml, application/atom+xml, application/xml, text/xml, text/html;q=0.8, */*;q=0.5")
        .send()
        .await
        .map_err(|e| format!("{e}"))?;
    let status = response.status();
    let final_url = response.url().to_string();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 8_000_000 {
        return Err("too large".into());
    }
    Ok((bytes.to_vec(), final_url))
}

async fn try_feed(client: &reqwest::Client, url: &str, how: &'static str) -> Result<Found, String> {
    let (body, final_url) = get(client, url).await?;
    // A page that answers 200 with HTML is the usual failure of a guessed path.
    let head = String::from_utf8_lossy(&body[..body.len().min(400)]).to_ascii_lowercase();
    if head.contains("<!doctype html") || head.contains("<html") {
        return Err("HTML, not a feed".into());
    }
    let feed = ih_ingest::feed::parse(&body, &final_url).map_err(|e| format!("parse: {e}"))?;
    if feed.entries.is_empty() {
        return Err("feed has no entries".into());
    }
    let latest = feed
        .entries
        .iter()
        .filter_map(|e| e.published_at)
        .max()
        .map(|d| d.format("%Y-%m-%d").to_string());
    Ok(Found {
        url: final_url,
        title: feed.title,
        entries: feed.entries.len(),
        how,
        latest,
    })
}

async fn discover(client: reqwest::Client, input: Input) -> Output {
    let mut tried = Vec::new();
    let mut out = Output {
        slug: input.slug,
        site: input.site.clone(),
        feed: None,
        tried: Vec::new(),
    };
    let Ok(site) = url::Url::parse(&input.site) else {
        out.tried.push("site is not a URL".into());
        return out;
    };
    let host = site.host_str().unwrap_or_default().to_owned();
    let mut seen = HashSet::new();
    let mut candidates: Vec<(String, &'static str)> = Vec::new();

    // 1. what the home page declares
    match get(&client, site.as_str()).await {
        Ok((body, final_url)) => {
            let base = url::Url::parse(&final_url).unwrap_or(site.clone());
            let html = String::from_utf8_lossy(&body);
            for u in declared(&html, &base) {
                candidates.push((u, "declared"));
            }
            // the blog sections of the page, when it links to them
            let lower = html.to_ascii_lowercase();
            for section in ["/blog", "/news", "/changelog", "/resources/blog"] {
                let linked = lower.contains(&format!("href=\"{section}\""))
                    || lower.contains(&format!("href=\"{section}/\""));
                if linked
                    && let Ok(page) = base.join(section)
                    && let Ok((body, final_url)) = get(&client, page.as_str()).await
                {
                    let base = url::Url::parse(&final_url).unwrap_or(page);
                    for u in declared(&String::from_utf8_lossy(&body), &base) {
                        candidates.push((u, "declared"));
                    }
                }
            }
        }
        Err(e) => tried.push(format!("home page: {e}")),
    }
    // 2. common paths on the same host
    let origin = format!(
        "{}://{}",
        site.scheme(),
        site.host_str().unwrap_or_default()
    );
    for p in PATHS {
        candidates.push((format!("{origin}{p}"), "path"));
    }

    for (u, how) in candidates {
        if !seen.insert(u.clone()) {
            continue;
        }
        let same_host = url::Url::parse(&u)
            .ok()
            .and_then(|x| {
                x.host_str()
                    .map(|h| h.trim_start_matches("www.").to_owned())
            })
            .is_some_and(|h| {
                host.trim_start_matches("www.") == h
                    || h.ends_with(&format!(".{}", host.trim_start_matches("www.")))
            });
        if how == "path" && !same_host {
            continue;
        }
        match try_feed(&client, &u, how).await {
            Ok(found) => {
                out.feed = Some(found);
                break;
            }
            Err(e) => {
                if how == "declared" {
                    tried.push(format!("{u}: {e}"));
                }
            }
        }
    }
    out.tried = tried;
    out
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: discover_feeds <input.json> <output.json>");
        std::process::exit(2);
    };
    let items: Vec<Input> = serde_json::from_slice(&std::fs::read(&input).expect("readable input"))
        .expect("valid input");
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "InferenceHubBot/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/chungquantin/inference-hub)"
        ))
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .expect("client");
    let gate = std::sync::Arc::new(Semaphore::new(16));
    let mut tasks = Vec::new();
    for item in items {
        let (client, gate) = (client.clone(), gate.clone());
        tasks.push(tokio::spawn(async move {
            let _permit = gate.acquire().await.expect("semaphore");
            discover(client, item).await
        }));
    }
    let mut results = Vec::new();
    for t in tasks {
        results.push(t.await.expect("task"));
    }
    results.sort_by(|a, b| a.slug.cmp(&b.slug));
    let found = results.iter().filter(|r| r.feed.is_some()).count();
    std::fs::write(&output, serde_json::to_vec_pretty(&results).expect("json"))
        .expect("writable output");
    eprintln!("{found} of {} sites have a feed", results.len());
}
