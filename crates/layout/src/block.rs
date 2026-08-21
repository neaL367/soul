//! Normal flow block layout algorithm with W3C box-sizing and CSS 2.1 §8.3.1 margin collapsing.

use crate::box_tree::LayoutBox;
use crate::calc::{LengthContext, resolve_length};
use crate::flex::layout_flex;
use crate::geometry::{Dimensions, EdgeSizes};
use crate::grid::layout_grid;
use css::{BoxSizing, ComputedStyle, Display, Position};

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

    // 3. Recursively layout in-flow children
    if depth < MAX_LAYOUT_DEPTH {
        layout_children(layout_box, depth);
    }

    // 4. Calculate final height
    calculate_block_height(layout_box);

    // 5. Layout out-of-flow children after container dimensions and height are finalized
    layout_out_of_flow_children(layout_box, depth);
}

/// Returns `true` if the box is a flex container (`display: flex`).
fn is_flex_container(layout_box: &LayoutBox) -> bool {
    layout_box
        .style
        .as_ref()
        .is_some_and(|s| s.display == Display::Flex)
}

/// Returns `true` if the box is a grid container (`display: grid`).
fn is_grid_container(layout_box: &LayoutBox) -> bool {
    layout_box
        .style
        .as_ref()
        .is_some_and(|s| s.display == Display::Grid)
}

/// Returns `true` if the box is out-of-flow (absolute or fixed).
fn is_out_of_flow(layout_box: &LayoutBox) -> bool {
    layout_box
        .style
        .as_ref()
        .is_some_and(|s| matches!(s.position, Position::Absolute | Position::Fixed))
}

