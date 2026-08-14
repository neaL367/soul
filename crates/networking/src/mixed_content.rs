//! Mixed content security policy enforcement blocking insecure HTTP subresources on HTTPS origins.

use url::Url;

/// Checks if fetching `resource_url` from document `doc_origin` constitutes insecure mixed content.
#[must_use]
pub fn is_insecure_mixed_content(doc_origin: &Url, resource_url: &Url) -> bool {
    doc_origin.scheme() == "https" && resource_url.scheme() == "http"
}
