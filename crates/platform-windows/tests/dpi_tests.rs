//! Integration tests for Windows DPI scaling and monitor bounds conversion.

use platform_windows::{BASELINE_DPI, DpiScale, MonitorBounds};

#[test]
fn test_dpi_scale_factors() {
    let scale_100 = DpiScale::from_dpi_value(96);
    assert!((scale_100.dpi() - BASELINE_DPI).abs() < f64::EPSILON);
    assert!((scale_100.scale_factor() - 1.0).abs() < f64::EPSILON);

    let scale_150 = DpiScale::from_dpi_value(144);
    assert!((scale_150.scale_factor() - 1.5).abs() < f64::EPSILON);

    let scale_200 = DpiScale::from_dpi_value(192);
    assert!((scale_200.scale_factor() - 2.0).abs() < f64::EPSILON);
}

#[test]
fn test_coordinate_conversions() {
    let scale = DpiScale::from_dpi_value(144); // 150% scaling

    // 150 physical pixels -> 100 logical pixels
    let logical = scale.physical_to_logical(150.0);
    assert!((logical - 100.0).abs() < f64::EPSILON);

    // 100 logical pixels -> 150 physical pixels
    let physical = scale.logical_to_physical(100.0);
    assert!((physical - 150.0).abs() < f64::EPSILON);

    assert_eq!(scale.physical_to_logical_u32(1920), 1280);
    assert_eq!(scale.logical_to_physical_u32(1280), 1920);
}

#[test]
fn test_multi_monitor_bounds() {
    let primary_4k = MonitorBounds {
        x: 0,
        y: 0,
        width: 3840,
        height: 2160,
        dpi: 192, // 200% scale
        is_primary: true,
    };

    assert_eq!(primary_4k.logical_width(), 1920);
    assert_eq!(primary_4k.logical_height(), 1080);

    let secondary_1080p = MonitorBounds {
        x: 3840,
        y: 0,
        width: 1920,
        height: 1080,
        dpi: 96, // 100% scale
        is_primary: false,
    };

    assert_eq!(secondary_1080p.logical_width(), 1920);
    assert_eq!(secondary_1080p.logical_height(), 1080);
}
