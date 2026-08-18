//! GPU context managing WGPU instance, adapter, device, and command queue.

use crate::error::GpuError;
use std::sync::Arc;
use wgpu::{
    Adapter, Device, DeviceDescriptor, Extent3d, Features, Instance, InstanceDescriptor, Limits,
    Origin3d, PowerPreference, Queue, RequestAdapterOptions, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};

/// Hardware GPU context providing device handles, queue dispatch, and resource allocation.
#[derive(Clone)]
pub struct GpuContext {
    instance: Arc<Instance>,
    adapter: Arc<Adapter>,
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl GpuContext {
    /// Asynchronously initializes a headless GPU context using default high-performance hardware.
    ///
    /// # Errors
    ///
    /// Returns `GpuError::NoAdapter` or `GpuError::DeviceRequestFailed` if hardware initialization fails.
    pub async fn new_headless() -> Result<Self, GpuError> {
        let instance = Instance::new(&InstanceDescriptor::default());

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Soul GPU Device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceRequestFailed(e.to_string()))?;

        Ok(Self {
            instance: Arc::new(instance),
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    /// Returns a reference to the `wgpu::Instance`.
    #[must_use]
    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    /// Returns a reference to the active `wgpu::Device`.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Returns a reference to the active `wgpu::Queue`.
    #[must_use]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Returns a reference to the `wgpu::Adapter`.
    #[must_use]
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    /// Allocates a 2D RGBA texture suitable for rendering and copying.
    #[must_use]
    pub fn create_texture_2d(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        usages: TextureUsages,
    ) -> Texture {
        self.device.create_texture(&TextureDescriptor {
            label: Some("Compositor 2D Texture"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: usages,
            view_formats: &[],
        })
    }

    /// Uploads an RGBA8 pixel byte array into the destination texture.
    pub fn upload_rgba(&self, texture: &Texture, width: u32, height: u32, pixels: &[u8]) {
        let rect = GpuRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        self.upload_rgba_rect(texture, rect, width, pixels);
    }

    /// Uploads a subregion (damage rect) of an RGBA8 pixel buffer into the destination texture.
    pub fn upload_rgba_rect(&self, texture: &Texture, rect: GpuRect, stride: u32, pixels: &[u8]) {
        if pixels.is_empty() || rect.width == 0 || rect.height == 0 {
            return;
        }

        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d {
                    x: rect.x,
                    y: rect.y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride * 4),
                rows_per_image: Some(rect.height),
            },
            Extent3d {
                width: rect.width,
                height: rect.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// 2D rectangular integer coordinates for GPU buffer uploads and damage regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GpuRect {
    /// Horizontal offset in pixels.
    pub x: u32,
    /// Vertical offset in pixels.
    pub y: u32,
    /// Rectangle width in pixels.
    pub width: u32,
    /// Rectangle height in pixels.
    pub height: u32,
}
