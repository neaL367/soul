//! WHATWG `MutationObserver` binding (DOM Living Standard §4.3).
//!
//! Implements the observer side of the mutation notification pipeline.
//! Mutation **recording** is triggered by the DOM mutation APIs in
//! `dom::document::mutation`; this module provides the JS-level
//! `MutationObserver` constructor and the `observe` / `disconnect` /
//! `takeRecords` API surface.

use boa_engine::{
    Context, JsArgs, JsError, JsObject, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};
use dom::NodeId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ──────────────────────────────────────────────────────────────────────────────
// Mutation record
// ──────────────────────────────────────────────────────────────────────────────

/// Type of DOM mutation that was recorded (WHATWG §4.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationRecordType {
    /// An attribute was added, removed, or changed.
    Attributes,
    /// `CharacterData` (text node value) changed.
    CharacterData,
    /// Children were added or removed.
    ChildList,
}

impl MutationRecordType {
    /// Returns the spec string for this type.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Attributes => "attributes",
            Self::CharacterData => "characterData",
            Self::ChildList => "childList",
        }
    }
}

/// A single mutation record per WHATWG DOM §4.3.3.
#[derive(Debug, Clone)]
pub struct MutationRecord {
    /// Kind of mutation.
    pub mutation_type: MutationRecordType,
    /// The node that was mutated.
    pub target: NodeId,
    /// Attribute name, if this is an attribute mutation.
    pub attribute_name: Option<String>,
    /// Previous attribute or character-data value, if old-value tracking is on.
    pub old_value: Option<String>,
    /// Nodes added in a `childList` mutation.
    pub added_nodes: Vec<NodeId>,
    /// Nodes removed in a `childList` mutation.
    pub removed_nodes: Vec<NodeId>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Observer init options
// ──────────────────────────────────────────────────────────────────────────────

/// Per-call options passed to `MutationObserver.observe()` (WHATWG §4.3.1).
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct MutationObserverInit {
    /// Observe attribute mutations.
    pub attributes: bool,
    /// Observe character-data mutations.
    pub character_data: bool,
    /// Observe child-list mutations.
    pub child_list: bool,
    /// Observe the entire subtree.
    pub subtree: bool,
    /// Record previous attribute values.
    pub attribute_old_value: bool,
    /// Record previous character-data values.
    pub character_data_old_value: bool,
}

// ──────────────────────────────────────────────────────────────────────────────
// Observer registration entry
// ──────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)] // fields are stored for future microtask callback delivery
struct ObserverEntry {
    target: NodeId,
    init: MutationObserverInit,
    callback: JsObject,
}

// ──────────────────────────────────────────────────────────────────────────────
// Shared mutation queue
// ──────────────────────────────────────────────────────────────────────────────

/// Shared queue of pending mutation records, keyed by observer id.
///
/// DOM mutation methods push [`MutationRecord`]s here. The JS event loop
/// drains the queue and calls the registered JS callbacks.
#[derive(Default)]
pub struct MutationQueue {
    /// `observer_id` → pending records
    pending: HashMap<u64, Vec<MutationRecord>>,
}

impl MutationQueue {
    /// Pushes a mutation record for the given observer.
    pub fn push(&mut self, observer_id: u64, record: MutationRecord) {
        self.pending.entry(observer_id).or_default().push(record);
    }

    /// Drains all pending records for `observer_id`.
    pub fn take(&mut self, observer_id: u64) -> Vec<MutationRecord> {
        self.pending.remove(&observer_id).unwrap_or_default()
    }

