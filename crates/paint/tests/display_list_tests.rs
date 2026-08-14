//! Integration tests for display list generation, CSS 2.1 Appendix E stacking order, and draw commands.

use css::{CascadeResolver, Color, Origin, parse_stylesheet};
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};
use paint::{DisplayItem, DisplayListBuilder};

#[test]
fn test_background_and_border_display_items() {
    let html = r#"<html><body>
        <div id="box">Box Content</div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #box {
            background-color: #ff0000;
            padding: 10px;
            margin: 5px;
            border-width: 2px;
            color: #000000;
            width: 200px;
            height: 100px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let box_id = doc.get_element_by_id("box").unwrap();
    let mut layout_box = build_box_tree(&doc, box_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);

    let display_list = DisplayListBuilder::build(&layout_box);
    assert!(!display_list.is_empty());

    let mut has_rect = false;
    let mut has_border = false;
    let mut has_text = false;

    for item in &display_list.items {
        match item {
            DisplayItem::DrawRect { color, .. } if *color == Color::rgb(255, 0, 0) => {
                has_rect = true;
            }
            DisplayItem::DrawBorder { widths, .. } if (widths.top - 2.0).abs() < f32::EPSILON => {
                has_border = true;
            }
            DisplayItem::DrawText { text, .. } if text.contains("Box Content") => {
                has_text = true;
            }
            _ => {}
        }
    }

    assert!(has_rect, "Expected DrawRect with red fill");
    assert!(has_border, "Expected DrawBorder with 2px width");
    assert!(has_text, "Expected DrawText with content");
}

#[test]
fn test_stacking_context_css21_order() {
    let html = r#"<html><body>
        <div id="container">
            <div id="pos">Positive Z</div>
            <div id="neg">Negative Z</div>
            <div id="normal">In-flow Normal</div>
        </div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #container {
            background-color: #111111;
            width: 400px;
            height: 300px;
        }
        #pos {
            position: relative;
            z-index: 10;
            background-color: #00ff00;
            width: 100px;
            height: 50px;
        }
        #neg {
            position: relative;
            z-index: -5;
            background-color: #0000ff;
            width: 100px;
            height: 50px;
        }
        #normal {
            background-color: #ffff00;
            width: 100px;
            height: 50px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let container_id = doc.get_element_by_id("container").unwrap();
    let mut root_box = build_box_tree(&doc, container_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut root_box, &viewport);

    let display_list = DisplayListBuilder::build(&root_box);

    let mut color_sequence = Vec::new();
    for item in &display_list.items {
        if let DisplayItem::DrawRect { color, .. } = item {
            color_sequence.push(*color);
        }
    }

    // Expected order:
    // 1. Container background (#111111)
    // 2. Negative z-index child (#neg = #0000ff)
    // 3. Normal in-flow child (#normal = #ffff00)
    // 4. Positive z-index child (#pos = #00ff00)
    assert_eq!(
        color_sequence,
        vec![
            Color::rgb(17, 17, 17),
            Color::rgb(0, 0, 255),
            Color::rgb(255, 255, 0),
            Color::rgb(0, 255, 0),
        ]
    );
}

#[test]
fn test_opacity_push_pop_wrapping() {
    let html = r#"<html><body>
        <div id="transparent_box">Opaque text</div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #transparent_box {
            opacity: 0.5;
            background-color: #ff0000;
            width: 200px;
            height: 100px;
        }
    ";
    let author_sheet = parse_stylesheet(css, Origin::Author);

    let resolver = CascadeResolver::new(&doc, &[&author_sheet]);
    let styles = resolver.resolve_all();

    let box_id = doc.get_element_by_id("transparent_box").unwrap();
    let mut layout_box = build_box_tree(&doc, box_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);

    let display_list = DisplayListBuilder::build(&layout_box);

    assert_eq!(
        display_list.items.first(),
        Some(&DisplayItem::PushOpacity { opacity: 0.5 })
    );
    assert_eq!(display_list.items.last(), Some(&DisplayItem::PopOpacity));
}
