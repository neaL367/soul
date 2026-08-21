//! Geometric shape painting primitives (rectangles, gradients, borders, box-shadows, clipping).

use layout::{EdgeSizes, Rect};
use tiny_skia::{
    GradientStop, LinearGradient, Paint, Pixmap, Point, RadialGradient, Rect as SkiaRect,
    SpreadMode, Transform,
};

/// Computes rectangular intersection between two rectangles.
#[must_use]
pub(crate) fn intersect_rect(a: SkiaRect, b: SkiaRect) -> Option<SkiaRect> {
    let left = a.left().max(b.left());
    let top = a.top().max(b.top());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    if right > left && bottom > top {
        SkiaRect::from_ltrb(left, top, right, bottom)
    } else {
        None
    }
}

/// Computes the effective alpha for a color under the current opacity stack.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub(crate) fn effective_alpha(color: css::Color, opacity: f32) -> u8 {
    ((f32::from(color.a)) * opacity).round() as u8
}

pub(crate) fn paint_box_shadows(
    pixmap: &mut Pixmap,
    rect: Rect,
    shadows: &[css::BoxShadow],
    opacity: f32,
    clip: Option<SkiaRect>,
    transform: Transform,
) {
    for shadow in shadows {
        if shadow.inset {
            continue;
        }
        let expand = shadow.spread_radius;
        let shadow_rect = Rect::new(
            rect.x + shadow.offset_x - expand,
            rect.y + shadow.offset_y - expand,
            expand.mul_add(2.0, rect.width),
            expand.mul_add(2.0, rect.height),
        );
        paint_rect(pixmap, shadow_rect, shadow.color, opacity, clip, transform);
    }
}

/// Fills a solid rectangle with clipping and transformation applied.
pub(crate) fn paint_rect(
    pixmap: &mut Pixmap,
    rect: Rect,
    color: css::Color,
    opacity: f32,
    clip: Option<SkiaRect>,
    transform: Transform,
) {
    let eff_a = effective_alpha(color, opacity);
    if eff_a == 0 {
        return;
    }

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, eff_a);

    if let Some(mut skia_rect) = SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height) {
        if let Some(clip_rect) = clip {
            if let Some(clipped) = intersect_rect(skia_rect, clip_rect) {
                skia_rect = clipped;
            } else {
                return;
            }
        }
        pixmap.fill_rect(skia_rect, &paint, transform, None);
    }
}

/// Fills a rectangle with a CSS linear or radial gradient.
pub(crate) fn paint_gradient(
    pixmap: &mut Pixmap,
    rect: Rect,
    gradient: &css::Gradient,
    opacity: f32,
    clip: Option<SkiaRect>,
    transform: Transform,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    let Some(mut skia_rect) = SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height) else {
        return;
    };

    if let Some(clip_rect) = clip {
        if let Some(clipped) = intersect_rect(skia_rect, clip_rect) {
            skia_rect = clipped;
        } else {
            return;
        }
    }

    let mut paint = Paint::default();

    match gradient {
        css::Gradient::Linear { angle_deg, stops } => {
            let skia_stops = convert_stops(stops, opacity);
            if skia_stops.len() < 2 {
                return;
            }

            let cx = rect.width.mul_add(0.5, rect.x);
            let cy = rect.height.mul_add(0.5, rect.y);
            let rad = angle_deg.to_radians();
            let dx = rad.sin() * rect.width * 0.5;
            let dy = -rad.cos() * rect.height * 0.5;

            let start = Point::from_xy(cx - dx, cy - dy);
            let end = Point::from_xy(cx + dx, cy + dy);

            if let Some(shader) = LinearGradient::new(
                start,
                end,
                skia_stops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
                pixmap.fill_rect(skia_rect, &paint, transform, None);
            }
        }
        css::Gradient::Radial {
            center,
            radius,
            stops,
        } => {
            let skia_stops = convert_stops(stops, opacity);
            if skia_stops.len() < 2 {
                return;
            }

            let cx = center.0.mul_add(rect.width, rect.x);
            let cy = center.1.mul_add(rect.height, rect.y);
            let r = *radius * rect.width.max(rect.height) * 0.5;
            let pt = Point::from_xy(cx, cy);

            if let Some(shader) = RadialGradient::new(
                pt,
                pt,
                r.max(0.001),
                skia_stops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
                pixmap.fill_rect(skia_rect, &paint, transform, None);
            }
        }
    }
}

fn convert_stops(stops: &[css::ColorStop], opacity: f32) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|s| {
            let eff_a = effective_alpha(s.color, opacity);
            let c = tiny_skia::Color::from_rgba8(s.color.r, s.color.g, s.color.b, eff_a);
            GradientStop::new(s.position.clamp(0.0, 1.0), c)
        })
        .collect()
}

/// Draws four-sided box borders with clipping and transformation.
pub(crate) fn paint_border(
    pixmap: &mut Pixmap,
    rect: Rect,
    widths: EdgeSizes,
    color: css::Color,
    opacity: f32,
    clip: Option<SkiaRect>,
    transform: Transform,
) {
    let eff_a = effective_alpha(color, opacity);
    if eff_a == 0 {
        return;
    }

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, eff_a);

    let draw_sub_rect = |pixmap: &mut Pixmap, r: SkiaRect| {
        let final_r = clip.map_or(Some(r), |clip_rect| intersect_rect(r, clip_rect));
        if let Some(r) = final_r {
            pixmap.fill_rect(r, &paint, transform, None);
        }
    };

    // Top border
    if widths.top > 0.0
        && let Some(r) = SkiaRect::from_xywh(rect.x, rect.y, rect.width, widths.top)
    {
        draw_sub_rect(pixmap, r);
    }
    // Bottom border
    if widths.bottom > 0.0
        && let Some(r) = SkiaRect::from_xywh(
            rect.x,
            rect.y + rect.height - widths.bottom,
            rect.width,
            widths.bottom,
        )
    {
        draw_sub_rect(pixmap, r);
    }
    // Left border
    let inner_h = (rect.height - widths.top - widths.bottom).max(0.0);
    if widths.left > 0.0
        && let Some(r) = SkiaRect::from_xywh(rect.x, rect.y + widths.top, widths.left, inner_h)
    {
        draw_sub_rect(pixmap, r);
    }
    // Right border
    if widths.right > 0.0
        && let Some(r) = SkiaRect::from_xywh(
            rect.x + rect.width - widths.right,
            rect.y + widths.top,
            widths.right,
            inner_h,
        )
    {
        draw_sub_rect(pixmap, r);
    }
}
