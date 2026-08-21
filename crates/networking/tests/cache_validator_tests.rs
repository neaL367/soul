//! Integration tests for RFC 9111 HTTP Cache validation and conditional revalidation.

use networking::types::{HttpRequest, HttpResponse};
use networking::{CacheValidator, HttpClient};
use std::collections::HashMap;
use std::sync::Arc;
use storage::HttpCacheStore;
use url::Url;

#[test]
fn test_cache_validator_fresh_response_bypass() {
    let db_path = std::env::temp_dir().join("soul_cache_val_fresh.db");
    let _ = std::fs::remove_file(&db_path);
    let store = Arc::new(HttpCacheStore::new(&db_path).expect("open cache store"));

    let url_str = "https://example.com/style.css";
    let mut headers = HashMap::new();
    headers.insert("cache-control".to_string(), "max-age=3600".to_string());
    headers.insert("etag".to_string(), "\"v100\"".to_string());

    store
        .store(url_str, 200, "text/css", &headers, b"body { margin: 0; }")
        .expect("store cache entry");

    let validator = CacheValidator::new(Some(store));
    let mut req = HttpRequest::get(Url::parse(url_str).unwrap());

    let served = validator.prepare_request(&mut req);
    assert!(served.is_some(), "fresh entry must be served immediately");

    let response = served.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body.as_ref(), b"body { margin: 0; }");
    assert_eq!(response.headers.get("etag").unwrap(), "\"v100\"");

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_cache_validator_stale_conditional_revalidation() {
    let db_path = std::env::temp_dir().join("soul_cache_val_stale.db");
    let _ = std::fs::remove_file(&db_path);
    let store = Arc::new(HttpCacheStore::new(&db_path).expect("open cache store"));

    let url_str = "https://example.com/api/data";
    let mut headers = HashMap::new();
    headers.insert("cache-control".to_string(), "max-age=0".to_string());
    headers.insert("etag".to_string(), "\"etag-abc\"".to_string());
    headers.insert(
        "last-modified".to_string(),
        "Wed, 21 Oct 2025 07:28:00 GMT".to_string(),
    );

    store
        .store(
            url_str,
            200,
            "application/json",
            &headers,
            b"{\"status\":\"ok\"}",
        )
        .expect("store cache entry");

    let validator = CacheValidator::new(Some(store.clone()));
    let mut req = HttpRequest::get(Url::parse(url_str).unwrap());

    // Prepare request attaches conditional headers
    let served = validator.prepare_request(&mut req);
    assert!(served.is_none(), "stale entry must NOT be served directly");

    let if_none_match = req
        .headers
        .iter()
        .find(|(k, _)| k == "if-none-match")
        .map(|(_, v)| v.as_str());
    assert_eq!(if_none_match, Some("\"etag-abc\""));

    let if_modified_since = req
        .headers
        .iter()
        .find(|(k, _)| k == "if-modified-since")
        .map(|(_, v)| v.as_str());
    assert_eq!(if_modified_since, Some("Wed, 21 Oct 2025 07:28:00 GMT"));

    // Simulate 304 response from server with updated max-age
    let mut resp_304_headers = HashMap::new();
    resp_304_headers.insert("cache-control".to_string(), "max-age=600".to_string());
    resp_304_headers.insert("etag".to_string(), "\"etag-abc\"".to_string());

    let resp_304 = HttpResponse {
        url: req.url.clone(),
        status_code: 304,
        headers: resp_304_headers,
        set_cookies: Vec::new(),
        body: bytes::Bytes::new(),
        mime_type: "application/json".to_string(),
    };

    let reconstructed = validator.handle_response(&req, resp_304);
    assert_eq!(reconstructed.status_code, 200);
    assert_eq!(reconstructed.body.as_ref(), b"{\"status\":\"ok\"}");

    // Cached entry is now refreshed
    let entry = store.lookup(url_str).unwrap().unwrap();
    assert_eq!(entry.max_age_secs, 600);

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_http_client_with_cache_store_builder() {
    let db_path = std::env::temp_dir().join("soul_client_cache.db");
    let _ = std::fs::remove_file(&db_path);
    let store = Arc::new(HttpCacheStore::new(&db_path).expect("open cache store"));

    let client = HttpClient::default().with_cache_store(store);
    let _ = client.dns_resolver().resolve("localhost").await;

    let _ = std::fs::remove_file(&db_path);
}
