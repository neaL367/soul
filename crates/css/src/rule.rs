//! CSS stylesheet, rules, selectors, specificity, and declaration models.

#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

use crate::selector_impl::SoulSelectorImpl;
use selectors::parser::Selector as GenericSelector;

/// Specificity 4-tuple: (inline, ID, class/attribute/pseudo-class, tag/pseudo-element).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Specificity {
    /// Inline style attribute (1 or 0).
    pub inline: u32,
    /// Number of ID selectors.
    pub ids: u32,
    /// Number of class selectors and attribute selectors.
    pub classes: u32,
    /// Number of element tag selectors.
    pub tags: u32,
}

impl Specificity {
    /// Creates a new `Specificity` tuple.
    #[must_use]
    pub const fn new(inline: u32, ids: u32, classes: u32, tags: u32) -> Self {
        Self {
            inline,
            ids,
            classes,
            tags,
        }
    }

    /// Decodes packed `u32` specificity from `selectors` crate.
    #[must_use]
    pub fn from_packed(packed: u32) -> Self {
        let ids = (packed >> 20) & 0x3FF;
        let classes = (packed >> 10) & 0x3FF;
        let tags = packed & 0x3FF;
        Self::new(0, ids, classes, tags)
    }
}

/// Cascade origin of a stylesheet per CSS Cascade Level 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Origin {
    /// Browser default User-Agent stylesheet.
    #[default]
    UserAgent,
    /// Page author stylesheet or `<style>` block.
    Author,
}

/// Atomic selector unit matching a single aspect of an element (deprecated: kept for API compat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleSelector {
    /// Universal selector `*`.
    Universal,
    /// Tag name selector (e.g., `div`, `h1`).
    Tag(String),
    /// ID selector `#id`.
    Id(String),
    /// Class selector `.class`.
    Class(String),
}

/// Relationship between selectors (deprecated: kept for API compat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// Descendant combinator (whitespace).
    Descendant,
    /// Child combinator `>`.
    Child,
}

/// Compound selector sequence — now backed by `selectors` crate.
#[derive(Debug, Clone)]
pub struct Selector {
    /// Inner `selectors` crate selector.
    pub inner: GenericSelector<SoulSelectorImpl>,
    /// Original source text for debugging/equality.
    pub source: String,
}

impl Selector {
    /// Computes the CSS specificity of this selector.
    #[must_use]
    pub fn specificity(&self) -> Specificity {
        Specificity::from_packed(self.inner.specificity())
    }
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.specificity() == other.specificity()
    }
}

impl Eq for Selector {}

/// Single CSS property-value declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// Property name in lowercase (e.g., "color", "margin-top").
    pub property: String,
    /// Raw property value string.
    pub value: String,
    /// Flag indicating `!important` declaration.
    pub important: bool,
}

impl Declaration {
    /// Creates a new declaration.
    #[must_use]
    pub fn new(property: &str, value: &str, important: bool) -> Self {
        Self {
            property: property.trim().to_ascii_lowercase(),
            value: value.trim().to_string(),
            important,
        }
    }
}

use crate::media::MediaCondition;

/// Single CSS style rule with selectors, declarations, origin, and optional media condition.
#[derive(Debug, Clone)]
pub struct Rule {
    /// List of comma-separated selectors for this rule.
    pub selectors: Vec<Selector>,
    /// Declaration block statements.
    pub declarations: Vec<Declaration>,
    /// Stylesheet origin.
    pub origin: Origin,
    /// Optional media condition restricting when this rule applies.
    pub media: Option<MediaCondition>,
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.selectors == other.selectors
            && self.declarations == other.declarations
            && self.origin == other.origin
            && self.media == other.media
    }
}

impl Eq for Rule {}

/// Parsed CSS stylesheet containing ordered style rules.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleSheet {
    /// Rules defined in this stylesheet.
    pub rules: Vec<Rule>,
    /// Origin of this stylesheet.
    pub origin: Origin,
}

impl StyleSheet {
    /// Creates a new empty `StyleSheet` with the specified origin.
    #[must_use]
    pub const fn new(origin: Origin) -> Self {
        Self {
            rules: Vec::new(),
            origin,
        }
    }
}
