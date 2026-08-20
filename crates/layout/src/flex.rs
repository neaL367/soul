//! CSS Flexbox layout via `taffy::TaffyTree` (CSS Flexbox Level 1).
//!
//! Translates Soul computed styles with `Display::Flex` into resolved
//! `Dimensions` by delegating to taffy for the flex algorithm, then writing
//! the taffy `Layout` output back into each child's `Dimensions`.
//!
//! # Taffy 0.13 API Notes
//! - `AlignItems`/`AlignSelf` expose constants like `AlignItems::STRETCH`,
//!   `AlignItems::FLEX_START`, `AlignItems::CENTER`, `AlignItems::BASELINE`.
//! - `JustifyContent` is a type alias for `AlignContent`; use
//!   `AlignContent::FLEX_START`, `AlignContent::SPACE_BETWEEN`, etc.
//! - `Dimension` uses `Dimension::auto()`, `Dimension::length(f32)`,
//!   `Dimension::percent(f32)`.

use crate::geometry::{Dimensions, EdgeSizes, Rect};
use css::{
    AlignItems as CssAlignItems, AlignSelf as CssAlignSelf, ComputedStyle, FlexDirection, FlexWrap,
    JustifyContent as CssJustifyContent, Length,
};
use taffy::prelude::*;

// -- CSS-to-taffy conversions -------------------------------------------------

const fn to_taffy_flex_direction(d: FlexDirection) -> taffy::FlexDirection {
    match d {
        FlexDirection::Row => taffy::FlexDirection::Row,
        FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
        FlexDirection::Column => taffy::FlexDirection::Column,
        FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
    }
}

const fn to_taffy_flex_wrap(w: FlexWrap) -> taffy::FlexWrap {
    match w {
        FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
        FlexWrap::Wrap => taffy::FlexWrap::Wrap,
        FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
    }
}

/// Maps `JustifyContent` to taffy's `AlignContent` (which is the underlying type alias).
const fn to_taffy_justify(j: CssJustifyContent) -> taffy::AlignContent {
    match j {
        CssJustifyContent::FlexStart => taffy::AlignContent::FLEX_START,
        CssJustifyContent::FlexEnd => taffy::AlignContent::FLEX_END,
        CssJustifyContent::Center => taffy::AlignContent::CENTER,
        CssJustifyContent::SpaceBetween => taffy::AlignContent::SPACE_BETWEEN,
        CssJustifyContent::SpaceAround => taffy::AlignContent::SPACE_AROUND,
        CssJustifyContent::SpaceEvenly => taffy::AlignContent::SPACE_EVENLY,
    }
}

const fn to_taffy_align_items(a: CssAlignItems) -> taffy::AlignItems {
    match a {
        CssAlignItems::Stretch => taffy::AlignItems::STRETCH,
        CssAlignItems::FlexStart => taffy::AlignItems::FLEX_START,
        CssAlignItems::FlexEnd => taffy::AlignItems::FLEX_END,
        CssAlignItems::Center => taffy::AlignItems::CENTER,
        CssAlignItems::Baseline => taffy::AlignItems::BASELINE,
    }
}

const fn to_taffy_align_self(a: CssAlignSelf) -> Option<taffy::AlignSelf> {
    match a {
        CssAlignSelf::Auto => None,
        CssAlignSelf::Stretch => Some(taffy::AlignSelf::STRETCH),
        CssAlignSelf::FlexStart => Some(taffy::AlignSelf::FLEX_START),
        CssAlignSelf::FlexEnd => Some(taffy::AlignSelf::FLEX_END),
        CssAlignSelf::Center => Some(taffy::AlignSelf::CENTER),
        CssAlignSelf::Baseline => Some(taffy::AlignSelf::BASELINE),
    }
}

fn length_to_dimension(l: Length) -> Dimension {
    match l {
        Length::Auto => Dimension::auto(),
        Length::Px(v) => Dimension::length(v),
        Length::Percent(v) => Dimension::percent(v / 100.0),
        Length::Em(v) | Length::Rem(v) => Dimension::length(v * 16.0),
    }
}

