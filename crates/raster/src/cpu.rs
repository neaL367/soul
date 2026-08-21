//! 2D CPU software rasterizer powered by `tiny-skia`.

pub mod image;
pub mod shapes;
pub mod text;

use self::image::{ImagePlacement, draw_image};
use self::shapes::{intersect_rect, paint_border, paint_box_shadows, paint_gradient, paint_rect};
use self::text::{TextPlacement, paint_shaped_text};
use crate::buffer::PixelBuffer;
use crate::error::RasterError;
use paint::{DisplayItem, DisplayList};
use tiny_skia::{Pixmap, Rect as SkiaRect, Transform};

/// 2D software CPU rasterizer rendering `DisplayList` items into a `PixelBuffer`.
pub struct CpuRasterizer;

impl CpuRasterizer {
    /// Rasterizes a `DisplayList` into an RGBA `PixelBuffer` with the given dimensions.
    ///
    /// # Errors
    ///
    /// Returns a `RasterError` if dimensions are invalid or pixmap allocation fails.
    #[allow(clippy::too_many_lines)]
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
        let mut transform_stack: Vec<Transform> = vec![Transform::identity()];

        for item in &display_list.items {
            let active_clip = *clip_stack.last().unwrap_or(&None);
            let active_opacity = *opacity_stack.last().unwrap_or(&1.0);
            let active_transform = transform_stack
                .last()
                .copied()
                .unwrap_or_else(Transform::identity);

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
                DisplayItem::PushTransform { transform, origin } => {
                    let matrix = Transform::from_row(
                        transform.a,
                        transform.b,
                        transform.c,
                        transform.d,
                        transform.e,
                        transform.f,
                    );
                    let local = Transform::from_translate(origin.0, origin.1)
                        .post_concat(matrix)
                        .post_translate(-origin.0, -origin.1);
                    let combined = active_transform.post_concat(local);
                    transform_stack.push(combined);
                }
                DisplayItem::PopTransform => {
                    if transform_stack.len() > 1 {
                        transform_stack.pop();
                    }
                }
                DisplayItem::DrawBoxShadow { rect, shadows } => {
                    paint_box_shadows(
                        &mut pixmap,
                        *rect,
                        shadows,
                        active_opacity,
                        active_clip,
                        active_transform,
                    );
                }
                DisplayItem::DrawRect { rect, color } => {
                    paint_rect(
                        &mut pixmap,
                        *rect,
                        *color,
                        active_opacity,
                        active_clip,
                        active_transform,
                    );
                }
                DisplayItem::DrawGradient { rect, gradient } => {
                    paint_gradient(
                        &mut pixmap,
                        *rect,
                        gradient,
                        active_opacity,
                        active_clip,
                        active_transform,
                    );
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
                        active_transform,
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
