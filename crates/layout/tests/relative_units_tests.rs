//! Integration tests for CSS Relative Units (`em`, `rem`, `vw`, `vh`) and `calc()` expressions.

#![allow(
    clippy::float_cmp,
    clippy::field_reassign_with_default,
    clippy::similar_names
)]

use css::{BoxSizing, ComputedStyle, Length};
use layout::calc::{LengthContext, evaluate_calc, resolve_length};
use layout::{Dimensions, LayoutBox, Rect, layout_block};

#[test]
fn test_resolve_em_and_rem_units() {
    let ctx = LengthContext::default().with_font_sizes(20.0, 16.0); // 20px local font-size, 16px root font-size

    let em_len = Length::Em(2.5);
    assert_eq!(resolve_length(&em_len, &ctx), Some(50.0)); // 2.5 * 20px = 50px

    let rem_len = Length::Rem(1.5);
    assert_eq!(resolve_length(&rem_len, &ctx), Some(24.0)); // 1.5 * 16px = 24px
}

#[test]
fn test_resolve_viewport_units() {
    let ctx = LengthContext::new(1000.0, 1920.0, 1080.0);

    let vw_len = Length::Vw(50.0);
    assert_eq!(resolve_length(&vw_len, &ctx), Some(960.0)); // 50% of 1920px = 960px

    let vh_len = Length::Vh(25.0);
    assert_eq!(resolve_length(&vh_len, &ctx), Some(270.0)); // 25% of 1080px = 270px
}

#[test]
fn test_calc_arithmetic_evaluation() {
    let ctx = LengthContext::new(500.0, 1000.0, 800.0).with_font_sizes(16.0, 16.0);

    // Simple addition
    assert_eq!(evaluate_calc("100px + 50px", &ctx), Some(150.0));

    // Subtraction with percentage basis: 100% (500px) - 30px = 470px
    assert_eq!(evaluate_calc("100% - 30px", &ctx), Some(470.0));

    // Relative rem and em units: 2rem (32px) + 1em (16px) = 48px
    assert_eq!(evaluate_calc("2rem + 1em", &ctx), Some(48.0));

    // Operator precedence: 10px + 20px * 2 = 50px
    assert_eq!(evaluate_calc("10px + 20px * 2", &ctx), Some(50.0));

    // Parentheses: (10px + 20px) * 2 = 60px
    assert_eq!(evaluate_calc("(10px + 20px) * 2", &ctx), Some(60.0));
}

#[test]
fn test_block_layout_with_calc_and_relative_units() {
    let mut root = LayoutBox::new(layout::BoxType::AnonymousBlock, None);
    let mut style = ComputedStyle::initial();
    style.width = Length::Calc("100% - 40px".to_string());
    style.height = Length::Rem(5.0); // 5 * 16px = 80px
    style.box_sizing = BoxSizing::ContentBox;

    let child = LayoutBox::new(layout::BoxType::AnonymousBlock, Some(style));
    root.children.push(child);

    let mut containing = Dimensions::default();
    containing.content = Rect {
        x: 0.0,
        y: 0.0,
        width: 600.0,
        height: 400.0,
    };

    layout_block(&mut root, &containing);

    let child_box = &root.children[0];
    assert_eq!(child_box.dimensions.content.width, 560.0); // 600 - 40
    assert_eq!(child_box.dimensions.content.height, 80.0); // 5 * 16
}
