//! Timer JavaScript bindings (`setTimeout`, `clearTimeout`).

use boa_engine::{
    Context, JsArgs, JsObject, JsResult, JsValue,
    gc::{Finalize, Gc, GcRefCell, Trace},
    js_string,
    native_function::NativeFunction,
};

/// Pending timer callback storage with monotonically increasing timer ids.
///
/// The stored callbacks are `JsObject`s and must remain visible to Boa's
/// garbage collector; the type therefore implements [`Trace`] and is stored
/// behind a [`GcRefCell`] so an unswept timer callback can never be collected
/// while still scheduled.
#[derive(Clone, Default, Trace, Finalize)]
pub struct TimerState {
    next_id: u64,
    pending: Vec<(u64, JsObject)>,
}

impl TimerState {
    /// Queues a timer callback and returns its unique timer id.
    pub(crate) fn push(&mut self, callback: JsObject) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push((id, callback));
        id
    }

    /// Removes a pending timer callback by id.
    pub(crate) fn remove(&mut self, id: u64) {
        self.pending.retain(|(pending_id, _)| *pending_id != id);
    }

    /// Returns the number of pending timer callbacks.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.pending.len()
    }

    /// Returns `true` if there are no pending timer callbacks.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Global timer queue storing pending callbacks for the JavaScript execution context.
///
/// `Gc` keeps the queue reachable from Boa's GC roots, and the `GcRefCell`
/// makes the stored callbacks traceable.
pub type TimerQueue = Gc<GcRefCell<TimerState>>;

#[derive(Clone, Trace, Finalize)]
struct TimerHolder(TimerQueue);

/// Registers `setTimeout` and `clearTimeout` global functions.
///
/// # Errors
///
/// Returns a `JsResult` error if global property registration fails.
pub fn register_timers(context: &mut Context, pending_callbacks: TimerQueue) -> JsResult<()> {
    let set_timeout_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, _ctx| {
            let callback = args.get_or_undefined(0);
            let id = callback
                .as_object()
                .map_or(0, |obj| captures.0.borrow_mut().push(obj));
            Ok(JsValue::from(id))
        },
        TimerHolder(pending_callbacks.clone()),
    );

    let clear_timeout_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
            captures.0.borrow_mut().remove(u64::from(id));
            Ok(JsValue::undefined())
        },
        TimerHolder(pending_callbacks),
    );

    context.register_global_callable(js_string!("setTimeout"), 2, set_timeout_fn)?;
    context.register_global_callable(js_string!("clearTimeout"), 1, clear_timeout_fn)?;

    Ok(())
}
