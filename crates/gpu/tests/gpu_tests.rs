//! Integration tests for GPU context initialization and texture management.

use gpu::{GpuContext, GpuTexture};

#[tokio::test]
async fn test_gpu_context_headless_init() {
    let ctx_res = GpuContext::new_headless().await;
    // On systems with DirectX/Vulkan hardware, context creation succeeds
    if let Ok(ctx) = ctx_res {
        assert!(ctx.device().limits().max_texture_dimension_2d > 0);

        // Allocate test texture
        let tex = GpuTexture::new_render_target(&ctx, 64, 64);
        assert_eq!(tex.width, 64);
        assert_eq!(tex.height, 64);

        // Upload test pixels
        let dummy_pixels = vec![255u8; 64 * 64 * 4];
        ctx.upload_rgba(&tex.texture, 64, 64, &dummy_pixels);
    }
}
