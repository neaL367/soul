//! 2D CPU software rasterizer powered by `tiny-skia`.

use crate::buffer::PixelBuffer;
use crate::error::RasterError;
use paint::{DisplayItem, DisplayList};
use tiny_skia::{Color as SkiaColor, Paint, Pixmap, Rect as SkiaRect, Transform};

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
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let eff_a = ((f32::from(color.a)) * opacity).round() as u8;
                    if eff_a == 0 {
                        continue;
                    }

                    let mut paint = Paint::default();
                    paint.set_color_rgba8(color.r, color.g, color.b, eff_a);

                    if let Some(skia_rect) =
                        SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height)
                    {
                        pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
                    }
                }
                DisplayItem::DrawBorder {
                    rect,
                    widths,
                    color,
                } => {
                    let opacity = *opacity_stack.last().unwrap_or(&1.0);
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let eff_a = ((f32::from(color.a)) * opacity).round() as u8;
                    if eff_a == 0 {
                        continue;
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
                        && let Some(r) =
                            SkiaRect::from_xywh(rect.x, rect.y + widths.top, widths.left, inner_h)
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
                DisplayItem::DrawText { rect, color, .. } => {
                    let opacity = *opacity_stack.last().unwrap_or(&1.0);
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let eff_a = ((f32::from(color.a)) * opacity).round() as u8;
                    if eff_a == 0 {
                        continue;
                    }

                    let mut paint = Paint::default();
                    paint.set_color(SkiaColor::from_rgba8(color.r, color.g, color.b, eff_a));

                    if let Some(skia_rect) =
                        SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height)
                    {
                        pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
                    }
                }
                DisplayItem::PushClip { .. } | DisplayItem::PopClip => {}
            }
        }

        Ok(PixelBuffer::from_raw(width, height, pixmap.take()))
    }
}
