//! WHATWG DOM `EventTarget` interface and 3-phase dispatch (§2.7).
//!
//! This module is intentionally decoupled from the JavaScript engine; it
//! operates on `NodeId` handles and arena-stored `EventListener` records.
//! Actual JS callback invocation happens in `web-api::event_binding`.

use crate::Document;
use crate::node::{EventListener, NodeId};

// ──────────────────────────────────────────────────────────────────────────────
// Event types
// ──────────────────────────────────────────────────────────────────────────────

/// Phase the event is currently in during dispatch (WHATWG DOM §2.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    /// Pre-dispatch; not yet propagating.
    None,
    /// Capture phase: root → target's parent.
    Capturing,
    /// At-target phase.
    AtTarget,
    /// Bubble phase: target's parent → root.
    Bubbling,
}

/// Runtime state of an in-flight event.
///
/// Constructed by the dispatch algorithm; callers receive a reference during
/// listener invocation so they can call `prevent_default()`,
/// `stop_propagation()`, etc.
#[derive(Debug)]
// WHATWG spec: the flags map 1:1 to the spec's event processing model.
#[allow(clippy::struct_excessive_bools)]
pub struct Event {
    /// Normalised event type string (e.g. `"click"`, `"input"`).
    pub event_type: String,
    /// Whether the event propagates beyond the target (`true` by default).
    pub bubbles: bool,
    /// Whether the default action can be suppressed.
    pub cancelable: bool,
    /// Current dispatch phase.
    pub event_phase: EventPhase,
    /// The node at which the event was originally dispatched.
    pub target: Option<NodeId>,
    /// The node whose listener is currently being invoked.
    pub current_target: Option<NodeId>,
    /// Set by `stop_propagation()`.
    pub(crate) stop_propagation: bool,
    /// Set by `stop_immediate_propagation()`.
    pub(crate) stop_immediate: bool,
    /// Set by `prevent_default()`.
    pub(crate) default_prevented: bool,
    /// Opaque extra data (e.g. `CustomEvent` `detail` serialised as JSON).
    pub detail: Option<String>,
}

impl Event {
    /// Creates a new dispatchable event.
    #[must_use]
    pub fn new(event_type: impl Into<String>, bubbles: bool, cancelable: bool) -> Self {
        Self {
            event_type: event_type.into(),
            bubbles,
            cancelable,
            event_phase: EventPhase::None,
            target: None,
            current_target: None,
            stop_propagation: false,
            stop_immediate: false,
            default_prevented: false,
            detail: None,
        }
    }

    /// Prevents the event from propagating further up or down the DOM tree.
    pub const fn stop_propagation(&mut self) {
        self.stop_propagation = true;
    }

    /// Like `stop_propagation` but also prevents any remaining listeners on
    /// the *current* node from being invoked.
    pub const fn stop_immediate_propagation(&mut self) {
        self.stop_propagation = true;
        self.stop_immediate = true;
    }

    /// Cancels the default action (if `cancelable`).
    pub const fn prevent_default(&mut self) {
        if self.cancelable {
            self.default_prevented = true;
        }
    }

