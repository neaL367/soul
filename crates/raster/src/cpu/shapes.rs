//! Geometric shape painting primitives (rectangles, borders, box-shadows, clipping).

use layout::{EdgeSizes, Rect};
use tiny_skia::{Paint, Pixmap, Rect as SkiaRect, Transform};

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
        paint_rect(pixmap, shadow_rect, shadow.color, opacity, clip);
    }
}

/// Fills a solid rectangle with clipping applied.
pub(crate) fn paint_rect(
    pixmap: &mut Pixmap,
    rect: Rect,
    color: css::Color,
    opacity: f32,
    clip: Option<SkiaRect>,
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
        pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
    }
}

/// Draws four-sided box borders with clipping.
pub(crate) fn paint_border(
    pixmap: &mut Pixmap,
    rect: Rect,
    widths: EdgeSizes,
    color: css::Color,
    opacity: f32,
    clip: Option<SkiaRect>,
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
            pixmap.fill_rect(r, &paint, Transform::identity(), None);
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
