//! Accessibility semantic tree model capturing roles, accessible names, and bounds for Windows UI Automation.

use crate::box_tree::{BoxType, LayoutBox};
use crate::geometry::Rect;
use dom::{Document, NodeData};

/// Standard WAI-ARIA and HTML semantic accessibility roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yRole {
    /// Root document container (`role="document"`).
    Document,
    /// Heading elements (`h1`–`h6`, `role="heading"`).
    Heading,
    /// Paragraph or textual block content (`p`).
    Paragraph,
    /// Interactive button element (`button`, `role="button"`).
    Button,
    /// Hyperlink destination (`a[href]`, `role="link"`).
    Link,
    /// Embedded image graphic (`img`, `role="img"`).
    Image,
    /// Generic semantic structural container (`div`, `section`, `article`).
    GenericContainer,
}

/// Semantic node in the accessibility tree ready for Windows UI Automation provider queries.
#[derive(Debug, Clone, PartialEq)]
pub struct A11yNode {
    /// Node identifier correlated to DOM node.
    pub id: u64,
    /// Accessibility role.
    pub role: A11yRole,
    /// Accessible name (e.g. text content, alt text, or aria-label).
    pub name: Option<String>,
    /// Computed screen bounding box geometry.
    pub bounds: Rect,
    /// Hierarchical accessible child nodes.
    pub children: Vec<Self>,
}

impl A11yNode {
    /// Constructs an `A11yNode` tree from a laid-out box tree and source DOM document.
    #[must_use]
    pub fn from_layout_box(doc: &Document, layout_box: &LayoutBox) -> Option<Self> {
        let (node_id, role, name) = match &layout_box.box_type {
            BoxType::BlockNode(id) | BoxType::InlineNode(id) => {
                let node = doc.get_node(*id)?;
                let (role, name) = match &node.data {
                    NodeData::Element(elem) => {
                        let role = match elem.tag_name.as_str() {
                            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => A11yRole::Heading,
                            "p" => A11yRole::Paragraph,
                            "button" => A11yRole::Button,
                            "a" => A11yRole::Link,
                            "img" => A11yRole::Image,
                            _ => A11yRole::GenericContainer,
                        };
                        let name = elem.attr("aria-label").map(String::from);
                        (role, name)
                    }
                    NodeData::Document => (A11yRole::Document, None),
                    _ => (A11yRole::GenericContainer, None),
                };
                (id.0 as u64, role, name)
            }
            BoxType::TextNode(id, text) => (id.0 as u64, A11yRole::Paragraph, Some(text.clone())),
            BoxType::AnonymousBlock => (0, A11yRole::GenericContainer, None),
        };

        let children: Vec<Self> = layout_box
            .children
            .iter()
            .filter_map(|c| Self::from_layout_box(doc, c))
            .collect();

        Some(Self {
            id: node_id,
            role,
            name,
            bounds: layout_box.dimensions.content,
            children,
        })
    }
}
