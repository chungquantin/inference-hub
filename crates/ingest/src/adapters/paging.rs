//! Fetching every page of a paginated list.
//!
//! Upstream catalogs paginate in different ways; each adapter describes its
//! scheme with a [`Paging`] and [`fetch_all`] walks the pages. Today most of
//! them answer with the whole list at once, but a catalog that starts paging
//! would otherwise be silently truncated to its first page: the store would
//! then refuse the run as a shrink, or worse, a growing catalog would stop
//! growing on the site. So every adapter goes through this, even for one page.
//!
//! Loops are bounded: at most [`MAX_PAGES`] pages, and a page that repeats the
//! previous one or a cursor seen before ends the walk with an error.

use std::collections::HashSet;

use reqwest::Client;
use serde_json::Value;

use super::{AdapterError, get_text};

/// Most pages one fetch may walk. Ten thousand items at 100 per page.
pub const MAX_PAGES: usize = 200;

/// How a list is paginated.
#[derive(Debug, Clone)]
pub enum Paging {
    /// One request returns everything.
    None,
    /// `?<offset>=N&<limit>=size`. The walk stops on a short or empty page, or
    /// when the running count reaches `total_field` when the response has one.
    Offset {
        offset: &'static str,
        limit: &'static str,
        size: usize,
        total_field: Option<&'static [&'static str]>,
    },
    /// `?<param>=N` starting at `first`, until `pages_field` (when present)
    /// or an empty page.
    Page {
        param: &'static str,
        first: usize,
        pages_field: Option<&'static [&'static str]>,
    },
    /// The next page's URL (absolute or relative) is in the body at `next_field`.
    NextLink { next_field: &'static [&'static str] },
    /// An opaque cursor at `cursor_field` goes back as `?<param>=cursor`.
    Cursor {
        param: &'static str,
        cursor_field: &'static [&'static str],
    },
}

/// Where the items are in a page.
#[derive(Debug, Clone, Copy)]
pub enum Items {
    /// The body is the array.
    Root,
    /// The array is at this path, such as `["data"]`.
    At(&'static [&'static str]),
}

fn at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, k| v.get(*k))
}

fn with_param(url: &str, key: &str, value: &str) -> Result<String, AdapterError> {
    let mut u = reqwest::Url::parse(url)
        .map_err(|e| AdapterError::Format("paging", format!("bad URL {url}: {e}")))?;
    let kept: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| k != key)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    u.query_pairs_mut()
        .clear()
        .extend_pairs(kept)
        .append_pair(key, value);
    Ok(u.into())
}

/// Takes the items out of one page.
fn items_of(adapter: &'static str, page: &Value, items: Items) -> Result<Vec<Value>, AdapterError> {
    let list = match items {
        Items::Root => Some(page),
        Items::At(path) => at(page, path),
    };
    match list {
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(Value::Null) | None if matches!(items, Items::At(_)) => Err(AdapterError::Format(
            adapter,
            "the response has no item list".into(),
        )),
        _ => Err(AdapterError::Format(
            adapter,
            "the item list is not an array".into(),
        )),
    }
}