/// Lays out a container's in-flow children.
fn layout_children(layout_box: &mut LayoutBox, depth: usize) {
    if is_flex_container(layout_box) {
        layout_flex_children(layout_box, depth);
    } else if is_grid_container(layout_box) {
        layout_grid_children(layout_box, depth);
    } else {
        layout_block_children(layout_box, depth);
    }
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

    let ctx = LengthContext::new(containing_block.content.width, 800.0, 600.0)
        .with_font_sizes(style.font_size, 16.0);

    let width = resolve_length(&style.width, &ctx).map_or_else(
        || (containing_block.content.width - total_spacing).max(0.0),
        |resolved| {
            if style.box_sizing == BoxSizing::BorderBox {
                (resolved - (padding.horizontal_total() + border.horizontal_total())).max(0.0)
            } else {
                resolved
            }
        },
    );

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
    // Only in-flow children participate in normal flow.
    let has_in_flow_inline = layout_box
        .children
        .iter()
        .filter(|c| !is_out_of_flow(c))
        .any(LayoutBox::is_inline);
    if has_in_flow_inline {
        let max_w = layout_box.dimensions.content.width;
        let inline_h = crate::inline::layout_inline_context(layout_box, max_w);
        // Out-of-flow children were skipped inside inline context; they will be
        // positioned in the second pass, so we do not include them in height here
        // beyond what inline context already did (which ignores them).
        layout_box.dimensions.content.height = inline_h;
        return;
    }

    let mut vertical_offset = 0.0;
    let mut prev_margin_bottom = 0.0;
    let mut is_first = true;

    for child in &mut layout_box.children {
        if is_out_of_flow(child) {
            continue;
        }
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
                layout_children(child, depth + 1);
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

fn layout_out_of_flow_children(layout_box: &mut LayoutBox, depth: usize) {
    // Position absolute/fixed children out-of-flow; they do not affect parent height.
    let containing = layout_box.dimensions;
    for child in &mut layout_box.children {
        if !is_out_of_flow(child) {
            continue;
        }
        calculate_block_width(child, &containing);

        if depth < MAX_LAYOUT_DEPTH {
            layout_children(child, depth + 1);
        }
        calculate_block_height(child);

        let (offset_x, offset_y) = if let Some(ref style) = child.style {
            let ctx_w = LengthContext::new(containing.content.width, 800.0, 600.0)
                .with_font_sizes(style.font_size, 16.0);
            let ctx_h = LengthContext::new(containing.content.height, 800.0, 600.0)
                .with_font_sizes(style.font_size, 16.0);

            let x = if let Some(left) = resolve_length(&style.left, &ctx_w) {
                containing.content.x
                    + left
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
            } else if let Some(right) = resolve_length(&style.right, &ctx_w) {
                let child_box_w = child.dimensions.content.width
                    + child.dimensions.padding.horizontal_total()
                    + child.dimensions.border.horizontal_total();
                containing.content.x + containing.content.width
                    - right
                    - child.dimensions.margin.right
                    - child_box_w
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
            } else {
                containing.content.x
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
            };

            let y = if let Some(top) = resolve_length(&style.top, &ctx_h) {
                containing.content.y
                    + top
                    + child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
            } else if let Some(bottom) = resolve_length(&style.bottom, &ctx_h) {
                let child_box_h = child.dimensions.content.height
                    + child.dimensions.padding.vertical_total()
                    + child.dimensions.border.vertical_total();
                containing.content.y + containing.content.height
                    - bottom
                    - child.dimensions.margin.bottom
                    - child_box_h
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
            } else {
                containing.content.y
                    + child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
            };

            (x, y)
        } else {
            (
                containing.content.x
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left,
                containing.content.y
                    + child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top,
            )
        };

        child.dimensions.content.x = offset_x;
        child.dimensions.content.y = offset_y;
    }
}

/// Lays out the children of a flex container.
///
/// The flex algorithm (taffy) resolves each child's border-box location and
/// size; this function writes those results back into the child `LayoutBox`
/// boxes (converting border-box geometry into content-box geometry using each
/// child's own padding and border), then recurses into each child's contents.
/// The container's content height comes from taffy's resolved root height.
fn layout_flex_children(layout_box: &mut LayoutBox, depth: usize) {
    let Some(container_style) = layout_box.style.clone() else {
        return;
    };
    let container_width = layout_box.dimensions.content.width;

    let owned_styles: Vec<(usize, ComputedStyle)> = layout_box
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| !is_out_of_flow(child))
        .map(|(i, child)| (i, child.style.clone().unwrap_or_default()))
        .collect();
    let child_refs: Vec<(usize, &ComputedStyle)> =
        owned_styles.iter().map(|(i, style)| (*i, style)).collect();
    let flex = layout_flex(&container_style, container_width, &child_refs);

    let origin_x = layout_box.dimensions.content.x;
    let origin_y = layout_box.dimensions.content.y;
    for result in &flex.items {
        let child = &mut layout_box.children[result.index];
        if child.style.is_some() {
            // Populates the child's padding, border, and margin from its style;
            // content width is overridden with the flex-resolved width below.
            calculate_block_width(child, &layout_box.dimensions);
        } else {
            child.dimensions.padding = EdgeSizes::default();
            child.dimensions.border = EdgeSizes::default();
            child.dimensions.margin = EdgeSizes::default();
        }

        // Taffy reports border-box location (relative to the container's
        // content-box origin) and border-box size; convert to content-box.
        let border_box = &result.dimensions.content;
        let pad_left = child.dimensions.padding.left;
        let pad_top = child.dimensions.padding.top;
        let border_left = child.dimensions.border.left;
        let border_top = child.dimensions.border.top;
        let pad_border_right = child.dimensions.padding.right + child.dimensions.border.right;
        let pad_border_bottom = child.dimensions.padding.bottom + child.dimensions.border.bottom;
        child.dimensions.content.x = origin_x + border_box.x + border_left + pad_left;
        child.dimensions.content.y = origin_y + border_box.y + border_top + pad_top;
        child.dimensions.content.width =
            (border_box.width - border_left - pad_left - pad_border_right).max(0.0);
        child.dimensions.content.height =
            (border_box.height - border_top - pad_top - pad_border_bottom).max(0.0);
    }

    for i in 0..layout_box.children.len() {
        if depth < MAX_LAYOUT_DEPTH {
            layout_children(&mut layout_box.children[i], depth + 1);
        }
        // Normal-flow child layout resets the height of empty containers to 0;
        // re-apply the flex-resolved height for items with definite heights
        // (style `height` or intrinsic ratio), matching block semantics.
        calculate_block_height(&mut layout_box.children[i]);
    }

    layout_box.dimensions.content.height = flex.container.content.height;
}

/// Lays out the children of a grid container via taffy.
fn layout_grid_children(layout_box: &mut LayoutBox, depth: usize) {
    let Some(container_style) = layout_box.style.clone() else {
        return;
    };
    let container_width = layout_box.dimensions.content.width;

    let owned_styles: Vec<(usize, ComputedStyle)> = layout_box
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| !is_out_of_flow(child))
        .map(|(i, child)| (i, child.style.clone().unwrap_or_default()))
        .collect();
    let child_refs: Vec<(usize, &ComputedStyle)> =
        owned_styles.iter().map(|(i, style)| (*i, style)).collect();
    let grid = layout_grid(&container_style, container_width, &child_refs);

    let origin_x = layout_box.dimensions.content.x;
    let origin_y = layout_box.dimensions.content.y;
    for result in &grid.items {
        let child = &mut layout_box.children[result.index];
        if child.style.is_some() {
            calculate_block_width(child, &layout_box.dimensions);
        } else {
            child.dimensions.padding = EdgeSizes::default();
            child.dimensions.border = EdgeSizes::default();
            child.dimensions.margin = EdgeSizes::default();
        }
        let border_box = &result.dimensions.content;
        let pad_left = child.dimensions.padding.left;
        let pad_top = child.dimensions.padding.top;
        let border_left = child.dimensions.border.left;
        let border_top = child.dimensions.border.top;
        let pad_border_right = child.dimensions.padding.right + child.dimensions.border.right;
        let pad_border_bottom = child.dimensions.padding.bottom + child.dimensions.border.bottom;
        child.dimensions.content.x = origin_x + border_box.x + border_left + pad_left;
        child.dimensions.content.y = origin_y + border_box.y + border_top + pad_top;
        child.dimensions.content.width =
            (border_box.width - border_left - pad_left - pad_border_right).max(0.0);
        child.dimensions.content.height =
            (border_box.height - border_top - pad_top - pad_border_bottom).max(0.0);
    }

    for i in 0..layout_box.children.len() {
        if depth < MAX_LAYOUT_DEPTH {
            layout_children(&mut layout_box.children[i], depth + 1);
        }
        calculate_block_height(&mut layout_box.children[i]);
    }

    layout_box.dimensions.content.height = grid.container.content.height;
}

#[allow(clippy::cast_precision_loss)]
fn calculate_block_height(layout_box: &mut LayoutBox) {
    if let Some(ref style) = layout_box.style {
        let ctx = LengthContext::new(layout_box.dimensions.content.height, 800.0, 600.0)
            .with_font_sizes(style.font_size, 16.0);
        if let Some(resolved) = resolve_length(&style.height, &ctx) {
            if style.box_sizing == BoxSizing::BorderBox {
                let padding_border = layout_box.dimensions.padding.vertical_total()
                    + layout_box.dimensions.border.vertical_total();
                layout_box.dimensions.content.height = (resolved - padding_border).max(0.0);
            } else {
                layout_box.dimensions.content.height = resolved;
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
}
