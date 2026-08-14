//! Error types for Windows sandboxing, Job Objects, and restricted tokens.

use thiserror::Error;

/// Errors arising during sandbox configuration or process isolation.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// Underlying Windows Win32 API failure.
    #[error("Windows API error: {0}")]
    Win32(#[from] windows::core::Error),

    /// Process launch or I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Process handle is invalid or unassigned.
    #[error("Invalid process handle: {0}")]
    InvalidHandle(String),

    /// Security policy or token restriction error.
    #[error("Security token configuration error: {0}")]
    TokenError(String),

    /// Limit or quota configuration out of bounds.
    #[error("Invalid resource limit: {0}")]
    InvalidLimit(String),
}
