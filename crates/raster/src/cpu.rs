//! 2D CPU software rasterizer powered by `tiny-skia`.

use crate::buffer::PixelBuffer;
use crate::error::RasterError;
use layout::{EdgeSizes, Rect};
use paint::{DisplayItem, DisplayList};
use tiny_skia::{
    BlendMode, FilterQuality, IntSize, Paint, Pixmap, PixmapPaint, Rect as SkiaRect, Transform,
};

/// 2D software CPU rasterizer rendering `DisplayList` items into a `PixelBuffer`.
pub struct CpuRasterizer;

impl CpuRasterizer {
    /// Rasterizes a `DisplayList` into an RGBA `PixelBuffer` with the given dimensions.
    ///
    /// # Errors
    ///
    /// Returns a `RasterError` if dimensions are invalid or pixmap allocation fails.
    pub fn rasterize(
        display_list: &DisplayList,
        width: u32,
        height: u32,
    ) -> Result<PixelBuffer, RasterError> {
        if width == 0 || height == 0 {
            return Err(RasterError::InvalidDimensions { width, height });
        }
        if (u64::from(width) * u64::from(height)) > (usize::MAX as u64) / 4 {
            return Err(RasterError::InvalidDimensions { width, height });
        }

        let mut pixmap = Pixmap::new(width, height)
            .ok_or(RasterError::PixmapAllocationFailed { width, height })?;

        let mut opacity_stack = vec![1.0f32];
        let mut clip_stack: Vec<Option<SkiaRect>> = vec![None];

        for item in &display_list.items {
            let active_clip = *clip_stack.last().unwrap_or(&None);
            let active_opacity = *opacity_stack.last().unwrap_or(&1.0);

            match item {
                DisplayItem::PushOpacity { opacity } => {
                    opacity_stack.push(active_opacity * opacity.clamp(0.0, 1.0));
                }
                DisplayItem::PopOpacity => {
                    if opacity_stack.len() > 1 {
                        opacity_stack.pop();
                    }
                }
                DisplayItem::PushClip { rect } => {
                    let item_skia = SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height);
                    let combined = match (active_clip, item_skia) {
                        (Some(p), Some(i)) => intersect_rect(p, i),
                        (None, Some(i)) => Some(i),
                        (Some(p), None) => Some(p),
                        (None, None) => None,
                    };
                    clip_stack.push(combined);
                }
                DisplayItem::PopClip => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                }
                DisplayItem::DrawRect { rect, color } => {
                    paint_rect(&mut pixmap, *rect, *color, active_opacity, active_clip);
                }
                DisplayItem::DrawBorder {
                    rect,
                    widths,
                    color,
                } => {
                    paint_border(
                        &mut pixmap,
                        *rect,
                        *widths,
                        *color,
                        active_opacity,
                        active_clip,
                    );
                }
                DisplayItem::DrawText {
                    rect,
                    text,
                    color,
                    font_size,
                    font_family,
                    is_bold,
                } => {
                    let placement = TextPlacement {
                        rect: *rect,
                        text,
                        color: *color,
                        font_size: *font_size,
                        font_family,
                        is_bold: *is_bold,
                        opacity: active_opacity,
                        clip: active_clip,
                    };
                    paint_shaped_text(&mut pixmap, &placement);
                }

                DisplayItem::DrawImage {
                    rect,
                    width,
                    height,
                    pixels,
                } => {
                    let placement = ImagePlacement {
                        x: rect.x,
                        y: rect.y,
                        dest_width: rect.width,
                        dest_height: rect.height,
                        natural_width: *width,
                        natural_height: *height,
                        opacity: active_opacity,
                    };
                    draw_image(&mut pixmap, &placement, pixels);
                }
            }
        }

        Ok(PixelBuffer::from_raw(width, height, pixmap.take()))
    }
}

/// Computes rectangular intersection between two rectangles.
fn intersect_rect(a: SkiaRect, b: SkiaRect) -> Option<SkiaRect> {
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
fn effective_alpha(color: css::Color, opacity: f32) -> u8 {
    ((f32::from(color.a)) * opacity).round() as u8
}

/// Fills a solid rectangle with clipping applied.
fn paint_rect(
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
fn paint_border(
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

/// Text rendering layout and style placement descriptor.
struct TextPlacement<'a> {
    rect: Rect,
    text: &'a str,
    color: css::Color,
    font_size: f32,
    font_family: &'a str,
    is_bold: bool,
    opacity: f32,
    clip: Option<SkiaRect>,
}

/// Rasterizes shaped text content onto the pixmap using cosmic-text glyph rasterization.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn paint_shaped_text(pixmap: &mut Pixmap, placement: &TextPlacement<'_>) {
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

/// Geometry and opacity descriptor for blitting a decoded image.
struct ImagePlacement {
    x: f32,
    y: f32,
    dest_width: f32,
    dest_height: f32,
    natural_width: u32,
    natural_height: u32,
    opacity: f32,
}

/// Draws an RGBA image onto the pixmap applying scaling and opacity.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn draw_image(pixmap: &mut Pixmap, placement: &ImagePlacement, pixels: &[u8]) {
    if pixels.is_empty() || placement.natural_width == 0 || placement.natural_height == 0 {
        return;
    }

    let expected_len = (placement.natural_width as usize) * (placement.natural_height as usize) * 4;
    if pixels.len() != expected_len {
        return;
    }

    let Some(src_size) = IntSize::from_wh(placement.natural_width, placement.natural_height) else {
        return;
    };
    let Some(src_pixmap) =
        tiny_skia::PixmapRef::from_bytes(pixels, src_size.width(), src_size.height())
    else {
        return;
    };

    let scale_x = placement.dest_width / (placement.natural_width as f32);
    let scale_y = placement.dest_height / (placement.natural_height as f32);
    let transform =
        Transform::from_scale(scale_x, scale_y).post_translate(placement.x, placement.y);

    let paint = PixmapPaint {
        opacity: placement.opacity,
        quality: FilterQuality::Bilinear,
        blend_mode: BlendMode::SourceOver,
    };

    pixmap.draw_pixmap(0, 0, src_pixmap, &paint, transform, None);
}
