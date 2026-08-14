//! 2D CPU software rasterizer powered by `tiny-skia`.

use crate::buffer::PixelBuffer;
use crate::error::RasterError;
use layout::{EdgeSizes, Rect};
use paint::{DisplayItem, DisplayList};
use tiny_skia::{
    BlendMode, Color as SkiaColor, FilterQuality, IntSize, Paint, Pixmap, PixmapPaint,
    Rect as SkiaRect, Transform,
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

        let mut pixmap = Pixmap::new(width, height)
            .ok_or(RasterError::PixmapAllocationFailed { width, height })?;

        let mut opacity_stack = vec![1.0f32];

        for item in &display_list.items {
            match item {
                DisplayItem::PushOpacity { opacity } => {
                    let current = *opacity_stack.last().unwrap_or(&1.0);
                    opacity_stack.push(current * opacity.clamp(0.0, 1.0));
                }
                DisplayItem::PopOpacity => {
                    if opacity_stack.len() > 1 {
                        opacity_stack.pop();
                    }
                }
                DisplayItem::DrawRect { rect, color } => {
                    let opacity = *opacity_stack.last().unwrap_or(&1.0);
                    paint_rect(&mut pixmap, *rect, *color, opacity);
                }
                DisplayItem::DrawBorder {
                    rect,
                    widths,
                    color,
                } => {
                    let opacity = *opacity_stack.last().unwrap_or(&1.0);
                    paint_border(&mut pixmap, *rect, *widths, *color, opacity);
                }
                DisplayItem::DrawText { rect, color, .. } => {
                    let opacity = *opacity_stack.last().unwrap_or(&1.0);
                    paint_text_placeholder(&mut pixmap, *rect, *color, opacity);
                }
                DisplayItem::DrawImage {
                    rect,
                    width,
                    height,
                    pixels,
                } => {
                    let opacity = *opacity_stack.last().unwrap_or(&1.0);
                    let placement = ImagePlacement {
                        x: rect.x,
                        y: rect.y,
                        dest_width: rect.width,
                        dest_height: rect.height,
                        natural_width: *width,
                        natural_height: *height,
                        opacity,
                    };
                    draw_image(&mut pixmap, &placement, pixels);
                }
                DisplayItem::PushClip { .. } | DisplayItem::PopClip => {}
            }
        }

        Ok(PixelBuffer::from_raw(width, height, pixmap.take()))
    }
}

/// Computes the effective alpha for a color under the current opacity stack.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn effective_alpha(color: css::Color, opacity: f32) -> u8 {
    ((f32::from(color.a)) * opacity).round() as u8
}

/// Fills a solid rectangle.
fn paint_rect(pixmap: &mut Pixmap, rect: Rect, color: css::Color, opacity: f32) {
    let eff_a = effective_alpha(color, opacity);
    if eff_a == 0 {
        return;
    }

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, eff_a);

    if let Some(skia_rect) = SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height) {
        pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
    }
}

/// Draws four-sided box borders.
fn paint_border(
    pixmap: &mut Pixmap,
    rect: Rect,
    widths: EdgeSizes,
    color: css::Color,
    opacity: f32,
) {
    let eff_a = effective_alpha(color, opacity);
    if eff_a == 0 {
        return;
    }

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, eff_a);

    // Top border
    if widths.top > 0.0
        && let Some(r) = SkiaRect::from_xywh(rect.x, rect.y, rect.width, widths.top)
    {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
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
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }
    // Left border
    let inner_h = (rect.height - widths.top - widths.bottom).max(0.0);
    if widths.left > 0.0
        && let Some(r) = SkiaRect::from_xywh(rect.x, rect.y + widths.top, widths.left, inner_h)
    {
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
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
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
    }
}

/// Placeholder text rendering: fills the text bounding box (glyph rasterization
/// through `text-shaping` is outstanding work).
fn paint_text_placeholder(pixmap: &mut Pixmap, rect: Rect, color: css::Color, opacity: f32) {
    let eff_a = effective_alpha(color, opacity);
    if eff_a == 0 {
        return;
    }

    let mut paint = Paint::default();
    paint.set_color(SkiaColor::from_rgba8(color.r, color.g, color.b, eff_a));

    if let Some(skia_rect) = SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height) {
        pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
    }
}

/// Destination placement for a decoded image draw.
struct ImagePlacement {
    /// Destination top-left x in layout pixels.
    x: f32,
    /// Destination top-left y in layout pixels.
    y: f32,
    /// Destination width in layout pixels.
    dest_width: f32,
    /// Destination height in layout pixels.
    dest_height: f32,
    /// Natural bitmap width in pixels.
    natural_width: u32,
    /// Natural bitmap height in pixels.
    natural_height: u32,
    /// Composited opacity.
    opacity: f32,
}

/// Draws a decoded RGBA bitmap scaled into the destination rectangle.
fn draw_image(pixmap: &mut Pixmap, placement: &ImagePlacement, pixels: &[u8]) {
    let Some(size) = IntSize::from_wh(placement.natural_width, placement.natural_height) else {
        return;
    };
    let Some(src) = Pixmap::from_vec(pixels.to_vec(), size) else {
        return;
    };

    let paint = PixmapPaint {
        opacity: placement.opacity.clamp(0.0, 1.0),
        blend_mode: BlendMode::SourceOver,
        quality: FilterQuality::Bilinear,
    };

    // Scale natural image dimensions to the layout box: the destination rect is
    // the natural-size rect at the origin, transformed by translate(x,y)*scale.
    #[allow(clippy::cast_precision_loss)]
    let sx = if placement.natural_width > 0 {
        placement.dest_width / (placement.natural_width as f32)
    } else {
        1.0
    };
    #[allow(clippy::cast_precision_loss)]
    let sy = if placement.natural_height > 0 {
        placement.dest_height / (placement.natural_height as f32)
    } else {
        1.0
    };
    let transform = Transform::from_translate(placement.x, placement.y).pre_scale(sx, sy);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pixmap.draw_pixmap(0, 0, src.as_ref(), &paint, transform, None);
}
