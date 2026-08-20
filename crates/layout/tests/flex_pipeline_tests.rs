//! Integration tests proving flex containers reach the live layout pipeline:
//! `layout_block` must dispatch `display: flex` containers to the flex
//! algorithm rather than laying them out as normal-flow blocks.

use css::{ComputedStyle, Display, FlexDirection, Length};
use dom::{Document, NodeData, NodeId};
use layout::{BoxType, Dimensions, LayoutBox, Rect, build_box_tree, layout_block};
use std::collections::HashMap;

fn viewport() -> Dimensions {
    Dimensions {
        content: Rect::new(0.0, 0.0, 1000.0, 800.0),
        ..Default::default()
    }
}

fn flex_container(width: f32, height: f32) -> ComputedStyle {
    ComputedStyle {
        display: Display::Flex,
        width: Length::Px(width),
        height: Length::Px(height),
        flex_direction: FlexDirection::Row,
        ..ComputedStyle::initial()
    }
}

fn fixed_item(width: f32, height: f32) -> ComputedStyle {
    ComputedStyle {
        width: Length::Px(width),
        height: Length::Px(height),
        flex_shrink: 0.0,
        ..ComputedStyle::initial()
    }
}

fn growing_item(height: f32) -> ComputedStyle {
    ComputedStyle {
        width: Length::Auto,
        height: Length::Px(height),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..ComputedStyle::initial()
    }
}

#[test]
fn flex_container_distributes_children_via_layout_block() {
    let mut container = LayoutBox::new(
        BoxType::BlockNode(NodeId(0)),
        Some(flex_container(300.0, 50.0)),
    );
    container.children.push(LayoutBox::new(
        BoxType::BlockNode(NodeId(1)),
        Some(growing_item(50.0)),
    ));
    container.children.push(LayoutBox::new(
        BoxType::BlockNode(NodeId(2)),
        Some(growing_item(50.0)),
    ));

    layout_block(&mut container, &viewport());

    assert_eq!(container.children.len(), 2);
    // Both items grow to split the container equally (150px each).
    let a = &container.children[0];
    let b = &container.children[1];
    assert!(
        (a.dimensions.content.x - 0.0).abs() < 1.0,
        "first flex item must start at x=0"
    );
    assert!(
        (a.dimensions.content.width - 150.0).abs() < 1.0,
        "first flex item must be 150px wide, got {}",
        a.dimensions.content.width
    );
    assert!(
        (b.dimensions.content.x - 150.0).abs() < 1.0,
        "second flex item must start at x=150"
    );
    assert!(
        (b.dimensions.content.width - 150.0).abs() < 1.0,
        "second flex item must be 150px wide, got {}",
        b.dimensions.content.width
    );
}

#[test]
fn flex_column_stacks_children_vertically() {
    let mut container = LayoutBox::new(
        BoxType::BlockNode(NodeId(0)),
        Some(ComputedStyle {
            display: Display::Flex,
            width: Length::Px(200.0),
            height: Length::Px(100.0),
            flex_direction: FlexDirection::Column,
            ..ComputedStyle::initial()
        }),
    );
    container.children.push(LayoutBox::new(
        BoxType::BlockNode(NodeId(1)),
        Some(fixed_item(200.0, 40.0)),
    ));
    container.children.push(LayoutBox::new(
        BoxType::BlockNode(NodeId(2)),
        Some(fixed_item(200.0, 60.0)),
    ));

    layout_block(&mut container, &viewport());

    let a = &container.children[0];
    let b = &container.children[1];
    assert!(
        (a.dimensions.content.y - 0.0).abs() < 1.0,
        "first item at y=0"
    );
    assert!(
        (a.dimensions.content.height - 40.0).abs() < 1.0,
        "first item 40px tall"
    );
    assert!(
        (b.dimensions.content.y - 40.0).abs() < 1.0,
        "second item below first"
    );
    assert!(
        (b.dimensions.content.height - 60.0).abs() < 1.0,
        "second item 60px tall"
    );
    assert!(
        (container.dimensions.content.height - 100.0).abs() < 1.0,
        "column container is 100px tall, got {}",
        container.dimensions.content.height
    );
}

#[test]
fn nested_flex_containers_dispatch_recursively() {
    let inner_style = flex_container(200.0, 50.0);
    let mut inner = LayoutBox::new(BoxType::BlockNode(NodeId(1)), Some(inner_style));
    inner.children.push(LayoutBox::new(
        BoxType::BlockNode(NodeId(2)),
        Some(fixed_item(100.0, 50.0)),
    ));
    inner.children.push(LayoutBox::new(
        BoxType::BlockNode(NodeId(3)),
        Some(fixed_item(100.0, 50.0)),
    ));

    let mut outer = LayoutBox::new(
        BoxType::BlockNode(NodeId(0)),
        Some(flex_container(300.0, 50.0)),
    );
    outer.children.push(inner);

    layout_block(&mut outer, &viewport());

    let inner = &outer.children[0];
    assert!(
        (inner.dimensions.content.width - 200.0).abs() < 1.0,
        "nested flex container keeps its flex-resolved width, got {}",
        inner.dimensions.content.width
    );
    let a = &inner.children[0];
    let b = &inner.children[1];
    assert!(
        (a.dimensions.content.width - 100.0).abs() < 1.0,
        "nested item must be flex-laid-out at 100px, got {}",
        a.dimensions.content.width
    );
    assert!(
        (b.dimensions.content.width - 100.0).abs() < 1.0,
        "nested item must be flex-laid-out at 100px, got {}",
        b.dimensions.content.width
    );
}

#[test]
fn flex_container_wraps_inline_children_as_anonymous_items() {
    let mut doc = Document::new();
    let flex_id = doc.alloc_node(NodeData::Element(dom::ElementData::new(
        "div",
        HashMap::new(),
    )));
    doc.append_child(doc.root_id(), flex_id);
    let span_a = doc.alloc_node(NodeData::Element(dom::ElementData::new(
        "span",
        HashMap::new(),
    )));
    doc.append_child(flex_id, span_a);
    let span_b = doc.alloc_node(NodeData::Element(dom::ElementData::new(
        "span",
        HashMap::new(),
    )));
    doc.append_child(flex_id, span_b);

    let mut styles: HashMap<NodeId, ComputedStyle> = HashMap::new();
    styles.insert(flex_id, flex_container(300.0, 50.0));
    styles.insert(span_a, ComputedStyle::initial());
    styles.insert(span_b, ComputedStyle::initial());

    let mut root_box = build_box_tree(&doc, doc.root_id(), &styles).expect("box tree");
    layout_block(&mut root_box, &viewport());

    let flex_box = &root_box.children[0];
    assert_eq!(
        flex_box.box_type,
        BoxType::BlockNode(flex_id),
        "flex element becomes a block-level box"
    );
    // Consecutive inline children are wrapped into a single anonymous flex item.
    assert_eq!(
        flex_box.children.len(),
        1,
        "inline run wrapped in one anonymous item"
    );
    assert_eq!(flex_box.children[0].box_type, BoxType::AnonymousBlock);
    assert_eq!(
        flex_box.children[0].children.len(),
        2,
        "anonymous flex item retains both inline children"
    );
}
