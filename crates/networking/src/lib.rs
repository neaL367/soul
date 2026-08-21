//! Networking subsystem providing HTTP/1.1, TLS, CORS, CSP evaluation, and out-of-process `NetworkService`.

pub mod client;
pub mod cors;
pub mod csp;
pub mod decompression;
pub mod dns;
pub mod error;
pub mod mixed_content;
pub mod network_client;
pub mod service;
pub mod streaming;
pub mod types;
pub mod websocket;

pub use client::{HttpClient, HttpClientConfig};
pub use cors::CorsEvaluator;
pub use csp::{CspDirective, CspPolicy, CspSource, CspViolationReport};
pub use decompression::decompress_payload;
pub use dns::DnsResolver;
pub use error::NetworkError;
pub use mixed_content::is_insecure_mixed_content;
pub use network_client::{NetworkClient, NetworkClientConfig};
pub use service::NetworkService;
pub use streaming::{MAX_DOWNLOAD_BYTES, StreamingResponse};
pub use types::{HttpRequest, HttpResponse};
pub use websocket::{Frame, OpCode, WebSocketClient, WebSocketMessage};
