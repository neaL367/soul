//! Integration tests for `SQLite` database, history, bookmarks, cookie jar, and web storage.

use storage::{
    BookmarkStore, Cookie, CookieJar, HistoryStore, LocalStorage, SessionStorage, StorageDatabase,
};
use url::Url;

#[test]
fn test_history_recording_and_search() {
    let db = StorageDatabase::open_in_memory().expect("failed to open in-memory db");
    let history = HistoryStore::new(db);

    history
        .record_visit("https://rust-lang.org", Some("Rust Programming"), 1000)
        .unwrap();
    history
        .record_visit("https://github.com/neaL367/soul", Some("Soul"), 1010)
        .unwrap();
    history
        .record_visit(
            "https://rust-lang.org",
            Some("Rust Programming Language"),
            1020,
        )
        .unwrap();

    let search_results = history.query_history("rust", 10).unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].url, "https://rust-lang.org");
    assert_eq!(search_results[0].visit_count, 2);
    assert_eq!(search_results[0].last_visited_at, 1020);

    let all = history.query_history("", 10).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_bookmarks_crud() {
    let db = StorageDatabase::open_in_memory().expect("failed to open in-memory db");
    let bookmarks = BookmarkStore::new(db);

    let id1 = bookmarks
        .add_bookmark("https://rust-lang.org", "Rust", Some("Dev"), 1000)
        .unwrap();

    let id2 = bookmarks
        .add_bookmark(
            "https://news.ycombinator.com",
            "Hacker News",
            Some("News"),
            1010,
        )
        .unwrap();

    let dev_bookmarks = bookmarks.list_bookmarks(Some("Dev")).unwrap();
    assert_eq!(dev_bookmarks.len(), 1);
    assert_eq!(dev_bookmarks[0].id, id1);
    assert_eq!(dev_bookmarks[0].title, "Rust");

    let deleted = bookmarks.delete_bookmark(id2).unwrap();
    assert!(deleted);

    let all = bookmarks.list_bookmarks(None).unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn test_cookie_jar_rfc6265bis_matching_and_expiry() {
    let db = StorageDatabase::open_in_memory().expect("failed to open in-memory db");
    let jar = CookieJar::new(db);

    let domain_cookie = Cookie {
        name: "session_id".to_string(),
        domain: "example.com".to_string(),
        path: "/app".to_string(),
        value: "secret_123".to_string(),
        expires_at: Some(2000),
        is_secure: false,
        is_http_only: true,
        same_site: "Lax".to_string(),
        host_only: false,
    };
    jar.set_cookie(&domain_cookie).unwrap();

    let secure_cookie = Cookie {
        name: "secure_token".to_string(),
        domain: "secure.example.com".to_string(),
        path: "/".to_string(),
        value: "token_abc".to_string(),
        expires_at: Some(2000),
        is_secure: true,
        is_http_only: false,
        same_site: "Strict".to_string(),
        host_only: false,
    };
    jar.set_cookie(&secure_cookie).unwrap();

    // 1. Subdomain matching with path match on HTTP
    let cookies1 = jar
        .get_cookies_for_url("http://sub.example.com/app/dashboard", 1500)
        .unwrap();
    assert_eq!(cookies1.len(), 1);
    assert_eq!(cookies1[0].name, "session_id");

    // 2. HTTPS matching secure cookie
    let cookies2 = jar
        .get_cookies_for_url("https://secure.example.com/profile", 1500)
        .unwrap();
    assert_eq!(cookies2.len(), 1);
    assert_eq!(cookies2[0].name, "secure_token");

    // 3. HTTP should NOT receive secure cookie
    let cookies3 = jar
        .get_cookies_for_url("http://secure.example.com/profile", 1500)
        .unwrap();
    assert!(cookies3.is_empty());

    // 4. Expired cookies purge
    let purged = jar.clear_expired(2500).unwrap();
    assert_eq!(purged, 2);

    let remaining = jar
        .get_cookies_for_url("http://sub.example.com/app/dashboard", 2500)
        .unwrap();
    assert!(remaining.is_empty());
}

#[test]
fn test_cookie_domain_attribute_is_contained_by_request_host() {
    // A subdomain may scope a cookie to its parent domain.
    let ok = Cookie::parse(
        "sid=1; Domain=example.com",
        &Url::parse("https://sub.example.com/").unwrap(),
    );
    assert!(ok.is_some());
    assert!(!ok.unwrap().host_only);

    // A totally unrelated domain must be rejected, not silently accepted.
    let evil = Cookie::parse(
        "sid=1; Domain=evil.com",
        &Url::parse("https://bank.com/").unwrap(),
    );
    assert!(evil.is_none());

    // A suffix-spoofing domain ("com" is not a registrable domain) must be rejected.
    let psl_evasion = Cookie::parse(
        "sid=1; Domain=com",
        &Url::parse("https://example.com/").unwrap(),
    );
    assert!(psl_evasion.is_none());

    // Domain cookies are not allowed on IP hosts.
    let ip_host = Cookie::parse(
        "sid=1; Domain=127.0.0.1",
        &Url::parse("http://127.0.0.1/").unwrap(),
    );
    assert!(ip_host.is_none());

    // Without a Domain attribute the cookie is host-only.
    let host_only = Cookie::parse("sid=1", &Url::parse("https://a.example.com/").unwrap());
    assert!(host_only.is_some());
    assert!(host_only.unwrap().host_only);
}

#[test]
fn test_host_only_cookies_are_not_shared_with_subdomains() {
    let db = StorageDatabase::open_in_memory().expect("failed to open in-memory db");
    let jar = CookieJar::new(db);

    let session = Cookie::parse(
        "session=abc",
        &Url::parse("https://a.example.com/").unwrap(),
    )
    .unwrap();
    assert!(session.host_only);
    jar.set_cookie(&session).unwrap();

    // The exact host that set the cookie receives it...
    let exact = jar
        .get_cookies_for_url("https://a.example.com/page", 100)
        .unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].name, "session");

    // ...but sibling and parent hosts must not (previously the cookie was
    // stored as a domain cookie and leaked to every subdomain).
    let sibling = jar
        .get_cookies_for_url("https://b.example.com/page", 100)
        .unwrap();
    assert!(sibling.is_empty());

    // A cookie set WITH an explicit Domain attribute still matches subdomains.
    let scoped = Cookie::parse(
        "wide=1; Domain=example.com",
        &Url::parse("https://a.example.com/").unwrap(),
    )
    .unwrap();
    jar.set_cookie(&scoped).unwrap();
    let sub = jar
        .get_cookies_for_url("https://b.example.com/page", 100)
        .unwrap();
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0].name, "wide");
}

