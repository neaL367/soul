//! Streaming response support for large transfers such as file downloads.

use crate::client::RawResponse;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Limited};
use std::collections::HashMap;
use url::Url;

/// Wire-size cap for a streamed download body.
///
/// Guards against unbounded disk consumption from a hostile or broken server
/// while still allowing legitimate large files. The body is streamed to disk,
/// so this is a disk budget, not a memory budget.
pub const MAX_DOWNLOAD_BYTES: usize = 8 * 1024 * 1024 * 1024; // 8 GiB

/// A fetched response whose body is still streamable, never buffered whole.
pub struct StreamingResponse {
    /// The final URL after redirect resolution.
    pub url: Url,
    /// HTTP status code of the final response.
    pub status_code: u16,
    /// Response headers lower-cased to ASCII.
    pub headers: HashMap<String, String>,
    body: BoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>,
}

impl StreamingResponse {
    /// The `Content-Length` header, if present and parseable.
    #[must_use]
    pub fn content_length(&self) -> Option<u64> {
        self.headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
    }

    /// Consumes the response, returning its streamable body.
    #[must_use]
    pub fn into_body(self) -> BoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>> {
        self.body
    }
}

/// Builds a [`StreamingResponse`] from a raw response, applying the download
/// wire-size cap to the body.
pub(crate) fn build_streaming_response(response: RawResponse, url: Url) -> StreamingResponse {
    let status_code = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|s| (name.as_str().to_ascii_lowercase(), s.to_string()))
        })
        .collect();

    let body = Limited::new(response.into_body(), MAX_DOWNLOAD_BYTES).boxed();

    StreamingResponse {
        url,
        status_code,
        headers,
        body,
    }
}
