//! Image decoding subsystem supporting PNG, JPEG, WebP, GIF, and SVG formats.

pub mod decoder;
pub mod encoder;
pub mod error;

pub use decoder::{AnimatedImage, AnimationFrame, DecodedImage, ImageDecoder};
pub use encoder::encode_png;
pub use error::ImageError;
