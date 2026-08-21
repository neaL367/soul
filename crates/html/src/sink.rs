//! `TreeSink` implementation translating `html5ever` parser events into an arena `Document`.

use dom::{Document, DocumentTypeData, ElementData, MAX_NODES, NodeData, NodeId, ShadowRootMode};
use html5ever::tendril::StrTendril;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ExpandedName, QualName, local_name, namespace_url, ns};
use std::borrow::Cow;
use std::collections::HashMap;

/// Adapter implementing `html5ever::tree_builder::TreeSink` for `dom::Document`.
pub struct HtmlTreeSink {
    /// Constructed DOM document.
    pub document: Document,
    /// Quirks mode determined by doctype.
    pub quirks_mode: QuirksMode,
    /// Qualified names stored per element handle.
    pub qual_names: HashMap<NodeId, QualName>,
    /// Maps each `<template>` element `NodeId` to its `DocumentFragment` content host.
    pub template_contents: HashMap<NodeId, NodeId>,
    /// Name returned for sentinel handles whose allocation was refused.
    fallback_name: QualName,
}

impl Default for HtmlTreeSink {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlTreeSink {
    /// Creates a new `HtmlTreeSink` with an empty document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            document: Document::new(),
            quirks_mode: QuirksMode::NoQuirks,
            qual_names: HashMap::new(),
            template_contents: HashMap::new(),
            fallback_name: QualName::new(None, ns!(html), local_name!("")),
        }
    }

    /// True once the DOM arena is at its `MAX_NODES` ceiling.
    ///
    /// Beyond this point element creation is refused and subsequent tree
    /// mutations are no-ops so that a hostile page cannot exhaust memory.
    const fn at_node_limit(&self) -> bool {
        self.document.node_count() >= MAX_NODES
    }
}

impl TreeSink for HtmlTreeSink {
    type Handle = NodeId;
    type Output = Document;

    fn finish(self) -> Self::Output {
        self.document
    }

    fn parse_error(&mut self, msg: Cow<'static, str>) {
        tracing::debug!(message = %msg, "HTML5 parse diagnostic");
    }

    fn get_document(&mut self) -> Self::Handle {
        self.document.root_id()
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> ExpandedName<'a> {
        self.qual_names
            .get(target)
            .map_or_else(|| self.fallback_name.expanded(), QualName::expanded)
    }

