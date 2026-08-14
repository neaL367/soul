//! Cross-Origin Resource Sharing (CORS) origin validation and header checks.

use std::collections::HashMap;
use url::Url;

/// Evaluator enforcing W3C CORS access controls.
pub struct CorsEvaluator;

impl CorsEvaluator {
    /// Validates whether a response permits access from `request_origin` based on CORS headers.
    #[must_use]
    pub fn is_allowed(request_origin: &Url, response_headers: &HashMap<String, String>) -> bool {
        let origin_str = request_origin.origin().ascii_serialization();

        // Check Access-Control-Allow-Origin header (case-insensitive key)
        let allow_origin = response_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("access-control-allow-origin"))
            .map(|(_, v)| v.trim());

        match allow_origin {
            Some("*") => true,
            Some(allowed) if allowed == origin_str => true,
            _ => false,
        }
    }
}
