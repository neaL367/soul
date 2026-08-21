//! Image pixel scaling and blitting onto CPU pixmap surfaces.

use tiny_skia::{BlendMode, FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

/// Geometry and opacity descriptor for blitting a decoded image.
pub(crate) struct ImagePlacement {
    pub x: f32,
    pub y: f32,
    pub dest_width: f32,
    pub dest_height: f32,
    pub natural_width: u32,
    pub natural_height: u32,
    pub opacity: f32,
}

/// Draws an RGBA image onto the pixmap applying scaling and opacity.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub(crate) fn draw_image(pixmap: &mut Pixmap, placement: &ImagePlacement, pixels: &[u8]) {
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
