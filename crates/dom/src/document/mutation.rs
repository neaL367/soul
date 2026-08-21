//! High-level DOM node and attribute mutation APIs with dirty flag invalidations.

use crate::Document;
use crate::node::{ElementData, InvalidationFlags, NodeData, NodeId};

impl Document {
    /// Appends text to a text node or creates a new text child if the last child isn't text.
    pub fn append_text(&mut self, parent_id: NodeId, text: &str) {
        if self.get_node(parent_id).is_none() {
            return;
        }
        let last_child_id = self.nodes[parent_id.0].last_child;
        if let Some(last_child_id) = last_child_id
            && let Some(node) = self.get_node_mut(last_child_id)
            && let NodeData::Text(ref mut existing) = node.data
        {
            existing.push_str(text);
            node.dirty_flags = InvalidationFlags::all();
            return;
        }
        let text_node_id = self.alloc_node(NodeData::Text(text.to_string()));
        self.append_child(parent_id, text_node_id);
    }

    /// Creates a new element node in the arena with the given tag name.
    pub fn create_element(&mut self, tag_name: &str) -> NodeId {
        let elem_data = ElementData::new(tag_name, std::collections::HashMap::new());
        self.alloc_node(NodeData::Element(elem_data))
    }

    /// Sets an attribute on an element and invalidates style/layout dirty flags.
    pub fn set_attribute(&mut self, node_id: NodeId, name: &str, value: &str) {
        if let Some(node) = self.get_node_mut(node_id)
            && let NodeData::Element(ref mut elem) = node.data
        {
            elem.set_attribute(name, value);
            node.dirty_flags.style = true;
            node.dirty_flags.layout = true;
        }
    }

    /// Removes an attribute from an element and invalidates style/layout dirty flags.
    pub fn remove_attribute(&mut self, node_id: NodeId, name: &str) {
        if let Some(node) = self.get_node_mut(node_id)
            && let NodeData::Element(ref mut elem) = node.data
        {
            elem.remove_attribute(name);
            node.dirty_flags.style = true;
            node.dirty_flags.layout = true;
        }
    }

    /// Sets the text content of an element, replacing its children with a single text node.
    pub fn set_text_content(&mut self, node_id: NodeId, text: &str) {
        let children = self.children(node_id);
        for child_id in children {
            self.remove_child(node_id, child_id);
        }
        let text_id = self.alloc_node(NodeData::Text(text.to_string()));
        self.append_child(node_id, text_id);
        if let Some(node) = self.get_node_mut(node_id) {
            node.dirty_flags.style = true;
            node.dirty_flags.layout = true;
            node.dirty_flags.paint = true;
        }
    }

    /// Clones a node and optionally all of its descendants.
    pub fn clone_node(&mut self, node_id: NodeId, deep: bool) -> NodeId {
        let Some(node) = self.get_node(node_id) else {
            return self.root_id;
        };

        let cloned_data = match &node.data {
            NodeData::Element(elem) => {
                let mut new_elem = ElementData::new(&elem.tag_name, elem.attributes.clone());
                new_elem.id.clone_from(&elem.id);
                new_elem.classes.clone_from(&elem.classes);
                NodeData::Element(new_elem)
            }
            NodeData::Text(text) => NodeData::Text(text.clone()),
            NodeData::Comment(text) => NodeData::Comment(text.clone()),
            NodeData::DocumentType(doctype) => NodeData::DocumentType(doctype.clone()),
            NodeData::Document => NodeData::Document,
            NodeData::DocumentFragment => NodeData::DocumentFragment,
            NodeData::ShadowRoot(mode) => NodeData::ShadowRoot(*mode),
        };

        let new_id = self.alloc_node(cloned_data);

        if deep {
            let children = self.children(node_id);
            for child in children {
                let cloned_child = self.clone_node(child, true);
                self.append_child(new_id, cloned_child);
            }
        }

        new_id
    }
}
