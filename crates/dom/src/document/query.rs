//! Document querying, tree traversal, and element searching utilities.

use crate::Document;
use crate::node::{NodeData, NodeId};

impl Document {
    /// Returns a list of direct children `NodeId`s for the specified node.
    #[must_use]
    pub fn children(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut list = Vec::new();
        let mut current = match self.get_node(node_id) {
            Some(node) => node.first_child,
            None => return list,
        };
        while let Some(id) = current {
            list.push(id);
            current = self.get_node(id).and_then(|n| n.next_sibling);
        }
        list
    }

    /// Returns a list of all descendant `NodeId`s in depth-first pre-order.
    #[must_use]
    pub fn descendants(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut list = Vec::new();
        let mut stack = self.children(node_id);
        stack.reverse();

        while let Some(current) = stack.pop() {
            list.push(current);
            let mut children = self.children(current);
            children.reverse();
            stack.extend(children);
        }
        list
    }

    /// Returns the concatenated text content of a node and all its descendants.
    #[must_use]
    pub fn text_content(&self, node_id: NodeId) -> String {
        let mut result = String::new();
        for id in std::iter::once(node_id).chain(self.descendants(node_id)) {
            if let NodeData::Text(ref text) = self.nodes[id.0].data {
                result.push_str(text);
            }
        }
        result
    }

    /// Finds the first element in the document with the matching `id` attribute.
    #[must_use]
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        self.descendants(self.root_id).into_iter().find(|&node_id| {
            self.nodes[node_id.0]
                .as_element()
                .and_then(|elem| elem.id.as_deref())
                == Some(id)
        })
    }

    /// Finds all elements matching the given tag name (case-insensitive).
    #[must_use]
    pub fn get_elements_by_tag_name(&self, tag: &str) -> Vec<NodeId> {
        let lower = tag.to_ascii_lowercase();
        self.descendants(self.root_id)
            .into_iter()
            .filter(|&node_id| {
                self.nodes[node_id.0]
                    .as_element()
                    .is_some_and(|elem| elem.tag_name == lower)
            })
            .collect()
    }

    /// Finds all elements containing the given CSS class.
    #[must_use]
    pub fn get_elements_by_class_name(&self, class_name: &str) -> Vec<NodeId> {
        self.descendants(self.root_id)
            .into_iter()
            .filter(|&node_id| {
                self.nodes[node_id.0]
                    .as_element()
                    .is_some_and(|elem| elem.has_class(class_name))
            })
            .collect()
    }
}
