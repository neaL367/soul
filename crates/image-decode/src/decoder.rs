//! Image decoding pipeline for raster bitmaps and SVG vector graphics.

use crate::error::ImageError;

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

/// High-performance image decoder supporting PNG, JPEG, WebP, GIF, and SVG.
pub struct ImageDecoder;

impl ImageDecoder {
    /// Decodes raw raster image bytes (PNG, JPEG, WebP, GIF) into raw RGBA pixels.
    ///
    /// # Errors
    ///
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

    /// Parses and rasterizes vector SVG graphics into RGBA pixels at specified dimensions.
    ///
    /// # Errors
    ///
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
