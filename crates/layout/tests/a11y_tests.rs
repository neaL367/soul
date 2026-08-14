//! Integration tests for accessibility tree extraction and semantic role assignment.

use css::CascadeResolver;
use html::parse_html;
use layout::{A11yNode, A11yRole, Dimensions, Rect, build_box_tree, layout_block};

fn collect_roles(node: &A11yNode, roles: &mut Vec<A11yRole>) {
    roles.push(node.role);
    for child in &node.children {
        collect_roles(child, roles);
    }
}

#[test]
fn test_a11y_tree_semantic_roles() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <body>
            <h1 aria-label="Main Heading">Page Title</h1>
            <p>Introductory paragraph</p>
            <button aria-label="Submit Button">Click</button>
        </body>
        </html>
    "#;

    let doc = parse_html(html);
    let resolver = CascadeResolver::new(&doc, &[]);
    let styles = resolver.resolve_all();
    let mut box_tree = build_box_tree(&doc, doc.root_id(), &styles).unwrap();

    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        ..Default::default()
    };
    layout_block(&mut box_tree, &viewport);

    let a11y_tree = A11yNode::from_layout_box(&doc, &box_tree).unwrap();
    assert_eq!(a11y_tree.role, A11yRole::Document);

    let mut roles = Vec::new();
    collect_roles(&a11y_tree, &mut roles);

    assert!(roles.contains(&A11yRole::Heading));
    assert!(roles.contains(&A11yRole::Paragraph));
    assert!(roles.contains(&A11yRole::Button));
}
