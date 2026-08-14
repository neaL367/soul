//! Composited surface layers with independent transforms, opacity, and damage tracking.

use crate::damage::DamageTracker;
use raster::PixelBuffer;
use tiny_skia::{PixmapMut, Rect, Transform};

/// An independent visual layer composited onto the final page frame.
pub struct CompositorLayer {
    /// Layer identifier.
    pub id: u64,
    /// Layer bounds in logical pixels.
    pub bounds: Rect,
    /// Layer opacity level (0.0 to 1.0).
    pub opacity: f32,
    /// Rasterized pixel buffer backing this layer.
    pub pixel_buffer: Option<PixelBuffer>,
    /// Accumulated damage tracker for this layer.
    pub damage: DamageTracker,
}

impl CompositorLayer {
    /// Creates a new `CompositorLayer` with the specified bounds.
    #[must_use]
    pub const fn new(id: u64, bounds: Rect) -> Self {
        Self {
            id,
            bounds,
            opacity: 1.0,
            pixel_buffer: None,
            damage: DamageTracker::new(),
        }
    }

    /// Sets the layer opacity (clamped between 0.0 and 1.0).
    pub const fn set_opacity(&mut self, opacity: f32) {
        if opacity < 0.0 {
            self.opacity = 0.0;
        } else if opacity > 1.0 {
            self.opacity = 1.0;
        } else {
            self.opacity = opacity;
        }
    }

    /// Sets the backing raster buffer for this layer and marks the entire layer as damaged.
    pub fn set_pixel_buffer(&mut self, buffer: PixelBuffer) {
        self.pixel_buffer = Some(buffer);
        self.damage.add_damage(self.bounds);
    }

    /// Composites this layer's pixel buffer into a destination pixmap at the specified offset.
    pub fn composite_to(&self, dest: &mut PixmapMut, offset_x: f32, offset_y: f32) {
        if let Some(buf) = &self.pixel_buffer {
            let Some(src_pixmap) =
                tiny_skia::PixmapRef::from_bytes(&buf.data, buf.width, buf.height)
            else {
                return;
            };

            let paint = tiny_skia::PixmapPaint {
                opacity: self.opacity,
                ..Default::default()
            };

            let transform =
                Transform::from_translate(self.bounds.x() + offset_x, self.bounds.y() + offset_y);

            dest.draw_pixmap(0, 0, src_pixmap, &paint, transform, None);
        }
    }
}
