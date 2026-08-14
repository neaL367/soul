//! URL parsing, DNS resolution, TCP/QUIC, TLS, HTTP/1.1-HTTP/3, cookies, CORS, and CSP.

pub mod client;
pub mod error;
pub mod types;

pub use client::{HttpClient, HttpClientConfig};
pub use error::NetworkError;
pub use types::{HttpMethod, HttpRequest, HttpResponse};
