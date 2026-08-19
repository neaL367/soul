//! Flat arena-based DOM document containing node storage, pointer updates, and tree traversal.

use crate::node::{InvalidationFlags, Node, NodeData, NodeId};

/// Hard ceiling on arena size.
///
/// Once reached, `alloc_node` refuses to grow and returns the (always-valid)
/// root id; `append_child`/`insert_before` reject root as a child, so refused
/// allocations are simply dropped. This prevents unbounded memory growth from
/// untrusted parse feeds.
pub const MAX_NODES: usize = 1_000_000;

/// Maximum DOM tree depth per the W3C DOM recommendation (`deep` tree limit,
/// ~512 levels). Appends beyond this depth are refused instead of exhausting
/// the stack in recursive consumers.
pub const MAX_DOM_DEPTH: usize = 512;

/// Arena-based DOM document holding all nodes in a flat contiguous vector.
#[derive(Debug, Clone)]
pub struct Document {
    nodes: Vec<Node>,
    root_id: NodeId,
    doctype_id: Option<NodeId>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Creates a new empty `Document` with a root `Document` node at index 0.
    #[must_use]
    pub fn new() -> Self {
        let root_id = NodeId(0);
        let root_node = Node::new(root_id, NodeData::Document);
        Self {
            nodes: vec![root_node],
            root_id,
            doctype_id: None,
        }
    }

    /// Returns the root `Document` node identifier.
    #[must_use]
    pub const fn root_id(&self) -> NodeId {
        self.root_id
    }

    /// Returns the doctype node ID if one was parsed.
    #[must_use]
    pub const fn doctype_id(&self) -> Option<NodeId> {
        self.doctype_id
    }

    /// Returns the total number of nodes allocated in the arena.
    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Allocates a new node in the arena and returns its `NodeId`.
    ///
    /// If the arena has reached `MAX_NODES` the allocation is refused and the
    /// root id is returned; callers must treat that as a no-op and must not
    /// attempt to append the returned id (append APIs reject the root).
    pub fn alloc_node(&mut self, data: NodeData) -> NodeId {
        if self.nodes.len() >= MAX_NODES {
            return self.root_id;
        }
        let id = NodeId(self.nodes.len());
        if matches!(data, NodeData::DocumentType(_)) {
            self.doctype_id = Some(id);
        }
        self.nodes.push(Node::new(id, data));
        id
    }

