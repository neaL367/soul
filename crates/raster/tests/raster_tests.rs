//! Integration tests for CPU rasterization from HTML and display lists to pixel buffers.

use css::{CascadeResolver, Origin, parse_stylesheet};
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};
use paint::DisplayListBuilder;
use raster::CpuRasterizer;

#[test]
fn test_end_to_end_html_to_pixel_buffer() {
    let html = r#"<html><body>
        <div id="card">Hello</div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        body {
            margin: 0px;
            padding: 0px;
        }
        #card {
            background-color: #ff0000;
            width: 200px;
            height: 100px;
            margin: 0px;
            padding: 0px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let card_id = doc.get_element_by_id("card").unwrap();
    let mut layout_box = build_box_tree(&doc, card_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 400.0, 300.0),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);

    let display_list = DisplayListBuilder::build(&layout_box, &std::collections::HashMap::new());
    let pixel_buffer = CpuRasterizer::rasterize(&display_list, 400, 300).expect("rasterize failed");

    assert_eq!(pixel_buffer.width, 400);
    assert_eq!(pixel_buffer.height, 300);

    // Coordinate (50, 50) is inside the red card
    let pixel_inside = pixel_buffer.get_pixel(50, 50).expect("pixel out of bounds");
    assert_eq!(pixel_inside, [255, 0, 0, 255]);

    // Coordinate (350, 250) is outside the red card (transparent background)
    let pixel_outside = pixel_buffer
        .get_pixel(350, 250)
        .expect("pixel out of bounds");
    assert_eq!(pixel_outside, [0, 0, 0, 0]);
}

#[test]
fn test_border_and_content_rasterization() {
    let html = r#"<html><body>
        <div id="target">Content</div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        body { margin: 0px; padding: 0px; }
        #target {
            background-color: #0000ff;
            color: #00ff00;
            border-width: 6px;
            width: 100px;
            height: 100px;
            margin: 0px;
            padding: 0px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let target_id = doc.get_element_by_id("target").unwrap();
    let mut layout_box = build_box_tree(&doc, target_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 300.0, 300.0),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);

    let display_list = DisplayListBuilder::build(&layout_box, &std::collections::HashMap::new());
    let pixel_buffer = CpuRasterizer::rasterize(&display_list, 300, 300).expect("rasterize failed");

    // Pixel at (2, 2) is within the 6px top/left green border
    let border_pixel = pixel_buffer.get_pixel(2, 2).expect("pixel out of bounds");
    assert_eq!(border_pixel, [0, 255, 0, 255]);

    // Pixel at (50, 50) is within the blue content box
    let content_pixel = pixel_buffer.get_pixel(50, 50).expect("pixel out of bounds");
    assert_eq!(content_pixel, [0, 0, 255, 255]);
}

#[test]
fn test_opacity_alpha_blending() {
    let html = r#"<html><body>
        <div id="opaque_box"></div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        body { margin: 0px; }
        #opaque_box {
            opacity: 0.5;
            background-color: #ff0000;
            width: 100px;
            height: 100px;
            margin: 0px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let box_id = doc.get_element_by_id("opaque_box").unwrap();
    let mut layout_box = build_box_tree(&doc, box_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 200.0, 200.0),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);

    let display_list = DisplayListBuilder::build(&layout_box, &std::collections::HashMap::new());
    let pixel_buffer = CpuRasterizer::rasterize(&display_list, 200, 200).expect("rasterize failed");

    let pixel = pixel_buffer.get_pixel(50, 50).expect("pixel out of bounds");
    // Red color with 50% opacity in premultiplied RGBA: (255 * 0.5 = 128)
    assert_eq!(pixel[0], 128);
    assert_eq!(pixel[1], 0);
    assert_eq!(pixel[2], 0);
    assert_eq!(pixel[3], 128);
}
