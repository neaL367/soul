//! Error types for the download manager and Mark of the Web attachments.

use thiserror::Error;

/// Errors that can occur during file download operations.
#[derive(Debug, Error)]
pub enum DownloadError {
    /// File I/O or disk write failure.
    #[error("Download I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Network transfer error.
    #[error("Download network error: {0}")]
    Network(#[from] networking::NetworkError),

    /// Invalid or malformed download URL.
    #[error("Invalid download URL: {0}")]
    InvalidUrl(String),

    /// Download with specified ID was not found.
    #[error("Download not found: {0}")]
    NotFound(u64),
}