fn css_style_to_taffy(style: &ComputedStyle, is_container: bool) -> Style {
    let width = length_to_dimension(style.width);
    let height = length_to_dimension(style.height);

    let margin = taffy::Rect {
        left: LengthPercentageAuto::length(style.margin_left),
        right: LengthPercentageAuto::length(style.margin_right),
        top: LengthPercentageAuto::length(style.margin_top),
        bottom: LengthPercentageAuto::length(style.margin_bottom),
    };
    let padding = taffy::Rect {
        left: LengthPercentage::length(style.padding_left),
        right: LengthPercentage::length(style.padding_right),
        top: LengthPercentage::length(style.padding_top),
        bottom: LengthPercentage::length(style.padding_bottom),
    };

    let mut s = Style {
        size: Size { width, height },
        margin,
        padding,
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis: length_to_dimension(style.flex_basis),
        align_self: to_taffy_align_self(style.align_self),
        ..Default::default()
    };

    if is_container {
        s.display = taffy::Display::Flex;
        s.flex_direction = to_taffy_flex_direction(style.flex_direction);
        s.flex_wrap = to_taffy_flex_wrap(style.flex_wrap);
        s.justify_content = Some(to_taffy_justify(style.justify_content));
        s.align_items = Some(to_taffy_align_items(style.align_items));
    }

    s
}

// -- Public API ----------------------------------------------------------------

/// A flex-laid-out child with the resolved border-box origin and size.
#[derive(Debug, Clone)]
pub struct FlexResult {
    /// The child index (matching the input order).
    pub index: usize,
    /// Resolved border-box location (relative to the container's content box
    /// origin) and size after flex layout.
    pub dimensions: Dimensions,
}

/// Complete result of a flex layout pass: per-item results plus the
/// container's own resolved height.
#[derive(Debug, Clone)]
pub struct FlexContainerResult {
    /// Per-item flex results, one per input child, in input order.
    pub items: Vec<FlexResult>,
    /// The container's resolved content box after flex layout (width matches
    /// `container_width`; height is taffy's resolved border-box height minus
    /// the container's own padding and border).
    pub container: Dimensions,
}

/// Runs CSS Flexbox layout for a container with `display: flex`.
///
/// Returns one `FlexResult` per child, in the same order as `children_styles`,
/// plus the resolved container dimensions.
///
/// # Arguments
/// * `container_style` — computed style of the flex container.
/// * `container_width` — available width for the container in pixels.
/// * `children_styles` — slice of `(index, computed style)` for each flex item.
#[must_use]
pub fn layout_flex(
    container_style: &ComputedStyle,
    container_width: f32,
    children_styles: &[(usize, &ComputedStyle)],
) -> FlexContainerResult {
    let mut tree: TaffyTree<()> = TaffyTree::new();

    let child_nodes: Vec<NodeId> = children_styles
        .iter()
        .map(|(_, child_style)| {
            let s = css_style_to_taffy(child_style, false);
            tree.new_leaf(s).expect("taffy new_leaf failed")
        })
        .collect();

    let container_taffy = css_style_to_taffy(container_style, true);
    let root = tree
        .new_with_children(container_taffy, &child_nodes)
        .expect("taffy new_with_children failed");

    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(container_width),
            height: AvailableSpace::MaxContent,
        },
    )
    .expect("taffy compute_layout failed");

    let items = children_styles
        .iter()
        .zip(child_nodes.iter())
        .map(|((idx, _), node)| {
            let layout = tree.layout(*node).expect("taffy layout missing");
            FlexResult {
                index: *idx,
                dimensions: taffy_layout_to_dimensions(layout),
            }
        })
        .collect();

    let root_layout = tree.layout(root).expect("taffy layout missing");
    let padding_v = container_style.padding_top + container_style.padding_bottom;
    let border_v = container_style.border_top_width + container_style.border_bottom_width;
    let container = Dimensions {
        content: Rect::new(
            0.0,
            0.0,
            container_width,
            (root_layout.size.height - padding_v - border_v).max(0.0),
        ),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };

    FlexContainerResult { items, container }
}

/// Converts a taffy `Layout` into a Soul `Dimensions` struct.
///
/// The resulting `content` rect holds the taffy border-box location (relative
/// to the container's content-box origin) and border-box size; padding, border,
/// and margin are zeroed because the caller applies them from the child's own
/// computed style.
fn taffy_layout_to_dimensions(layout: &taffy::Layout) -> Dimensions {
    Dimensions {
        content: Rect {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        },
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    }
}
