//! Error types for rasterization operations.

use thiserror::Error;

/// Errors that can occur during 2D pixel rasterization.
#[derive(Debug, Error)]
pub enum RasterError {
    /// Target raster dimensions were invalid (e.g. 0 width or height).
    #[error("Invalid raster buffer dimensions: {width}x{height}")]
    InvalidDimensions {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// Failed to allocate pixel buffer pixmap.
    #[error("Failed to allocate pixel pixmap for dimensions: {width}x{height}")]
    PixmapAllocationFailed {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
}
