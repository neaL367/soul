//! Chrome `DevTools` Protocol (CDP) JSON-RPC 2.0 message definitions.

use serde::{Deserialize, Serialize};

/// Incoming JSON-RPC CDP command request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdpRequest {
    /// Request sequence identifier.
    pub id: u64,
    /// Target CDP method (e.g. `"DOM.getDocument"`).
    pub method: String,
    /// Optional parameter payload.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// Outgoing JSON-RPC CDP response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdpResponse {
    /// Correlated request sequence identifier.
    pub id: u64,
    /// Result payload if call succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error message if call failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CdpResponse {
    /// Creates a successful CDP response.
    #[must_use]
    pub const fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error CDP response.
    #[must_use]
    pub const fn error(id: u64, message: String) -> Self {
        Self {
            id,
            result: None,
            error: Some(message),
        }
    }
}
