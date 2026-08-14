//! Image decoding subsystem supporting PNG, JPEG, WebP, GIF, and SVG formats.

pub mod decoder;
pub mod error;

pub use decoder::{DecodedImage, ImageDecoder};
pub use error::ImageError;
