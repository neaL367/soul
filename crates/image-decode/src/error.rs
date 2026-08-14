//! Error types for raster image and SVG decoding.

use thiserror::Error;

/// Errors arising during image format parsing or SVG rasterization.
#[derive(Debug, Error)]
pub enum ImageError {
    /// Raster image decoding failed (PNG, JPEG, WebP, GIF).
    #[error("Raster image decode error: {0}")]
    RasterDecode(#[from] image::ImageError),

    /// Vector SVG parsing or rendering error.
    #[error("SVG decode error: {0}")]
    SvgDecode(String),

    /// Unsupported or unrecognized image format.
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),
}
