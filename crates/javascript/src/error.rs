//! Error types for JavaScript execution and runtime operations.

use thiserror::Error;

/// Errors that can occur during JavaScript compilation or execution.
#[derive(Debug, Error)]
pub enum JsError {
    /// JavaScript evaluation or syntax error.
    #[error("JavaScript evaluation error: {0}")]
    EvaluationError(String),
    /// Event loop or job queue execution error.
    #[error("JavaScript event loop error: {0}")]
    EventLoopError(String),
}
