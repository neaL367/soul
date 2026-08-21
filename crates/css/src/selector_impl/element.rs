//! DOM Element adapter implementing `selectors::Element`.

use super::types::{SoulName, SoulNamespace, SoulPseudoClass, SoulPseudoElement, SoulSelectorImpl};
use dom::{Document, NodeId};
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::MatchingContext;
use selectors::matching::ElementSelectorFlags;
use selectors::{Element, OpaqueElement};

/// Wrapper that adapts `dom::Document` + `NodeId` to `selectors::Element`.
#[derive(Clone, Debug)]
pub struct DomElement<'a> {
    /// Reference to the document arena.
    pub document: &'a Document,
    /// Node identifier of the element.
    pub id: NodeId,
}

impl<'a> DomElement<'a> {
    /// Creates a new `DomElement` wrapper.
    #[must_use]
    pub const fn new(document: &'a Document, id: NodeId) -> Self {
        Self { document, id }
    }

    fn element_data(&self) -> Option<&dom::ElementData> {
        self.document.get_node(self.id)?.as_element()
    }

    fn is_element_node(&self) -> bool {
        self.document
            .get_node(self.id)
            .is_some_and(dom::Node::is_element)
    }
}

impl Element for DomElement<'_> {
    type Impl = SoulSelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        self.document
            .get_node(self.id)
            .map_or_else(|| OpaqueElement::new(&self.id), OpaqueElement::new)
    }

    fn parent_element(&self) -> Option<Self> {
        let node = self.document.get_node(self.id)?;
        let parent_id = node.parent?;
        let parent_node = self.document.get_node(parent_id)?;
        if parent_node.is_element() {
            Some(Self {
                document: self.document,
                id: parent_id,
            })
        } else if parent_node.data == dom::NodeData::Document {
            None
        } else {
            let mut cur = parent_node.parent;
            while let Some(pid) = cur {
                if let Some(n) = self.document.get_node(pid) {
                    if n.is_element() {
                        return Some(Self {
                            document: self.document,
                            id: pid,
                        });
                    }
                    cur = n.parent;
                } else {
                    break;
                }
            }
            None
        }
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let node = self.document.get_node(self.id)?;
        let mut cur = node.prev_sibling;
        while let Some(sib_id) = cur {
            if let Some(sib) = self.document.get_node(sib_id) {
                if sib.is_element() {
                    return Some(Self {
                        document: self.document,
                        id: sib_id,
                    });
                }
                cur = sib.prev_sibling;
            } else {
                break;
            }
        }
        None
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let node = self.document.get_node(self.id)?;
        let mut cur = node.next_sibling;
        while let Some(sib_id) = cur {
            if let Some(sib) = self.document.get_node(sib_id) {
                if sib.is_element() {
                    return Some(Self {
                        document: self.document,
                        id: sib_id,
                    });
                }
                cur = sib.next_sibling;
            } else {
                break;
            }
        }
        None
    }

    fn first_element_child(&self) -> Option<Self> {
        let node = self.document.get_node(self.id)?;
        let mut cur = node.first_child;
        while let Some(child_id) = cur {
            if let Some(child) = self.document.get_node(child_id) {
                if child.is_element() {
                    return Some(Self {
                        document: self.document,
                        id: child_id,
                    });
                }
                cur = child.next_sibling;
            } else {
                break;
            }
        }
        None
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.is_element_node()
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.element_data()
            .is_some_and(|e| e.tag_name == local_name.to_ascii_lowercase())
    }

    fn has_namespace(&self, ns: &str) -> bool {
        ns.is_empty()
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.element_data()
            .zip(other.element_data())
            .is_some_and(|(a, b)| a.tag_name == b.tag_name)
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&SoulNamespace>,
        local_name: &SoulName,
        operation: &AttrSelectorOperation<&SoulName>,
    ) -> bool {
        if let NamespaceConstraint::Specific(url) = ns
            && !url.0.is_empty()
        {
            return false;
        }
        let Some(elem) = self.element_data() else {
            return false;
        };
        let key = local_name.0.to_ascii_lowercase();
        let Some(attr_val) = elem
            .attributes
            .get(key.as_str())
            .or_else(|| elem.attributes.get(local_name.0.as_str()))
        else {
            return false;
        };
        if matches!(operation, AttrSelectorOperation::Exists) {
            true
        } else {
            operation.eval_str(attr_val)
        }
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &SoulPseudoClass,
        _context: &mut MatchingContext<SoulSelectorImpl>,
    ) -> bool {
        match pc.0.as_str() {
            "link" | "any-link" => self.element_data().is_some_and(|e| {
                matches!(e.tag_name.as_str(), "a" | "area" | "link")
                    && e.attributes.contains_key("href")
            }),
            _ => false,
        }
    }

    fn match_pseudo_element(
        &self,
        _pe: &SoulPseudoElement,
        _context: &mut MatchingContext<SoulSelectorImpl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        self.element_data().is_some_and(|e| {
            matches!(e.tag_name.as_str(), "a" | "area" | "link")
                && e.attributes.contains_key("href")
        })
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &SoulName, case_sensitivity: CaseSensitivity) -> bool {
        self.element_data().is_some_and(|e| {
            e.id.as_ref()
                .is_some_and(|elem_id| case_sensitivity.eq(id.0.as_bytes(), elem_id.as_bytes()))
        })
    }

    fn has_class(&self, name: &SoulName, case_sensitivity: CaseSensitivity) -> bool {
        self.element_data().is_some_and(|e| {
            e.classes
                .iter()
                .any(|c| case_sensitivity.eq(c.as_bytes(), name.0.as_bytes()))
        })
    }

    fn has_custom_state(&self, _name: &SoulName) -> bool {
        false
    }

    fn imported_part(&self, _name: &SoulName) -> Option<SoulName> {
        None
    }

    fn is_part(&self, _name: &SoulName) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        let Some(node) = self.document.get_node(self.id) else {
            return false;
        };
        let mut child = node.first_child;
        while let Some(child_id) = child {
            if let Some(cnode) = self.document.get_node(child_id) {
                match &cnode.data {
                    dom::NodeData::Element(_) => return false,
                    dom::NodeData::Text(t) if !t.is_empty() => return false,
                    _ => {}
                }
                child = cnode.next_sibling;
            } else {
                break;
            }
        }
        true
    }

    fn is_root(&self) -> bool {
        let Some(node) = self.document.get_node(self.id) else {
            return false;
        };
        if let Some(parent_id) = node.parent
            && let Some(parent) = self.document.get_node(parent_id)
        {
            return matches!(parent.data, dom::NodeData::Document);
        }
        false
    }

    fn add_element_unique_hashes(&self, filter: &mut BloomFilter) -> bool {
        let mut added = false;
        if let Some(elem) = self.element_data() {
            filter.insert_hash(SoulName(elem.tag_name.clone()).precomputed_hash());
            added = true;
            if let Some(id) = &elem.id {
                filter.insert_hash(SoulName(id.clone()).precomputed_hash());
                added = true;
            }
            for class in &elem.classes {
                filter.insert_hash(SoulName(class.clone()).precomputed_hash());
                added = true;
            }
            for name in elem.attributes.keys() {
                filter.insert_hash(SoulName(name.clone()).precomputed_hash());
                added = true;
            }
        }
        added
    }
}
