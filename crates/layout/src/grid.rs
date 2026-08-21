//! CSS Grid layout via `taffy::TaffyTree` (CSS Grid Level 1).

use crate::geometry::{Dimensions, EdgeSizes, Rect};
use css::{ComputedStyle, GridTrack, Length};
use taffy::prelude::*;
use taffy::style::GridTemplateComponent;

fn grid_track_to_taffy(track: GridTrack) -> GridTemplateComponent<String> {
    match track {
        GridTrack::Auto => auto(),
        GridTrack::Px(px) => length(px),
        GridTrack::Fr(f) => fr(f),
        GridTrack::Percent(p) => percent(p / 100.0),
    }
}

fn css_style_to_taffy_grid(style: &ComputedStyle, is_container: bool) -> Style {
    let width = match style.width {
        Length::Auto => Dimension::auto(),
        Length::Px(v) => Dimension::length(v),
        Length::Percent(v) => Dimension::percent(v / 100.0),
        Length::Em(v) | Length::Rem(v) => Dimension::length(v * 16.0),
    };
    let height = match style.height {
        Length::Auto => Dimension::auto(),
        Length::Px(v) => Dimension::length(v),
        Length::Percent(v) => Dimension::percent(v / 100.0),
        Length::Em(v) | Length::Rem(v) => Dimension::length(v * 16.0),
    };

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
        ..Default::default()
    };

    if is_container {
        s.display = Display::Grid;
        if style.grid_template_columns.is_empty() {
            // Default single auto column if none specified (behaves like block)
            s.grid_template_columns = vec![auto()];
        } else {
            s.grid_template_columns = style
                .grid_template_columns
                .iter()
                .copied()
                .map(grid_track_to_taffy)
                .collect();
        }
        if !style.grid_template_rows.is_empty() {
            s.grid_template_rows = style
                .grid_template_rows
                .iter()
                .copied()
                .map(grid_track_to_taffy)
                .collect();
        }
        if style.grid_gap > 0.0 {
            s.gap = Size {
                width: LengthPercentage::length(style.grid_gap),
                height: LengthPercentage::length(style.grid_gap),
            };
        }
    }

    s
}

/// Result of a grid layout pass for a single item.
#[derive(Debug, Clone)]
pub struct GridResult {
    /// Index of the child in the parent container's children list.
    pub index: usize,
    /// Resolved dimensions for this grid item.
    pub dimensions: Dimensions,
}

/// Aggregated result of a grid container layout pass.
#[derive(Debug, Clone)]
pub struct GridContainerResult {
    /// Resolved dimensions for each grid item.
    pub items: Vec<GridResult>,
    /// Resolved dimensions for the grid container itself.
    pub container: Dimensions,
}

/// Runs CSS Grid layout for a container with `display: grid`.
#[must_use]
pub fn layout_grid(
    container_style: &ComputedStyle,
    container_width: f32,
    children_styles: &[(usize, &ComputedStyle)],
) -> GridContainerResult {
    let mut tree: TaffyTree<()> = TaffyTree::new();

    let child_nodes: Vec<NodeId> = children_styles
        .iter()
        .map(|(_, child_style)| {
            let s = css_style_to_taffy_grid(child_style, false);
            tree.new_leaf(s).expect("taffy new_leaf failed")
        })
        .collect();

    let container_taffy = css_style_to_taffy_grid(container_style, true);
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
            GridResult {
                index: *idx,
                dimensions: Dimensions {
                    content: Rect {
                        x: layout.location.x,
                        y: layout.location.y,
                        width: layout.size.width,
                        height: layout.size.height,
                    },
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                },
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

    GridContainerResult { items, container }
}
