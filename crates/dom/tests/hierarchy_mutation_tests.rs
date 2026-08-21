//! Integration tests for DOM `clone_node`, `replace_child`, `contains`, `matches`, and `closest`.

use dom::Document;

#[test]
fn test_dom_clone_node_deep_and_shallow() {
    let mut doc = Document::new();
    let parent = doc.create_element("div");
    doc.set_attribute(parent, "id", "card-1");
    doc.set_attribute(parent, "class", "card active");

    let child = doc.create_element("p");
    doc.append_text(child, "Hello Clone");
    doc.append_child(parent, child);

    // Deep clone
    let deep_clone = doc.clone_node(parent, true);
    assert_ne!(deep_clone, parent);
    let deep_children = doc.children(deep_clone);
    assert_eq!(deep_children.len(), 1);
    assert_eq!(doc.text_content(deep_clone), "Hello Clone");

    // Shallow clone
    let shallow_clone = doc.clone_node(parent, false);
    assert_ne!(shallow_clone, parent);
    let shallow_children = doc.children(shallow_clone);
    assert_eq!(shallow_children.len(), 0);
}

#[test]
fn test_dom_replace_child() {
    let mut doc = Document::new();
    let list = doc.create_element("ul");
    let item1 = doc.create_element("li");
    let item2 = doc.create_element("li");
    let item3 = doc.create_element("li");
    let replacement = doc.create_element("li");
    doc.set_attribute(replacement, "id", "new-item");

    doc.append_child(list, item1);
    doc.append_child(list, item2);
    doc.append_child(list, item3);

    assert_eq!(doc.children(list), vec![item1, item2, item3]);

    doc.replace_child(list, replacement, item2);
    assert_eq!(doc.children(list), vec![item1, replacement, item3]);
}

#[test]
fn test_dom_matches_and_closest() {
    let mut doc = Document::new();
    let container = doc.create_element("section");
    doc.set_attribute(container, "class", "main-wrapper");

    let div = doc.create_element("div");
    doc.set_attribute(div, "id", "widget");
    doc.set_attribute(div, "class", "box elevated");

    let button = doc.create_element("button");
    doc.set_attribute(button, "class", "btn btn-primary");

    doc.append_child(container, div);
    doc.append_child(div, button);

    assert!(doc.matches(button, "button"));
    assert!(doc.matches(button, ".btn-primary"));
    assert!(doc.matches(div, "#widget"));
    assert!(doc.matches(div, ".elevated"));
    assert!(!doc.matches(button, ".elevated"));

    // Closest traversal
    assert_eq!(doc.closest(button, "button"), Some(button));
    assert_eq!(doc.closest(button, "#widget"), Some(div));
    assert_eq!(doc.closest(button, ".main-wrapper"), Some(container));
    assert_eq!(doc.closest(button, ".missing"), None);
}
