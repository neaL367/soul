//! Cascade algorithm, selector matching, and top-down computed style resolution.

mod apply;
mod matching;

use apply::apply_declaration;
use matching::matches_selector;

use crate::properties::ComputedStyle;
use crate::rule::{Declaration, Origin, Specificity, StyleSheet};
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
                        if matches_selector(self.document, node_id, selector) {
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
}
