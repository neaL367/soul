//! Integration tests for paint builder stacking context ordering and visibility filtering.

use css::{Color, ComputedStyle, Display, Position, Visibility};
use dom::NodeId;
use layout::{BoxType, Dimensions, LayoutBox, Rect};
use paint::{DisplayItem, DisplayListBuilder};
use std::collections::HashMap;

fn create_test_box(
    z_index: Option<i32>,
    opacity: f32,
    visibility: Visibility,
    bg_color: Color,
) -> LayoutBox {
    let style = ComputedStyle {
        display: Display::Block,
        position: Position::Absolute,
        z_index,
        opacity,
        visibility,
        background_color: bg_color,
        ..ComputedStyle::default()
    };

    let mut b = LayoutBox::new(BoxType::BlockNode(NodeId(1)), Some(style));
    b.dimensions = Dimensions {
        content: Rect::new(10.0, 10.0, 100.0, 100.0),
        ..Dimensions::default()
    };
    b
}

#[test]
fn test_stacking_context_z_index_and_opacity_display_items() {
    let root_style = ComputedStyle {
        display: Display::Block,
        ..ComputedStyle::default()
    };
    let mut root = LayoutBox::new(BoxType::BlockNode(NodeId(0)), Some(root_style));
    root.dimensions.content = Rect::new(0.0, 0.0, 800.0, 600.0);

    let red = Color::rgb(255, 0, 0);
    let blue = Color::rgb(0, 0, 255);

    let child_pos = create_test_box(Some(10), 1.0, Visibility::Visible, red);
    let child_neg = create_test_box(Some(-5), 0.5, Visibility::Visible, blue);

    root.children.push(child_pos);
    root.children.push(child_neg);

    let images = HashMap::new();
    let display_list = DisplayListBuilder::build(&root, &images);

    // Negative z-index child should appear before positive z-index child in display list items
    let has_push_opacity = display_list
        .items
        .iter()
        .any(|item| matches!(item, DisplayItem::PushOpacity { opacity } if (*opacity - 0.5).abs() < f32::EPSILON));
    assert!(has_push_opacity);

    let rect_colors: Vec<Color> = display_list
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::DrawRect { color, .. } => Some(*color),
            _ => None,
        })
        .collect();

    assert_eq!(rect_colors, vec![blue, red]);
}

#[test]
fn test_visibility_hidden_skips_drawing() {
    let green = Color::rgb(0, 255, 0);
    let root_style = ComputedStyle {
        display: Display::Block,
        ..ComputedStyle::default()
    };
    let mut root = LayoutBox::new(BoxType::BlockNode(NodeId(0)), Some(root_style));
    root.dimensions.content = Rect::new(0.0, 0.0, 800.0, 600.0);

    let hidden_box = create_test_box(None, 1.0, Visibility::Hidden, green);
    root.children.push(hidden_box);

    let images = HashMap::new();
    let display_list = DisplayListBuilder::build(&root, &images);

    let has_green_rect = display_list
        .items
        .iter()
        .any(|item| matches!(item, DisplayItem::DrawRect { color, .. } if *color == green));
    assert!(!has_green_rect);
}
