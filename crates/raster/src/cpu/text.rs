//! Text and glyph run rasterization using `text_shaping` and fallback placeholders.

use super::shapes::{effective_alpha, intersect_rect};
use layout::Rect;
use tiny_skia::{Paint, Pixmap, Rect as SkiaRect, Transform};

/// Text rendering layout and style placement descriptor.
pub(crate) struct TextPlacement<'a> {
    pub rect: Rect,
    pub text: &'a str,
    pub color: css::Color,
    pub font_size: f32,
    pub font_family: &'a str,
    pub is_bold: bool,
    pub opacity: f32,
    pub clip: Option<SkiaRect>,
}

/// Rasterizes shaped text content onto the pixmap using cosmic-text glyph rasterization.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(crate) fn paint_shaped_text(pixmap: &mut Pixmap, placement: &TextPlacement<'_>) {
    let eff_a = effective_alpha(placement.color, placement.opacity);
    if eff_a == 0 || placement.text.is_empty() || placement.font_size <= 0.0 {
        return;
    }

    let origin_x = placement.rect.x;
    let origin_y = placement.rect.y;
    let pixmap_w = pixmap.width() as i32;
    let pixmap_h = pixmap.height() as i32;

    let mut glyph_rendered = false;

    text_shaping::rasterize_text_to_callback(
        placement.text,
        placement.font_family,
        placement.font_size,
        placement.is_bold,
        (
            placement.color.r,
            placement.color.g,
            placement.color.b,
            eff_a,
        ),
        |gx, gy, gw, gh, gcolor| {
            glyph_rendered = true;
            let dest_x = (origin_x as i32) + gx;
            let dest_y = (origin_y as i32) + gy;

            if dest_x >= pixmap_w || dest_y >= pixmap_h || gw == 0 || gh == 0 {
                return;
            }

            let Some(sub_rect) =
                SkiaRect::from_xywh(dest_x as f32, dest_y as f32, gw as f32, gh as f32)
            else {
                return;
            };

            let final_rect = if let Some(clip_rect) = placement.clip {
                let Some(r) = intersect_rect(sub_rect, clip_rect) else {
                    return;
                };
                r
            } else {
                sub_rect
            };

            let mut paint = Paint::default();
            paint.set_color_rgba8(gcolor.r(), gcolor.g(), gcolor.b(), gcolor.a());
            pixmap.fill_rect(final_rect, &paint, Transform::identity(), None);
        },
    );

    if !glyph_rendered {
        paint_text_placeholder(
            pixmap,
            placement.rect,
            placement.color,
            placement.opacity,
            placement.clip,
        );
    }
}

/// Draws a subtle placeholder for text glyph runs when no font face is matched.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn paint_text_placeholder(
    pixmap: &mut Pixmap,
    rect: Rect,
    color: css::Color,
    opacity: f32,
    clip: Option<SkiaRect>,
) {
    let eff_a = ((f32::from(color.a)) * opacity * 0.85).round() as u8;
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
