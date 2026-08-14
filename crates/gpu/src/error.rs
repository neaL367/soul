//! Error types for GPU initialization, surface management, and texture allocation.

use thiserror::Error;

/// Errors arising from GPU device and surface operations.
#[derive(Debug, Error)]
pub enum GpuError {
    /// No compatible graphics adapter was found on the system.
    #[error("No compatible GPU adapter found")]
    NoAdapter,

    /// Requesting the GPU device failed.
    #[error("GPU device request failed: {0}")]
    DeviceRequestFailed(String),

    /// GPU surface presentation or acquisition failure.
    #[error("Surface error: {0}")]
    Surface(String),

    /// GPU texture creation or memory allocation error.
    #[error("Texture creation failed: {0}")]
    TextureCreation(String),

    /// The GPU device was lost or reset by the OS.
    #[error("GPU device was lost")]
    DeviceLost,
}
