//! Integration tests for CSS `opacity`, `visibility`, and `z-index` cascading.

use css::{CascadeResolver, Origin, Position, Visibility, parse_stylesheet};
use html::parse_html;

#[test]
fn test_css_opacity_visibility_z_index_cascade() {
    let html = r#"<html><body><div id="modal" class="layer popup">Modal Window</div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        #modal {
            position: absolute;
            z-index: 1050;
            opacity: 0.85;
            visibility: visible;
        }
        #modal.hidden {
            visibility: hidden;
            opacity: 0.0;
        }
    ";

    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let modal_id = doc.get_element_by_id("modal").unwrap();
    let style = styles.get(&modal_id).unwrap();

    assert_eq!(style.position, Position::Absolute);
    assert_eq!(style.z_index, Some(1050));
    assert!((style.opacity - 0.85).abs() < f32::EPSILON);
    assert_eq!(style.visibility, Visibility::Visible);

    // Hidden variant
    let hidden_html = r#"<html><body><div id="modal" class="layer popup hidden">Modal Window</div></body></html>"#;
    let hidden_doc = parse_html(hidden_html);
    let hidden_resolver = CascadeResolver::new(&hidden_doc, &[&sheet]);
    let hidden_styles = hidden_resolver.resolve_all();
    let hidden_modal_id = hidden_doc.get_element_by_id("modal").unwrap();
    let hidden_style = hidden_styles.get(&hidden_modal_id).unwrap();

    assert_eq!(hidden_style.visibility, Visibility::Hidden);
    assert!((hidden_style.opacity - 0.0).abs() < f32::EPSILON);
}

#[test]
fn test_css_visibility_inheritance() {
    let html = r#"<html><body><div class="invisible-wrapper"><span id="child">Nested</span></div></body></html>"#;
    let doc = parse_html(html);

    let css = r"
        .invisible-wrapper {
            visibility: hidden;
        }
    ";

    let sheet = parse_stylesheet(css, Origin::Author);
    let resolver = CascadeResolver::new(&doc, &[&sheet]);
    let styles = resolver.resolve_all();

    let child_id = doc.get_element_by_id("child").unwrap();
    let child_style = styles.get(&child_id).unwrap();

    assert_eq!(child_style.visibility, Visibility::Hidden);
}