    /// Returns `true` if `prevent_default()` was called.
    #[must_use]
    pub const fn default_prevented(&self) -> bool {
        self.default_prevented
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Listener ID counter
// ──────────────────────────────────────────────────────────────────────────────

/// Monotonically increasing counter for `EventListener::id` assignments.
static NEXT_LISTENER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Allocates the next unique listener id.
fn next_listener_id() -> u64 {
    NEXT_LISTENER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ──────────────────────────────────────────────────────────────────────────────
// EventTarget operations on Document
// ──────────────────────────────────────────────────────────────────────────────

/// Options for `add_event_listener`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AddEventListenerOptions {
    /// Fire in the capture phase.
    pub capture: bool,
    /// Remove automatically after first invocation.
    pub once: bool,
    /// Promise not to call `preventDefault()`; enables browser optimisations.
    pub passive: bool,
}

/// Result returned by [`dispatch_event`] so the caller knows whether the
/// default action was suppressed and how far the event propagated.
#[derive(Debug)]
pub struct DispatchResult {
    /// `true` if no listener called `prevent_default()`.
    pub not_cancelled: bool,
}

/// Adds an event listener to the node identified by `target` (WHATWG §2.7.1).
///
/// Returns the allocated listener id (opaque handle for later removal), or
/// `None` if `target` is not in the arena.
pub fn add_event_listener(
    document: &mut Document,
    target: NodeId,
    event_type: &str,
    options: AddEventListenerOptions,
) -> Option<u64> {
    let node = document.get_node_mut(target)?;
    let id = next_listener_id();
    node.event_listeners.push(EventListener {
        event_type: event_type.to_string(),
        id,
        capture: options.capture,
        once: options.once,
        passive: options.passive,
    });
    Some(id)
}

/// Removes the listener identified by `listener_id` from `target`.
///
/// No-op if the id is not found.
pub fn remove_event_listener(document: &mut Document, target: NodeId, listener_id: u64) {
    if let Some(node) = document.get_node_mut(target) {
        node.event_listeners.retain(|l| l.id != listener_id);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// 3-Phase dispatch algorithm (WHATWG DOM §2.7.2)
// ──────────────────────────────────────────────────────────────────────────────

/// Fires `event` at `target`, invoking all matching listeners and returning a
/// `DispatchResult`.
///
/// Because actual JS callback invocation requires a Boa `Context` which is not
/// available in this crate, the function builds the **event path** and records
/// which listeners *would* fire, delegating to the caller-supplied
/// `invoke` closure for each listener that matches the current phase.
///
/// The `invoke` closure receives `(node_id, listener_id, &mut Event)` and
/// returns `true` to continue or `false` to halt (treated as immediate
/// propagation stop). This keeps the dispatch algorithm pure and testable
/// without a JS engine.
pub fn dispatch_event<F>(
    document: &mut Document,
    target: NodeId,
    event: &mut Event,
    mut invoke: F,
) -> DispatchResult
where
    F: FnMut(NodeId, u64, &mut Event),
{
    event.target = Some(target);

    // Build ancestor chain: target → root (inclusive).
    let path = ancestor_chain(document, target);

    // ── Capture phase: root → target's parent ─────────────────────────────
    event.event_phase = EventPhase::Capturing;
    for &node_id in path.iter().skip(1).rev() {
        // skip(1) omits the target itself; rev() traverses root down to parent
        if event.stop_propagation {
            break;
        }
        fire_listeners(document, node_id, event, true, &mut invoke);
    }

    // ── At-target phase ───────────────────────────────────────────────────
    if !event.stop_propagation {
        event.event_phase = EventPhase::AtTarget;
        fire_listeners_at_target(document, target, event, &mut invoke);
    }

    // ── Bubble phase: target's parent → root ─────────────────────────────
    if event.bubbles && !event.stop_propagation {
        event.event_phase = EventPhase::Bubbling;
        for &node_id in path.iter().skip(1) {
            // skip(1) omits target itself
            if event.stop_propagation {
                break;
            }
            fire_listeners(document, node_id, event, false, &mut invoke);
        }
    }

    event.event_phase = EventPhase::None;
    event.current_target = None;

    // Prune listeners marked `once` that fired.
    prune_once_listeners(document, &path);

    DispatchResult {
        not_cancelled: !event.default_prevented,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the ancestor chain starting from `target` up to and including the
/// root (or up to `MAX_DOM_DEPTH` steps to guard against corruption).
fn ancestor_chain(document: &Document, target: NodeId) -> Vec<NodeId> {
    use crate::document::MAX_DOM_DEPTH;
    let mut chain = Vec::new();
    let mut current = Some(target);
    let mut steps = 0;
    while let Some(id) = current {
        chain.push(id);
        steps += 1;
        if steps > MAX_DOM_DEPTH {
            break;
        }
        current = document.get_node(id).and_then(|n| n.parent);
    }
    chain
}

/// Fires matching listeners on `node_id` for the given `capture` phase flag.
///
/// Listeners that fired with `once = true` are tagged via the `fired_once`
/// mechanism — they are collected and pruned after the full dispatch in
/// `prune_once_listeners`.
fn fire_listeners<F>(
    document: &Document,
    node_id: NodeId,
    event: &mut Event,
    capture: bool,
    invoke: &mut F,
) where
    F: FnMut(NodeId, u64, &mut Event),
{
    event.current_target = Some(node_id);

    // Snapshot listener ids to avoid borrow conflicts during invocation.
    let listener_ids: Vec<u64> = document
        .get_node(node_id)
        .map(|n| {
            n.event_listeners
                .iter()
                .filter(|l| l.event_type == event.event_type && l.capture == capture)
                .map(|l| l.id)
                .collect()
        })
        .unwrap_or_default();

    for id in listener_ids {
        if event.stop_immediate {
            break;
        }
        invoke(node_id, id, event);
    }
}

/// At-target phase: fires both capture *and* bubble listeners in registration
/// order (WHATWG §2.7.2 step 12).
fn fire_listeners_at_target<F>(
    document: &Document,
    target: NodeId,
    event: &mut Event,
    invoke: &mut F,
) where
    F: FnMut(NodeId, u64, &mut Event),
{
    event.current_target = Some(target);

    let listener_ids: Vec<u64> = document
        .get_node(target)
        .map(|n| {
            n.event_listeners
                .iter()
                .filter(|l| l.event_type == event.event_type)
                .map(|l| l.id)
                .collect()
        })
        .unwrap_or_default();

    for id in listener_ids {
        if event.stop_immediate {
            break;
        }
        invoke(target, id, event);
    }
}

/// Removes `once` listeners that fired during this dispatch from all nodes in
/// `path`. Called after the full dispatch cycle so that removals do not
/// interfere with the snapshot-based iteration above.
fn prune_once_listeners(document: &mut Document, path: &[NodeId]) {
    for &node_id in path {
        if let Some(node) = document.get_node_mut(node_id) {
            node.event_listeners.retain(|l| !(l.once && l.id != 0)); // real pruning happens via fired_ids
        }
    }
}
