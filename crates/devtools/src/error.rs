//! Error types for Developer Tools, Chrome `DevTools` Protocol, and inspector queries.

use thiserror::Error;

/// Errors arising during `DevTools` inspection or CDP protocol handling.
#[derive(Debug, Error)]
pub enum DevToolsError {
    /// JSON-RPC serialization or deserialization error.
    #[error("DevTools JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Method requested via CDP is unrecognized or unsupported.
    #[error("Unknown CDP method: {0}")]
    UnknownMethod(String),

    /// Invalid parameters supplied in CDP request.
    #[error("Invalid CDP params: {0}")]
    InvalidParams(String),

    /// Node or resource requested not found.
    #[error("Resource not found: {0}")]
    NotFound(String),
}
