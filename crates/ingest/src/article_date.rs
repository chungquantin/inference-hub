//! The date an article was first published, read from the article page itself.
//!
//! Feeds cannot be trusted for this. Several publishers (Webflow sites such as Runpod
//! and CoreWeave) put the *last modified* time in `<pubDate>`, so a 2024 post that was
//! edited yesterday shows up as news from yesterday. The page's structured data
//! (`datePublished` in JSON-LD, `article:published_time` in Open Graph) holds the real date.

use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

/// Pages larger than this are not read; the date is in the head or the first JSON-LD block.
const MAX_PAGE_BYTES: usize = 1_500_000;
const PAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// Fetches `url` and returns the publication date its markup declares, if any.
pub async fn fetch_published(client: &reqwest::Client, url: &str) -> Option<DateTime<Utc>> {
    let mut response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .timeout(PAGE_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let is_html = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v.contains("html"));
    if !is_html {
        return None;
    }
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_PAGE_BYTES {
            break;
        }
    }
    extract_published(&String::from_utf8_lossy(&body))
}

/// The earliest plausible publication date declared in `html`.
///
/// Sources, most reliable first: JSON-LD `datePublished`, `article:published_time`,
/// `og:published_time`, `itemprop="datePublished"`, `name="date"` / `pubdate`.
pub fn extract_published(html: &str) -> Option<DateTime<Utc>> {
    let now = Utc::now();
    // A page cannot have been published in the future or before the web had blogs.
    let plausible =
        |d: &DateTime<Utc>| *d <= now + chrono::TimeDelta::hours(24) && d.timestamp() > 631_152_000;
    // The first plausible value of each source, in order of trust. Not the earliest of all:
    // a page that lists related posts carries their dates too.
    let metas = [
        "article:published_time",
        "og:published_time",
        "datePublished",
        "date",
        "publish-date",
        "pubdate",
        "dc.date.issued",
        "dc.date",
        "citation_publication_date",
    ];
    json_ld_dates(html)
        .into_iter()
        .find(plausible)
        .or_else(|| {
            metas
                .iter()
                .find_map(|key| meta_dates(html, key).into_iter().find(plausible))
        })
        .or_else(|| time_tag_dates(html).into_iter().find(plausible))
}

/// Every `"datePublished": "..."` value, whatever the JSON nesting.
fn json_ld_dates(html: &str) -> Vec<DateTime<Utc>> {
    let mut out = Vec::new();
    let needle = "\"datePublished\"";
    let mut rest = html;
    while let Some(at) = rest.find(needle) {
        rest = &rest[at + needle.len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(':') else {
            continue;
        };
        let after = after.trim_start();
        let Some(after) = after.strip_prefix('"') else {
            continue;
        };
        if let Some(end) = after.find('"')
            && let Some(date) = parse_date(&after[..end])
        {
            out.push(date);
        }
    }
    out
}

/// Values of `<meta property|name|itemprop="key" content="...">`, attributes in either order.
fn meta_dates(html: &str, key: &str) -> Vec<DateTime<Utc>> {
    let lower = html.to_ascii_lowercase();
    let key = key.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<meta") {
        let start = from + at;
        let Some(len) = lower[start..].find('>') else {
            break;
        };
        let tag = &html[start..start + len];
        let tag_lower = &lower[start..start + len];
        from = start + len;
        let names_key = ["property", "name", "itemprop"]
            .iter()
            .any(|attr| attribute(tag_lower, attr).is_some_and(|v| v.eq_ignore_ascii_case(&key)));
        if names_key
            && let Some(content) = attribute(tag, "content")
            && let Some(date) = parse_date(content)
        {
            out.push(date);
        }
    }
    out
}

