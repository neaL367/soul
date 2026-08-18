//! DOM tree traversal and W3C Element Traversal helpers.

use crate::document::Document;
use crate::node::NodeId;

impl Document {
    /// Returns the first child that is an HTML element, or `None`.
    #[must_use]
    pub fn first_element_child(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = self.get_node(node_id).and_then(|n| n.first_child);
        while let Some(id) = current {
            if let Some(node) = self.get_node(id) {
                if node.is_element() {
                    return Some(id);
                }
                current = node.next_sibling;
            } else {
                break;
            }
        }
        None
    }

    /// Returns the last child that is an HTML element, or `None`.
    #[must_use]
    pub fn last_element_child(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = self.get_node(node_id).and_then(|n| n.last_child);
        while let Some(id) = current {
            if let Some(node) = self.get_node(id) {
                if node.is_element() {
                    return Some(id);
                }
                current = node.prev_sibling;
            } else {
                break;
            }
        }
        None
    }

    /// Returns the next sibling node that is an HTML element.
    #[must_use]
    pub fn next_element_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = self.get_node(node_id).and_then(|n| n.next_sibling);
        while let Some(id) = current {
            if let Some(node) = self.get_node(id) {
                if node.is_element() {
                    return Some(id);
                }
                current = node.next_sibling;
            } else {
                break;
            }
        }
        None
    }

    /// Returns the previous sibling node that is an HTML element.
    #[must_use]
    pub fn previous_element_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = self.get_node(node_id).and_then(|n| n.prev_sibling);
        while let Some(id) = current {
            if let Some(node) = self.get_node(id) {
                if node.is_element() {
                    return Some(id);
                }
                current = node.prev_sibling;
            } else {
                break;
            }
        }
        None
    }

    /// Returns the number of child element nodes under the given parent.
    #[must_use]
    pub fn child_element_count(&self, node_id: NodeId) -> usize {
        let mut count = 0;
        let mut current = self.get_node(node_id).and_then(|n| n.first_child);
        while let Some(id) = current {
            if let Some(node) = self.get_node(id) {
                if node.is_element() {
                    count += 1;
                }
                current = node.next_sibling;
            } else {
                break;
            }
        }
        count
    }

    /// Returns `true` if `other_id` is a descendant of `parent_id` or equal to `parent_id`.
    #[must_use]
    pub fn contains(&self, parent_id: NodeId, other_id: NodeId) -> bool {
        if parent_id == other_id {
            return true;
        }
        let mut current = self.get_node(other_id).and_then(|n| n.parent);
        while let Some(id) = current {
            if id == parent_id {
                return true;
            }
            current = self.get_node(id).and_then(|n| n.parent);
        }
        false
    }
}
