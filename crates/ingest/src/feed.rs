//! Turns RSS, Atom and JSON Feed documents into normalized entries.

use chrono::{DateTime, Utc};
use feed_rs::model::{Entry, Link};

use crate::url::canonicalize;

/// Longest summary kept, in characters.
const SUMMARY_MAX_CHARS: usize = 400;

/// One article from a feed, ready to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedEntry {
    pub guid: String,
    /// Canonical article URL.
    pub url: String,
    pub title: String,
    /// Plain-text summary, trimmed to a few sentences.
    pub summary: Option<String>,
    pub author: Option<String>,
    pub image_url: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub entries: Vec<FeedEntry>,
    /// Entries left out because they had no usable link or title.
    pub skipped: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("not a valid feed: {0}")]
pub struct ParseError(String);

/// Parses a feed. `base_url` resolves relative links.
pub fn parse(body: &[u8], base_url: &str) -> Result<ParsedFeed, ParseError> {
    // A feed-rs parser keeps state after a failed parse, so each attempt gets a new one.
    let parser = || {
        feed_rs::parser::Builder::new()
            .base_uri(Some(base_url))
            .build()
    };
    let feed = match parser().parse(body) {
        Ok(feed) => feed,
        // feed-rs 3 rejects empty MediaRSS text elements, which WordPress emits
        // (`<media:description type="html"></media:description>`). They carry
        // nothing, so drop them and try once more.
        Err(first) => match strip_empty_media_text(body) {
            Some(cleaned) => parser()
                .parse(cleaned.as_slice())
                .map_err(|_| ParseError(first.to_string()))?,
            None => return Err(ParseError(first.to_string())),
        },
    };

    let mut skipped = 0;
    let entries = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let converted = convert(entry, base_url);
            if converted.is_none() {
                skipped += 1;
            }
            converted
        })
        .collect();

    Ok(ParsedFeed {
        title: feed.title.as_ref().map(text_of),
        entries,
        skipped,
    })
}

/// Removes empty `<media:title>`, `<media:description>` and `<media:text>` elements.
/// Returns `None` when there were none, so the caller does not reparse in vain.
fn strip_empty_media_text(body: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(body).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut changed = false;
    'scan: while let Some(start) = rest.find("<media:") {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        for name in ["media:title", "media:description", "media:text"] {
            let Some(after_name) = tail.strip_prefix('<').and_then(|t| t.strip_prefix(name)) else {
                continue;
            };
            if !after_name.starts_with([' ', '>', '/', '\t', '\n', '\r']) {
                continue;
            }
            let Some(open_end) = after_name.find('>') else {
                continue;
            };
            let consumed = 1 + name.len() + open_end + 1;
            // Self-closing: <media:description type="html"/>
            if after_name[..open_end].ends_with('/') {
                rest = &tail[consumed..];
                changed = true;
                continue 'scan;
            }
            // Empty pair: <media:description ...>   </media:description>
            let close = format!("</{name}>");
            let after_open = &tail[consumed..];
            let ws = after_open.len() - after_open.trim_start().len();
            if after_open[ws..].starts_with(&close) {
                rest = &after_open[ws + close.len()..];
                changed = true;
                continue 'scan;
            }
        }
        out.push('<');
        rest = &tail[1..];
    }
    out.push_str(rest);
    changed.then(|| out.into_bytes())
}

fn convert(entry: Entry, base_url: &str) -> Option<FeedEntry> {
    let link = article_link(&entry.links)?;
    let url = canonicalize(&resolve(base_url, &link.href)?)?;
    let title = text_of(entry.title.as_ref()?);
    if title.is_empty() {
        return None;
    }

    // Summaries and content bodies are HTML in practice whatever they declare.
    let summary = entry
        .summary
        .map(|s| s.content)
        .or_else(|| entry.content.and_then(|c| c.body))
        .map(|html| truncate(&clean_text(&html), SUMMARY_MAX_CHARS))
        .filter(|s| !s.is_empty());

    // feed-rs 3 keeps <media:content> and <enclosure> but drops a <media:thumbnail>
    // that stands alone on an RSS item, so thumbnails only come from Atom and groups.
    let image_url = entry
        .media
        .iter()
        .flat_map(|m| {
            m.thumbnails
                .iter()
                .map(|t| t.image.uri.clone())
                .chain(m.content.iter().filter_map(|c| {
                    let is_image = c
                        .content_type
                        .as_ref()
                        .is_none_or(|t| t.ty().as_str() == "image");
                    is_image
                        .then(|| c.url.as_ref().map(|u| u.to_string()))
                        .flatten()
                }))
        })
        .find(|u| u.starts_with("https://"));

    Some(FeedEntry {
        guid: entry.id,
        url,
        title,
        summary,
        author: entry
            .authors
            .into_iter()
            .filter_map(|a| a.name)
            .map(|n| collapse_whitespace(&n))
            .find(|n| !n.is_empty()),
        image_url,
        published_at: entry.published.or(entry.updated),
    })
}