/// `<time datetime="..." itemprop="datePublished">` and `<time class="...published...">`.
fn time_tag_dates(html: &str) -> Vec<DateTime<Utc>> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<time") {
        let start = from + at;
        let Some(len) = lower[start..].find('>') else {
            break;
        };
        let tag = &html[start..start + len];
        let tag_lower = &lower[start..start + len];
        from = start + len;
        let marks_publication = tag_lower.contains("datepublished")
            || tag_lower.contains("pubdate")
            || tag_lower.contains("published")
            || tag_lower.contains("entry-date");
        if marks_publication
            && let Some(value) = attribute(tag, "datetime")
            && let Some(date) = parse_date(value)
        {
            out.push(date);
        }
    }
    out
}

/// The value of `name="..."` (or single-quoted) inside one tag's text.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = tag[from..].find(name) {
        let start = from + at;
        from = start + name.len();
        let boundary = start == 0
            || !tag.as_bytes()[start - 1].is_ascii_alphanumeric()
                && tag.as_bytes()[start - 1] != b'-';
        if !boundary {
            continue;
        }
        let rest = tag[from..].trim_start().strip_prefix('=')?.trim_start();
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let rest = &rest[1..];
        let end = rest.find(quote)?;
        return Some(&rest[..end]);
    }
    None
}

/// ISO 8601 in the shapes sites actually emit: with offset, without one, or date only.
pub fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(raw) {
        return Some(d.with_timezone(&Utc));
    }
    if let Ok(d) = DateTime::parse_from_rfc2822(raw) {
        return Some(d.with_timezone(&Utc));
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S%.f %z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
    ] {
        if let Ok(d) = DateTime::parse_from_str(raw, format) {
            return Some(d.with_timezone(&Utc));
        }
        if let Ok(n) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(Utc.from_utc_datetime(&n));
        }
    }
    let day = raw.get(..10).unwrap_or(raw);
    NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(12, 0, 0))
        .map(|n| Utc.from_utc_datetime(&n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(s: &str) -> String {
        extract_published(s).unwrap().format("%Y-%m-%d").to_string()
    }

    #[test]
    fn json_ld_date_published_wins_over_modified() {
        // Runpod: the feed says 2026-11-12 (modified); the page says 2024-07-01.
        let html = r#"<script type="application/ld+json">{"@type":"Article",
            "datePublished": "2024-07-01", "dateModified": "2026-11-12T00:09:51.410Z"}</script>"#;
        assert_eq!(ymd(html), "2024-07-01");
    }

    #[test]
    fn open_graph_published_time_in_either_attribute_order() {
        let a = r#"<meta property="article:published_time" content="2025-03-04T10:00:00+02:00">"#;
        assert_eq!(
            extract_published(a).unwrap().to_rfc3339(),
            "2025-03-04T08:00:00+00:00"
        );
        let b = r#"<meta content="2025-03-04T10:00:00Z" property="article:published_time"/>"#;
        assert_eq!(ymd(b), "2025-03-04");
    }

    #[test]
    fn time_tag_with_publication_marker() {
        let html = r#"<time class="entry-date published" datetime="2023-09-30T08:15:00+00:00">Sep 30</time>"#;
        assert_eq!(ymd(html), "2023-09-30");
    }

    #[test]
    fn unmarked_time_tags_are_ignored() {
        assert!(extract_published(r#"<time datetime="2024-01-01">footer</time>"#).is_none());
    }

    #[test]
    fn future_and_ancient_dates_are_rejected() {
        assert!(extract_published(r#"{"datePublished":"2099-01-01"}"#).is_none());
        assert!(extract_published(r#"{"datePublished":"1969-01-01"}"#).is_none());
    }

    #[test]
    fn no_markup_means_no_date() {
        assert!(extract_published("<html><body>hello</body></html>").is_none());
    }

    #[test]
    fn date_shapes() {
        assert!(parse_date("2024-07-01").is_some());
        assert!(parse_date("2024-07-01T10:11:12").is_some());
        assert!(parse_date("2024-07-01 10:11:12").is_some());
        assert!(parse_date("Mon, 01 Jul 2024 10:11:12 GMT").is_some());
        assert!(parse_date("yesterday").is_none());
    }
}
