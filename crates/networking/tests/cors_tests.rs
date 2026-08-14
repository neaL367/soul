//! Integration tests for CORS validation and mixed content security checks.

use networking::{CorsEvaluator, is_insecure_mixed_content};
use std::collections::HashMap;
use url::Url;

#[test]
fn test_cors_origin_evaluation() {
    let origin = Url::parse("https://app.example.com").unwrap();

    let mut headers = HashMap::new();
    headers.insert("access-control-allow-origin".to_string(), "*".to_string());
    assert!(CorsEvaluator::is_allowed(&origin, &headers));

    headers.insert(
        "access-control-allow-origin".to_string(),
        "https://app.example.com".to_string(),
    );
    assert!(CorsEvaluator::is_allowed(&origin, &headers));

    headers.insert(
        "access-control-allow-origin".to_string(),
        "https://other.com".to_string(),
    );
    assert!(!CorsEvaluator::is_allowed(&origin, &headers));
}

#[test]
fn test_mixed_content_detection() {
    let secure_page = Url::parse("https://bank.example.com/portal").unwrap();
    let insecure_asset = Url::parse("http://insecure.cdn.com/tracker.js").unwrap();
    let secure_asset = Url::parse("https://secure.cdn.com/style.css").unwrap();

    assert!(is_insecure_mixed_content(&secure_page, &insecure_asset));
    assert!(!is_insecure_mixed_content(&secure_page, &secure_asset));
}
