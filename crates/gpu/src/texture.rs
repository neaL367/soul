//! GPU texture abstractions and views for composited surface layers.

use crate::context::GpuContext;
use wgpu::{Texture, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor};

/// Wrapper holding an allocated GPU texture along with its default texture view.
pub struct GpuTexture {
    /// Texture resource width in pixels.
    pub width: u32,
    /// Texture resource height in pixels.
    pub height: u32,
    /// Underlying WGPU texture.
    pub texture: Texture,
    /// Default shader view targeting the entire texture.
    pub view: TextureView,
}

impl GpuTexture {
    /// Allocates a new 2D `GpuTexture` with standard render and copy usages.
    #[must_use]
    pub fn new_render_target(context: &GpuContext, width: u32, height: u32) -> Self {
        let usages = TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::COPY_SRC;

        let texture =
            context.create_texture_2d(width, height, TextureFormat::Rgba8UnormSrgb, usages);
        let view = texture.create_view(&TextureViewDescriptor::default());

        Self {
            width,
            height,
            texture,
            view,
        }
    }
}
