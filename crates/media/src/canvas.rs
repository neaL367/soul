//! HTML5 Canvas 2D raster drawing context and state machine.

use crate::error::MediaError;
use raster::PixelBuffer;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// Canvas state snapshot saved and restored on the canvas stack.
#[derive(Debug, Clone)]
struct CanvasState {
    fill_color: Color,
    stroke_color: Color,
    line_width: f32,
    transform: Transform,
}

/// HTML5 Canvas 2D drawing state and pixel buffer.
pub struct Canvas2DContext {
    width: u32,
    height: u32,
    pixmap: Pixmap,
    fill_color: Color,
    stroke_color: Color,
    line_width: f32,
    transform: Transform,
    path_builder: PathBuilder,
    state_stack: Vec<CanvasState>,
}

impl Canvas2DContext {
    /// Creates a new `Canvas2DContext` with the specified pixel dimensions.
    ///
    /// # Errors
    ///
    /// Returns `MediaError::CanvasError` if pixel buffer allocation fails.
    pub fn new(width: u32, height: u32) -> Result<Self, MediaError> {
        let pixmap = Pixmap::new(width.max(1), height.max(1)).ok_or_else(|| {
            MediaError::CanvasError(format!("failed to allocate {width}x{height} canvas"))
        })?;

        Ok(Self {
            width,
            height,
            pixmap,
            fill_color: Color::BLACK,
            stroke_color: Color::BLACK,
            line_width: 1.0,
            transform: Transform::identity(),
            path_builder: PathBuilder::new(),
            state_stack: Vec::new(),
        })
    }

    /// Saves the current drawing state onto the state stack.
    pub fn save(&mut self) {
        self.state_stack.push(CanvasState {
            fill_color: self.fill_color,
            stroke_color: self.stroke_color,
            line_width: self.line_width,
            transform: self.transform,
        });
    }

    /// Restores the most recently saved drawing state from the stack.
    pub fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.fill_color = state.fill_color;
            self.stroke_color = state.stroke_color;
            self.line_width = state.line_width;
            self.transform = state.transform;
        }
    }

    /// Sets the active fill style RGBA color (components 0.0 to 1.0).
    pub fn set_fill_style(&mut self, r: f32, g: f32, b: f32, a: f32) {
        if let Some(col) = Color::from_rgba(r, g, b, a) {
            self.fill_color = col;
        }
    }

    /// Sets the active stroke style RGBA color (components 0.0 to 1.0).
    pub fn set_stroke_style(&mut self, r: f32, g: f32, b: f32, a: f32) {
        if let Some(col) = Color::from_rgba(r, g, b, a) {
            self.stroke_color = col;
        }
    }

    /// Sets the active stroke line width in pixels.
    pub fn set_line_width(&mut self, width: f32) {
        self.line_width = if width > 0.1 { width } else { 0.1 };
    }

    /// Translates the current coordinate transformation matrix.
    pub fn translate(&mut self, tx: f32, ty: f32) {
        self.transform = self.transform.post_translate(tx, ty);
    }

    /// Scales the current coordinate transformation matrix.
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.transform = self.transform.post_scale(sx, sy);
    }

    /// Resets the current sub-path list.
    pub fn begin_path(&mut self) {
        self.path_builder = PathBuilder::new();
    }

    /// Moves the sub-path starting point to `(x, y)`.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.path_builder.move_to(x, y);
    }

    /// Connects the current point to `(x, y)` with a straight line.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.path_builder.line_to(x, y);
    }

    /// Closes the current sub-path with a straight line to the start.
    pub fn close_path(&mut self) {
        self.path_builder.close();
    }

    /// Fills the current path with the active fill style.
    pub fn fill(&mut self) {
        let builder_clone = self.path_builder.clone();
        if let Some(path) = builder_clone.finish() {
            let mut paint = Paint::default();
            paint.set_color(self.fill_color);
            self.pixmap
                .fill_path(&path, &paint, FillRule::Winding, self.transform, None);
        }
    }

    /// Strokes the current path with the active stroke style and line width.
    pub fn stroke(&mut self) {
        let builder_clone = self.path_builder.clone();
        if let Some(path) = builder_clone.finish() {
            let mut paint = Paint::default();
            paint.set_color(self.stroke_color);
            let stroke = Stroke {
                width: self.line_width,
                ..Default::default()
            };
            self.pixmap
                .stroke_path(&path, &paint, &stroke, self.transform, None);
        }
    }

    /// Fills a solid rectangle with the active fill style.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(self.fill_color);
            self.pixmap.fill_rect(rect, &paint, self.transform, None);
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
                    .stroke_path(&path, &paint, &stroke, self.transform, None);
            }
        }
    }

    /// Clears the pixels within the specified rectangle to transparent black.
    pub fn clear_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(Color::TRANSPARENT);
            paint.blend_mode = tiny_skia::BlendMode::Source;
            self.pixmap.fill_rect(rect, &paint, self.transform, None);
        }
    }

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
