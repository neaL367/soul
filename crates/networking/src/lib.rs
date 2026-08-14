//! Networking subsystem providing HTTP/1.1, TLS, and out-of-process `NetworkService`.

pub mod client;
pub mod error;
pub mod service;
pub mod types;

pub use client::HttpClient;
pub use error::NetworkError;
pub use service::NetworkService;
pub use types::{HttpRequest, HttpResponse};
