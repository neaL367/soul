//! Hardware-accelerated layer compositor backed by WGPU textures and damage tracking.

use crate::layer::CompositorLayer;
use gpu::{GpuContext, GpuRect, GpuTexture};
use tiny_skia::{Pixmap, Rect};

/// Hardware-accelerated compositor orchestrating layer textures and partial damage uploads.
pub struct GpuCompositor {
    gpu_context: GpuContext,
    output_target: GpuTexture,
    staging_pixmap: Pixmap,
}

impl GpuCompositor {
    /// Creates a new `GpuCompositor` for the given dimensions.
    #[must_use]
    pub fn new(gpu_context: GpuContext, width: u32, height: u32) -> Self {
        let output_target = GpuTexture::new_render_target(&gpu_context, width, height);
        let staging_pixmap =
            Pixmap::new(width.max(1), height.max(1)).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());

        Self {
            gpu_context,
            output_target,
            staging_pixmap,
        }
    }

    /// Returns a reference to the composite output GPU texture.
    #[must_use]
    pub const fn output_target(&self) -> &GpuTexture {
        &self.output_target
    }

    /// Resizes the compositor output texture and internal staging buffer.
    pub fn resize(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        self.output_target = GpuTexture::new_render_target(&self.gpu_context, w, h);
        self.staging_pixmap = Pixmap::new(w, h).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    }

    /// Composites an ordered slice of `CompositorLayer` objects and uploads the full frame to the GPU texture.
    pub fn composite_layers(&mut self, layers: &[CompositorLayer]) {
        self.composite_layers_with_damage(layers, None);
    }

    /// Composites layers and uploads only the damaged subregion if present, or the full frame.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn composite_layers_with_damage(
        &mut self,
        layers: &[CompositorLayer],
        damage_rect: Option<Rect>,
    ) {
        self.staging_pixmap.fill(tiny_skia::Color::TRANSPARENT);

        for layer in layers {
            layer.composite_to(&mut self.staging_pixmap.as_mut(), 0.0, 0.0);
        }

        if let Some(r) = damage_rect {
            let x = (r.left().max(0.0).floor() as u32).min(self.output_target.width);
            let y = (r.top().max(0.0).floor() as u32).min(self.output_target.height);
            let right = (r.right().ceil() as u32).min(self.output_target.width);
            let bottom = (r.bottom().ceil() as u32).min(self.output_target.height);
            let w = right.saturating_sub(x);
            let h = bottom.saturating_sub(y);

            if w > 0 && h > 0 {
                let rect = GpuRect {
                    x,
                    y,
                    width: w,
                    height: h,
                };
                let stride = self.output_target.width;
                let offset = ((y as usize) * (stride as usize) + (x as usize)) * 4;
                if offset < self.staging_pixmap.data().len() {
                    let pixel_slice = &self.staging_pixmap.data()[offset..];
                    self.gpu_context.upload_rgba_rect(
                        &self.output_target.texture,
                        rect,
                        stride,
                        pixel_slice,
                    );
                    return;
                }
            }
        }

        // Full upload fallback
        self.gpu_context.upload_rgba(
            &self.output_target.texture,
            self.output_target.width,
            self.output_target.height,
            self.staging_pixmap.data(),
        );
    }
}
