//! Flat arena-based DOM document containing node storage, pointer updates, and tree traversal.

pub mod mutation;
pub mod query;

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
    pub(crate) nodes: Vec<Node>,
    pub(crate) root_id: NodeId,
    pub(crate) doctype_id: Option<NodeId>,
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
            cur = self.nodes.get(id.0).and_then(|n| n.parent);
        }
        false
    }

    /// Returns true if `parent_id` is within `MAX_DOM_DEPTH` of the root.
    fn depth_ok(&self, parent_id: NodeId) -> bool {
        let mut depth = 0;
        let mut cur = Some(parent_id);
        while let Some(id) = cur {
            depth += 1;
            if depth > MAX_DOM_DEPTH {
                return false;
            }
            cur = self.nodes.get(id.0).and_then(|n| n.parent);
        }
        true
    }

    /// Appends `child_id` as the last child of `parent_id`.
    ///
    /// Refuses to attach the root node, avoids self-attachment, prevents cyclic
    /// parent chains, and enforces the `MAX_DOM_DEPTH` limit.
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        if child_id == self.root_id || child_id == parent_id {
            return;
        }
        if self.get_node(parent_id).is_none() || self.get_node(child_id).is_none() {
            return;
        }
        if self.is_ancestor(child_id, parent_id) {
            return;
        }
        if !self.depth_ok(parent_id) {
            return;
        }

        if let Some(old_parent) = self.nodes[child_id.0].parent {
            self.remove_child(old_parent, child_id);
        }

        let old_last = self.nodes[parent_id.0].last_child;

        self.nodes[child_id.0].parent = Some(parent_id);
        self.nodes[child_id.0].prev_sibling = old_last;
        self.nodes[child_id.0].next_sibling = None;

        if let Some(last_id) = old_last {
            self.nodes[last_id.0].next_sibling = Some(child_id);
        } else {
            self.nodes[parent_id.0].first_child = Some(child_id);
        }
        self.nodes[parent_id.0].last_child = Some(child_id);
        self.nodes[parent_id.0].dirty_flags = InvalidationFlags::all();
    }

    /// Inserts `child_id` immediately before the `before` sibling node.
    ///
    /// If `before` is `None`, `child_id` is appended as the last child.
    pub fn insert_before(&mut self, parent_id: NodeId, child_id: NodeId, before: Option<NodeId>) {
        let Some(before_id) = before else {
            self.append_child(parent_id, child_id);
            return;
        };

        if child_id == self.root_id || child_id == parent_id || child_id == before_id {
            return;
        }
        if self.get_node(parent_id).is_none()
            || self.get_node(child_id).is_none()
            || self.get_node(before_id).is_none()
        {
            return;
        }
        if self.nodes[before_id.0].parent != Some(parent_id) {
            return;
        }
        if self.is_ancestor(child_id, parent_id) {
            return;
        }
        if !self.depth_ok(parent_id) {
            return;
        }

        if let Some(old_parent) = self.nodes[child_id.0].parent {
            self.remove_child(old_parent, child_id);
        }

        let prev = self.nodes[before_id.0].prev_sibling;

        self.nodes[child_id.0].parent = Some(parent_id);
        self.nodes[child_id.0].prev_sibling = prev;
        self.nodes[child_id.0].next_sibling = Some(before_id);

        self.nodes[before_id.0].prev_sibling = Some(child_id);

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
}
