//! CSS selector matching against the DOM tree.

use crate::rule::{Combinator, Selector, SimpleSelector};
use dom::{Document, NodeId};

pub(super) fn matches_selector(document: &Document, node_id: NodeId, selector: &Selector) -> bool {
    let mut curr_node = Some(node_id);
    let mut parts = selector.sequence.iter().rev();

    let Some((_, first_simple)) = parts.next() else {
        return false;
    };

    if !matches_simple(document, node_id, first_simple) {
        return false;
    }

    for (comb, simple) in parts {
        let combinator = comb.unwrap_or(Combinator::Descendant);
        match combinator {
            Combinator::Child => {
                curr_node = document.get_node(curr_node.unwrap()).and_then(|n| n.parent);
                match curr_node {
                    Some(p) if matches_simple(document, p, simple) => {}
                    _ => return false,
                }
            }
            Combinator::Descendant => {
                let mut matched = false;
                while let Some(parent_id) =
                    document.get_node(curr_node.unwrap()).and_then(|n| n.parent)
                {
                    curr_node = Some(parent_id);
                    if matches_simple(document, parent_id, simple) {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return false;
                }
            }
        }
    }

    true
}

pub(super) fn matches_simple(
    document: &Document,
    node_id: NodeId,
    simple: &SimpleSelector,
) -> bool {
    let Some(node) = document.get_node(node_id) else {
        return false;
    };

    let Some(elem) = node.as_element() else {
        return false;
    };

    match simple {
        SimpleSelector::Universal => true,
        SimpleSelector::Tag(tag) => elem.tag_name == *tag,
        SimpleSelector::Id(id) => elem.id.as_deref() == Some(id.as_str()),
        SimpleSelector::Class(class) => elem.has_class(class),
    }
}
