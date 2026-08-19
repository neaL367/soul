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

#[test]
fn test_csp_host_source_is_not_a_string_prefix_match() {
    let policy = CspPolicy::parse("script-src https://api.example.com");
    let doc_origin = Url::parse("https://example.com/index.html").unwrap();

    // Exact host match is allowed.
    let exact = Url::parse("https://api.example.com/v1.js").unwrap();
    assert!(policy.allows(CspDirective::ScriptSrc, &exact, &doc_origin));

    // A *subdomain* of the allowed host is allowed per CSP3 host matching.
    let subdomain = Url::parse("https://cdn.api.example.com/v1.js").unwrap();
    assert!(policy.allows(CspDirective::ScriptSrc, &subdomain, &doc_origin));

    // A suffix-spoofing host must NOT be allowed: `api.example.com.evil.com`
    // used to pass because the full URL started with the source string.
    let spoofed = Url::parse("https://api.example.com.evil.com/v1.js").unwrap();
    assert!(!policy.allows(CspDirective::ScriptSrc, &spoofed, &doc_origin));

    // Unrelated host and bare-source form without scheme.
    let other = Url::parse("https://evil.com/x.js").unwrap();
    assert!(!policy.allows(CspDirective::ScriptSrc, &other, &doc_origin));

    let bare_policy = CspPolicy::parse("script-src example.com");
    let http_target = Url::parse("http://example.com/a.js").unwrap();
    assert!(bare_policy.allows(CspDirective::ScriptSrc, &http_target, &doc_origin));
    assert!(!bare_policy.allows(CspDirective::ScriptSrc, &other, &doc_origin));
}

#[test]
fn test_csp_host_source_with_port() {
    let policy = CspPolicy::parse("script-src https://api.example.com:8443");
    let doc_origin = Url::parse("https://example.com/index.html").unwrap();

    let correct_port = Url::parse("https://api.example.com:8443/x.js").unwrap();
    assert!(policy.allows(CspDirective::ScriptSrc, &correct_port, &doc_origin));

    // A different port on the same host is not covered by the ported source.
    let wrong_port = Url::parse("https://api.example.com:443/x.js").unwrap();
    assert!(!policy.allows(CspDirective::ScriptSrc, &wrong_port, &doc_origin));

    // A port-less source matches the scheme-default port of the target.
    let portless_policy = CspPolicy::parse("script-src api.example.com");
    let https_default = Url::parse("https://api.example.com/x.js").unwrap();
    assert!(portless_policy.allows(CspDirective::ScriptSrc, &https_default, &doc_origin));
}
