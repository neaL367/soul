//! Integration tests for chrome geometry, frame validation, and handle accessors.

use soul_backend_gpui::{
    CHROME_HEIGHT, GpuiSoulBackend, TAB_STRIP_HEIGHT, TOOLBAR_HEIGHT, page_coordinate,
};
use soul_ui::{SoulBackend, SoulError, ViewportFrame, WindowId, WindowSpec};

#[test]
fn test_chrome_geometry_constants_sum_to_chrome_height() {
    assert!((CHROME_HEIGHT - (TAB_STRIP_HEIGHT + TOOLBAR_HEIGHT)).abs() < 1e-3);
    assert!((CHROME_HEIGHT - 76.0).abs() < 1e-3);
}

#[test]
fn test_page_coordinate_maps_below_chrome_only() {
    // Inside the tab strip or toolbar: no page coordinate.
    assert_eq!(page_coordinate(10.0, 0.0), None);
    assert_eq!(page_coordinate(10.0, CHROME_HEIGHT), None);
    assert_eq!(page_coordinate(10.0, CHROME_HEIGHT - 0.1), None);

    // Just below the chrome: coordinates translated by the chrome height.
    let (x, y) = page_coordinate(120.0, CHROME_HEIGHT + 0.1).expect("page coordinate expected");
    assert!((x - 120.0).abs() < 1e-3);
    assert!((y - 0.1).abs() < 1e-3);

    let (_, y) = page_coordinate(50.0, CHROME_HEIGHT + 300.0).unwrap();
    assert!((y - 300.0).abs() < 1e-3);
}

#[test]
fn test_update_viewport_rejects_mismatched_frame() {
    let mut backend = GpuiSoulBackend::new();
    let window_id = backend
        .open_window(WindowSpec::default())
        .expect("open window");

    // 2x2 frame must carry 16 bytes of RGBA, not 15.
    let bad_frame = ViewportFrame::SoftwareRgba {
        width: 2,
        height: 2,
        pixels: vec![0; 15],
    };
    let err = backend
        .update_viewport(window_id, bad_frame)
        .expect_err("mismatched frame must be rejected");
    assert!(
        matches!(err, SoulError::PresentationFailed(_)),
        "unexpected error: {err}"
    );
}

#[test]
fn test_update_viewport_accepts_valid_frame_and_has_frame_tracks_it() {
    let mut backend = GpuiSoulBackend::new();
    let window_id = backend
        .open_window(WindowSpec::default())
        .expect("open window");
    let handle = backend.shared_handle();

    assert!(!handle.has_frame(window_id));

    let frame = ViewportFrame::SoftwareRgba {
        width: 2,
        height: 2,
        pixels: vec![0; 16],
    };
    backend
        .update_viewport(window_id, frame)
        .expect("valid frame should publish");
    assert!(handle.has_frame(window_id));
    assert!(!handle.has_frame(WindowId(999)));
}
