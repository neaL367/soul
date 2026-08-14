//! Integration tests for GPU context initialization and texture management.

use gpu::{GpuContext, GpuError, GpuTexture};

#[tokio::test]
async fn test_gpu_context_headless_init() {
    match GpuContext::new_headless().await {
        Ok(ctx) => {
            assert!(ctx.device().limits().max_texture_dimension_2d > 0);

            // Allocate test texture
            let tex = GpuTexture::new_render_target(&ctx, 64, 64);
            assert_eq!(tex.width, 64);
            assert_eq!(tex.height, 64);

            // Upload test pixels
            let dummy_pixels = vec![255u8; 64 * 64 * 4];
            ctx.upload_rgba(&tex.texture, 64, 64, &dummy_pixels);
        }
        Err(GpuError::NoAdapter | GpuError::DeviceRequestFailed(_)) => {
            // Explicitly valid failure mode on environments lacking hardware DXGI/Vulkan drivers
        }
        Err(other) => panic!("Unexpected GPU init failure: {other:?}"),
    }
}