    fn create_element(
        &mut self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> Self::Handle {
        if self.at_node_limit() {
            self.parse_error(Cow::Borrowed("DOM node limit exceeded; element dropped"));
            return self.document.root_id();
        }
        let mut attributes = HashMap::new();
        for attr in &attrs {
            attributes.insert(attr.name.local.to_string(), attr.value.to_string());
        }

        let tag_name = name.local.to_string();
        let element_data = ElementData::new(&tag_name, attributes);
        let node_id = self.document.alloc_node(NodeData::Element(element_data));
        self.qual_names.insert(node_id, name);

        // For `<template>` elements, allocate a DocumentFragment as the
        // content host per WHATWG HTML §4.12.3.
        if tag_name == "template" {
            // Detect Declarative Shadow DOM: `shadowrootmode="open"|"closed"`
            let shadow_mode = attrs.iter().find_map(|a| {
                if a.name.local.as_ref() == "shadowrootmode" {
                    match a.value.as_ref() {
                        "open" => Some(ShadowRootMode::Open),
                        "closed" => Some(ShadowRootMode::Closed),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            let content_id = if let Some(mode) = shadow_mode {
                self.document.alloc_node(NodeData::ShadowRoot(mode))
            } else {
                self.document.alloc_node(NodeData::DocumentFragment)
            };

            // Record the host relationship.
            if let Some(n) = self.document.get_node_mut(content_id) {
                n.host = Some(node_id);
            }
            self.template_contents.insert(node_id, content_id);
        }

        node_id
    }

    fn create_comment(&mut self, text: StrTendril) -> Self::Handle {
        if self.at_node_limit() {
            return self.document.root_id();
        }
        self.document
            .alloc_node(NodeData::Comment(text.to_string()))
    }

    fn create_pi(&mut self, target: StrTendril, data: StrTendril) -> Self::Handle {
        if self.at_node_limit() {
            return self.document.root_id();
        }
        let content = format!("?{target} {data}?");
        self.document.alloc_node(NodeData::Comment(content))
    }

    fn append(&mut self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        match child {
            NodeOrText::AppendNode(node_id) => {
                // The root sentinel can never be a real child; appending it
                // would be rejected as a cycle anyway, but skip it explicitly.
                if node_id != self.document.root_id() {
                    self.document.append_child(*parent, node_id);
                }
            }
            NodeOrText::AppendText(text) => {
                if !self.at_node_limit() {
                    self.document.append_text(*parent, &text);
                }
            }
        }
    }

    fn append_before_sibling(
        &mut self,
        sibling: &Self::Handle,
        new_node: NodeOrText<Self::Handle>,
    ) {
        // A parentless sibling (e.g. a node whose parent was removed during a
        // foster-parenting edge case) cannot be inserted before; drop the
        // mutation instead of panicking on untrusted input.
        let Some(parent) = self.document.get_node(*sibling).and_then(|n| n.parent) else {
            self.parse_error(Cow::Borrowed(
                "append_before_sibling on parentless node; mutation dropped",
            ));
            return;
        };

        match new_node {
            NodeOrText::AppendNode(node_id) => {
                if node_id != self.document.root_id() {
                    self.document.insert_before(parent, node_id, Some(*sibling));
                }
            }
            NodeOrText::AppendText(text) => {
                if !self.at_node_limit() {
                    let text_id = self.document.alloc_node(NodeData::Text(text.to_string()));
                    self.document.insert_before(parent, text_id, Some(*sibling));
                }
            }
        }
    }

    fn append_based_on_parent_node(
        &mut self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        if self
            .document
            .get_node(*element)
            .and_then(|n| n.parent)
            .is_some()
        {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &mut self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        if self.at_node_limit() {
            return;
        }
        let doctype_data = DocumentTypeData {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        };
        let doctype_id = self
            .document
            .alloc_node(NodeData::DocumentType(doctype_data));
        self.document
            .append_child(self.document.root_id(), doctype_id);
    }

    fn add_attrs_if_missing(&mut self, target: &Self::Handle, attrs: Vec<Attribute>) {
        if let Some(node) = self.document.get_node_mut(*target)
            && let Some(elem) = node.as_element_mut()
        {
            for attr in attrs {
                elem.attributes
                    .entry(attr.name.local.to_string())
                    .or_insert_with(|| attr.value.to_string());
            }
            // `id`/`class` caches must track attributes inserted outside `set_attribute`.
            elem.id = elem.attributes.get("id").cloned();
            elem.classes = elem
                .attributes
                .get("class")
                .map(|c| c.split_whitespace().map(String::from).collect())
                .unwrap_or_default();
        }
    }

    fn get_template_contents(&mut self, target: &Self::Handle) -> Self::Handle {
        // Return the allocated DocumentFragment content host if one exists,
        // otherwise fall back to the element itself (should not happen for
        // well-formed trees, but prevents a panic on malformed input).
        self.template_contents
            .get(target)
            .copied()
            .unwrap_or(*target)
    }

    fn remove_from_parent(&mut self, target: &Self::Handle) {
        if let Some(parent) = self.document.get_node(*target).and_then(|n| n.parent) {
            self.document.remove_child(parent, *target);
        }
    }

    fn reparent_children(&mut self, node: &Self::Handle, new_parent: &Self::Handle) {
        self.document.reparent_children(*node, *new_parent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use html5ever::tree_builder::{ElementFlags, NodeOrText};
    use html5ever::{local_name, ns};

    #[test]
    fn append_before_sibling_on_parentless_sibling_drops_mutation() {
        let mut sink = HtmlTreeSink::new();
        let root = sink.get_document();
        let name = QualName::new(None, ns!(html), local_name!("span"));
        let sibling = sink.create_element(name, Vec::new(), ElementFlags::default());

        // Sibling has no parent; the mutation must be dropped, not panic.
        sink.append_before_sibling(&sibling, NodeOrText::AppendText("x".into()));
        assert_eq!(sink.document.children(root).len(), 0);
    }
}
