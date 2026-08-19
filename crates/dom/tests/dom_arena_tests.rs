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

#[test]
fn test_invalid_node_ids_are_noops_not_panics() {
    let mut doc = Document::new();
    let parent = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    doc.append_child(doc.root_id(), parent);

    // Out-of-range ids must be silently rejected, never panic or corrupt.
    let ghost = NodeId(9999);
    doc.append_child(ghost, parent);
    doc.append_child(parent, ghost);
    doc.append_child(ghost, ghost);
    doc.insert_before(parent, ghost, Some(parent));
    doc.remove_child(parent, ghost);
    doc.reparent_children(ghost, parent);
    doc.reparent_children(parent, ghost);
    doc.append_text(ghost, "x");
    assert_eq!(doc.children(parent), vec![]);
    assert_eq!(doc.children(ghost), vec![]);

    // Valid appends still work afterwards.
    doc.append_text(parent, "x");
    assert_eq!(doc.children(parent), vec![NodeId(2)]);
    assert_eq!(doc.node_count(), 3);
}

#[test]
fn test_ancestor_cycle_is_rejected() {
    let mut doc = Document::new();
    let a = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    let b = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    let c = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    doc.append_child(doc.root_id(), a);
    doc.append_child(a, b);
    doc.append_child(b, c);

    // Appending an ancestor under its own descendant would create a cycle.
    doc.append_child(c, a);
    assert_eq!(doc.get_node(a).unwrap().parent, Some(doc.root_id()));
    assert_eq!(doc.children(c), vec![]);

    doc.insert_before(c, a, None);
    assert_eq!(doc.get_node(a).unwrap().parent, Some(doc.root_id()));

    // The tree is still fully traversable and uncorrupted.
    assert_eq!(doc.descendants(doc.root_id()).len(), 3);
}

#[test]
fn test_insert_before_rejects_non_child_sibling() {
    let mut doc = Document::new();
    let p1 = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    let p2 = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    let child = doc.alloc_node(NodeData::Element(ElementData::new("p", HashMap::new())));
    doc.append_child(doc.root_id(), p1);
    doc.append_child(doc.root_id(), p2);
    doc.append_child(p2, child);

    // `before` must be a direct child of the given parent.
    doc.insert_before(p1, child, Some(p2));
    assert_eq!(doc.children(p1), vec![]);
    assert_eq!(doc.children(p2), vec![child]);
}

#[test]
fn test_remove_child_requires_real_parent() {
    let mut doc = Document::new();
    let p1 = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    let p2 = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    let child = doc.alloc_node(NodeData::Element(ElementData::new("p", HashMap::new())));
    doc.append_child(doc.root_id(), p1);
    doc.append_child(doc.root_id(), p2);
    doc.append_child(p1, child);

    // Removing with the wrong parent must not corrupt either chain.
    doc.remove_child(p2, child);
    assert_eq!(doc.children(p1), vec![child]);
    assert_eq!(doc.children(p2), vec![]);
    assert_eq!(doc.get_node(child).unwrap().parent, Some(p1));
}

#[test]
fn test_append_text_marks_dirty_and_merges() {
    let mut doc = Document::new();
    let parent = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
    doc.append_child(doc.root_id(), parent);

    doc.append_text(parent, "Hello ");
    doc.append_text(parent, "world");
    assert_eq!(doc.text_content(parent), "Hello world");
    assert_eq!(doc.children(parent).len(), 1);

    let text_node = doc.children(parent)[0];
    assert!(doc.get_node(text_node).unwrap().dirty_flags.paint);

    // Text must not be appended to element parents.
    doc.append_child(parent, parent);
    assert_eq!(doc.get_node(parent).unwrap().parent, Some(doc.root_id()));
}

#[test]
fn test_depth_limit_refuses_deep_appends() {
    let mut doc = Document::new();
    let mut cur = doc.root_id();
    for _ in 0..dom::MAX_DOM_DEPTH + 100 {
        let next = doc.alloc_node(NodeData::Element(ElementData::new("div", HashMap::new())));
        doc.append_child(cur, next);
        cur = next;
    }
    // Tree stops growing at the depth ceiling.
    assert_eq!(doc.descendants(doc.root_id()).len(), dom::MAX_DOM_DEPTH);
    assert!(doc.get_node(cur).is_some());
}
