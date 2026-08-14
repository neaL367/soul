//! Network, HTTP protocol, and TLS error types.

use thiserror::Error;

/// Errors originating in the networking subsystem.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// The URL scheme is not supported (only `http` and `https` are supported).
    #[error("Unsupported URL scheme: {0}")]
    UnsupportedScheme(String),

    /// Missing host in URL.
    #[error("Missing host in URL: {0}")]
    MissingHost(String),

    /// DNS lookup failed for host.
    #[error("DNS resolution failed for '{0}': {1}")]
    DnsLookupFailed(String, std::io::Error),

    /// No IP address found for host.
    #[error("No IP addresses found for host: {0}")]
    NoAddressesFound(String),

    /// TCP connection failed.
    #[error("TCP connection failed to '{0}': {1}")]
    ConnectionFailed(String, std::io::Error),

    /// TLS configuration or handshake error.
    #[error("TLS error: {0}")]
    TlsError(String),

    /// Hyper / HTTP protocol error.
    #[error("HTTP protocol error: {0}")]
    HttpError(#[from] hyper::Error),

    /// Invalid HTTP header or request formatting.
    #[error("HTTP formatting error: {0}")]
    HttpFormatError(#[from] http::Error),

    /// IO error during payload transfer.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Request timed out.
    #[error("Network request timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// IPC transport communication error.
    #[error("IPC transport error: {0}")]
    TransportError(String),
}
