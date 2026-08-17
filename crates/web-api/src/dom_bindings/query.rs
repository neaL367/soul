//! DOM selector query helpers for `querySelector` and `querySelectorAll`.

use dom::{Document, NodeId};

/// Finds the first matching `NodeId` for a simple CSS selector string.
#[must_use]
pub fn query_selector(doc: &Document, selector: &str) -> Option<NodeId> {
    query_selector_all(doc, selector).into_iter().next()
}

/// Finds all matching `NodeId`s for a simple CSS selector string.
#[must_use]
pub fn query_selector_all(doc: &Document, selector: &str) -> Vec<NodeId> {
    let sel = selector.trim();
    if sel.is_empty() {
        return Vec::new();
    }

    if let Some(id) = sel.strip_prefix('#') {
        return doc.get_element_by_id(id).into_iter().collect();
    }

    if let Some(class) = sel.strip_prefix('.') {
        return doc.get_elements_by_class_name(class);
    }

    if sel == "*" {
        return doc
            .descendants(doc.root_id())
            .into_iter()
            .filter(|&id| doc.get_node(id).is_some_and(dom::Node::is_element))
            .collect();
    }

    doc.get_elements_by_tag_name(sel)
}
