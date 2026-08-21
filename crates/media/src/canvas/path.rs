//! Sub-path and vector drawing algorithms for Canvas 2D.

use super::Canvas2DContext;
use std::f32::consts::PI;
use tiny_skia::{FillRule, Paint, PathBuilder, Rect, Stroke};

impl Canvas2DContext {
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

    /// Adds a quadratic Bézier curve to the path.
    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.path_builder.quad_to(cpx, cpy, x, y);
    }

    /// Adds a cubic Bézier curve to the path.
    #[allow(clippy::similar_names)]
    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.path_builder.cubic_to(cp1x, cp1y, cp2x, cp2y, x, y);
    }

    /// Creates a new subpath in the shape of a rectangle.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if let Some(r) = Rect::from_xywh(x, y, w, h) {
            self.path_builder.push_rect(r);
        }
    }

    /// Adds a circular or elliptical arc to the path.
    #[allow(
        clippy::suboptimal_flops,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        anticlockwise: bool,
    ) {
        if radius <= 0.0 {
            self.path_builder.line_to(cx, cy);
            return;
        }

        let two_pi = 2.0 * PI;
        let mut sweep = end_angle - start_angle;

        if anticlockwise {
            if sweep > 0.0 {
                sweep -= two_pi * ((sweep / two_pi).floor() + 1.0);
            }
        } else if sweep < 0.0 {
            sweep += two_pi * ((-sweep / two_pi).floor() + 1.0);
        }

        let segments = ((sweep.abs() / (PI / 2.0)).ceil() as usize).max(1);
        let segment_sweep = sweep / (segments as f32);

        let start_x = cx + radius * start_angle.cos();
        let start_y = cy + radius * start_angle.sin();
        self.path_builder.line_to(start_x, start_y);

        let mut current_angle = start_angle;
        for _ in 0..segments {
            let next_angle = current_angle + segment_sweep;
            let half_delta = (next_angle - current_angle) / 2.0;
            let k = (4.0 / 3.0) * (half_delta / 2.0).tan();

            let p0_x = cx + radius * current_angle.cos();
            let p0_y = cy + radius * current_angle.sin();

            let p3_x = cx + radius * next_angle.cos();
            let p3_y = cy + radius * next_angle.sin();

            let p1_x = p0_x - k * radius * current_angle.sin();
            let p1_y = p0_y + k * radius * current_angle.cos();

            let p2_x = p3_x + k * radius * next_angle.sin();
            let p2_y = p3_y - k * radius * next_angle.cos();

            self.path_builder
                .cubic_to(p1_x, p1_y, p2_x, p2_y, p3_x, p3_y);
            current_angle = next_angle;
        }
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
}
