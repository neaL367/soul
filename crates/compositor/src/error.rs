//! Error types for the compositor, layer assembly, and damage tracking.

use thiserror::Error;

/// Errors arising during layer composition or GPU surface presentations.
#[derive(Debug, Error)]
pub enum CompositorError {
    /// Device lost or GPU context invalidation.
    #[error("GPU device lost: {0}")]
    DeviceLost(String),

    /// Layer dimensions or surface allocation failure.
    #[error("Invalid layer bounds: {0}")]
    InvalidBounds(String),

    /// Raster buffer blit or composite error.
    #[error("Composition raster error: {0}")]
    RasterError(String),
}
