//! Error types for database and persistent storage operations.

use thiserror::Error;

/// Errors that can occur during storage and database interactions.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Underlying `SQLite` engine error.
    #[error("SQLite database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// URL parsing error.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Missing or corrupted data.
    #[error("Storage data error: {0}")]
    InvalidData(String),

    /// Mutex lock contention or poisoning.
    #[error("Database lock error: {0}")]
    LockError(String),
}