/// The link to the article itself: `rel="alternate"` (or no rel) and not a media enclosure.
fn article_link(links: &[Link]) -> Option<&Link> {
    links
        .iter()
        .find(|l| matches!(l.rel.as_deref(), None | Some("alternate")))
        .or_else(|| links.iter().find(|l| l.rel.as_deref() != Some("enclosure")))
}

fn resolve(base: &str, href: &str) -> Option<String> {
    let base = url::Url::parse(base).ok()?;
    base.join(href.trim()).ok().map(String::from)
}

/// Turns feed text into plain text. Only HTML is stripped of tags: in plain text
/// a `<` is a literal character (feed-rs has already decoded `&lt;`).
fn text_of(text: &feed_rs::model::Text) -> String {
    let is_html = matches!(text.content_type.subty().as_str(), "html" | "xhtml+xml");
    if is_html {
        clean_text(&text.content)
    } else {
        collapse_whitespace(&text.content)
    }
}

/// Strips HTML tags, decodes common entities and collapses whitespace.
/// Block-level tags become spaces; inline tags vanish so "world</em>'s" stays one word.
pub fn clean_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' if !in_tag => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                if is_block_tag(&tag) {
                    out.push(' ');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    collapse_whitespace(&decode_entities(&out))
}

fn is_block_tag(tag: &str) -> bool {
    let name: String = tag
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "p" | "br"
            | "div"
            | "li"
            | "ul"
            | "ol"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "pre"
            | "tr"
            | "td"
            | "th"
            | "table"
            | "hr"
            | "img"
            | "figure"
            | "figcaption"
            | "section"
            | "article"
    )
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        // An entity is short; look at most 12 bytes ahead, cut on a character boundary
        // (the text after a bare `&` can be any script).
        let mut window = rest.len().min(12);
        while !rest.is_char_boundary(window) {
            window -= 1;
        }
        let Some(end) = rest[..window].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            "hellip" => Some('…'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "rsquo" => Some('’'),
            "lsquo" => Some('‘'),
            "rdquo" => Some('”'),
            "ldquo" => Some('“'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Cuts text at a word boundary and adds an ellipsis when it is too long.
fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max_chars).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", cut.trim_end_matches([',', '.', ';', ':']))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/" xmlns:dc="http://purl.org/dc/elements/1.1/">
<channel>
  <title>Example &amp; Co Blog</title>
  <link>https://example.com/</link>
  <item>
    <title>Launching &lt;Fast&gt; Inference</title>
    <link>https://www.example.com/blog/fast-inference/?utm_source=rss</link>
    <guid>post-1</guid>
    <dc:creator>Ada Lovelace</dc:creator>
    <pubDate>Tue, 04 Nov 2025 10:00:00 GMT</pubDate>
    <description><![CDATA[<p>We are <b>launching</b> fast&nbsp;inference.</p>]]></description>
    <media:content url="https://cdn.example.com/a.png" medium="image" type="image/png"/>
  </item>
  <item>
    <title>Relative link</title>
    <link>/blog/relative</link>
  </item>
  <item>
    <title>No link</title>
  </item>
  <item>
    <link>https://example.com/blog/untitled</link>
  </item>
</channel>
</rss>"#;

    const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Feed</title>
  <id>urn:feed</id>
  <updated>2025-11-05T00:00:00Z</updated>
  <entry>
    <title type="html">Release &amp;amp; notes</title>
    <id>urn:entry:1</id>
    <link rel="enclosure" href="https://example.com/audio.mp3"/>
    <link rel="alternate" href="https://example.org/posts/1"/>
    <updated>2025-11-05T12:00:00Z</updated>
    <content type="html">&lt;p&gt;Body text only.&lt;/p&gt;</content>
  </entry>
</feed>"#;

    #[test]
    fn parses_rss_items_and_skips_unusable_ones() {
        let feed = parse(RSS.as_bytes(), "https://example.com/feed.xml").unwrap();
        assert_eq!(feed.title.as_deref(), Some("Example & Co Blog"));
        assert_eq!(feed.entries.len(), 2);
        assert_eq!(feed.skipped, 2);

        let first = &feed.entries[0];
        assert_eq!(first.url, "https://example.com/blog/fast-inference");
        assert_eq!(first.title, "Launching <Fast> Inference");
        assert_eq!(
            first.summary.as_deref(),
            Some("We are launching fast inference.")
        );
        assert_eq!(first.author.as_deref(), Some("Ada Lovelace"));
        assert_eq!(
            first.image_url.as_deref(),
            Some("https://cdn.example.com/a.png")
        );
        assert_eq!(
            first.published_at.map(|d| d.to_rfc3339()),
            Some("2025-11-04T10:00:00+00:00".to_owned())
        );

        assert_eq!(feed.entries[1].url, "https://example.com/blog/relative");
        assert_eq!(feed.entries[1].published_at, None);
    }

    #[test]
    fn parses_atom_and_prefers_alternate_link_over_enclosure() {
        let feed = parse(ATOM.as_bytes(), "https://example.org/atom.xml").unwrap();
        let entry = &feed.entries[0];
        assert_eq!(entry.url, "https://example.org/posts/1");
        assert_eq!(entry.title, "Release & notes");
        assert_eq!(entry.summary.as_deref(), Some("Body text only."));
        assert!(entry.published_at.is_some(), "falls back to <updated>");
    }

    #[test]
    fn tolerates_empty_media_text_elements() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>WP</title>
  <item>
    <title>Post</title>
    <link>https://wp.example/post</link>
    <media:content url="https://wp.example/a.jpg" medium="image">
      <media:title type="html">Caption</media:title>
      <media:description type="html"></media:description>
      <media:text type="html"/>
    </media:content>
  </item>
</channel></rss>"#;
        let feed = parse(xml.as_bytes(), "https://wp.example/feed").unwrap();
        assert_eq!(feed.entries.len(), 1);
        assert_eq!(
            feed.entries[0].image_url.as_deref(),
            Some("https://wp.example/a.jpg")
        );
    }

    #[test]
    fn strip_only_touches_empty_media_text() {
        let input = "<a><media:title>x</media:title><media:description type=\"html\"> </media:description><media:descriptionx/></a>";
        let out = String::from_utf8(strip_empty_media_text(input.as_bytes()).unwrap()).unwrap();
        assert_eq!(
            out,
            "<a><media:title>x</media:title><media:descriptionx/></a>"
        );
        assert_eq!(
            strip_empty_media_text(b"<a><media:title>x</media:title></a>"),
            None
        );
    }

    #[test]
    fn rejects_html_pages() {
        assert!(parse(b"<html><body>Not found</body></html>", "https://e.com/").is_err());
    }

    #[test]
    fn clean_text_handles_tags_entities_and_whitespace() {
        assert_eq!(
            clean_text("<p>Hello&nbsp;&amp;\n  <em>world</em>&#8217;s &#x2014; &bogus; a & b</p>"),
            "Hello & world’s — &bogus; a & b"
        );
        assert_eq!(
            clean_text("<p>One</p><p>Two<br/>Three</p>"),
            "One Two Three"
        );
    }

    #[test]
    fn a_bare_ampersand_before_multibyte_text_does_not_panic() {
        // A byte offset of 12 falls inside the curly quote here.
        assert_eq!(
            clean_text("Q&A: &“Quoted text” and more"),
            "Q&A: &“Quoted text” and more"
        );
        assert_eq!(clean_text("AT&T &amp; AT&”T"), "AT&T & AT&”T");
        assert_eq!(
            clean_text("&日本語日本語日本語; end"),
            "&日本語日本語日本語; end"
        );
    }

    #[test]
    fn truncate_cuts_at_word_boundary() {
        assert_eq!(truncate("one two three", 100), "one two three");
        assert_eq!(truncate("one two three four", 10), "one two…");
    }
}
