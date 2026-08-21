//! Integration tests for pseudo-element and generated content layout generation.

use css::ComputedStyle;
use dom::Document;
use layout::box_tree::{BoxType, build_box_tree};
use std::collections::HashMap;

#[test]
fn test_generated_content_layout_box_creation() {
    let mut doc = Document::new();
    let root = doc.root_id();
    let p = doc.create_element("p");
    doc.append_child(root, p);

    let mut styles = HashMap::new();
    let mut root_style = ComputedStyle::initial();
    root_style.display = css::Display::Block;
    styles.insert(root, root_style);

    let mut p_style = ComputedStyle::initial();
    p_style.display = css::Display::Block;
    p_style.content = Some("Generated Note: ".to_string());
    styles.insert(p, p_style);

    let box_tree = build_box_tree(&doc, root, &styles).expect("box tree");
    let p_box = &box_tree.children[0];
    assert_eq!(p_box.children.len(), 1);
    match &p_box.children[0].box_type {
        BoxType::TextNode(_, text) => assert_eq!(text, "Generated Note: "),
        _ => panic!("expected TextNode for generated content"),
    }
}
