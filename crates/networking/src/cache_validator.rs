//! RFC 9111 HTTP cache conditional request evaluation and 304 revalidation.

use crate::types::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::sync::Arc;
use storage::HttpCacheStore;

/// RFC 9111 HTTP Cache Validator.
#[derive(Clone, Default)]
pub struct CacheValidator {
    store: Option<Arc<HttpCacheStore>>,
}

impl CacheValidator {
    /// Creates a new validator with an optional backing `SQLite` cache store.
    #[must_use]
    pub const fn new(store: Option<Arc<HttpCacheStore>>) -> Self {
        Self { store }
    }

    /// Evaluates if a request can be satisfied directly from cache, or modifies
    /// the request with conditional headers (`If-None-Match`, `If-Modified-Since`)
    /// if a stale entry is found.
    pub fn prepare_request(&self, request: &mut HttpRequest) -> Option<HttpResponse> {
        let store = self.store.as_ref()?;
        let url_str = request.url.as_str();
        let entry = store.lookup(url_str).ok().flatten()?;

        // If fresh per RFC 9111 §4.2, serve directly from cache
        if HttpCacheStore::is_fresh(&entry) {
            tracing::debug!(
                url = url_str,
                "Serving response directly from RFC 9111 cache"
            );
            let mut headers = HashMap::new();
            if let Some(ref etag) = entry.etag {
                headers.insert("etag".to_string(), etag.clone());
            }
            if let Some(ref lm) = entry.last_modified {
                headers.insert("last-modified".to_string(), lm.clone());
            }
            return Some(HttpResponse {
                url: request.url.clone(),
                status_code: entry.status_code,
                headers,
                set_cookies: Vec::new(),
                body: entry.body.into(),
                mime_type: entry.mime_type,
            });
        }

        // Otherwise attach conditional headers for revalidation (RFC 9111 §4.3)
        if let Some(ref etag) = entry.etag {
            request
                .headers
                .push(("if-none-match".to_string(), etag.clone()));
        }
        if let Some(ref last_mod) = entry.last_modified {
            request
                .headers
                .push(("if-modified-since".to_string(), last_mod.clone()));
        }

        None
    }

    /// Handles a response outcome:
    /// - On 304 Not Modified: refreshes metadata and returns cached body.
    /// - On 200 OK: stores in cache if cacheable per RFC 9111.
    pub fn handle_response(&self, request: &HttpRequest, response: HttpResponse) -> HttpResponse {
        let Some(store) = self.store.as_ref() else {
            return response;
        };

        let url_str = request.url.as_str();

        if response.status_code == 304 {
            if let Ok(Some(cached)) = store.lookup(url_str) {
                let max_age = storage::http_cache::parse_max_age(&response.headers);
                let new_etag = response.headers.get("etag").map(String::as_str);
                let _ = store.update_metadata(url_str, new_etag, max_age);

                tracing::debug!(url = url_str, "Revalidated 304 Not Modified from cache");
                return HttpResponse {
                    url: response.url,
                    status_code: cached.status_code,
                    headers: response.headers,
                    set_cookies: response.set_cookies,
                    body: cached.body.into(),
                    mime_type: cached.mime_type,
                };
            }
        } else if response.status_code == 200 {
            let _ = store.store(
                url_str,
                response.status_code,
                &response.mime_type,
                &response.headers,
                &response.body,
            );
        }

        response
    }
}
