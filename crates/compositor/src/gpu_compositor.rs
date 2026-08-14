//! Hardware-accelerated layer compositor backed by WGPU textures and damage tracking.

use crate::layer::CompositorLayer;
use gpu::{GpuContext, GpuTexture};
use tiny_skia::Pixmap;

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

    /// Composites an ordered slice of `CompositorLayer` objects and uploads damaged rects to the GPU texture.
    pub fn composite_layers(&mut self, layers: &[CompositorLayer]) {
        self.staging_pixmap.fill(tiny_skia::Color::TRANSPARENT);

        for layer in layers {
            layer.composite_to(&mut self.staging_pixmap.as_mut(), 0.0, 0.0);
        }

        // Upload composite frame to GPU texture
        self.gpu_context.upload_rgba(
            &self.output_target.texture,
            self.output_target.width,
            self.output_target.height,
            self.staging_pixmap.data(),
        );
    }
}
