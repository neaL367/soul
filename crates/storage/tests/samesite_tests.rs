//! Integration tests for RFC 6265bis `SameSite` and `Secure` cookie policies.

use storage::{Cookie, CookieJar, StorageDatabase};
use url::Url;

#[test]
fn test_samesite_strict_lax_and_none_filtering() {
    let db = StorageDatabase::open_in_memory().expect("failed to open in-memory db");
    let jar = CookieJar::new(db);
    let target_url = Url::parse("https://example.com/api").unwrap();

    let strict_cookie = Cookie::parse(
        "strict_key=strict_val; SameSite=Strict; Path=/",
        &target_url,
    )
    .unwrap();
    let lax_cookie = Cookie::parse("lax_key=lax_val; SameSite=Lax; Path=/", &target_url).unwrap();
    let none_insecure = Cookie::parse("none_insecure=val; SameSite=None; Path=/", &target_url);
    let none_secure = Cookie::parse(
        "none_secure=val; SameSite=None; Secure; Path=/",
        &target_url,
    )
    .unwrap();

    assert!(
        none_insecure.is_none(),
        "SameSite=None without Secure must be rejected per RFC 6265bis"
    );

    jar.set_cookie(&strict_cookie).unwrap();
    jar.set_cookie(&lax_cookie).unwrap();
    jar.set_cookie(&none_secure).unwrap();

    // 1. Same-site context: all cookies should be sent
    let same_site_cookies = jar
        .get_cookies_for_request(
            "https://example.com/api",
            0,
            Some("https://example.com"),
            false,
        )
        .unwrap();
    assert!(same_site_cookies.iter().any(|c| c.name == "strict_key"));
    assert!(same_site_cookies.iter().any(|c| c.name == "lax_key"));
    assert!(same_site_cookies.iter().any(|c| c.name == "none_secure"));
    assert!(!same_site_cookies.iter().any(|c| c.name == "none_insecure")); // Insecure SameSite=None is rejected

    // 2. Cross-site unsafe request (e.g. POST from attacker.com)
    let cross_site_unsafe = jar
        .get_cookies_for_request(
            "https://example.com/api",
            0,
            Some("https://attacker.com"),
            false,
        )
        .unwrap();
    assert!(!cross_site_unsafe.iter().any(|c| c.name == "strict_key"));
    assert!(!cross_site_unsafe.iter().any(|c| c.name == "lax_key"));
    assert!(cross_site_unsafe.iter().any(|c| c.name == "none_secure"));

    // 3. Cross-site safe navigation (e.g. user clicked link GET from external.com)
    let cross_site_safe = jar
        .get_cookies_for_request(
            "https://example.com/api",
            0,
            Some("https://external.com"),
            true,
        )
        .unwrap();
    assert!(!cross_site_safe.iter().any(|c| c.name == "strict_key"));
    assert!(cross_site_safe.iter().any(|c| c.name == "lax_key"));
    assert!(cross_site_safe.iter().any(|c| c.name == "none_secure"));
}
