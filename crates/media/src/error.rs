//! Error types for audio/video playback and Canvas 2D raster operations.

use thiserror::Error;

/// Errors arising during media stream decoding or Canvas 2D operations.
#[derive(Debug, Error)]
pub enum MediaError {
    /// Format or codec decoding failure.
    #[error("Media decoding error: {0}")]
    DecodeError(String),

    /// Media source stream not found or invalid URL.
    #[error("Invalid media source: {0}")]
    InvalidSource(String),

    /// Canvas 2D drawing error.
    #[error("Canvas 2D error: {0}")]
    CanvasError(String),

    /// Unsupported media container format.
    #[error("Unsupported media format: {0}")]
    UnsupportedFormat(String),
}
