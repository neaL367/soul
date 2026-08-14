//! Integration tests for damage tracking and layer composition.

use compositor::{CompositorLayer, DamageTracker};
use raster::PixelBuffer;
use tiny_skia::{Pixmap, Rect};

#[test]
#[allow(clippy::float_cmp)]
fn test_damage_tracker_union() {
    let mut tracker = DamageTracker::new();
    assert!(tracker.is_empty());
    assert!(tracker.union_bounds().is_none());

    let r1 = Rect::from_xywh(10.0, 10.0, 50.0, 50.0).unwrap();
    let r2 = Rect::from_xywh(40.0, 40.0, 60.0, 60.0).unwrap();

    tracker.add_damage(r1);
    tracker.add_damage(r2);

    assert!(!tracker.is_empty());
    let union = tracker.union_bounds().unwrap();
    assert_eq!(union.left(), 10.0);
    assert_eq!(union.top(), 10.0);
    assert_eq!(union.right(), 100.0);
    assert_eq!(union.bottom(), 100.0);

    tracker.clear();
    assert!(tracker.is_empty());
}

#[test]
fn test_compositor_layer_blit() {
    let bounds = Rect::from_xywh(0.0, 0.0, 20.0, 20.0).unwrap();
    let mut layer = CompositorLayer::new(1, bounds);
    layer.set_opacity(0.8);

    // Create 20x20 red pixel buffer
    let mut pixels = vec![0u8; 20 * 20 * 4];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = 255; // R
        chunk[3] = 255; // A
    }
    let buffer = PixelBuffer::from_raw(20, 20, pixels);
    layer.set_pixel_buffer(buffer);

    let mut target_pixmap = Pixmap::new(100, 100).unwrap();
    layer.composite_to(&mut target_pixmap.as_mut(), 10.0, 10.0);

    // Verify composite happened at offset (10, 10)
    let p = target_pixmap.pixel(10, 10).unwrap();
    assert!(p.red() > 0);
    assert!(p.alpha() > 0);
}
