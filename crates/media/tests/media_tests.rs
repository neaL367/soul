//! Integration tests for Canvas 2D raster drawing and `MediaPipeline` state machine.

use media::{Canvas2DContext, MediaPipeline, MediaPlaybackState};
use raster::PixelBuffer;

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
fn test_media_pipeline_lifecycle_and_stepping() {
    let mut pipeline = MediaPipeline::new("https://example.com/video.mp4".to_string());
    assert_eq!(pipeline.state(), MediaPlaybackState::Idle);

    pipeline.set_duration(60.0);
    assert!((pipeline.duration() - 60.0).abs() < f64::EPSILON);

    pipeline.set_playback_rate(2.0);
    assert!((pipeline.playback_rate() - 2.0).abs() < f64::EPSILON);

    pipeline.set_volume(0.75);
    assert!((pipeline.volume() - 0.75).abs() < f32::EPSILON);

    pipeline.set_muted(true);
    assert!(pipeline.is_muted());

    pipeline.play();
    assert_eq!(pipeline.state(), MediaPlaybackState::Playing);

    pipeline.step_time(5.0); // 5s * 2x = 10s
    assert!((pipeline.position() - 10.0).abs() < f64::EPSILON);

    pipeline.seek(12.5);
    assert!((pipeline.position() - 12.5).abs() < f64::EPSILON);

    pipeline.pause();
    assert_eq!(pipeline.state(), MediaPlaybackState::Paused);

    pipeline.play();
    pipeline.step_time(30.0); // 12.5 + (30 * 2) = 72.5 -> clamped to 60 (Ended)
    assert_eq!(pipeline.state(), MediaPlaybackState::Ended);
    assert!((pipeline.position() - 60.0).abs() < f64::EPSILON);
}

#[test]
fn test_canvas_2d_state_stack_and_paths() {
    let mut ctx = Canvas2DContext::new(100, 100).expect("create canvas");

    // Save initial state (black fill)
    ctx.save();

    // Change fill to red
    ctx.set_fill_style(1.0, 0.0, 0.0, 1.0);
    ctx.begin_path();
    ctx.move_to(10.0, 10.0);
    ctx.line_to(40.0, 10.0);
    ctx.line_to(40.0, 40.0);
    ctx.close_path();
    ctx.fill();

    // Restore back to black
    ctx.restore();

    let buf = ctx.to_pixel_buffer();
    assert_eq!(buf.width, 100);
    assert_eq!(buf.height, 100);
}

#[test]
fn test_canvas_2d_draw_pixel_buffer_and_image_data() {
    let mut ctx = Canvas2DContext::new(64, 64).expect("create canvas");

    // Create a 16x16 solid blue buffer
    let mut src_buffer = PixelBuffer::new(16, 16);
    for chunk in src_buffer.data.chunks_exact_mut(4) {
        chunk[0] = 0; // R
        chunk[1] = 0; // G
        chunk[2] = 255; // B
        chunk[3] = 255; // A
    }

    // Draw buffer at (10, 10) with size 16x16
    ctx.draw_pixel_buffer(&src_buffer, 10.0, 10.0, 16.0, 16.0)
        .expect("draw pixel buffer");

    // Extract image data
    let extracted = ctx.get_image_data(10, 10, 16, 16).expect("get image data");
    assert_eq!(extracted.width, 16);
    assert_eq!(extracted.height, 16);
    assert_eq!(extracted.data[0], 0);
    assert_eq!(extracted.data[2], 255); // B

    // Test put_image_data directly
    let mut red_buffer = PixelBuffer::new(8, 8);
    for chunk in red_buffer.data.chunks_exact_mut(4) {
        chunk[0] = 255; // R
        chunk[1] = 0;
        chunk[2] = 0;
        chunk[3] = 255;
    }

    ctx.put_image_data(&red_buffer, 0, 0)
        .expect("put image data");
    let pixels = ctx.pixel_data();
    assert_eq!(pixels[0], 255); // R at (0,0)
    assert_eq!(pixels[1], 0);
}
