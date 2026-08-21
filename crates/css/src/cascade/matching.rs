//! CSS selector matching against the DOM tree via `selectors` crate.

use crate::rule::Selector;
use crate::selector_impl::DomElement;
use dom::{Document, NodeId};
use selectors::context::MatchingForInvalidation;
use selectors::context::{
    MatchingContext, MatchingMode, NeedsSelectorFlags, QuirksMode, SelectorCaches,
};
use selectors::matching;

pub(super) fn matches_selector(document: &Document, node_id: NodeId, selector: &Selector) -> bool {
    // Only elements can match selectors.
    let Some(node) = document.get_node(node_id) else {
        return false;
    };
    if !node.is_element() {
        return false;
    }

    let element = DomElement::new(document, node_id);
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    matching::matches_selector(&selector.inner, 0, None, &element, &mut context)
}