#[test]
fn test_local_storage_persistence() {
    let db = StorageDatabase::open_in_memory().expect("failed to open in-memory db");
    let storage = LocalStorage::new(db);

    storage
        .set_item("https://example.com", "theme", "dark")
        .unwrap();
    storage
        .set_item("https://example.com", "fontSize", "16px")
        .unwrap();

    assert_eq!(
        storage.get_item("https://example.com", "theme").unwrap(),
        Some("dark".to_string())
    );
    assert_eq!(
        storage.get_item("https://other.com", "theme").unwrap(),
        None
    );
    assert_eq!(storage.len("https://example.com").unwrap(), 2);

    storage
        .remove_item("https://example.com", "fontSize")
        .unwrap();
    assert_eq!(storage.len("https://example.com").unwrap(), 1);

    storage.clear_origin("https://example.com").unwrap();
    assert_eq!(storage.len("https://example.com").unwrap(), 0);
}

#[test]
fn test_session_storage_in_memory_isolation() {
    let session = SessionStorage::new();

    session.set_item("https://app.local", "token", "xyz");
    assert_eq!(
        session.get_item("https://app.local", "token"),
        Some("xyz".to_string())
    );
    assert_eq!(session.get_item("https://other.local", "token"), None);

    assert_eq!(session.len("https://app.local"), 1);
    assert!(!session.is_empty("https://app.local"));

    assert!(session.remove_item("https://app.local", "token"));
    assert_eq!(session.len("https://app.local"), 0);
    assert!(session.is_empty("https://app.local"));
}

// ── RFC 9111 HTTP Cache Tests ─────────────────────────────────────────────

#[test]
fn test_http_cache_store_and_fresh_lookup() {
    use std::collections::HashMap;
    use storage::HttpCacheStore;

    let dir = std::env::temp_dir().join("soul_cache_test_fresh.db");
    let _ = std::fs::remove_file(&dir);
    let store = HttpCacheStore::new(&dir).unwrap();

    let mut headers = HashMap::new();
    headers.insert("cache-control".to_string(), "max-age=3600".to_string());
    headers.insert("etag".to_string(), "\"abc123\"".to_string());

    store
        .store(
            "https://example.com/style.css",
            200,
            "text/css",
            &headers,
            b"body { color: red; }",
        )
        .unwrap();

    let entry = store
        .lookup("https://example.com/style.css")
        .unwrap()
        .expect("entry must exist");

    assert_eq!(entry.url, "https://example.com/style.css");
    assert_eq!(entry.status_code, 200);
    assert_eq!(entry.mime_type, "text/css");
    assert_eq!(entry.etag.as_deref(), Some("\"abc123\""));
    assert_eq!(entry.max_age_secs, 3600);
    assert_eq!(entry.body, b"body { color: red; }");
    assert!(
        HttpCacheStore::is_fresh(&entry),
        "entry with 1-hour max-age must be fresh"
    );

    let _ = std::fs::remove_file(&dir);
}

