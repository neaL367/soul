//! Coordinate transformations for Canvas 2D.

use super::Canvas2DContext;
use tiny_skia::Transform;

impl Canvas2DContext {
    /// Translates the current coordinate transformation matrix.
    pub fn translate(&mut self, tx: f32, ty: f32) {
        self.transform = self.transform.post_translate(tx, ty);
    }

    /// Scales the current coordinate transformation matrix.
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.transform = self.transform.post_scale(sx, sy);
    }

    /// Rotates the current transformation matrix clockwise by `angle_rad` radians.
    pub fn rotate(&mut self, angle_rad: f32) {
        let (sin, cos) = angle_rad.sin_cos();
        let rot = Transform::from_row(cos, sin, -sin, cos, 0.0, 0.0);
        self.transform = self.transform.post_concat(rot);
    }

    /// Multiplies the current transformation matrix with the given 2D affine matrix.
    #[allow(clippy::many_single_char_names)]
    pub fn transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        let mat = Transform::from_row(a, b, c, d, e, f);
        self.transform = self.transform.post_concat(mat);
    }

    /// Resets the current transformation matrix to identity.
    pub fn reset_transform(&mut self) {
        self.transform = Transform::identity();
    }
}
