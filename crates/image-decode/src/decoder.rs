//! Image decoding pipeline for raster bitmaps, animated sequences, and SVG vector graphics.

use crate::error::ImageError;
use image::AnimationDecoder;
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use std::io::Cursor;

/// Decoded image buffer containing raw 32-bit RGBA pixel bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA 8-bit per channel pixel byte array (stride = width * 4).
    pub rgba_pixels: Vec<u8>,
}

/// A single frame in an animated image sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationFrame {
    /// Width of this frame in pixels.
    pub width: u32,
    /// Height of this frame in pixels.
    pub height: u32,
    /// RGBA 8-bit pixel byte array for this frame.
    pub rgba_pixels: Vec<u8>,
    /// Display duration for this frame in milliseconds.
    pub duration_ms: u32,
}

/// An animated image containing an ordered sequence of frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimatedImage {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Ordered frames comprising the animation.
    pub frames: Vec<AnimationFrame>,
}

/// High-performance image decoder supporting PNG, JPEG, WebP, GIF, ICO, BMP, and SVG.
pub struct ImageDecoder;

impl ImageDecoder {
    /// Decodes raw raster image bytes into raw RGBA pixels.
    ///
    /// # Errors
    /// Returns `ImageError::RasterDecode` if format parsing fails.
    pub fn decode_raster(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        let img = image::load_from_memory(bytes)?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        Ok(DecodedImage {
            width,
            height,
            rgba_pixels: rgba.into_raw(),
        })
    }

    /// Automatically detects format (SVG vs raster) and decodes to RGBA pixels.
    ///
    /// # Errors
    /// Returns `ImageError` if neither SVG nor raster decoding succeeds.
    pub fn decode_auto(bytes: &[u8]) -> Result<DecodedImage, ImageError> {
        if is_svg(bytes) {
            Self::decode_svg(bytes, 0, 0)
        } else {
            Self::decode_raster(bytes)
        }
    }

    /// Decodes an animated image (GIF or animated WebP) into an `AnimatedImage`.
    ///
    /// # Errors
    /// Returns `ImageError` if animation decoding fails or format is unsupported.
    pub fn decode_animation(bytes: &[u8]) -> Result<AnimatedImage, ImageError> {
        let cursor = Cursor::new(bytes);

        let frames_result = if let Ok(decoder) = GifDecoder::new(cursor) {
            decoder.into_frames().collect_frames()
        } else {
            let cursor = Cursor::new(bytes);
            let decoder = WebPDecoder::new(cursor)
                .map_err(|e| ImageError::UnsupportedFormat(e.to_string()))?;
            decoder.into_frames().collect_frames()
        };

        let raw_frames = frames_result.map_err(ImageError::RasterDecode)?;
        if raw_frames.is_empty() {
            return Err(ImageError::UnsupportedFormat(
                "image contains no animation frames".to_string(),
            ));
        }

        let mut frames = Vec::with_capacity(raw_frames.len());
        let mut max_w = 0u32;
        let mut max_h = 0u32;

        for frame in raw_frames {
            let (numer, denom) = frame.delay().numer_denom_ms();
            let duration_ms = numer
                .saturating_add(denom / 2)
                .checked_div(denom)
                .unwrap_or(numer);

            let buffer = frame.into_buffer();
            let (w, h) = buffer.dimensions();
            max_w = max_w.max(w);
            max_h = max_h.max(h);

            frames.push(AnimationFrame {
                width: w,
                height: h,
                rgba_pixels: buffer.into_raw(),
                duration_ms,
            });
        }

        Ok(AnimatedImage {
            width: max_w,
            height: max_h,
            frames,
        })
    }

    /// Parses and rasterizes vector SVG graphics into RGBA pixels at specified dimensions.
    ///
    /// # Errors
    /// Returns `ImageError::SvgDecode` if SVG parsing or rendering fails.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn decode_svg(
        svg_data: &[u8],
        target_width: u32,
        target_height: u32,
    ) -> Result<DecodedImage, ImageError> {
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_data(svg_data, &opt)
            .map_err(|e| ImageError::SvgDecode(e.to_string()))?;

        let (w, h) = if target_width > 0 && target_height > 0 {
            (target_width, target_height)
        } else {
            let size = tree.size();
            (size.width().ceil() as u32, size.height().ceil() as u32)
        };

        let mut pixmap = tiny_skia::Pixmap::new(w, h)
            .ok_or_else(|| ImageError::SvgDecode(format!("failed to allocate {w}x{h} pixmap")))?;

        let svg_w = tree.size().width();
        let svg_h = tree.size().height();
        let scale_x = f32::from(w as u16) / svg_w;
        let scale_y = f32::from(h as u16) / svg_h;
        let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Ok(DecodedImage {
            width: w,
            height: h,
            rgba_pixels: pixmap.take(),
        })
    }
}

/// Sniffs whether the byte stream looks like SVG vector XML.
fn is_svg(bytes: &[u8]) -> bool {
    let prefix = std::str::from_utf8(&bytes[..bytes.len().min(512)]).unwrap_or("");
    let trimmed = prefix.trim_start();
    trimmed.starts_with("<svg")
        || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))
        || trimmed.starts_with("<!DOCTYPE svg")
}
