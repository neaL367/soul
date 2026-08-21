//! Integration tests for CSS out-of-flow positioning (`position: absolute`, `position: fixed`) and top/right/bottom/left offsets.

#![allow(clippy::float_cmp, clippy::field_reassign_with_default)]

use css::{ComputedStyle, Length, Position};
use layout::{Dimensions, LayoutBox, Rect, layout_block};

#[test]
fn test_absolute_positioning_with_top_left() {
    let mut parent = LayoutBox::new(layout::BoxType::AnonymousBlock, None);
    parent.dimensions.content = Rect {
        x: 50.0,
        y: 100.0,
        width: 500.0,
        height: 300.0,
    };

    let mut abs_style = ComputedStyle::initial();
    abs_style.position = Position::Absolute;
    abs_style.width = Length::Px(150.0);
    abs_style.height = Length::Px(80.0);
    abs_style.left = Length::Px(20.0);
    abs_style.top = Length::Px(30.0);

    let abs_child = LayoutBox::new(layout::BoxType::AnonymousBlock, Some(abs_style));
    parent.children.push(abs_child);

    let mut containing = Dimensions::default();
    containing.content = Rect {
        x: 50.0,
        y: 100.0,
        width: 500.0,
        height: 300.0,
    };

    layout_block(&mut parent, &containing);

    let child = &parent.children[0];
    assert_eq!(child.dimensions.content.x, 70.0); // 50 + 20
    assert_eq!(child.dimensions.content.y, 130.0); // 100 + 30
    assert_eq!(child.dimensions.content.width, 150.0);
    assert_eq!(child.dimensions.content.height, 80.0);
}

#[test]
fn test_absolute_positioning_with_bottom_right() {
    let mut parent_style = ComputedStyle::initial();
    parent_style.width = Length::Px(400.0);
    parent_style.height = Length::Px(200.0);

    let mut parent = LayoutBox::new(layout::BoxType::AnonymousBlock, Some(parent_style));
    parent.dimensions.content = Rect {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 200.0,
    };

    let mut abs_style = ComputedStyle::initial();
    abs_style.position = Position::Absolute;
    abs_style.width = Length::Px(100.0);
    abs_style.height = Length::Px(50.0);
    abs_style.right = Length::Px(10.0);
    abs_style.bottom = Length::Px(20.0);

    let abs_child = LayoutBox::new(layout::BoxType::AnonymousBlock, Some(abs_style));
    parent.children.push(abs_child);

    let mut containing = Dimensions::default();
    containing.content = Rect {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 200.0,
    };

    layout_block(&mut parent, &containing);

    let child = &parent.children[0];
    // x = 0 + 400 - 10 - 100 = 290
    assert_eq!(child.dimensions.content.x, 290.0);
    // y = 0 + 200 - 20 - 50 = 130
    assert_eq!(child.dimensions.content.y, 130.0);
}
