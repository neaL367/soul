//! CSS 2D Transforms, Gradients, and Transitions data types and matrix operations.

use crate::properties::Color;

/// A single CSS transform function (CSS Transforms Module Level 1 §3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum TransformOp {
    /// `translate(tx, ty)` in layout pixels.
    Translate(f32, f32),
    /// `scale(sx, sy)` multipliers.
    Scale(f32, f32),
    /// `rotate(angle)` in radians clockwise.
    Rotate(f32),
    /// `skew(ax, ay)` in radians.
    Skew(f32, f32),
    /// `matrix(a, b, c, d, e, f)` 2D affine transformation matrix.
    Matrix(f32, f32, f32, f32, f32, f32),
}

/// 2D affine transformation matrix representation:
///
/// | a  c  e |
/// | b  d  f |
/// | 0  0  1 |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    /// Horizontal scaling / cosine.
    pub a: f32,
    /// Vertical shearing / sine.
    pub b: f32,
    /// Horizontal shearing / -sine.
    pub c: f32,
    /// Vertical scaling / cosine.
    pub d: f32,
    /// Horizontal translation.
    pub e: f32,
    /// Vertical translation.
    pub f: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform2D {
    /// Returns the identity matrix (no transform).
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Creates a pure translation matrix.
    #[must_use]
    pub const fn translate(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    /// Creates a pure scaling matrix.
    #[must_use]
    pub const fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Creates a pure clockwise rotation matrix from an angle in radians.
    #[must_use]
    pub fn rotate(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Creates a skew transformation matrix from angles in radians.
    #[must_use]
    pub fn skew(ax: f32, ay: f32) -> Self {
        Self {
            a: 1.0,
            b: ay.tan(),
            c: ax.tan(),
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Multiplies this matrix by another: `self * other`.
    #[must_use]
    pub fn multiply(&self, other: &Self) -> Self {
        Self {
            a: self.a.mul_add(other.a, self.c * other.b),
            b: self.b.mul_add(other.a, self.d * other.b),
            c: self.a.mul_add(other.c, self.c * other.d),
            d: self.b.mul_add(other.c, self.d * other.d),
            e: self.a.mul_add(other.e, self.c.mul_add(other.f, self.e)),
            f: self.b.mul_add(other.e, self.d.mul_add(other.f, self.f)),
        }
    }

    /// Transforms a 2D coordinate point `(x, y)` using this affine matrix.
    #[must_use]
    pub const fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        let nx = self.a * x + self.c * y + self.e;
        let ny = self.b * x + self.d * y + self.f;
        (nx, ny)
    }

    /// Combines a list of [`TransformOp`] operations from left-to-right into a single matrix.
    #[must_use]
    #[allow(clippy::many_single_char_names)]
    pub fn from_operations(ops: &[TransformOp]) -> Self {
        let mut result = Self::identity();
        for op in ops {
            let m = match op {
                TransformOp::Translate(tx, ty) => Self::translate(*tx, *ty),
                TransformOp::Scale(sx, sy) => Self::scale(*sx, *sy),
                TransformOp::Rotate(rad) => Self::rotate(*rad),
                TransformOp::Skew(ax, ay) => Self::skew(*ax, *ay),
                TransformOp::Matrix(a, b, c, d, e, f) => Self {
                    a: *a,
                    b: *b,
                    c: *c,
                    d: *d,
                    e: *e,
                    f: *f,
                },
            };
            result = result.multiply(&m);
        }
        result
    }
}

/// A color stop in a CSS gradient with normalized position (0.0 to 1.0).
#[derive(Debug, Clone, PartialEq)]
pub struct ColorStop {
    /// Normalized position along the gradient line (0.0 to 1.0).
    pub position: f32,
    /// Color at this stop.
    pub color: Color,
}

/// CSS Gradient definitions (CSS Images Module Level 3 §3).
#[derive(Debug, Clone, PartialEq)]
pub enum Gradient {
    /// Linear gradient with angle in degrees (0deg = to top, 90deg = to right, 180deg = to bottom).
    Linear {
        /// Gradient angle in degrees clockwise from vertical upward.
        angle_deg: f32,
        /// Ordered list of color stops.
        stops: Vec<ColorStop>,
    },
    /// Radial gradient radiating from a center point.
    Radial {
        /// Center coordinate (normalized 0.0-1.0 or pixel relative).
        center: (f32, f32),
        /// Ending shape radius.
        radius: f32,
        /// Ordered list of color stops.
        stops: Vec<ColorStop>,
    },
}

/// Timing function for CSS Transitions and Animations (CSS Easing Functions Level 1 §3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimingFunction {
    /// `linear` (constant rate).
    Linear,
    /// `ease` (cubic-bezier(0.25, 0.1, 0.25, 1.0)).
    Ease,
    /// `ease-in` (cubic-bezier(0.42, 0.0, 1.0, 1.0)).
    EaseIn,
    /// `ease-out` (cubic-bezier(0.0, 0.0, 0.58, 1.0)).
    EaseOut,
    /// `ease-in-out` (cubic-bezier(0.42, 0.0, 0.58, 1.0)).
    EaseInOut,
    /// `cubic-bezier(x1, y1, x2, y2)`.
    CubicBezier(f32, f32, f32, f32),
}

/// Single CSS transition property definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// Property name (e.g. `"opacity"`, `"transform"`, `"all"`).
    pub property: String,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    /// Timing function.
    pub timing_function: TimingFunction,
    /// Delay in milliseconds before beginning transition.
    pub delay_ms: f32,
}
