//! Integration tests for the arena-based `Document` DOM tree.

use dom::{Document, ElementData, NodeData, NodeId};
use std::collections::HashMap;

#[test]
fn test_dom_arena_node_allocation_and_linking() {
    let mut doc = Document::new();
    assert_eq!(doc.root_id(), NodeId(0));
    assert_eq!(doc.node_count(), 1);

    let html_id = doc.alloc_node(NodeData::Element(ElementData::new("html", HashMap::new())));
    doc.append_child(doc.root_id(), html_id);

    let body_id = doc.alloc_node(NodeData::Element(ElementData::new("body", HashMap::new())));
    doc.append_child(html_id, body_id);

    let h1_id = doc.alloc_node(NodeData::Element(ElementData::new("h1", HashMap::new())));
    doc.append_child(body_id, h1_id);

    doc.append_text(h1_id, "Title");

    assert_eq!(doc.get_node(h1_id).unwrap().parent, Some(body_id));
    assert_eq!(doc.get_node(body_id).unwrap().parent, Some(html_id));
    assert_eq!(doc.children(body_id), vec![h1_id]);
    assert_eq!(doc.text_content(h1_id), "Title");
}

#[test]
fn test_dom_insert_before_and_sibling_pointers() {
    let mut doc = Document::new();
    let body_id = doc.alloc_node(NodeData::Element(ElementData::new("body", HashMap::new())));
    doc.append_child(doc.root_id(), body_id);

    let child_1 = doc.alloc_node(NodeData::Element(ElementData::new("p", HashMap::new())));
    let child_3 = doc.alloc_node(NodeData::Element(ElementData::new("p", HashMap::new())));
    doc.append_child(body_id, child_1);
    doc.append_child(body_id, child_3);

    // Insert child_2 between child_1 and child_3
    let child_2 = doc.alloc_node(NodeData::Element(ElementData::new("p", HashMap::new())));
    doc.insert_before(body_id, child_2, Some(child_3));

    assert_eq!(doc.children(body_id), vec![child_1, child_2, child_3]);
    assert_eq!(doc.get_node(child_2).unwrap().prev_sibling, Some(child_1));
    assert_eq!(doc.get_node(child_2).unwrap().next_sibling, Some(child_3));
    assert_eq!(doc.get_node(child_1).unwrap().next_sibling, Some(child_2));
    assert_eq!(doc.get_node(child_3).unwrap().prev_sibling, Some(child_2));
}

#[test]
fn test_dom_remove_child() {
    let mut doc = Document::new();
    let parent = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    doc.append_child(doc.root_id(), parent);

    let c1 = doc.alloc_node(NodeData::Element(ElementData::new("span", HashMap::new())));
    let c2 = doc.alloc_node(NodeData::Element(ElementData::new("span", HashMap::new())));
    let c3 = doc.alloc_node(NodeData::Element(ElementData::new("span", HashMap::new())));

    doc.append_child(parent, c1);
    doc.append_child(parent, c2);
    doc.append_child(parent, c3);

    // Remove middle child c2
    doc.remove_child(parent, c2);
    assert_eq!(doc.children(parent), vec![c1, c3]);
    assert_eq!(doc.get_node(c1).unwrap().next_sibling, Some(c3));
    assert_eq!(doc.get_node(c3).unwrap().prev_sibling, Some(c1));
    assert_eq!(doc.get_node(c2).unwrap().parent, None);

    // Remove first child c1
    doc.remove_child(parent, c1);
    assert_eq!(doc.children(parent), vec![c3]);
    assert_eq!(doc.get_node(parent).unwrap().first_child, Some(c3));
    assert_eq!(doc.get_node(c3).unwrap().prev_sibling, None);
}

#[test]
fn test_dom_queries_by_id_tag_class() {
    let mut doc = Document::new();
    let mut attrs1 = HashMap::new();
    attrs1.insert("id".to_string(), "main-content".to_string());
    attrs1.insert("class".to_string(), "article highlight".to_string());

    let mut attrs2 = HashMap::new();
    attrs2.insert("class".to_string(), "highlight".to_string());

    let div_id = doc.alloc_node(NodeData::Element(ElementData::new("div", attrs1)));
    let span_id = doc.alloc_node(NodeData::Element(ElementData::new("span", attrs2)));

    doc.append_child(doc.root_id(), div_id);
    doc.append_child(div_id, span_id);

    assert_eq!(doc.get_element_by_id("main-content"), Some(div_id));
    assert_eq!(doc.get_element_by_id("non-existent"), None);

    let divs = doc.get_elements_by_tag_name("DIV");
    assert_eq!(divs, vec![div_id]);

    let highlighted = doc.get_elements_by_class_name("highlight");
    assert_eq!(highlighted, vec![div_id, span_id]);

    let articles = doc.get_elements_by_class_name("article");
    assert_eq!(articles, vec![div_id]);
}

#[test]
fn test_dom_element_traversal_helpers() {
    let mut doc = Document::new();
    let parent = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    doc.append_child(doc.root_id(), parent);

    // Text node before elements
    let text1 = doc.alloc_node(NodeData::Text("Intro".to_string()));
    doc.append_child(parent, text1);

    let elem1 = doc.alloc_node(NodeData::Element(ElementData::new("p", HashMap::new())));
    doc.append_child(parent, elem1);

    // Comment node between elements
    let comment = doc.alloc_node(NodeData::Comment("note".to_string()));
    doc.append_child(parent, comment);

    let elem2 = doc.alloc_node(NodeData::Element(ElementData::new("span", HashMap::new())));
    doc.append_child(parent, elem2);

    assert_eq!(doc.first_element_child(parent), Some(elem1));
    assert_eq!(doc.last_element_child(parent), Some(elem2));
    assert_eq!(doc.next_element_sibling(elem1), Some(elem2));
    assert_eq!(doc.previous_element_sibling(elem2), Some(elem1));
    assert_eq!(doc.child_element_count(parent), 2);
    assert!(doc.contains(parent, elem2));
    assert!(doc.contains(parent, text1));
    assert!(!doc.contains(elem1, elem2));
}
