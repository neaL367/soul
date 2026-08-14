//! Error types for IPC transport, serialization, and stream framing.

use thiserror::Error;

/// Errors that can occur during IPC communication and message framing.
#[derive(Debug, Error)]
pub enum IpcError {
    /// Error during message payload serialization or deserialization.
    #[error("IPC serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Underlying I/O error on stream or pipe.
    #[error("IPC I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Incoming frame payload exceeds maximum configured size.
    #[error("IPC frame size {size} exceeds maximum limit of {max} bytes")]
    FrameTooLarge {
        /// Attempted frame size in bytes.
        size: usize,
        /// Maximum allowed limit in bytes.
        max: usize,
    },

    /// The remote endpoint disconnected or closed the connection stream.
    #[error("IPC connection closed by peer")]
    ConnectionClosed,

    /// In-memory asynchronous channel closed.
    #[error("IPC in-memory channel disconnected")]
    ChannelClosed,

    /// Message format or header validation failed.
    #[error("Invalid IPC message: {0}")]
    InvalidMessage(String),
}