    /// Returns a reference to the node with the given `NodeId`.
    #[must_use]
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// Returns a mutable reference to the node with the given `NodeId`.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0)
    }

    /// Returns true if `ancestor` is an ancestor of (or identical to) `of`.
    ///
    /// The walk is bounded by `MAX_DOM_DEPTH` iterations so it terminates even
    /// on a corrupted or cyclic parent chain.
    fn is_ancestor(&self, ancestor: NodeId, of: NodeId) -> bool {
        let mut steps = 0;
        let mut cur = Some(of);
        while let Some(id) = cur {
            if id == ancestor {
                return true;
            }
            steps += 1;
            if steps > MAX_DOM_DEPTH {
                return false;
            }
            cur = self.get_node(id).and_then(|n| n.parent);
        }
        false
    }

    /// Returns true if appending under `parent_id` keeps the tree within
    /// `MAX_DOM_DEPTH` levels (root = depth 0). Also bounded so it terminates
    /// on corrupt chains.
    fn depth_ok(&self, parent_id: NodeId) -> bool {
        let mut hops = 0;
        let mut cur = self.get_node(parent_id).and_then(|n| n.parent);
        while let Some(id) = cur {
            hops += 1;
            if hops >= MAX_DOM_DEPTH {
                return false;
            }
            cur = self.get_node(id).and_then(|n| n.parent);
        }
        true
    }

    /// Appends a child node to the end of the specified parent node's children list.
    ///
    /// No-ops (rather than panicking or corrupting the tree) when ids are
    /// invalid, when the append would create an ancestor cycle, or when the
    /// depth or arena limits would be exceeded.
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        if parent_id == child_id {
            return;
        }
        if self.get_node(parent_id).is_none() || self.get_node(child_id).is_none() {
            return;
        }
        // Reject cycles: `child` must not be an ancestor of `parent`.
        if self.is_ancestor(child_id, parent_id) {
            return;
        }
        if !self.depth_ok(parent_id) {
            return;
        }

        // Remove from existing parent if any
        if let Some(old_parent) = self.nodes[child_id.0].parent {
            self.remove_child(old_parent, child_id);
        }

        let last_child_id = self.nodes[parent_id.0].last_child;

        self.nodes[child_id.0].parent = Some(parent_id);
        self.nodes[child_id.0].prev_sibling = last_child_id;
        self.nodes[child_id.0].next_sibling = None;

        if let Some(last_id) = last_child_id {
            self.nodes[last_id.0].next_sibling = Some(child_id);
        } else {
            self.nodes[parent_id.0].first_child = Some(child_id);
        }

        self.nodes[parent_id.0].last_child = Some(child_id);
        self.nodes[parent_id.0].dirty_flags = InvalidationFlags::all();
    }

    /// Inserts a child node before a designated sibling node under a parent.
    ///
    /// No-ops when ids are invalid, when `before` is not a direct child of
    /// `parent`, or when the insert would create a cycle or exceed the limits.
    pub fn insert_before(
        &mut self,
        parent_id: NodeId,
        child_id: NodeId,
        before_id: Option<NodeId>,
    ) {
        let Some(before) = before_id else {
            self.append_child(parent_id, child_id);
            return;
        };

        if before == child_id || parent_id == child_id {
            return;
        }

        // `before` must actually be a child of `parent`, otherwise the sibling
        // chain would be corrupted.
        if self
            .get_node(before)
            .is_none_or(|n| n.parent != Some(parent_id))
        {
            return;
        }
        if self.get_node(parent_id).is_none() || self.get_node(child_id).is_none() {
            return;
        }
        // Reject cycles: `child` must not be an ancestor of `parent`.
        if self.is_ancestor(child_id, parent_id) {
            return;
        }
        if !self.depth_ok(parent_id) {
            return;
        }

        if let Some(old_parent) = self.nodes[child_id.0].parent {
            self.remove_child(old_parent, child_id);
        }

        let prev = self.nodes[before.0].prev_sibling;

        self.nodes[child_id.0].parent = Some(parent_id);
        self.nodes[child_id.0].prev_sibling = prev;
        self.nodes[child_id.0].next_sibling = Some(before);

        self.nodes[before.0].prev_sibling = Some(child_id);

        if let Some(prev_id) = prev {
            self.nodes[prev_id.0].next_sibling = Some(child_id);
        } else {
            self.nodes[parent_id.0].first_child = Some(child_id);
        }
        self.nodes[parent_id.0].dirty_flags = InvalidationFlags::all();
    }

    /// Removes a child node from its parent, repairing sibling and parent pointers.
    ///
    /// No-ops unless `child_id` is actually a direct child of `parent_id`.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        if self
            .get_node(child_id)
            .is_none_or(|n| n.parent != Some(parent_id))
        {
            return;
        }

        let prev = self.nodes[child_id.0].prev_sibling;
        let next = self.nodes[child_id.0].next_sibling;

        if let Some(prev_id) = prev {
            self.nodes[prev_id.0].next_sibling = next;
        } else if self.nodes[parent_id.0].first_child == Some(child_id) {
            self.nodes[parent_id.0].first_child = next;
        }

        if let Some(next_id) = next {
            self.nodes[next_id.0].prev_sibling = prev;
        } else if self.nodes[parent_id.0].last_child == Some(child_id) {
            self.nodes[parent_id.0].last_child = prev;
        }

        self.nodes[child_id.0].parent = None;
        self.nodes[child_id.0].prev_sibling = None;
        self.nodes[child_id.0].next_sibling = None;
        self.nodes[parent_id.0].dirty_flags = InvalidationFlags::all();
    }

    /// Moves all children of `old_parent` to the end of `new_parent`'s children list.
    pub fn reparent_children(&mut self, old_parent: NodeId, new_parent: NodeId) {
        if self.get_node(old_parent).is_none() || self.get_node(new_parent).is_none() {
            return;
        }
        let mut current = self.nodes[old_parent.0].first_child;
        while let Some(child_id) = current {
            let next = self.nodes[child_id.0].next_sibling;
            self.remove_child(old_parent, child_id);
            self.append_child(new_parent, child_id);
            current = next;
        }
    }

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

    /// Creates a new element node in the arena with the given tag name.
    pub fn create_element(&mut self, tag_name: &str) -> NodeId {
        let elem_data = crate::node::ElementData::new(tag_name, std::collections::HashMap::new());
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
}
