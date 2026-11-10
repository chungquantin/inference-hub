//! Canonical form of article URLs, used as the dedupe key of news items.

use url::Url;

/// Query parameters that only track where a click came from.
const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "dclid", "msclkid", "mc_cid", "mc_eid", "ref", "ref_src", "source",
    "_hsenc", "_hsmi", "mkt_tok", "igshid", "yclid", "twclid",
];

/// Normalizes an article URL so the same article from two feeds gets one key.
///
/// It keeps the scheme (upgrading http to https), lowercases the host, drops a
/// leading `www.`, the fragment, tracking parameters and a trailing slash, and
/// sorts the remaining query parameters. Returns `None` for anything that is not
/// an absolute http(s) URL.
pub fn canonicalize(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.set_scheme("https").ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    url.set_host(Some(&host)).ok()?;
    url.set_fragment(None);
    if url.port() == Some(443) {
        url.set_port(None).ok()?;
    }

    let mut params: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| {
            let k = k.to_ascii_lowercase();
            !k.starts_with("utm_") && !TRACKING_PARAMS.contains(&k.as_str())
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    params.sort();
    if params.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(params);
    }

    let path = url.path();
    if path.len() > 1 && path.ends_with('/') {
        let trimmed = path.trim_end_matches('/').to_owned();
        url.set_path(&trimmed);
    }
    Some(url.into())
}

/// The host of a URL without `www.`, used as the publisher when a feed names none.
pub fn host(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_equivalent_urls_to_one_key() {
        let canonical = "https://example.com/blog/post";
        for raw in [
            "https://example.com/blog/post",
            "http://example.com/blog/post",
            "https://WWW.Example.com/blog/post/",
            "https://example.com/blog/post#comments",
            "https://example.com:443/blog/post",
            "https://example.com/blog/post?utm_source=x&utm_medium=rss",
            "https://example.com/blog/post?ref=hn&fbclid=abc",
        ] {
            assert_eq!(canonicalize(raw).as_deref(), Some(canonical), "{raw}");
        }
    }

    #[test]
    fn keeps_meaningful_query_parameters_in_sorted_order() {
        assert_eq!(
            canonicalize("https://example.com/a?b=2&utm_campaign=z&a=1").as_deref(),
            Some("https://example.com/a?a=1&b=2")
        );
    }

    #[test]
    fn keeps_root_path_and_rejects_non_web_urls() {
        assert_eq!(
            canonicalize("https://example.com/").as_deref(),
            Some("https://example.com/")
        );
        assert_eq!(canonicalize("ftp://example.com/a"), None);
        assert_eq!(canonicalize("/relative/path"), None);
        assert_eq!(canonicalize("not a url"), None);
    }

    #[test]
    fn host_strips_www() {
        assert_eq!(
            host("https://www.Example.com/x").as_deref(),
            Some("example.com")
        );
    }
}
