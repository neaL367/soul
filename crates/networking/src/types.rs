//! HTTP request, response, and header data structures.

use bytes::Bytes;
use std::collections::HashMap;
use url::Url;

/// Standard HTTP request methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpMethod {
    /// GET method.
    #[default]
    Get,
    /// POST method.
    Post,
    /// HEAD method.
    Head,
    /// PUT method.
    Put,
    /// DELETE method.
    Delete,
}

impl HttpMethod {
    /// Returns the uppercase ASCII string representation of this HTTP method.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Head => "HEAD",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

/// Outgoing HTTP request specification.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Target destination URL.
    pub url: Url,
    /// HTTP method.
    pub method: HttpMethod,
    /// Request headers as key-value pairs.
    pub headers: Vec<(String, String)>,
    /// Optional request body payload.
    pub body: Option<Bytes>,
}

impl HttpRequest {
    /// Constructs a basic GET request for the given URL.
    #[must_use]
    pub const fn get(url: Url) -> Self {
        Self {
            url,
            method: HttpMethod::Get,
            headers: Vec::new(),
            body: None,
        }
    }

    /// Adds a header to the request.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Incoming HTTP response received from the network.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Final response URL (after following any redirects).
    pub url: Url,
    /// HTTP status code (e.g. 200, 404, 500).
    pub status_code: u16,
    /// HTTP response headers.
    pub headers: HashMap<String, String>,
    /// Raw `Set-Cookie` header values.
    pub set_cookies: Vec<String>,
    /// Response payload bytes.
    pub body: Bytes,
    /// MIME content type extracted from headers (defaults to `text/html`).
    pub mime_type: String,
}

impl HttpResponse {
    /// Returns `true` if the HTTP status code is 2xx.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }

    /// Looks up a response header value by case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers.get(&lower).map(String::as_str)
    }

    /// Decodes the response body bytes as a UTF-8 string.
    ///
    /// # Errors
    /// Returns an error if the body contains invalid UTF-8 sequences.
    pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.to_vec())
    }
}
