//! DOM node representations, identifiers, and element metadata.

use std::collections::HashMap;

/// Strongly-typed arena index referencing a DOM node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

/// Invalidation dirty flags for style, layout, and paint passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InvalidationFlags {
    /// Element or subtree requires style recalculation.
    pub style: bool,
    /// Element or subtree requires layout reflow.
    pub layout: bool,
    /// Element or subtree requires repaint.
    pub paint: bool,
}

impl InvalidationFlags {
    /// Creates flags with all dirty bits set.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            style: true,
            layout: true,
            paint: true,
        }
    }

    /// Clears all dirty flags.
    pub const fn clear(&mut self) {
        self.style = false;
        self.layout = false;
        self.paint = false;
    }
}

/// Metadata stored on HTML element nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementData {
    /// Local tag name in lowercase (e.g., "div", "span", "p").
    pub tag_name: String,
    /// Element attributes key-value map.
    pub attributes: HashMap<String, String>,
    /// Value of the `id` attribute if present.
    pub id: Option<String>,
    /// CSS class names from the `class` attribute.
    pub classes: Vec<String>,
}

impl ElementData {
    /// Creates a new `ElementData` with the given tag name and attributes.
    #[must_use]
    pub fn new(tag_name: &str, attributes: HashMap<String, String>) -> Self {
        let id = attributes.get("id").cloned();
        let classes = attributes
            .get("class")
            .map(|c| c.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        Self {
            tag_name: tag_name.to_ascii_lowercase(),
            attributes,
            id,
            classes,
        }
    }

    /// Returns the value of the specified attribute if present.
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// Checks if this element has the given CSS class.
    #[must_use]
    pub fn has_class(&self, class_name: &str) -> bool {
        self.classes.iter().any(|c| c == class_name)
    }

    /// Sets the value of an attribute and syncs `id` or `class` if modified.
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        self.attributes.insert(name.to_string(), value.to_string());
        if name.eq_ignore_ascii_case("id") {
            self.id = Some(value.to_string());
        } else if name.eq_ignore_ascii_case("class") {
            self.classes = value.split_whitespace().map(String::from).collect();
        }
    }

    /// Removes an attribute from the element.
    pub fn remove_attribute(&mut self, name: &str) {
        self.attributes.remove(name);
        if name.eq_ignore_ascii_case("id") {
            self.id = None;
        } else if name.eq_ignore_ascii_case("class") {
            self.classes.clear();
        }
    }

    /// Adds a CSS class name if not already present.
    pub fn add_class(&mut self, class_name: &str) {
        if !self.has_class(class_name) {
            self.classes.push(class_name.to_string());
            self.sync_class_attribute();
        }
    }

    /// Removes a CSS class name if present.
    pub fn remove_class(&mut self, class_name: &str) {
        if let Some(idx) = self.classes.iter().position(|c| c == class_name) {
            self.classes.remove(idx);
            self.sync_class_attribute();
        }
    }

    /// Toggles a CSS class name (returns `true` if added, `false` if removed).
    pub fn toggle_class(&mut self, class_name: &str) -> bool {
        if self.has_class(class_name) {
            self.remove_class(class_name);
            false
        } else {
            self.add_class(class_name);
            true
        }
    }

    fn sync_class_attribute(&mut self) {
        if self.classes.is_empty() {
            self.attributes.remove("class");
        } else {
            self.attributes
                .insert("class".to_string(), self.classes.join(" "));
        }
    }
}

/// Document type declaration metadata (e.g. `<!DOCTYPE html>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTypeData {
    /// Doctype name (e.g. "html").
    pub name: String,
    /// Public identifier.
    pub public_id: String,
    /// System identifier.
    pub system_id: String,
}

/// Payload contained within a DOM node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeData {
    /// Root document node.
    Document,
    /// Document type declaration.
    DocumentType(DocumentTypeData),
    /// HTML element with tag name and attributes.
    Element(ElementData),
    /// Text content.
    Text(String),
    /// HTML comment.
    Comment(String),
}

/// Arena-allocated DOM node containing data and structural bidirectional pointers.
#[derive(Debug, Clone)]
pub struct Node {
    /// Unique index of this node in the document arena.
    pub id: NodeId,
    /// Node type and payload data.
    pub data: NodeData,
    /// Parent node identifier.
    pub parent: Option<NodeId>,
    /// First child node identifier.
    pub first_child: Option<NodeId>,
    /// Last child node identifier.
    pub last_child: Option<NodeId>,
    /// Previous sibling node identifier.
    pub prev_sibling: Option<NodeId>,
    /// Next sibling node identifier.
    pub next_sibling: Option<NodeId>,
    /// Invalidation dirty flags.
    pub dirty_flags: InvalidationFlags,
}

impl Node {
    /// Creates a new unlinked arena node.
    #[must_use]
    pub const fn new(id: NodeId, data: NodeData) -> Self {
        Self {
            id,
            data,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            dirty_flags: InvalidationFlags::all(),
        }
    }

    /// Returns `true` if this node is an HTML element.
    #[must_use]
    pub const fn is_element(&self) -> bool {
        matches!(self.data, NodeData::Element(_))
    }

    /// Returns `true` if this node is a text node.
    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self.data, NodeData::Text(_))
    }

    /// Returns a reference to `ElementData` if this is an element node.
    #[must_use]
    pub const fn as_element(&self) -> Option<&ElementData> {
        match &self.data {
            NodeData::Element(data) => Some(data),
            _ => None,
        }
    }

    /// Returns a mutable reference to `ElementData` if this is an element node.
    pub const fn as_element_mut(&mut self) -> Option<&mut ElementData> {
        match &mut self.data {
            NodeData::Element(data) => Some(data),
            _ => None,
        }
    }

    /// Returns text content if this is a text node.
    #[must_use]
    pub const fn as_text(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }
}
