//! Integration tests for Content Security Policy (CSP Level 3) directives and enforcement.

use networking::{CspDirective, CspPolicy};
use url::Url;

#[test]
fn test_csp_self_origin_and_wildcard() {
    let policy = CspPolicy::parse(
        "default-src 'self'; script-src 'self' https://trusted.cdn.com; img-src *",
    );
    let doc_origin = Url::parse("https://example.com/index.html").unwrap();

    // Script from same origin: allowed
    let script_self = Url::parse("https://example.com/app.js").unwrap();
    assert!(policy.allows(CspDirective::ScriptSrc, &script_self, &doc_origin));

    // Script from trusted CDN: allowed
    let script_cdn = Url::parse("https://trusted.cdn.com/bundle.js").unwrap();
    assert!(policy.allows(CspDirective::ScriptSrc, &script_cdn, &doc_origin));

    // Script from untrusted third party: blocked
    let script_evil = Url::parse("https://evil.com/tracker.js").unwrap();
    assert!(!policy.allows(CspDirective::ScriptSrc, &script_evil, &doc_origin));

    // Images from anywhere: allowed
    let img_external = Url::parse("https://images.unsplash.com/photo.jpg").unwrap();
    assert!(policy.allows(CspDirective::ImgSrc, &img_external, &doc_origin));

    // Connect-src fallback to default-src ('self'):
    let connect_self = Url::parse("https://example.com/api/v1").unwrap();
    assert!(policy.allows(CspDirective::ConnectSrc, &connect_self, &doc_origin));

    let connect_evil = Url::parse("https://api.evil.com/exfiltrate").unwrap();
    assert!(!policy.allows(CspDirective::ConnectSrc, &connect_evil, &doc_origin));
}

#[test]
fn test_csp_nonce_and_violation_reporting() {
    let raw_policy = "script-src 'self' 'nonce-rAnd0m123'; style-src 'self'";
    let policy = CspPolicy::parse(raw_policy);
    let doc_origin = Url::parse("https://example.com/index.html").unwrap();

    // Nonce verification
    assert!(policy.allows_nonce(CspDirective::ScriptSrc, "rAnd0m123"));
    assert!(!policy.allows_nonce(CspDirective::ScriptSrc, "wrong_nonce"));

    // Violation report generation
    let blocked_url = Url::parse("https://evil.com/malicious.js").unwrap();
    let report = policy.create_violation_report(
        CspDirective::ScriptSrc,
        &blocked_url,
        &doc_origin,
        raw_policy,
    );

    assert_eq!(report.violated_directive, "script-src");
    assert_eq!(report.blocked_uri, "https://evil.com/malicious.js");
    assert_eq!(report.document_uri, "https://example.com/index.html");
    assert_eq!(report.original_policy, raw_policy);
}
