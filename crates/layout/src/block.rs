//! Normal flow block layout algorithm.

use crate::box_tree::LayoutBox;
use crate::geometry::{Dimensions, EdgeSizes};
use css::Length;

/// Computes normal flow block layout for a layout box and all of its descendants.
pub fn layout_block(layout_box: &mut LayoutBox, containing_block: &Dimensions) {
    // 1. Calculate width, margins, padding, and borders
    calculate_block_width(layout_box, containing_block);

    // 2. Position the box within the containing block at offset 0.0
    calculate_block_position(layout_box, containing_block, 0.0);

    // 3. Recursively layout children
    layout_block_children(layout_box);

    // 4. Calculate final height
    calculate_block_height(layout_box);
}

fn calculate_block_width(layout_box: &mut LayoutBox, containing_block: &Dimensions) {
    let Some(style) = &layout_box.style else {
        layout_box.dimensions.content.width = containing_block.content.width;
        return;
    };

    let padding = EdgeSizes::new(
        style.padding_top,
        style.padding_right,
        style.padding_bottom,
        style.padding_left,
    );
    let border = EdgeSizes::new(
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
    );
    let margin = EdgeSizes::new(
        style.margin_top,
        style.margin_right,
        style.margin_bottom,
        style.margin_left,
    );

    let total_spacing =
        padding.horizontal_total() + border.horizontal_total() + margin.horizontal_total();

    let width = match style.width {
        Length::Px(px) => px,
        Length::Percent(pct) => {
            (containing_block.content.width * pct / 100.0)
                - (padding.horizontal_total() + border.horizontal_total())
        }
        Length::Auto | Length::Em(_) | Length::Rem(_) => {
            (containing_block.content.width - total_spacing).max(0.0)
        }
    };

    layout_box.dimensions.content.width = width;
    layout_box.dimensions.padding = padding;
    layout_box.dimensions.border = border;
    layout_box.dimensions.margin = margin;
}

fn calculate_block_position(
    layout_box: &mut LayoutBox,
    containing_block: &Dimensions,
    vertical_offset: f32,
) {
    let margin = layout_box.dimensions.margin;
    let border = layout_box.dimensions.border;
    let padding = layout_box.dimensions.padding;

    layout_box.dimensions.content.x =
        containing_block.content.x + margin.left + border.left + padding.left;

    layout_box.dimensions.content.y =
        containing_block.content.y + vertical_offset + margin.top + border.top + padding.top;
}

fn layout_block_children(layout_box: &mut LayoutBox) {
    if layout_box.children.iter().any(LayoutBox::is_inline) {
        let max_w = layout_box.dimensions.content.width;
        let inline_h = crate::inline::layout_inline_context(layout_box, max_w);
        layout_box.dimensions.content.height = inline_h;
        return;
    }

    let mut vertical_offset = 0.0;

    for child in &mut layout_box.children {
        if child.is_block() {
            calculate_block_width(child, &layout_box.dimensions);
            calculate_block_position(child, &layout_box.dimensions, vertical_offset);
            layout_block_children(child);
            calculate_block_height(child);

            let child_margin_box_h = child.dimensions.margin_box().height;
            vertical_offset += child_margin_box_h;
        }
    }

    layout_box.dimensions.content.height = vertical_offset;
}

#[allow(clippy::cast_precision_loss)]
const fn calculate_block_height(layout_box: &mut LayoutBox) {
    if let Some(ref style) = layout_box.style
        && let Length::Px(px) = style.height
    {
        layout_box.dimensions.content.height = px;
    } else if let Some(intrinsic) = layout_box.intrinsic
        && intrinsic.width > 0
        && layout_box.dimensions.content.width > 0.0
    {
        // Replaced-element sizing: width already resolved to the containing
        // block; height follows the intrinsic aspect ratio.
        let width = layout_box.dimensions.content.width;
        layout_box.dimensions.content.height =
            width * (intrinsic.height as f32 / intrinsic.width as f32);
    }
}
