//! Selector implementation bridge for `selectors` crate.
//! Provides `SelectorImpl` and `Element` adapter for `dom::Document`.

pub mod element;
pub mod types;

pub use element::DomElement;
pub use types::{
    SoulName, SoulNamespace, SoulParser, SoulPseudoClass, SoulPseudoElement, SoulSelectorImpl,
};