#[test]
fn test_http_cache_staleness_and_metadata_update() {
    use std::collections::HashMap;
    use storage::HttpCacheStore;

    let dir = std::env::temp_dir().join("soul_cache_test_stale.db");
    let _ = std::fs::remove_file(&dir);
    let store = HttpCacheStore::new(&dir).unwrap();

    let mut headers = HashMap::new();
    headers.insert("cache-control".to_string(), "max-age=0".to_string());
    headers.insert("etag".to_string(), "\"v1\"".to_string());

    store
        .store(
            "https://example.com/data.json",
            200,
            "application/json",
            &headers,
            b"{}",
        )
        .unwrap();

    let entry = store
        .lookup("https://example.com/data.json")
        .unwrap()
        .expect("entry must exist");

    // max-age=0 means always stale
    assert!(
        !HttpCacheStore::is_fresh(&entry),
        "max-age=0 entry must be stale"
    );

    // Simulate 304 response refreshing metadata with new max-age
    store
        .update_metadata("https://example.com/data.json", Some("\"v2\""), 600)
        .unwrap();

    let refreshed = store
        .lookup("https://example.com/data.json")
        .unwrap()
        .expect("entry must still exist");

    assert_eq!(refreshed.etag.as_deref(), Some("\"v2\""));
    assert_eq!(refreshed.max_age_secs, 600);
    assert_eq!(
        refreshed.body, b"{}",
        "body unchanged after metadata update"
    );
    assert!(
        HttpCacheStore::is_fresh(&refreshed),
        "refreshed entry must be fresh"
    );

    let _ = std::fs::remove_file(&dir);
}

#[test]
fn test_http_cache_no_store_is_never_cached() {
    use std::collections::HashMap;
    use storage::HttpCacheStore;

    let dir = std::env::temp_dir().join("soul_cache_test_nostore.db");
    let _ = std::fs::remove_file(&dir);
    let store = HttpCacheStore::new(&dir).unwrap();

    let mut headers = HashMap::new();
    headers.insert("cache-control".to_string(), "no-store".to_string());

    store
        .store(
            "https://example.com/private",
            200,
            "text/html",
            &headers,
            b"<secret>",
        )
        .unwrap();

    let entry = store.lookup("https://example.com/private").unwrap();
    assert!(entry.is_none(), "no-store response must never be persisted");

    let _ = std::fs::remove_file(&dir);
}

#[test]
fn test_http_cache_set_cookie_and_authorization_are_never_cached() {
    use std::collections::HashMap;
    use storage::HttpCacheStore;

    let dir = std::env::temp_dir().join("soul_cache_test_sensitive.db");
    let _ = std::fs::remove_file(&dir);
    let store = HttpCacheStore::new(&dir).unwrap();

    let mut set_cookie = HashMap::new();
    set_cookie.insert("cache-control".to_string(), "max-age=3600".to_string());
    set_cookie.insert("Set-Cookie".to_string(), "session=abc123".to_string());
    store
        .store(
            "https://example.com/page",
            200,
            "text/html",
            &set_cookie,
            b"<html>",
        )
        .unwrap();
    assert!(
        store.lookup("https://example.com/page").unwrap().is_none(),
        "Set-Cookie responses must not be cached without explicit permission"
    );

    let mut authorized = HashMap::new();
    authorized.insert("cache-control".to_string(), "max-age=3600".to_string());
    authorized.insert("authorization".to_string(), "Bearer secret".to_string());
    store
        .store(
            "https://example.com/account",
            200,
            "application/json",
            &authorized,
            b"{\"balance\":123}",
        )
        .unwrap();
    assert!(
        store
            .lookup("https://example.com/account")
            .unwrap()
            .is_none(),
        "Authorization responses must not be cached without explicit permission"
    );

    let mut permitted = HashMap::new();
    permitted.insert(
        "cache-control".to_string(),
        "public, max-age=3600".to_string(),
    );
    permitted.insert("set-cookie".to_string(), "session=abc123".to_string());
    store
        .store(
            "https://example.com/public",
            200,
            "text/html",
            &permitted,
            b"<html>",
        )
        .unwrap();
    assert!(
        store
            .lookup("https://example.com/public")
            .unwrap()
            .is_some(),
        "Cache-Control: public explicitly permits caching Set-Cookie responses"
    );

    let _ = std::fs::remove_file(&dir);
}

#[test]
fn test_cookie_public_suffix_rejection() {
    let url = Url::parse("https://app.example.co.uk/index.html").unwrap();

    // Suffix domain `example.co.uk`: allowed
    let valid_cookie = Cookie::parse("session=123; Domain=example.co.uk", &url);
    assert!(valid_cookie.is_some());
    assert_eq!(valid_cookie.unwrap().domain, "example.co.uk");

    // Public suffix `co.uk`: must be rejected
    let public_suffix_cookie = Cookie::parse("session=123; Domain=co.uk", &url);
    assert!(public_suffix_cookie.is_none());

    // Bare TLD `uk`: must be rejected
    let bare_tld_cookie = Cookie::parse("session=123; Domain=uk", &url);
    assert!(bare_tld_cookie.is_none());
}
