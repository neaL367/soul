//! Integration tests for inline formatting context, text wrapping, and line box layout.

use css::{CascadeResolver, Origin, parse_stylesheet};
use html::parse_html;
use layout::{Dimensions, Rect, build_box_tree, layout_block};

#[test]
fn test_inline_multi_line_wrapping_in_constrained_container() {
    let html = r#"<html><body>
        <div id="container">
            <p>The quick brown fox jumps over the lazy dog and runs swiftly through the dense green forest</p>
        </div>
    </body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #container {
            width: 140px;
            margin: 0px;
            padding: 0px;
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

    // Height of paragraph should have wrapped to multiple lines (> 40px)
    let p_box = &root_box.children[0];
    assert!(p_box.dimensions.content.height > 40.0);
    assert!((root_box.dimensions.content.width - 140.0).abs() < f32::EPSILON);
}

#[test]
fn test_mixed_bold_and_normal_inline_fragments() {
    let html = r#"<html><body>
        <p id="target"><span>Normal</span> <strong>Bold text</strong></p>
    </body></html>"#;
    let doc = parse_html(html);

    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();

    let p_id = doc.get_element_by_id("target").unwrap();
    let mut p_box = build_box_tree(&doc, p_id, &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 500.0, 600.0),
        ..Default::default()
    };

    layout_block(&mut p_box, &viewport);

    // Paragraph should have resolved non-zero height
    assert!(p_box.dimensions.content.height > 0.0);
}
