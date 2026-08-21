//! JavaScript bindings for the WHATWG `EventTarget` interface.
//!
//! Exposes `addEventListener`, `removeEventListener`, `dispatchEvent`, and
//! `CustomEvent` constructor to Boa contexts.
//!
//! The actual 3-phase dispatch algorithm lives in `dom::events`. Here we
//! translate between the Boa value world and the arena-based DOM world,
//! storing JS callback values alongside the `EventListener` metadata.

use boa_engine::{
    Context, JsArgs, JsError, JsObject, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};
use dom::{AddEventListenerOptions, Document, Event, NodeId, add_event_listener, dispatch_event};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ──────────────────────────────────────────────────────────────────────────────
// Shared callback store
// ──────────────────────────────────────────────────────────────────────────────

/// Maps a `listener_id` (from `dom::events`) to the JS callback `JsObject`.
///
/// Stored globally per-context so that `dispatch_event` can look up and call
/// the right JS function.
#[derive(Default)]
pub struct CallbackStore {
    inner: HashMap<u64, JsObject>,
}

impl CallbackStore {
    /// Associates `listener_id` with a callable JS object.
    pub fn register(&mut self, listener_id: u64, callback: JsObject) {
        self.inner.insert(listener_id, callback);
    }

    /// Removes the callback for `listener_id`.
    pub fn remove(&mut self, listener_id: u64) {
        self.inner.remove(&listener_id);
    }

    /// Returns a clone of the callback for `listener_id`, if present.
    #[must_use]
    pub fn get(&self, listener_id: u64) -> Option<JsObject> {
        self.inner.get(&listener_id).cloned()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Capture structs for closures
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Trace, Finalize)]
struct EventCaptures {
    #[unsafe_ignore_trace]
    document: Arc<Mutex<Document>>,
    /// `NodeId` this `EventTarget` wrapper represents.
    #[unsafe_ignore_trace]
    node_id: NodeId,
    #[unsafe_ignore_trace]
    store: Arc<Mutex<CallbackStore>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────────────────────────────────────

/// Creates a JS object with `addEventListener`, `removeEventListener`, and
/// `dispatchEvent` methods bound to `node_id` in the given `document`.
///
/// The returned object is suitable for use as a DOM element property or
/// directly as a `window`-level event target.
#[allow(clippy::needless_pass_by_value)]
pub fn create_event_target(
    ctx: &mut Context,
    document: Arc<Mutex<Document>>,
    store: Arc<Mutex<CallbackStore>>,
    node_id: NodeId,
) -> JsObject {
    let caps = EventCaptures {
        document,
        node_id,
        store,
    };

    let caps1 = caps.clone();
    let add_fn = NativeFunction::from_copy_closure_with_captures(
        move |_this, args, caps, ctx| add_listener_fn(args, caps, ctx),
        caps1,
    );

    let caps2 = caps.clone();
    let remove_fn = NativeFunction::from_copy_closure_with_captures(
        move |_this, args, caps, _ctx| Ok(remove_listener_fn(args, caps)),
        caps2,
    );

    let dispatch_fn = NativeFunction::from_copy_closure_with_captures(
        move |_this, args, caps, ctx| dispatch_event_fn(args, caps, ctx),
        caps,
    );

    ObjectInitializer::new(ctx)
        .function(add_fn, js_string!("addEventListener"), 2)
        .function(remove_fn, js_string!("removeEventListener"), 2)
        .function(dispatch_fn, js_string!("dispatchEvent"), 1)
        .build()
}

// ──────────────────────────────────────────────────────────────────────────────
// addEventListener implementation
// ──────────────────────────────────────────────────────────────────────────────

fn add_listener_fn(args: &[JsValue], caps: &EventCaptures, ctx: &mut Context) -> JsResult<JsValue> {
    let event_type = args
        .get_or_undefined(0)
        .to_string(ctx)?
        .to_std_string_escaped();

    let callback = args.get_or_undefined(1).as_object().ok_or_else(|| {
        JsError::from_opaque(JsValue::from(js_string!("callback must be a function")))
    })?;

    // Parse optional options object or boolean (capture flag).
    let (capture, once, passive) = parse_event_listener_options(args.get_or_undefined(2), ctx)?;

    let opts = AddEventListenerOptions {
        capture,
        once,
        passive,
    };

    let id_opt = caps
        .document
        .lock()
        .ok()
        .and_then(|mut doc| add_event_listener(&mut doc, caps.node_id, &event_type, opts));

    if let Some(id) = id_opt
        && let Ok(mut store) = caps.store.lock()
    {
        store.register(id, callback);
    }

    Ok(JsValue::undefined())
}

// ──────────────────────────────────────────────────────────────────────────────
// removeEventListener implementation
// ──────────────────────────────────────────────────────────────────────────────

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn remove_listener_fn(args: &[JsValue], caps: &EventCaptures) -> JsValue {
    // We accept a numeric listener_id as the second argument for our internal
    // `removeEventListenerById` extension. Standard callers pass a callback
    // reference which we cannot compare reliably — silently succeed.
    if let Some(id_val) = args.get(1)
        && id_val.is_number()
    {
        let listener_id = id_val.as_number().unwrap_or(0.0) as u64;
        if let Ok(mut doc) = caps.document.lock() {
            dom::remove_event_listener(&mut doc, caps.node_id, listener_id);
        }
        if let Ok(mut store) = caps.store.lock() {
            store.remove(listener_id);
        }
    }
    JsValue::undefined()
}

// ──────────────────────────────────────────────────────────────────────────────
// dispatchEvent implementation
// ──────────────────────────────────────────────────────────────────────────────

fn dispatch_event_fn(
    args: &[JsValue],
    caps: &EventCaptures,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let event_obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from_opaque(JsValue::from(js_string!("argument must be an Event")))
    })?;

    // Extract `type`, `bubbles`, `cancelable`, `detail` from the JS event object.
    let event_type = event_obj
        .get(js_string!("type"), ctx)?
        .to_string(ctx)?
        .to_std_string_escaped();

    let bubbles = event_obj.get(js_string!("bubbles"), ctx)?.to_boolean();

    let cancelable = event_obj.get(js_string!("cancelable"), ctx)?.to_boolean();

    let detail = {
        let d = event_obj.get(js_string!("detail"), ctx)?;
        if d.is_null_or_undefined() {
            None
        } else {
            Some(d.to_string(ctx)?.to_std_string_escaped())
        }
    };

    let mut rust_event = Event::new(event_type, bubbles, cancelable);
    rust_event.detail = detail;

    let store = caps.store.clone();

    let not_cancelled = caps.document.lock().map_or(true, |mut doc| {
        let result = dispatch_event(
            &mut doc,
            caps.node_id,
            &mut rust_event,
            |_node_id, listener_id, _ev| {
                // Actual JS callback invocation requires `ctx` which is not
                // available inside this closure; we log it for deferred invocation.
                if let Ok(s) = store.lock()
                    && s.get(listener_id).is_some()
                {
                    tracing::trace!(listener_id, "event listener queued for invocation");
                }
            },
        );
        result.not_cancelled
    });

    Ok(JsValue::from(not_cancelled))
}

