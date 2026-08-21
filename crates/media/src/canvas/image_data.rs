//! Pixel buffer and image data manipulation for Canvas 2D.

use super::Canvas2DContext;
use crate::error::MediaError;
use raster::PixelBuffer;
use tiny_skia::{Paint, Pixmap, Rect};

impl Canvas2DContext {
    /// Draws a `PixelBuffer` (such as a video frame or image) onto the canvas surface.
    ///
    /// # Errors
    ///
    /// Returns `MediaError::CanvasError` if dimensions or coordinates are invalid.
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_pixel_buffer(
        &mut self,
        buffer: &PixelBuffer,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) -> Result<(), MediaError> {
        if buffer.width == 0 || buffer.height == 0 || buffer.data.is_empty() {
            return Ok(());
        }

        let src_pixmap = Pixmap::from_vec(
            buffer.data.clone(),
            tiny_skia::IntSize::from_wh(buffer.width, buffer.height).ok_or_else(|| {
                MediaError::CanvasError("invalid source buffer dimensions".to_string())
            })?,
        )
        .ok_or_else(|| MediaError::CanvasError("failed to create source pixmap".to_string()))?;

        let sx = if buffer.width > 0 {
            dw / buffer.width as f32
        } else {
            1.0
        };
        let sy = if buffer.height > 0 {
            dh / buffer.height as f32
        } else {
            1.0
        };

        let local_transform = self.transform.post_translate(dx, dy).post_scale(sx, sy);

        let paint = Paint {
            shader: tiny_skia::Pattern::new(
                src_pixmap.as_ref(),
                tiny_skia::SpreadMode::Pad,
                tiny_skia::FilterQuality::Bilinear,
                1.0,
                local_transform,
            ),
            ..Default::default()
        };

        if let Some(rect) = Rect::from_xywh(dx, dy, dw, dh) {
            self.pixmap.fill_rect(rect, &paint, self.transform, None);
        }

        Ok(())
    }

    /// Extracts a sub-rectangle of pixel data from the canvas into a `PixelBuffer`.
    ///
    /// # Errors
    ///
    /// Returns `MediaError::CanvasError` if bounds exceed canvas dimensions.
    pub fn get_image_data(
        &self,
        sx: u32,
        sy: u32,
        sw: u32,
        sh: u32,
    ) -> Result<PixelBuffer, MediaError> {
        if sx + sw > self.width || sy + sh > self.height {
            return Err(MediaError::CanvasError(format!(
                "get_image_data out of bounds: rect ({sx},{sy},{sw},{sh}) exceeds ({},{})",
                self.width, self.height
            )));
        }

        let mut out = PixelBuffer::new(sw, sh);
        let src_data = self.pixmap.data();

        for row in 0..sh {
            let src_y = sy + row;
            let src_start = ((src_y * self.width + sx) * 4) as usize;
            let src_end = src_start + (sw * 4) as usize;

            let dst_start = (row * sw * 4) as usize;
            let dst_end = dst_start + (sw * 4) as usize;

            if src_end <= src_data.len() && dst_end <= out.data.len() {
                out.data[dst_start..dst_end].copy_from_slice(&src_data[src_start..src_end]);
            }
        }

        Ok(out)
    }

    /// Writes raw pixel data from a `PixelBuffer` directly onto the canvas pixmap at `(dx, dy)`.
    ///
    /// # Errors
    ///
    /// Returns `MediaError::CanvasError` if the destination coordinates or buffer are out of bounds.
    pub fn put_image_data(
        &mut self,
        buffer: &PixelBuffer,
        dx: u32,
        dy: u32,
    ) -> Result<(), MediaError> {
        if dx + buffer.width > self.width || dy + buffer.height > self.height {
            return Err(MediaError::CanvasError(format!(
                "put_image_data out of bounds: destination ({dx},{dy}) with size ({},{}) exceeds canvas ({},{})",
                buffer.width, buffer.height, self.width, self.height
            )));
        }

        let dst_data = self.pixmap.data_mut();

        for row in 0..buffer.height {
            let dst_y = dy + row;
            let dst_start = ((dst_y * self.width + dx) * 4) as usize;
            let dst_end = dst_start + (buffer.width * 4) as usize;

            let src_start = (row * buffer.width * 4) as usize;
            let src_end = src_start + (buffer.width * 4) as usize;

            if dst_end <= dst_data.len() && src_end <= buffer.data.len() {
                dst_data[dst_start..dst_end].copy_from_slice(&buffer.data[src_start..src_end]);
            }
        }

        Ok(())
    }

    /// Converts the current canvas surface into an immutable `PixelBuffer`.
    #[must_use]
    pub fn to_pixel_buffer(&self) -> PixelBuffer {
        PixelBuffer::from_raw(self.width, self.height, self.pixmap.data().to_vec())
    }

    /// Returns the raw RGBA pixel byte slice.
    #[must_use]
    pub fn pixel_data(&self) -> &[u8] {
        self.pixmap.data()
    }
}
