//! Selector bridge types, newtypes, and parser implementation.

use cssparser::{CowRcStr, ParseError, SourceLocation, ToCss};
use selectors::parser::{
    NonTSPseudoClass, Parser, PseudoElement, SelectorImpl, SelectorParseErrorKind,
};
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
    #[allow(clippy::cast_possible_truncation)]
    fn precomputed_hash(&self) -> u32 {
        let mut hash: u32 = 2_166_136_261;
        for b in self.0.as_bytes() {
            hash ^= u32::from(*b);
            hash = hash.wrapping_mul(16_777_619);
        }
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
        let mut hash: u32 = 2_166_136_261;
        for b in self.0.as_bytes() {
            hash ^= u32::from(*b);
            hash = hash.wrapping_mul(16_777_619);
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
