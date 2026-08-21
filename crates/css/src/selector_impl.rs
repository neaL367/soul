//! Selector implementation bridge for `selectors` crate.
//! Provides `SelectorImpl` and `Element` adapter for `dom::Document`.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

use cssparser::{CowRcStr, ParseError, SourceLocation, ToCss};
use dom::{Document, NodeId};
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::MatchingContext;
use selectors::matching::ElementSelectorFlags;
use selectors::parser::{
    NonTSPseudoClass, Parser, PseudoElement, SelectorImpl, SelectorParseErrorKind,
};
use selectors::{Element, OpaqueElement};
use std::borrow::Borrow;
use std::fmt;

/// Newtype wrappers implementing required traits for selectors.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SoulName(pub String);

impl AsRef<str> for SoulName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SoulName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl ToCss for SoulName {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl Borrow<str> for SoulName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl precomputed_hash::PrecomputedHash for SoulName {
    fn precomputed_hash(&self) -> u32 {
        // Deterministic FNV-like hash for bloom filter (must be stable across calls).
        let mut hash: u32 = 2166136261;
        for b in self.0.as_bytes() {
            hash ^= u32::from(*b);
            hash = hash.wrapping_mul(16777619);
        }
        // Mix in length to distinguish e.g., "ab" vs "a" + "b" edge.
        hash.wrapping_add(self.0.len() as u32)
    }
}

/// Namespace URL wrapper - for HTML we treat everything as no namespace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SoulNamespace(pub String);

impl From<&str> for SoulNamespace {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl ToCss for SoulNamespace {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl Borrow<str> for SoulNamespace {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl precomputed_hash::PrecomputedHash for SoulNamespace {
    fn precomputed_hash(&self) -> u32 {
        let mut hash: u32 = 2166136261;
        for b in self.0.as_bytes() {
            hash ^= u32::from(*b);
            hash = hash.wrapping_mul(16777619);
        }
        hash
    }
}

/// Non-tree-structural pseudo-class for `SoulSelectorImpl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoulPseudoClass(pub String);

impl NonTSPseudoClass for SoulPseudoClass {
    type Impl = SoulSelectorImpl;
    fn is_active_or_hover(&self) -> bool {
        matches!(self.0.as_str(), "active" | "hover")
    }
    fn is_user_action_state(&self) -> bool {
        matches!(self.0.as_str(), "active" | "hover" | "focus")
    }
}

impl ToCss for SoulPseudoClass {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(":")?;
        dest.write_str(&self.0)
    }
}

/// Pseudo-element for `SoulSelectorImpl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoulPseudoElement(pub String);

impl PseudoElement for SoulPseudoElement {
    type Impl = SoulSelectorImpl;
}

impl ToCss for SoulPseudoElement {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str("::")?;
        dest.write_str(&self.0)
    }
}

/// `SelectorImpl` for the Soul browser.
#[derive(Clone, Debug)]
pub struct SoulSelectorImpl;

impl SelectorImpl for SoulSelectorImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = SoulName;
    type Identifier = SoulName;
    type LocalName = SoulName;
    type NamespaceUrl = SoulNamespace;
    type NamespacePrefix = SoulName;
    type BorrowedLocalName = str;
    type BorrowedNamespaceUrl = str;
    type NonTSPseudoClass = SoulPseudoClass;
    type PseudoElement = SoulPseudoElement;
}

/// Parser for `SoulSelectorImpl`.
pub struct SoulParser;

impl<'i> Parser<'i> for SoulParser {
    type Impl = SoulSelectorImpl;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_non_ts_pseudo_class(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<SoulPseudoClass, ParseError<'i, SelectorParseErrorKind<'i>>> {
        // Support common non-tree-structural pseudo-classes; reject others.
        // For MVP we support :link, :visited, :active, :hover, :focus.
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "link" | "visited" | "active" | "hover" | "focus" | "enabled" | "disabled"
            | "checked" | "indeterminate" | "any-link" => Ok(SoulPseudoClass(lower)),
            _ => Err(location.new_custom_error(
                SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
            )),
        }
    }

    fn parse_pseudo_element(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<SoulPseudoElement, ParseError<'i, SelectorParseErrorKind<'i>>> {
        Err(
            location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
                name,
            )),
        )
    }
}

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
    pub fn new(document: &'a Document, id: NodeId) -> Self {
        Self { document, id }
    }

    fn element_data(&self) -> Option<&dom::ElementData> {
        self.document.get_node(self.id)?.as_element()
    }

    fn is_element_node(&self) -> bool {
        self.document
            .get_node(self.id)
            .is_some_and(|n| n.is_element())
    }
}

impl<'a> Element for DomElement<'a> {
    type Impl = SoulSelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        // Use node's address for opaque identity; fallback to NodeId hashing if needed.
        // The Document arena stores nodes in Vec, so address is stable for borrow duration.
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
            // Walk up through non-element ancestors (should not happen in flat HTML, but handle)
            // Find nearest element ancestor.
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
        // HTML has no namespaces; empty string means no namespace.
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
        // We ignore namespace; only check local name.
        if let NamespaceConstraint::Specific(url) = ns
            && !url.0.is_empty()
        {
            return false;
        }
        let Some(elem) = self.element_data() else {
            return false;
        };
        // Attributes are stored lowercased.
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
