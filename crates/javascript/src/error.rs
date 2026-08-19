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
    /// The script exceeded the maximum accepted size and was rejected before
    /// parsing, so a hostile page cannot force unbounded parser/memory work.
    #[error("script of {0} bytes exceeds the maximum of {1} bytes")]
    ScriptTooLarge(usize, usize),
}
