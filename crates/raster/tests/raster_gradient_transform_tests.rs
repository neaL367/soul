//! Integration tests for rasterizing linear/radial gradients and 2D transforms.

use css::{Color, ColorStop, Gradient, Transform2D};
use layout::Rect;
use paint::{DisplayItem, DisplayList};
use raster::CpuRasterizer;

#[test]
fn test_rasterize_linear_gradient() {
    let mut list = DisplayList::new();
    list.push(DisplayItem::DrawGradient {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        gradient: Gradient::Linear {
            angle_deg: 90.0, // left to right
            stops: vec![
                ColorStop {
                    position: 0.0,
                    color: Color::rgb(255, 0, 0),
                },
                ColorStop {
                    position: 1.0,
                    color: Color::rgb(0, 0, 255),
                },
            ],
        },
    });

    let buffer = CpuRasterizer::rasterize(&list, 100, 100).expect("rasterize succeeds");
    assert_eq!(buffer.width, 100);
    assert_eq!(buffer.height, 100);
    assert_eq!(buffer.data.len(), 100 * 100 * 4);

    // Leftmost pixel (x=0, y=50) should be predominantly red
    let left_r = buffer.data[50 * 100 * 4];
    let left_b = buffer.data[50 * 100 * 4 + 2];
    assert!(left_r > 200, "left side should be red");
    assert!(left_b < 50, "left side should have low blue");

    // Rightmost pixel (x=99, y=50) should be predominantly blue
    let right_r = buffer.data[(50 * 100 + 99) * 4];
    let right_b = buffer.data[(50 * 100 + 99) * 4 + 2];
    assert!(right_b > 200, "right side should be blue");
    assert!(right_r < 50, "right side should have low red");
}

#[test]
fn test_rasterize_radial_gradient() {
    let mut list = DisplayList::new();
    list.push(DisplayItem::DrawGradient {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        gradient: Gradient::Radial {
            center: (0.5, 0.5),
            radius: 1.0,
            stops: vec![
                ColorStop {
                    position: 0.0,
                    color: Color::rgb(255, 255, 255),
                },
                ColorStop {
                    position: 1.0,
                    color: Color::rgb(0, 0, 0),
                },
            ],
        },
    });

    let buffer = CpuRasterizer::rasterize(&list, 100, 100).expect("rasterize succeeds");
    assert_eq!(buffer.width, 100);
    assert_eq!(buffer.height, 100);

    // Center pixel (50, 50) should be bright
    let center_r = buffer.data[(50 * 100 + 50) * 4];
    assert!(center_r > 200, "center should be bright white");
}

#[test]
fn test_rasterize_with_push_pop_transform() {
    let mut list = DisplayList::new();
    // Translate by 20, 20
    list.push(DisplayItem::PushTransform {
        transform: Transform2D::translate(20.0, 20.0),
        origin: (0.0, 0.0),
    });
    list.push(DisplayItem::DrawRect {
        rect: Rect::new(0.0, 0.0, 20.0, 20.0),
        color: Color::rgb(255, 0, 0),
    });
    list.push(DisplayItem::PopTransform);

    let buffer = CpuRasterizer::rasterize(&list, 100, 100).expect("rasterize succeeds");

    // Pixel at (0, 0) should be transparent/blank
    let p0_a = buffer.data[3];
    assert_eq!(p0_a, 0, "origin outside transformed box should be empty");

    // Pixel at (25, 25) (inside translated box 20..40, 20..40) should be red
    let idx = (25 * 100 + 25) * 4;
    assert_eq!(
        buffer.data[idx], 255,
        "inside transformed box should be red"
    );
    assert_eq!(
        buffer.data[idx + 3],
        255,
        "inside transformed box should be opaque"
    );
}
