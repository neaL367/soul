//! Integration tests for Windows 11 window visual attributes and DPI queries.

use platform_windows::{
    BackdropType, query_window_dpi, set_immersive_dark_mode, set_system_backdrop,
};

#[test]
fn test_window_dpi_query_fallback() {
    let dpi = query_window_dpi(0);
    assert!((dpi.scale_factor() - 1.0).abs() < f64::EPSILON);
    assert!((dpi.dpi() - 96.0).abs() < f64::EPSILON);
}

#[test]
fn test_backdrop_type_variants() {
    assert_eq!(BackdropType::default(), BackdropType::Auto);
    assert_ne!(BackdropType::Mica, BackdropType::Acrylic);
    assert_ne!(BackdropType::MicaAlt, BackdropType::None);
}

#[test]
fn test_dwm_set_attributes_on_null_hwnd_handled_gracefully() {
    // Calling DWM APIs on invalid HWND (0) must return false and not crash or panic
    let dark_res = set_immersive_dark_mode(0, true);
    assert!(!dark_res);

    let mica_res = set_system_backdrop(0, BackdropType::Mica);
    assert!(!mica_res);
}