/// Fetches every page of `url` and returns all items plus the first page
/// (for adapters that read other fields of it).
pub async fn fetch_all(
    client: &Client,
    adapter: &'static str,
    url: &str,
    items: Items,
    paging: &Paging,
) -> Result<(Vec<Value>, Value), AdapterError> {
    let mut all = Vec::new();
    let mut first: Option<Value> = None;
    let mut seen_urls = HashSet::new();
    let mut previous_first_item: Option<Value> = None;
    let mut next_url = match paging {
        Paging::Offset {
            offset,
            limit,
            size,
            ..
        } => with_param(&with_param(url, limit, &size.to_string())?, offset, "0")?,
        Paging::Page { param, first, .. } => with_param(url, param, &first.to_string())?,
        _ => url.to_owned(),
    };
    let mut page_number = 0usize;

    loop {
        if page_number >= MAX_PAGES {
            return Err(AdapterError::Format(
                adapter,
                format!("more than {MAX_PAGES} pages; refusing to keep paging"),
            ));
        }
        if !seen_urls.insert(next_url.clone()) {
            return Err(AdapterError::Format(
                adapter,
                "pagination loops back on itself".into(),
            ));
        }
        let body = get_text(client, adapter, &next_url).await?;
        let page: Value = serde_json::from_str(&body)
            .map_err(|e| AdapterError::Format(adapter, e.to_string()))?;
        let page_items = items_of(adapter, &page, items)?;
        // A server that ignores the paging parameters returns page one again.
        if page_number > 0
            && !page_items.is_empty()
            && page_items.first() == previous_first_item.as_ref()
        {
            break;
        }
        previous_first_item = page_items.first().cloned();
        let count = page_items.len();
        all.extend(page_items);
        if first.is_none() {
            first = Some(page.clone());
        }
        page_number += 1;

        next_url = match paging {
            Paging::None => break,
            Paging::Offset {
                offset,
                size,
                total_field,
                ..
            } => {
                let total = total_field
                    .and_then(|p| at(&page, p))
                    .and_then(Value::as_u64);
                let done =
                    count == 0 || count < *size || total.is_some_and(|t| all.len() as u64 >= t);
                if done {
                    break;
                }
                with_param(&next_url, offset, &all.len().to_string())?
            }
            Paging::Page {
                param,
                first,
                pages_field,
            } => {
                // Without a page count, the empty page after the last ends the walk.
                let pages = pages_field
                    .and_then(|p| at(&page, p))
                    .and_then(Value::as_u64);
                let current = first + page_number - 1;
                if count == 0 || pages.is_some_and(|p| current as u64 >= p) {
                    break;
                }
                with_param(&next_url, param, &(current + 1).to_string())?
            }
            Paging::NextLink { next_field } => {
                match at(&page, next_field).and_then(Value::as_str) {
                    Some(next) if !next.is_empty() => reqwest::Url::parse(&next_url)
                        .and_then(|base| base.join(next))
                        .map_err(|e| {
                            AdapterError::Format(adapter, format!("bad next link {next}: {e}"))
                        })?
                        .into(),
                    _ => break,
                }
            }
            Paging::Cursor {
                param,
                cursor_field,
            } => {
                match at(&page, cursor_field).and_then(|v| match v {
                    Value::String(s) if !s.is_empty() => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                }) {
                    Some(cursor) if count > 0 => with_param(&next_url, param, &cursor)?,
                    _ => break,
                }
            }
        };
    }
    Ok((all, first.unwrap_or(Value::Null)))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{Json, Router, extract::Query, routing::get};
    use serde_json::json;

    use super::*;

    async fn serve(app: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        base
    }

    fn ids(items: &[Value]) -> Vec<u64> {
        items.iter().map(|v| v["id"].as_u64().unwrap()).collect()
    }

    const N: u64 = 25;

    #[tokio::test]
    async fn offset_paging_reads_every_page_and_stops_at_the_total() {
        let calls = Arc::new(Mutex::new(0));
        let c = calls.clone();
        let app = Router::new().route(
            "/m",
            get(
                move |Query(q): Query<std::collections::HashMap<String, u64>>| {
                    let c = c.clone();
                    async move {
                        *c.lock().unwrap() += 1;
                        let (o, l) = (q["offset"], q["limit"]);
                        let data: Vec<_> = (o..(o + l).min(N)).map(|i| json!({"id": i})).collect();
                        Json(json!({"data": data, "total_count": N}))
                    }
                },
            ),
        );
        let base = serve(app).await;
        let paging = Paging::Offset {
            offset: "offset",
            limit: "limit",
            size: 10,
            total_field: Some(&["total_count"]),
        };
        let (items, _) = fetch_all(
            &Client::new(),
            "t",
            &format!("{base}/m"),
            Items::At(&["data"]),
            &paging,
        )
        .await
        .unwrap();
        assert_eq!(ids(&items), (0..N).collect::<Vec<_>>());
        assert_eq!(*calls.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn a_server_that_ignores_paging_is_read_once() {
        let app = Router::new().route(
            "/m",
            get(|| async {
                Json(json!({"data": (0..N).map(|i| json!({"id": i})).collect::<Vec<_>>()}))
            }),
        );
        let base = serve(app).await;
        let paging = Paging::Offset {
            offset: "offset",
            limit: "limit",
            size: 10,
            total_field: None,
        };
        let (items, _) = fetch_all(
            &Client::new(),
            "t",
            &format!("{base}/m"),
            Items::At(&["data"]),
            &paging,
        )
        .await
        .unwrap();
        assert_eq!(
            items.len(),
            N as usize,
            "the repeated first page is not added again"
        );
    }

    #[tokio::test]
    async fn next_links_and_cursors_are_followed() {
        let app = Router::new()
            .route(
                "/links",
                get(
                    |Query(q): Query<std::collections::HashMap<String, u64>>| async move {
                        let o = q.get("offset").copied().unwrap_or(0);
                        let data: Vec<_> = (o..(o + 10).min(N)).map(|i| json!({"id": i})).collect();
                        let next = (o + 10 < N).then(|| format!("/links?offset={}", o + 10));
                        Json(json!({"data": data, "links": {"next": next}}))
                    },
                ),
            )
            .route(
                "/cursor",
                get(
                    |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                        let o: u64 = q
                            .get("after")
                            .map_or(0, |c| c.trim_start_matches('c').parse().unwrap());
                        let data: Vec<_> = (o..(o + 10).min(N)).map(|i| json!({"id": i})).collect();
                        let next = (o + 10 < N).then(|| format!("c{}", o + 10));
                        Json(json!({"items": data, "next_cursor": next}))
                    },
                ),
            );
        let base = serve(app).await;
        let (links, _) = fetch_all(
            &Client::new(),
            "t",
            &format!("{base}/links"),
            Items::At(&["data"]),
            &Paging::NextLink {
                next_field: &["links", "next"],
            },
        )
        .await
        .unwrap();
        assert_eq!(links.len(), N as usize);
        let (cursor, _) = fetch_all(
            &Client::new(),
            "t",
            &format!("{base}/cursor"),
            Items::At(&["items"]),
            &Paging::Cursor {
                param: "after",
                cursor_field: &["next_cursor"],
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&cursor), (0..N).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn page_numbers_stop_at_the_page_count_and_loops_fail() {
        let app = Router::new()
            .route(
                "/p",
                get(
                    |Query(q): Query<std::collections::HashMap<String, u64>>| async move {
                        let p = q["page"];
                        let data: Vec<_> = ((p - 1) * 10..(p * 10).min(N))
                            .map(|i| json!({"id": i}))
                            .collect();
                        Json(json!({"data": data, "page": p, "pages": 3}))
                    },
                ),
            )
            .route(
                "/loop",
                get(|| async { Json(json!({"data": [{"id": 1}], "next": "/loop"})) }),
            );
        let base = serve(app).await;
        let (pages, _) = fetch_all(
            &Client::new(),
            "t",
            &format!("{base}/p"),
            Items::At(&["data"]),
            &Paging::Page {
                param: "page",
                first: 1,
                pages_field: Some(&["pages"]),
            },
        )
        .await
        .unwrap();
        assert_eq!(ids(&pages), (0..N).collect::<Vec<_>>());
        let err = fetch_all(
            &Client::new(),
            "t",
            &format!("{base}/loop"),
            Items::At(&["data"]),
            &Paging::NextLink {
                next_field: &["next"],
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("loops back"), "{err}");
    }

    #[test]
    fn replaces_a_parameter_and_keeps_the_others() {
        assert_eq!(
            with_param("https://x.test/m?type=all&offset=0", "offset", "100").unwrap(),
            "https://x.test/m?type=all&offset=100"
        );
    }
}
