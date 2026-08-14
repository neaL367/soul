//! PNG encoding of raw RGBA pixel buffers (screenshot output, golden-image tests).

use crate::error::ImageError;
use image::{ImageFormat, RgbaImage};
use std::io::Cursor;

/// Encodes an 8-bit RGBA pixel buffer into a PNG byte stream.
///
/// The pixel slice must contain exactly `width * height * 4` bytes.
///
/// # Errors
///
/// Returns `ImageError::PngEncode` if the buffer dimensions are invalid or PNG encoding fails.
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ImageError> {
    let expected = width as usize * height as usize * 4;
    if pixels.len() != expected {
        return Err(ImageError::PngEncode(format!(
            "pixel buffer size mismatch: expected {expected} bytes for {width}x{height}, got {}",
            pixels.len()
        )));
    }

    let img = RgbaImage::from_raw(width, height, pixels.to_vec())
        .ok_or_else(|| ImageError::PngEncode(format!("invalid dimensions {width}x{height}")))?;

    let mut output = Vec::new();
    let mut cursor = Cursor::new(&mut output);
    img.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| ImageError::PngEncode(e.to_string()))?;

    Ok(output)
}
