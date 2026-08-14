//! Networking subsystem providing HTTP/1.1, TLS, CORS, CSP evaluation, and out-of-process `NetworkService`.

pub mod client;
pub mod cors;
pub mod csp;
pub mod error;
pub mod mixed_content;
pub mod service;
pub mod types;

pub use client::HttpClient;
pub use cors::CorsEvaluator;
pub use csp::{CspDirective, CspPolicy, CspSource};
pub use error::NetworkError;
pub use mixed_content::is_insecure_mixed_content;
pub use service::NetworkService;
pub use types::{HttpRequest, HttpResponse};
