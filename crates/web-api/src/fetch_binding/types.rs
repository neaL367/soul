//! Core data types for Fetch Web API (`FetchRequest`, `FetchResponse`, and handlers).

use std::sync::Arc;

/// Fetch response representation.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    /// HTTP status code (e.g. 200, 404).
    pub status: u16,
    /// HTTP status message (e.g. "OK", "Not Found").
    pub status_text: String,
    /// HTTP response headers as key-value pairs.
    pub headers: Vec<(String, String)>,
    /// Response body payload bytes.
    pub body: Vec<u8>,
    /// Final URL of the response.
    pub url: String,
}

impl Default for FetchResponse {
    fn default() -> Self {
        Self {
            status: 200,
            status_text: "OK".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            url: String::new(),
        }
    }
}

impl FetchResponse {
    /// Creates a 200 OK response with a text body.
    #[must_use]
    pub fn ok_text(url: impl Into<String>, body: impl Into<String>) -> Self {
        let body_str = body.into();
        Self {
            status: 200,
            status_text: "OK".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: body_str.into_bytes(),
            url: url.into(),
        }
    }

    /// Creates a response from status and body.
    #[must_use]
    pub fn from_status_and_body(status: u16, body: Vec<u8>) -> Self {
        let status_text = match status {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        }
        .to_string();

        Self {
            status,
            status_text,
            headers: Vec::new(),
            body,
            url: String::new(),
        }
    }
}

/// Outgoing HTTP request representation for `fetch()`.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// Target request URL.
    pub url: String,
    /// HTTP method (e.g. "GET", "POST").
    pub method: String,
    /// Outgoing headers.
    pub headers: Vec<(String, String)>,
    /// Optional body bytes.
    pub body: Option<Vec<u8>>,
}

impl Default for FetchRequest {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
        }
    }
}

impl FetchRequest {
    /// Creates a GET request for a URL.
    #[must_use]
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
        }
    }
}

/// Simple string-to-string fetch callback handler.
pub type FetchHandler = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// Rich HTTP request to response fetch callback handler.
pub type RichFetchHandler =
    Arc<dyn Fn(&FetchRequest) -> Result<FetchResponse, String> + Send + Sync>;
