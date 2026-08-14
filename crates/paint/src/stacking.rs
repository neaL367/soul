//! Stacking context tree construction and hierarchy management per CSS 2.1 Appendix E.

use css::Position;
use layout::{BoxType, LayoutBox};
use std::cmp::Ordering;

/// Stacking context node managing child contexts and in-flow paint layers.
#[derive(Debug)]
pub struct StackingContext<'a> {
    /// Root layout box establishing this stacking context.
    pub root: &'a LayoutBox,
    /// Explicit or effective z-index stacking integer.
    pub z_index: i32,
    /// Element opacity multiplier.
    pub opacity: f32,
    /// Child stacking contexts with negative z-index (< 0), sorted ascending.
    pub negative_z_children: Vec<Self>,
    /// In-flow block-level descendant boxes.
    pub in_flow_blocks: Vec<&'a LayoutBox>,
    /// In-flow inline-level and text descendant boxes.
    pub in_flow_inlines: Vec<&'a LayoutBox>,
    /// Child stacking contexts with zero/auto z-index.
    pub zero_z_children: Vec<Self>,
    /// Child stacking contexts with positive z-index (> 0), sorted ascending.
    pub positive_z_children: Vec<Self>,
}

impl<'a> StackingContext<'a> {
    /// Creates a new `StackingContext` for the given root layout box.
    #[must_use]
    pub fn new(root: &'a LayoutBox) -> Self {
        let (z_index, opacity) = root
            .style
            .as_ref()
            .map_or((0, 1.0), |s| (s.z_index.unwrap_or(0), s.opacity));

        Self {
            root,
            z_index,
            opacity,
            negative_z_children: Vec::new(),
            in_flow_blocks: Vec::new(),
            in_flow_inlines: Vec::new(),
            zero_z_children: Vec::new(),
            positive_z_children: Vec::new(),
        }
    }
}

/// Builds a complete `StackingContext` tree from a root `LayoutBox`.
#[must_use]
pub fn build_stacking_tree(root_box: &LayoutBox) -> StackingContext<'_> {
    let mut root_context = StackingContext::new(root_box);

    for child in &root_box.children {
        collect_into_stacking_context(&mut root_context, child);
    }

    sort_stacking_context_children(&mut root_context);
    root_context
}

fn collect_into_stacking_context<'a>(context: &mut StackingContext<'a>, layout_box: &'a LayoutBox) {
    if creates_stacking_context(layout_box) {
        let mut child_context = StackingContext::new(layout_box);
        for child in &layout_box.children {
            collect_into_stacking_context(&mut child_context, child);
        }
        sort_stacking_context_children(&mut child_context);

        match child_context.z_index.cmp(&0) {
            Ordering::Less => context.negative_z_children.push(child_context),
            Ordering::Equal => context.zero_z_children.push(child_context),
            Ordering::Greater => context.positive_z_children.push(child_context),
        }
    } else {
        match layout_box.box_type {
            BoxType::BlockNode(_) | BoxType::AnonymousBlock => {
                context.in_flow_blocks.push(layout_box);
                for child in &layout_box.children {
                    collect_into_stacking_context(context, child);
                }
            }
            BoxType::InlineNode(_) | BoxType::TextNode(_, _) => {
                context.in_flow_inlines.push(layout_box);
                for child in &layout_box.children {
                    collect_into_stacking_context(context, child);
                }
            }
        }
    }
}

fn creates_stacking_context(layout_box: &LayoutBox) -> bool {
    let Some(ref style) = layout_box.style else {
        return false;
    };

    let is_positioned = style.position != Position::Static;
    let has_z_index = style.z_index.is_some();
    let has_opacity = style.opacity < 1.0 - f32::EPSILON;

    (is_positioned && has_z_index) || has_opacity
}

fn sort_stacking_context_children(context: &mut StackingContext) {
    context.negative_z_children.sort_by_key(|c| c.z_index);
    context.positive_z_children.sort_by_key(|c| c.z_index);
}
