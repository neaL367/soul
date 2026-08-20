//! Normal flow block layout algorithm with W3C box-sizing and CSS 2.1 §8.3.1 margin collapsing.

use crate::box_tree::LayoutBox;
use crate::geometry::{Dimensions, EdgeSizes};
use css::{BoxSizing, Length};

/// Maximum depth of recursive block layout before child layout is skipped.
///
/// Guards against stack exhaustion on hand-built `LayoutBox` trees with
/// unbounded nesting. DOM-derived trees are already capped by `dom`'s
/// `MAX_DOM_DEPTH`; this budget keeps that guarantee if box trees are ever
/// constructed independently. Boxes at or below the limit lay out normally;
/// deeper boxes get their own width/position/height but their children are
/// left unlaid-out rather than recursed into.
pub const MAX_LAYOUT_DEPTH: usize = 1024;

/// Computes normal flow block layout for a layout box and all of its descendants.
pub fn layout_block(layout_box: &mut LayoutBox, containing_block: &Dimensions) {
    layout_block_inner(layout_box, containing_block, 0);
}

fn layout_block_inner(layout_box: &mut LayoutBox, containing_block: &Dimensions, depth: usize) {
    // 1. Calculate width, margins, padding, and borders
    calculate_block_width(layout_box, containing_block);

    // 2. Position the box within the containing block at offset 0.0
    calculate_block_position(layout_box, containing_block, 0.0);

    // 3. Recursively layout children with margin collapsing
    if depth < MAX_LAYOUT_DEPTH {
        layout_block_children(layout_box, depth);
    }

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
        Length::Px(px) => {
            if style.box_sizing == BoxSizing::BorderBox {
                (px - (padding.horizontal_total() + border.horizontal_total())).max(0.0)
            } else {
                px
            }
        }
        Length::Percent(pct) => {
            let total_w = containing_block.content.width * pct / 100.0;
            if style.box_sizing == BoxSizing::BorderBox {
                (total_w - (padding.horizontal_total() + border.horizontal_total())).max(0.0)
            } else {
                total_w
            }
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

fn collapse_vertical_margins(prev_bottom: f32, current_top: f32) -> f32 {
    if prev_bottom >= 0.0 && current_top >= 0.0 {
        prev_bottom.max(current_top)
    } else if prev_bottom < 0.0 && current_top < 0.0 {
        prev_bottom.min(current_top)
    } else {
        prev_bottom + current_top
    }
}

fn layout_block_children(layout_box: &mut LayoutBox, depth: usize) {
    if layout_box.children.iter().any(LayoutBox::is_inline) {
        let max_w = layout_box.dimensions.content.width;
        let inline_h = crate::inline::layout_inline_context(layout_box, max_w);
        layout_box.dimensions.content.height = inline_h;
        return;
    }

    let mut vertical_offset = 0.0;
    let mut prev_margin_bottom = 0.0;
    let mut is_first = true;

    for child in &mut layout_box.children {
        if child.is_block() {
            calculate_block_width(child, &layout_box.dimensions);

            if is_first {
                vertical_offset += child.dimensions.margin.top;
                is_first = false;
            } else {
                let collapsed_margin =
                    collapse_vertical_margins(prev_margin_bottom, child.dimensions.margin.top);
                vertical_offset += collapsed_margin;
            }

            let border_padding_y = child.dimensions.border.top + child.dimensions.padding.top;
            child.dimensions.content.x = layout_box.dimensions.content.x
                + child.dimensions.margin.left
                + child.dimensions.border.left
                + child.dimensions.padding.left;
            child.dimensions.content.y =
                layout_box.dimensions.content.y + vertical_offset + border_padding_y;

            if depth < MAX_LAYOUT_DEPTH {
                layout_block_children(child, depth + 1);
            }
            calculate_block_height(child);

            vertical_offset += child.dimensions.content.height
                + child.dimensions.padding.vertical_total()
                + child.dimensions.border.vertical_total();
            prev_margin_bottom = child.dimensions.margin.bottom;
        }
    }

    vertical_offset += prev_margin_bottom;
    layout_box.dimensions.content.height = vertical_offset;
}

#[allow(clippy::cast_precision_loss)]
fn calculate_block_height(layout_box: &mut LayoutBox) {
    if let Some(ref style) = layout_box.style
        && let Length::Px(px) = style.height
    {
        if style.box_sizing == BoxSizing::BorderBox {
            let padding_border = layout_box.dimensions.padding.vertical_total()
                + layout_box.dimensions.border.vertical_total();
            layout_box.dimensions.content.height = (px - padding_border).max(0.0);
        } else {
            layout_box.dimensions.content.height = px;
        }
    } else if let Some(intrinsic) = layout_box.intrinsic
        && intrinsic.width > 0
        && layout_box.dimensions.content.width > 0.0
    {
        let width = layout_box.dimensions.content.width;
        layout_box.dimensions.content.height =
            width * (intrinsic.height as f32 / intrinsic.width as f32);
    }
}
