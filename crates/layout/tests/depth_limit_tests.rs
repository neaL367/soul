//! Tests for the recursive block-layout depth budget (`MAX_LAYOUT_DEPTH`).

use layout::{BoxType, Dimensions, LayoutBox, MAX_LAYOUT_DEPTH, Rect, layout_block};

/// Builds a chain of nested anonymous block boxes, `depth` boxes deep.
fn nested_anonymous_chain(depth: usize) -> LayoutBox {
    let mut box_tree = LayoutBox::new(BoxType::AnonymousBlock, None);
    for _ in 0..depth {
        let mut parent = LayoutBox::new(BoxType::AnonymousBlock, None);
        parent.children.push(box_tree);
        box_tree = parent;
    }
    box_tree
}

fn viewport(width: f32) -> Dimensions {
    Dimensions {
        content: Rect::new(0.0, 0.0, width, 0.0),
        ..Default::default()
    }
}

#[test]
fn test_shallow_tree_fully_laid_out() {
    let mut box_tree = nested_anonymous_chain(4);
    let containing = viewport(640.0);
    layout_block(&mut box_tree, &containing);

    assert!((box_tree.dimensions.content.width - 640.0).abs() < 1e-3);
    assert!((box_tree.children[0].dimensions.content.width - 640.0).abs() < 1e-3);
    assert!((box_tree.children[0].children[0].dimensions.content.width - 640.0).abs() < 1e-3);
    // Last leaf below the limit is still laid out.
    let leaf = &box_tree.children[0].children[0].children[0];
    assert!((leaf.dimensions.content.width - 640.0).abs() < 1e-3);
}

#[test]
fn test_recursion_stops_at_depth_budget() {
    let mut box_tree = nested_anonymous_chain(MAX_LAYOUT_DEPTH + 3);
    let containing = viewport(640.0);
    layout_block(&mut box_tree, &containing);

    let mut node = &box_tree;
    for _ in 0..MAX_LAYOUT_DEPTH {
        node = &node.children[0];
    }
    // Box at depth MAX_LAYOUT_DEPTH is laid out.
    assert!((node.dimensions.content.width - 640.0).abs() < 1e-3);

    // Its child at depth MAX_LAYOUT_DEPTH + 1 is laid out too...
    node = &node.children[0];
    assert!((node.dimensions.content.width - 640.0).abs() < 1e-3);
    // ...but the recursion stops before reaching depth MAX_LAYOUT_DEPTH + 2.
    node = &node.children[0];
    assert!((node.dimensions.content.width - 0.0).abs() < 1e-3);
}

#[test]
fn test_deep_tree_does_not_overflow_stack() {
    let mut box_tree = nested_anonymous_chain(MAX_LAYOUT_DEPTH + 512);
    let containing = viewport(100.0);
    layout_block(&mut box_tree, &containing);
    assert!((box_tree.dimensions.content.width - 100.0).abs() < 1e-3);
}
