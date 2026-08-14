//! Integration tests for Canvas 2D raster drawing and `MediaPipeline` state machine.

use media::{Canvas2DContext, MediaPipeline, MediaPlaybackState};

#[test]
fn test_canvas_2d_fill_and_clear() {
    let mut ctx = Canvas2DContext::new(100, 100).expect("failed to create Canvas2D");
    assert_eq!(ctx.width(), 100);
    assert_eq!(ctx.height(), 100);

    // Set green fill
    ctx.set_fill_style(0.0, 1.0, 0.0, 1.0);
    ctx.fill_rect(0.0, 0.0, 50.0, 50.0);

    let pixels = ctx.pixel_data();
    // Verify first pixel is green
    assert_eq!(pixels[0], 0); // R
    assert_eq!(pixels[1], 255); // G
    assert_eq!(pixels[2], 0); // B
    assert_eq!(pixels[3], 255); // A

    // Clear rect
    ctx.clear_rect(0.0, 0.0, 50.0, 50.0);
    let cleared_pixels = ctx.pixel_data();
    assert_eq!(cleared_pixels[0], 0);
    assert_eq!(cleared_pixels[3], 0); // Transparent
}

#[test]
fn test_media_pipeline_lifecycle() {
    let mut pipeline = MediaPipeline::new("https://example.com/video.mp4".to_string());
    assert_eq!(pipeline.state(), MediaPlaybackState::Idle);

    pipeline.play();
    assert_eq!(pipeline.state(), MediaPlaybackState::Playing);

    pipeline.seek(12.5);
    assert!((pipeline.position() - 12.5).abs() < f64::EPSILON);

    pipeline.pause();
    assert_eq!(pipeline.state(), MediaPlaybackState::Paused);
}
