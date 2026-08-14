//! HTML5 Canvas 2D raster drawing context and state machine.

use crate::error::MediaError;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// HTML5 Canvas 2D drawing state and pixel buffer.
pub struct Canvas2DContext {
    width: u32,
    height: u32,
    pixmap: Pixmap,
    fill_color: Color,
    stroke_color: Color,
    line_width: f32,
}

impl Canvas2DContext {
    /// Creates a new `Canvas2DContext` with the specified pixel dimensions.
    ///
    /// # Errors
    ///
    /// Returns `MediaError::CanvasError` if pixel buffer allocation fails.
    pub fn new(width: u32, height: u32) -> Result<Self, MediaError> {
        let pixmap = Pixmap::new(width, height).ok_or_else(|| {
            MediaError::CanvasError(format!("failed to allocate {width}x{height} canvas"))
        })?;

        Ok(Self {
            width,
            height,
            pixmap,
            fill_color: Color::BLACK,
            stroke_color: Color::BLACK,
            line_width: 1.0,
        })
    }

    /// Sets the active fill style RGBA color (components 0.0 to 1.0).
    pub fn set_fill_style(&mut self, r: f32, g: f32, b: f32, a: f32) {
        if let Some(col) = Color::from_rgba(r, g, b, a) {
            self.fill_color = col;
        }
    }

    /// Sets the active stroke line width in pixels.
    pub fn set_line_width(&mut self, width: f32) {
        if width > 0.1 {
            self.line_width = width;
        } else {
            self.line_width = 0.1;
        }
    }

    /// Fills a solid rectangle with the active fill style.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(self.fill_color);
            self.pixmap
                .fill_rect(rect, &paint, Transform::identity(), None);
        }
    }

    /// Outlines a rectangle with the active stroke style and line width.
    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(self.stroke_color);
            let stroke = Stroke {
                width: self.line_width,
                ..Default::default()
            };

            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                self.pixmap
                    .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    /// Clears the pixels within the specified rectangle to transparent black.
    pub fn clear_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(Color::TRANSPARENT);
            paint.blend_mode = tiny_skia::BlendMode::Source;
            self.pixmap
                .fill_rect(rect, &paint, Transform::identity(), None);
        }
    }

    /// Returns the raw RGBA pixel byte slice.
    #[must_use]
    pub fn pixel_data(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Width of the canvas in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height of the canvas in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
}
