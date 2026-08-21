//! Integration tests for RFC 6797 HSTS persistent policy store.

use storage::HstsStore;

#[test]
fn test_hsts_policy_recording_and_subdomains() {
    let store = HstsStore::in_memory().expect("in-memory hsts store");

    assert!(!store.is_hsts_enforced("example.com").expect("check"));
    assert!(!store.is_hsts_enforced("api.example.com").expect("check"));

    // Record HSTS for example.com with subdomains
    store
        .record_hsts("example.com", 3600, true)
        .expect("record hsts");

    assert!(store.is_hsts_enforced("example.com").expect("check"));
    assert!(store.is_hsts_enforced("api.example.com").expect("check"));
    assert!(
        store
            .is_hsts_enforced("deep.sub.example.com")
            .expect("check")
    );
    assert!(!store.is_hsts_enforced("other.org").expect("check"));

    // max_age=0 clears the policy
    store
        .record_hsts("example.com", 0, false)
        .expect("clear hsts");
    assert!(!store.is_hsts_enforced("example.com").expect("check"));
    assert!(!store.is_hsts_enforced("api.example.com").expect("check"));
}

#[test]
fn test_hsts_header_parsing() {
    let header1 = "max-age=31536000; includeSubDomains; preload";
    let (max_age, inc_sub) = HstsStore::parse_hsts_header(header1).expect("parse");
    assert_eq!(max_age, 31_536_000);
    assert!(inc_sub);

    let header2 = "max-age=0";
    let (max_age, inc_sub) = HstsStore::parse_hsts_header(header2).expect("parse");
    assert_eq!(max_age, 0);
    assert!(!inc_sub);
}