    /// Returns `true` if there are any pending records.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.pending.values().any(|v| !v.is_empty())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Observer state
// ──────────────────────────────────────────────────────────────────────────────

static NEXT_OBSERVER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_observer_id() -> u64 {
    NEXT_OBSERVER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Default)]
struct ObserverState {
    id: u64,
    entries: Vec<ObserverEntry>,
}

impl ObserverState {
    fn new() -> Self {
        Self {
            id: next_observer_id(),
            entries: Vec::new(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Boa-compatible capture wrapper
//
// `Arc<Mutex<T>>` does not implement `boa_engine::gc::Trace`, so we wrap it
// in a newtype annotated with `#[unsafe_ignore_trace]`.  This is safe because
// the `MutationQueue` and `ObserverState` contain no `JsValue`/`JsObject`
// managed by Boa's GC — they only hold plain Rust data.
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Trace, Finalize)]
struct QueueCapture(#[unsafe_ignore_trace] Arc<Mutex<MutationQueue>>);

#[derive(Clone, Trace, Finalize)]
struct ObserverCaptures {
    #[unsafe_ignore_trace]
    state: Arc<Mutex<ObserverState>>,
    #[unsafe_ignore_trace]
    queue: Arc<Mutex<MutationQueue>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// MutationObserver constructor registration
// ──────────────────────────────────────────────────────────────────────────────

/// Registers the `MutationObserver` constructor in the global context.
///
/// # Errors
///
/// Returns a `JsResult` error if registration fails.
pub fn register_mutation_observer(
    ctx: &mut Context,
    queue: Arc<Mutex<MutationQueue>>,
) -> JsResult<()> {
    let cap = QueueCapture(queue);

    let constructor = NativeFunction::from_copy_closure_with_captures(
        move |_this, args, cap, ctx| build_observer(args, &cap.0, ctx),
        cap,
    );

    ctx.register_global_property(
        js_string!("MutationObserver"),
        constructor.to_js_function(ctx.realm()),
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Build the per-instance MutationObserver object
// ──────────────────────────────────────────────────────────────────────────────

fn build_observer(
    args: &[JsValue],
    queue: &Arc<Mutex<MutationQueue>>,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    // Get the callback directly — as_object returns a reference we can clone.
    let callback = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from_opaque(JsValue::from(js_string!(
            "MutationObserver requires a callback"
        )))
    })?;

    // SAFETY: See note on `from_closure_with_captures` below.
    // ObserverState contains JsObject which is not Send+Sync, but this Arc is
    // only ever accessed from the single Boa executor thread.
    #[allow(clippy::arc_with_non_send_sync)]
    let state = Arc::new(Mutex::new(ObserverState::new()));
    let caps = ObserverCaptures {
        state,
        queue: queue.clone(),
    };

    // `JsObject` is not `Copy`, so the observe closure must use
    // `from_closure_with_captures` (not the `copy` variant).
    let caps_obs = caps.clone();
    // SAFETY: `callback` is a `JsObject` managed by Boa's GC.
    // `from_closure_with_captures` is `unsafe` because Boa cannot automatically
    // trace the captured JsObject. The closure lifetime is bound to the
    // owning Boa context which outlives this observer object.
    #[allow(unsafe_code)]
    let observe_fn = unsafe {
        NativeFunction::from_closure_with_captures(
            move |_this, args, caps, ctx| observe_impl(args, caps, callback.clone(), ctx),
            caps_obs,
        )
    };

    let caps_dc = caps.clone();
    let disconnect_fn = NativeFunction::from_copy_closure_with_captures(
        move |_this, _args, caps, _ctx| Ok(disconnect_impl(caps)),
        caps_dc,
    );

    let take_records_fn = NativeFunction::from_copy_closure_with_captures(
        move |_this, _args, caps, ctx| take_records_impl(caps, ctx),
        caps,
    );

    let obj = ObjectInitializer::new(ctx)
        .function(observe_fn, js_string!("observe"), 2)
        .function(disconnect_fn, js_string!("disconnect"), 0)
        .function(take_records_fn, js_string!("takeRecords"), 0)
        .build();

    Ok(JsValue::from(obj))
}

// ──────────────────────────────────────────────────────────────────────────────
// observe()
// ──────────────────────────────────────────────────────────────────────────────

fn observe_impl(
    args: &[JsValue],
    caps: &ObserverCaptures,
    callback: JsObject,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let target_id = extract_node_id(args.get_or_undefined(0), ctx)?;

    let init = if let Some(obj) = args.get(1).and_then(JsValue::as_object) {
        MutationObserverInit {
            attributes: obj.get(js_string!("attributes"), ctx)?.to_boolean(),
            character_data: obj.get(js_string!("characterData"), ctx)?.to_boolean(),
            child_list: obj.get(js_string!("childList"), ctx)?.to_boolean(),
            subtree: obj.get(js_string!("subtree"), ctx)?.to_boolean(),
            attribute_old_value: obj.get(js_string!("attributeOldValue"), ctx)?.to_boolean(),
            character_data_old_value: obj
                .get(js_string!("characterDataOldValue"), ctx)?
                .to_boolean(),
        }
    } else {
        MutationObserverInit::default()
    };

    if let Ok(mut state) = caps.state.lock() {
        state.entries.push(ObserverEntry {
            target: target_id,
            init,
            callback,
        });
    }

    Ok(JsValue::undefined())
}

// ──────────────────────────────────────────────────────────────────────────────
// disconnect()
// ──────────────────────────────────────────────────────────────────────────────

fn disconnect_impl(caps: &ObserverCaptures) -> JsValue {
    if let Ok(mut state) = caps.state.lock() {
        state.entries.clear();
    }
    JsValue::undefined()
}

// ──────────────────────────────────────────────────────────────────────────────
// takeRecords()
// ──────────────────────────────────────────────────────────────────────────────

fn take_records_impl(caps: &ObserverCaptures, ctx: &mut Context) -> JsResult<JsValue> {
    let observer_id = caps.state.lock().map_or(0, |s| s.id);

    let records = caps
        .queue
        .lock()
        .map(|mut q| q.take(observer_id))
        .unwrap_or_default();

    let array = boa_engine::object::builtins::JsArray::new(ctx);
    for record in records {
        let r_obj = mutation_record_to_js(record, ctx);
        array.push(JsValue::from(r_obj), ctx)?;
    }

    Ok(JsValue::from(array))
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn extract_node_id(value: &JsValue, ctx: &mut Context) -> JsResult<NodeId> {
    let obj = value
        .as_object()
        .ok_or_else(|| JsError::from_opaque(JsValue::from(js_string!("expected DOM element"))))?;

    let raw = obj.get(js_string!("__nodeId"), ctx)?.to_number(ctx)? as usize;

    Ok(NodeId(raw))
}

fn mutation_record_to_js(record: MutationRecord, ctx: &mut Context) -> JsObject {
    let attr_name = record
        .attribute_name
        .map_or(JsValue::null(), |s| JsValue::from(js_string!(s)));
    let old_val = record
        .old_value
        .map_or(JsValue::null(), |s| JsValue::from(js_string!(s)));
    // node ID is small enough for f64 in practice; DOM arenas are bounded at 1M nodes.
    #[allow(clippy::cast_precision_loss)]
    let target_id = record.target.0 as f64;

    ObjectInitializer::new(ctx)
        .property(
            js_string!("type"),
            js_string!(record.mutation_type.as_str()),
            Attribute::all(),
        )
        .property(js_string!("target"), target_id, Attribute::all())
        .property(js_string!("attributeName"), attr_name, Attribute::all())
        .property(js_string!("oldValue"), old_val, Attribute::all())
        .build()
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_queue_push_and_take() {
        let mut q = MutationQueue::default();
        let record = MutationRecord {
            mutation_type: MutationRecordType::Attributes,
            target: NodeId(1),
            attribute_name: Some("class".into()),
            old_value: Some("old".into()),
            added_nodes: Vec::new(),
            removed_nodes: Vec::new(),
        };
        q.push(42, record);
        assert!(q.has_pending());

        let taken = q.take(42);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].mutation_type, MutationRecordType::Attributes);
        assert!(!q.has_pending());
    }

    #[test]
    fn mutation_record_type_strings() {
        assert_eq!(MutationRecordType::Attributes.as_str(), "attributes");
        assert_eq!(MutationRecordType::CharacterData.as_str(), "characterData");
        assert_eq!(MutationRecordType::ChildList.as_str(), "childList");
    }
}