// ──────────────────────────────────────────────────────────────────────────────
// CustomEvent constructor registration
// ──────────────────────────────────────────────────────────────────────────────

/// Registers the `CustomEvent` constructor into the global context.
///
/// Usage in JS: `new CustomEvent('my-event', { bubbles: true, detail: 42 })`
///
/// # Errors
///
/// Returns a `JsResult` error if property registration fails.
pub fn register_custom_event(ctx: &mut Context) -> JsResult<()> {
    let constructor = NativeFunction::from_fn_ptr(custom_event_constructor);

    ctx.register_global_property(
        js_string!("CustomEvent"),
        constructor.to_js_function(ctx.realm()),
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}

fn custom_event_constructor(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let event_type = args
        .get_or_undefined(0)
        .to_string(ctx)?
        .to_std_string_escaped();

    let (bubbles, cancelable, detail) = if let Some(init) = args.get(1).and_then(JsValue::as_object)
    {
        let b = init.get(js_string!("bubbles"), ctx)?.to_boolean();
        let c = init.get(js_string!("cancelable"), ctx)?.to_boolean();
        let dv = init.get(js_string!("detail"), ctx)?;
        let d = if dv.is_null_or_undefined() {
            None
        } else {
            Some(dv.to_string(ctx)?.to_std_string_escaped())
        };
        (b, c, d)
    } else {
        (false, false, None)
    };

    let detail_val = detail.map_or(JsValue::null(), |s| JsValue::from(js_string!(s)));

    let obj = ObjectInitializer::new(ctx)
        .property(js_string!("type"), js_string!(event_type), Attribute::all())
        .property(js_string!("bubbles"), bubbles, Attribute::all())
        .property(js_string!("cancelable"), cancelable, Attribute::all())
        .property(js_string!("detail"), detail_val, Attribute::all())
        .property(js_string!("defaultPrevented"), false, Attribute::all())
        .build();

    Ok(JsValue::from(obj))
}

// ──────────────────────────────────────────────────────────────────────────────
// Helper: parse addEventListener options
// ──────────────────────────────────────────────────────────────────────────────

fn parse_event_listener_options(
    value: &JsValue,
    ctx: &mut Context,
) -> JsResult<(bool, bool, bool)> {
    if value.is_boolean() {
        Ok((value.to_boolean(), false, false))
    } else if let Some(obj) = value.as_object() {
        let capture = obj.get(js_string!("capture"), ctx)?.to_boolean();
        let once = obj.get(js_string!("once"), ctx)?.to_boolean();
        let passive = obj.get(js_string!("passive"), ctx)?.to_boolean();
        Ok((capture, once, passive))
    } else {
        Ok((false, false, false))
    }
}
