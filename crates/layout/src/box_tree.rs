//! Box tree representation and generation from DOM nodes and computed styles.

use crate::geometry::{Dimensions, IntrinsicSize};
use css::{ComputedStyle, Display};
use dom::{Document, NodeData, NodeId};
use std::collections::HashMap;
use std::hash::BuildHasher;

/// Classification of a layout box node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoxType {
    /// Block-level box associated with a DOM element.
    BlockNode(NodeId),
    /// Inline-level box associated with a DOM element.
    InlineNode(NodeId),
    /// Text leaf box containing string content.
    TextNode(NodeId, String),
    /// Anonymous block box created to wrap inline children under a block parent.
    AnonymousBlock,
}

/// Node in the layout box tree holding geometric dimensions, styles, and child boxes.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutBox {
    /// Resolved box dimensions (content, padding, border, margin).
    pub dimensions: Dimensions,
    /// Box classification and DOM association.
    pub box_type: BoxType,
    /// Computed style if this box originates from a styled element.
    pub style: Option<ComputedStyle>,
    /// Intrinsic (natural) size for replaced elements such as `<img>`.
    pub intrinsic: Option<IntrinsicSize>,
    /// Child layout boxes in tree order.
    pub children: Vec<Self>,
}

impl LayoutBox {
    /// Creates a new `LayoutBox` with default initial dimensions.
    #[must_use]
    pub fn new(box_type: BoxType, style: Option<ComputedStyle>) -> Self {
        Self {
            dimensions: Dimensions::default(),
            box_type,
            style,
            intrinsic: None,
            children: Vec::new(),
        }
    }

    /// Returns `true` if this is a block-level container (`BlockNode` or `AnonymousBlock`).
    #[must_use]
    pub const fn is_block(&self) -> bool {
        matches!(
            self.box_type,
            BoxType::BlockNode(_) | BoxType::AnonymousBlock
        )
    }

    /// Returns `true` if this is an inline-level box (`InlineNode` or `TextNode`).
    #[must_use]
    pub const fn is_inline(&self) -> bool {
        matches!(
            self.box_type,
            BoxType::InlineNode(_) | BoxType::TextNode(_, _)
        )
    }

    /// Returns the associated DOM `NodeId` if this box corresponds to a DOM node.
    #[must_use]
    pub const fn node_id(&self) -> Option<NodeId> {
        match self.box_type {
            BoxType::BlockNode(id) | BoxType::InlineNode(id) | BoxType::TextNode(id, _) => Some(id),
            BoxType::AnonymousBlock => None,
        }
    }
}

/// Generates a `LayoutBox` tree from a `dom::Document` and resolved computed styles.
#[must_use]
pub fn build_box_tree<S: BuildHasher>(
    document: &Document,
    root_id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle, S>,
) -> Option<LayoutBox> {
    build_box_tree_with_intrinsics(document, root_id, styles, &HashMap::new())
}

/// Generates a `LayoutBox` tree, attaching intrinsic sizes to replaced elements.
///
/// `intrinsics` maps `<img>`-style element node ids to their natural dimensions;
/// block layout uses these to compute proportional heights from resolved widths.
#[must_use]
pub fn build_box_tree_with_intrinsics<S: BuildHasher, S2: BuildHasher>(
    document: &Document,
    root_id: NodeId,
    styles: &HashMap<NodeId, ComputedStyle, S>,
    intrinsics: &HashMap<NodeId, IntrinsicSize, S2>,
) -> Option<LayoutBox> {
    let node = document.get_node(root_id)?;

    match &node.data {
        NodeData::Document => {
            let mut root_box = LayoutBox::new(BoxType::BlockNode(root_id), None);
            for child_id in document.children(root_id) {
                if let Some(child_box) =
                    build_box_tree_with_intrinsics(document, child_id, styles, intrinsics)
                {
                    root_box.children.push(child_box);
                }
            }
            normalize_box_children(&mut root_box);
            Some(root_box)
        }
        NodeData::Element(_) => {
            let style = styles.get(&root_id)?.clone();
            if style.display == Display::None {
                return None;
            }

            let box_type = match style.display {
                Display::Block | Display::Flex | Display::Grid => BoxType::BlockNode(root_id),
                Display::Inline | Display::InlineBlock => BoxType::InlineNode(root_id),
                Display::None => unreachable!(),
            };

            let mut layout_box = LayoutBox::new(box_type, Some(style));
            layout_box.intrinsic = intrinsics.get(&root_id).copied();
            for child_id in document.children(root_id) {
                if let Some(child_box) =
                    build_box_tree_with_intrinsics(document, child_id, styles, intrinsics)
                {
                    layout_box.children.push(child_box);
                }
            }

            if layout_box.is_block() {
                normalize_box_children(&mut layout_box);
            }

            Some(layout_box)
        }
        NodeData::Text(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(LayoutBox::new(
                    BoxType::TextNode(root_id, text.clone()),
                    None,
                ))
            }
        }
        NodeData::DocumentType(_) | NodeData::Comment(_) => None,
    }
}

/// Enforces CSS 2.1 §9.2.1.1: if a block container has both block and inline children,
/// wrap consecutive sequences of inline children in anonymous block boxes.
/// Flex containers instead always wrap inline children in anonymous block boxes
/// (anonymous flex items, CSS Flexbox §4), so every child is a flex item.
fn normalize_box_children(parent_box: &mut LayoutBox) {
    let is_flex = parent_box
        .style
        .as_ref()
        .is_some_and(|s| s.display == Display::Flex);

    if is_flex {
        wrap_inline_runs(&mut parent_box.children);
        return;
    }

    let has_block_child = parent_box.children.iter().any(LayoutBox::is_block);
    let has_inline_child = parent_box.children.iter().any(LayoutBox::is_inline);

    if has_block_child && has_inline_child {
        wrap_inline_runs(&mut parent_box.children);
    }
}

/// Wraps consecutive runs of inline children in anonymous block boxes.
fn wrap_inline_runs(children: &mut Vec<LayoutBox>) {
    let mut normalized = Vec::new();
    let mut current_anon: Option<LayoutBox> = None;

    for child in children.drain(..) {
        if child.is_inline() {
            if let Some(ref mut anon) = current_anon {
                anon.children.push(child);
            } else {
                let mut anon = LayoutBox::new(BoxType::AnonymousBlock, None);
                anon.children.push(child);
                current_anon = Some(anon);
            }
        } else {
            if let Some(anon) = current_anon.take() {
                normalized.push(anon);
            }
            normalized.push(child);
        }
    }

    if let Some(anon) = current_anon.take() {
        normalized.push(anon);
    }

    *children = normalized;
}
