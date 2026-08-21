//! Text layout and rendering support for Canvas 2D.

use super::Canvas2DContext;
use tiny_skia::{Paint, PathBuilder, Rect, Stroke};

/// Measurement result for text in Canvas 2D.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Advance width in CSS pixels.
    pub width: f32,
    /// Distance from the horizontal baseline to the top of the bounding box.
    pub actual_bounding_box_ascent: f32,
    /// Distance from the horizontal baseline to the bottom of the bounding box.
    pub actual_bounding_box_descent: f32,
}

impl Canvas2DContext {
    /// Measures the dimensions of `text` according to the current canvas font configuration.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_text(&self, text: &str) -> TextMetrics {
        let approx_char_width = self.font_size * 0.6;
        let width = (text.chars().count() as f32) * approx_char_width;
        let ascent = self.font_size * 0.8;
        let descent = self.font_size * 0.2;
        TextMetrics {
            width,
            actual_bounding_box_ascent: ascent,
            actual_bounding_box_descent: descent,
        }
    }

    /// Fills the given text at the given `(x, y)` position.
    pub fn fill_text(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        if text.is_empty() {
            return;
        }
        let metrics = self.measure_text(text);
        let mut render_width = metrics.width;
        if let Some(mw) = max_width
            && mw > 0.0
            && mw < render_width
        {
            render_width = mw;
        }

        let ascent = self.font_size * 0.8;
        if let Some(rect) = Rect::from_xywh(x, y - ascent, render_width, self.font_size) {
            let mut paint = Paint::default();
            paint.set_color(self.fill_color);
            self.pixmap.fill_rect(rect, &paint, self.transform, None);
        }
    }

    /// Strokes the given text at the given `(x, y)` position.
    pub fn stroke_text(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>) {
        if text.is_empty() {
            return;
        }
        let metrics = self.measure_text(text);
        let mut render_width = metrics.width;
        if let Some(mw) = max_width
            && mw > 0.0
            && mw < render_width
        {
            render_width = mw;
        }

        let ascent = self.font_size * 0.8;
        if let Some(rect) = Rect::from_xywh(x, y - ascent, render_width, self.font_size) {
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
}
