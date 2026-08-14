//! Cascade algorithm, selector matching, and top-down computed style resolution.

use crate::properties::{Color, ComputedStyle, Display, FontWeight, Position, TextAlign};
use crate::rule::{
    Combinator, Declaration, Origin, Selector, SimpleSelector, Specificity, StyleSheet,
};
use crate::ua::user_agent_stylesheet;
use dom::{Document, NodeId};
use std::collections::HashMap;

/// Matched declaration candidate with cascade sorting metadata.
#[derive(Debug, Clone)]
struct MatchedDecl<'a> {
    declaration: &'a Declaration,
    cascade_level: u8,
    specificity: Specificity,
    source_order: usize,
}

/// Resolves computed styles for all elements in a DOM document.
pub struct CascadeResolver<'a> {
    document: &'a Document,
    stylesheets: Vec<&'a StyleSheet>,
}

impl<'a> CascadeResolver<'a> {
    /// Creates a new `CascadeResolver` for the given document and author stylesheets.
    #[must_use]
    pub fn new(document: &'a Document, author_sheets: &[&'a StyleSheet]) -> Self {
        let mut stylesheets = vec![user_agent_stylesheet()];
        stylesheets.extend_from_slice(author_sheets);

        Self {
            document,
            stylesheets,
        }
    }

    /// Resolves the computed styles for every element in the document.
    #[must_use]
    pub fn resolve_all(&self) -> HashMap<NodeId, ComputedStyle> {
        let mut styles = HashMap::new();
        self.resolve_node(self.document.root_id(), None, &mut styles);
        styles
    }

    fn resolve_node(
        &self,
        node_id: NodeId,
        parent_style: Option<&ComputedStyle>,
        styles: &mut HashMap<NodeId, ComputedStyle>,
    ) {
        let Some(node) = self.document.get_node(node_id) else {
            return;
        };

        if node.is_element() {
            let mut style = ComputedStyle::initial();
            if let Some(parent) = parent_style {
                style.inherit_from(parent);
            }

            let mut matched = Vec::new();
            let mut order = 0;

            for sheet in &self.stylesheets {
                for rule in &sheet.rules {
                    for selector in &rule.selectors {
                        if self.matches_selector(node_id, selector) {
                            let spec = selector.specificity();
                            for decl in &rule.declarations {
                                order += 1;
                                let cascade_level = match (rule.origin, decl.important) {
                                    (Origin::UserAgent, false) => 1,
                                    (Origin::Author, false) => 2,
                                    (Origin::Author, true) => 3,
                                    (Origin::UserAgent, true) => 4,
                                };
                                matched.push(MatchedDecl {
                                    declaration: decl,
                                    cascade_level,
                                    specificity: spec,
                                    source_order: order,
                                });
                            }
                        }
                    }
                }
            }

            matched.sort_by(|a, b| {
                a.cascade_level
                    .cmp(&b.cascade_level)
                    .then_with(|| a.specificity.cmp(&b.specificity))
                    .then_with(|| a.source_order.cmp(&b.source_order))
            });

            for m in matched {
                apply_declaration(&mut style, m.declaration);
            }

            styles.insert(node_id, style);
        }

        let current_style = styles.get(&node_id).cloned();
        let pass_down_style = current_style.as_ref().or(parent_style);

        for child_id in self.document.children(node_id) {
            self.resolve_node(child_id, pass_down_style, styles);
        }
    }

    fn matches_selector(&self, node_id: NodeId, selector: &Selector) -> bool {
        let mut curr_node = Some(node_id);
        let mut parts = selector.sequence.iter().rev();

        let Some((_, first_simple)) = parts.next() else {
            return false;
        };

        if !self.matches_simple(node_id, first_simple) {
            return false;
        }

        for (comb, simple) in parts {
            let combinator = comb.unwrap_or(Combinator::Descendant);
            match combinator {
                Combinator::Child => {
                    curr_node = self
                        .document
                        .get_node(curr_node.unwrap())
                        .and_then(|n| n.parent);
                    match curr_node {
                        Some(p) if self.matches_simple(p, simple) => {}
                        _ => return false,
                    }
                }
                Combinator::Descendant => {
                    let mut matched = false;
                    while let Some(parent_id) = self
                        .document
                        .get_node(curr_node.unwrap())
                        .and_then(|n| n.parent)
                    {
                        curr_node = Some(parent_id);
                        if self.matches_simple(parent_id, simple) {
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn matches_simple(&self, node_id: NodeId, simple: &SimpleSelector) -> bool {
        let Some(node) = self.document.get_node(node_id) else {
            return false;
        };

        let Some(elem) = node.as_element() else {
            return false;
        };

        match simple {
            SimpleSelector::Universal => true,
            SimpleSelector::Tag(tag) => elem.tag_name == *tag,
            SimpleSelector::Id(id) => elem.id.as_deref() == Some(id.as_str()),
            SimpleSelector::Class(class) => elem.has_class(class),
        }
    }
}

#[allow(clippy::too_many_lines)]
fn apply_declaration(style: &mut ComputedStyle, decl: &Declaration) {
    match decl.property.as_str() {
        "display" => match decl.value.as_str() {
            "block" => style.display = Display::Block,
            "inline" => style.display = Display::Inline,
            "inline-block" => style.display = Display::InlineBlock,
            "flex" => style.display = Display::Flex,
            "grid" => style.display = Display::Grid,
            "none" => style.display = Display::None,
            _ => {}
        },
        "position" => match decl.value.as_str() {
            "static" => style.position = Position::Static,
            "relative" => style.position = Position::Relative,
            "absolute" => style.position = Position::Absolute,
            "fixed" => style.position = Position::Fixed,
            "sticky" => style.position = Position::Sticky,
            _ => {}
        },
        "color" => {
            if let Some(c) = Color::parse(&decl.value) {
                style.color = c;
            }
        }
        "background-color" | "background" => {
            if let Some(c) = Color::parse(&decl.value) {
                style.background_color = c;
            }
        }
        "opacity" => {
            if let Ok(val) = decl.value.parse::<f32>() {
                style.opacity = val.clamp(0.0, 1.0);
            }
        }
        "font-size" => {
            if let Some(px) = parse_px(&decl.value) {
                style.font_size = px;
            }
        }
        "font-weight" => match decl.value.as_str() {
            "bold" | "700" => style.font_weight = FontWeight::Bold,
            "normal" | "400" => style.font_weight = FontWeight::Normal,
            _ => {}
        },
        "text-align" => match decl.value.as_str() {
            "left" => style.text_align = TextAlign::Left,
            "right" => style.text_align = TextAlign::Right,
            "center" => style.text_align = TextAlign::Center,
            "justify" => style.text_align = TextAlign::Justify,
            _ => {}
        },
        "margin" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_top = px;
                style.margin_right = px;
                style.margin_bottom = px;
                style.margin_left = px;
            }
        }
        "margin-top" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_top = px;
            }
        }
        "margin-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_bottom = px;
            }
        }
        "margin-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_left = px;
            }
        }
        "margin-right" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_right = px;
            }
        }
        "padding" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_top = px;
                style.padding_right = px;
                style.padding_bottom = px;
                style.padding_left = px;
            }
        }
        "padding-top" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_top = px;
            }
        }
        "padding-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_bottom = px;
            }
        }
        "padding-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_left = px;
            }
        }
        "padding-right" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_right = px;
            }
        }
        _ => {}
    }
}

fn parse_px(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    trimmed.strip_suffix("px").map_or_else(
        || trimmed.parse::<f32>().ok(),
        |num| num.trim().parse::<f32>().ok(),
    )
}
