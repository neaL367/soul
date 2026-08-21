//! Integration tests for WHATWG DOM `EventTarget` and 3-phase event dispatch.

use dom::{
    AddEventListenerOptions, Document, ElementData, Event, EventPhase, NodeData, NodeId,
    add_event_listener, dispatch_event,
};
use std::collections::HashMap;

fn build_chain() -> (Document, NodeId, NodeId, NodeId) {
    let mut doc = Document::new();
    let root = doc.root_id();
    let parent = doc.alloc_node(NodeData::Element(ElementData::new(
        "div",
        HashMap::default(),
    )));
    let child = doc.alloc_node(NodeData::Element(ElementData::new(
        "span",
        HashMap::default(),
    )));
    doc.append_child(root, parent);
    doc.append_child(parent, child);
    (doc, root, parent, child)
}

#[test]
fn capture_and_bubble_order() {
    let (mut doc, root, parent, child) = build_chain();

    let opts_cap = AddEventListenerOptions {
        capture: true,
        ..Default::default()
    };
    let opts_bub = AddEventListenerOptions {
        capture: false,
        ..Default::default()
    };

    // Register listeners on all three nodes.
    add_event_listener(&mut doc, root, "click", opts_cap);
    add_event_listener(&mut doc, parent, "click", opts_cap);
    add_event_listener(&mut doc, child, "click", opts_bub); // at-target
    add_event_listener(&mut doc, parent, "click", opts_bub);
    add_event_listener(&mut doc, root, "click", opts_bub);

    let mut fired_order: Vec<(NodeId, &'static str)> = Vec::new();

    let mut event = Event::new("click", true, true);
    dispatch_event(&mut doc, child, &mut event, |node, _id, ev| {
        let phase = match ev.event_phase {
            EventPhase::Capturing => "cap",
            EventPhase::AtTarget => "target",
            EventPhase::Bubbling => "bub",
            EventPhase::None => "none",
        };
        fired_order.push((node, phase));
    });

    // Expected: root(cap) → parent(cap) → child(target) → parent(bub) → root(bub)
    assert_eq!(fired_order.len(), 5, "all 5 listeners must fire");
    assert_eq!(fired_order[0], (root, "cap"));
    assert_eq!(fired_order[1], (parent, "cap"));
    assert_eq!(fired_order[2], (child, "target"));
    assert_eq!(fired_order[3], (parent, "bub"));
    assert_eq!(fired_order[4], (root, "bub"));
}

#[test]
fn stop_propagation_halts_bubbling() {
    let (mut doc, _root, parent, child) = build_chain();

    let opts = AddEventListenerOptions {
        capture: false,
        ..Default::default()
    };
    add_event_listener(&mut doc, child, "input", opts);
    add_event_listener(&mut doc, parent, "input", opts);

    let mut count = 0u32;
    let mut event = Event::new("input", true, true);
    dispatch_event(&mut doc, child, &mut event, |_node, _id, ev| {
        count += 1;
        ev.stop_propagation(); // first invocation stops the rest
    });

    assert_eq!(count, 1, "only the target listener should fire");
}

#[test]
fn prevent_default_is_recorded() {
    let (mut doc, _, _, child) = build_chain();
    let opts = AddEventListenerOptions::default();
    add_event_listener(&mut doc, child, "submit", opts);

    let mut event = Event::new("submit", true, true);
    let result = dispatch_event(&mut doc, child, &mut event, |_, _, ev| {
        ev.prevent_default();
    });

    assert!(!result.not_cancelled);
    assert!(event.default_prevented());
}

#[test]
fn non_bubbling_event_does_not_reach_parent() {
    let (mut doc, _root, parent, child) = build_chain();
    let opts = AddEventListenerOptions::default();
    add_event_listener(&mut doc, child, "focus", opts);
    add_event_listener(&mut doc, parent, "focus", opts);

    let mut count = 0u32;
    let mut event = Event::new("focus", false /* does not bubble */, true);
    dispatch_event(&mut doc, child, &mut event, |_, _, _| count += 1);

    assert_eq!(count, 1, "non-bubbling event must not reach parent");
}
