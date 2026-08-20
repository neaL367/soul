//! Integration tests for CSS Flexbox layout via `layout::layout_flex`.

use css::{ComputedStyle, Display, FlexDirection, JustifyContent, Length};
use layout::layout_flex;

fn flex_container(width: f32, direction: FlexDirection, justify: JustifyContent) -> ComputedStyle {
    ComputedStyle {
        display: Display::Flex,
        width: Length::Px(width),
        height: Length::Px(100.0),
        flex_direction: direction,
        justify_content: justify,
        ..ComputedStyle::initial()
    }
}

fn flex_item(width: f32, height: f32) -> ComputedStyle {
    ComputedStyle {
        width: Length::Px(width),
        height: Length::Px(height),
        flex_shrink: 0.0,
        ..ComputedStyle::initial()
    }
}

#[test]
fn test_flex_row_items_laid_out_horizontally() {
    let container = flex_container(600.0, FlexDirection::Row, JustifyContent::FlexStart);
    let item_a = flex_item(100.0, 50.0);
    let item_b = flex_item(200.0, 50.0);
    let children: Vec<(usize, &ComputedStyle)> = vec![(0, &item_a), (1, &item_b)];

    let results = layout_flex(&container, 600.0, &children);

    assert_eq!(results.items.len(), 2, "must return one result per child");

    let r_a = &results.items[0];
    let r_b = &results.items[1];
    assert_eq!(r_a.index, 0, "first result index must be 0");
    assert_eq!(r_b.index, 1, "second result index must be 1");

    // Row direction: items placed left-to-right, a at x=0, b follows a
    assert!(
        (r_a.dimensions.content.x - 0.0).abs() < 1.0,
        "item a x must be ~0"
    );
    assert!(
        r_b.dimensions.content.x >= r_a.dimensions.content.x + r_a.dimensions.content.width - 1.0,
        "item b must follow item a on x axis"
    );
    assert!(
        (r_a.dimensions.content.width - 100.0).abs() < 1.0,
        "item a width must be ~100"
    );
    assert!(
        (r_b.dimensions.content.width - 200.0).abs() < 1.0,
        "item b width must be ~200"
    );
}

#[test]
fn test_flex_column_items_laid_out_vertically() {
    let container = flex_container(200.0, FlexDirection::Column, JustifyContent::FlexStart);
    let item_a = flex_item(200.0, 40.0);
    let item_b = flex_item(200.0, 60.0);
    let children: Vec<(usize, &ComputedStyle)> = vec![(0, &item_a), (1, &item_b)];

    let results = layout_flex(&container, 200.0, &children);

    assert_eq!(results.items.len(), 2);
    let r_a = &results.items[0];
    let r_b = &results.items[1];

    // Column direction: item a at y=0, item b follows below
    assert!(
        (r_a.dimensions.content.y - 0.0).abs() < 1.0,
        "item a y must be ~0"
    );
    assert!(
        r_b.dimensions.content.y >= r_a.dimensions.content.y + r_a.dimensions.content.height - 1.0,
        "item b must follow item a on y axis"
    );
    assert!(
        (r_a.dimensions.content.height - 40.0).abs() < 1.0,
        "item a height must be ~40"
    );
    assert!(
        (r_b.dimensions.content.height - 60.0).abs() < 1.0,
        "item b height must be ~60"
    );
}

#[test]
fn test_flex_grow_fills_container() {
    let container = flex_container(300.0, FlexDirection::Row, JustifyContent::FlexStart);
    let fixed_item = flex_item(100.0, 50.0);
    let growing_item = ComputedStyle {
        width: Length::Auto,
        height: Length::Px(50.0),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..ComputedStyle::initial()
    };
    let children: Vec<(usize, &ComputedStyle)> = vec![(0, &fixed_item), (1, &growing_item)];

    let results = layout_flex(&container, 300.0, &children);

    assert_eq!(results.items.len(), 2);
    let r_a = &results.items[0];
    let r_b = &results.items[1];

    // Fixed item stays at 100px, growing item absorbs remaining 200px
    assert!(
        (r_a.dimensions.content.width - 100.0).abs() < 1.0,
        "fixed item must be 100px wide"
    );
    assert!(
        r_b.dimensions.content.width > 150.0,
        "growing item must fill remaining space (>150px), got {}",
        r_b.dimensions.content.width
    );
}
