//! CSP <meta http-equiv> enforcement smoke tests.

use html::parse_html;
use networking::{CspDirective, CspPolicy};
use soul_shell::engine::extract_csp_meta_policies;
use url::Url;

#[test]
fn extract_csp_meta_from_document() {
    let html = r#"<html><head>
        <meta http-equiv="Content-Security-Policy" content="script-src 'none'; img-src 'self'">
        <meta http-equiv="Content-Security-Policy" content="style-src 'self'">
    </head><body></body></html>"#;
    let doc = parse_html(html);
    let policies = extract_csp_meta_policies(&doc);
    assert_eq!(policies.len(), 2);
    // First policy should block script
    let url = Url::parse("https://example.com/script.js").unwrap();
    let doc_origin = Url::parse("https://example.com/").unwrap();
    assert!(!policies[0].allows(CspDirective::ScriptSrc, &url, &doc_origin));
    // Second policy has no script-src, so it allows script
    assert!(policies[1].allows(CspDirective::ScriptSrc, &url, &doc_origin));
    // Second policy style-src 'self' should allow same origin and block cross-origin
    assert!(policies[1].allows(CspDirective::StyleSrc, &url, &doc_origin));
    let cross_url = Url::parse("https://evil.com/style.css").unwrap();
    assert!(!policies[1].allows(CspDirective::StyleSrc, &cross_url, &doc_origin));
}

#[test]
fn csp_meta_blocks_inline_script() {
    let html = r#"<html><head>
        <meta http-equiv="Content-Security-Policy" content="script-src 'none'">
    </head><body><p id="a">original</p><script>document.getElementById('a').textContent = 'hacked';</script></body></html>"#;
    let doc = parse_html(html);
    let policies = extract_csp_meta_policies(&doc);
    assert_eq!(policies.len(), 1);
    let policy = &policies[0];
    // Inline should be blocked
    assert!(!policy.allows_inline(CspDirective::ScriptSrc));
    assert!(!policy.allows(
        CspDirective::ScriptSrc,
        &Url::parse("https://example.com/inline.js").unwrap(),
        &Url::parse("https://example.com/").unwrap()
    ));
}

#[test]
fn csp_meta_and_header_combined_enforces_both() {
    let header_policy = CspPolicy::parse("script-src 'self'");
    let meta_html = r#"<html><head>
        <meta http-equiv="Content-Security-Policy" content="script-src 'none'">
    </head></html>"#;
    let doc = parse_html(meta_html);
    let meta_policies = extract_csp_meta_policies(&doc);
    assert_eq!(meta_policies.len(), 1);
    let meta_policy = &meta_policies[0];

    // Header allows self, meta denies all -> combined should deny
    let url = Url::parse("https://example.com/script.js").unwrap();
    let origin = Url::parse("https://example.com/").unwrap();
    assert!(header_policy.allows(CspDirective::ScriptSrc, &url, &origin));
    assert!(!meta_policy.allows(CspDirective::ScriptSrc, &url, &origin));
    // Combined check (both must allow) should be false
    let combined_allows = header_policy.allows(CspDirective::ScriptSrc, &url, &origin)
        && meta_policy.allows(CspDirective::ScriptSrc, &url, &origin);
    assert!(!combined_allows);
}

#[test]
fn csp_meta_case_insensitive_http_equiv() {
    let html = r#"<html><head>
        <meta http-equiv="content-security-policy" content="script-src 'self'">
    </head></html>"#;
    let doc = parse_html(html);
    let policies = extract_csp_meta_policies(&doc);
    assert_eq!(policies.len(), 1);
}
