//! WHATWG DOM specification `AbortController` and `AbortSignal` ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsArray;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsValue};

const SIGNAL_ABORTED_PROP: &str = "__soul_signal_aborted__";
const SIGNAL_REASON_PROP: &str = "__soul_signal_reason__";
const SIGNAL_LISTENERS_PROP: &str = "__soul_signal_listeners__";
const CONTROLLER_SIGNAL_PROP: &str = "__soul_controller_signal__";

/// Registers `AbortController` and `AbortSignal` into the JS context.
pub fn register_abort(context: &mut Context) {
    let abort_controller_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_abort_controller_constructor),
    )
    .constructor(true)
    .name(js_string!("AbortController"))
    .length(0)
    .build();

    let signal_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_abort_signal_constructor),
    )
    .constructor(true)
    .name(js_string!("AbortSignal"))
    .length(0)
    .build();

    // Static AbortSignal.abort(reason)
    let _ = signal_ctor.set(
        js_string!("abort"),
        NativeFunction::from_fn_ptr(js_abort_signal_static_abort).to_js_function(context.realm()),
        false,
        context,
    );

    // Static AbortSignal.timeout(delay)
    let _ = signal_ctor.set(
        js_string!("timeout"),
        NativeFunction::from_fn_ptr(js_abort_signal_static_timeout).to_js_function(context.realm()),
        false,
        context,
    );

    let global = context.global_object();
    let _ = global.set(
        js_string!("AbortController"),
        abort_controller_ctor,
        false,
        context,
    );
    let _ = global.set(js_string!("AbortSignal"), signal_ctor, false, context);
}

/// Creates a new `AbortSignal` JS instance.
#[must_use]
pub fn create_abort_signal(context: &mut Context) -> JsObject {
    let get_aborted = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::from(false)),
                |o| o.get(js_string!(SIGNAL_ABORTED_PROP), ctx),
            )
        }),
    )
    .build();

    let get_reason = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            this.as_object().map_or_else(
                || Ok(JsValue::undefined()),
                |o| o.get(js_string!(SIGNAL_REASON_PROP), ctx),
            )
        }),
    )
    .build();

    let listeners = JsArray::new(context);

    ObjectInitializer::new(context)
        .property(
            js_string!(SIGNAL_ABORTED_PROP),
            JsValue::from(false),
            Attribute::all(),
        )
        .property(
            js_string!(SIGNAL_REASON_PROP),
            JsValue::undefined(),
            Attribute::all(),
        )
        .property(
            js_string!(SIGNAL_LISTENERS_PROP),
            JsValue::from(listeners),
            Attribute::all(),
        )
        .accessor(
            js_string!("aborted"),
            Some(get_aborted),
            None,
            Attribute::all(),
        )
        .accessor(
            js_string!("reason"),
            Some(get_reason),
            None,
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(js_signal_throw_if_aborted),
            js_string!("throwIfAborted"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_signal_add_event_listener),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_signal_remove_event_listener),
            js_string!("removeEventListener"),
            2,
        )
        .build()
}

fn js_abort_controller_constructor(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let signal = create_abort_signal(context);

    let instance = ObjectInitializer::new(context)
        .property(
            js_string!(CONTROLLER_SIGNAL_PROP),
            JsValue::from(signal.clone()),
            Attribute::all(),
        )
        .property(
            js_string!("signal"),
            JsValue::from(signal),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .function(
            NativeFunction::from_fn_ptr(js_controller_abort),
            js_string!("abort"),
            1,
        )
        .build();

    Ok(JsValue::from(instance))
}

fn js_abort_signal_constructor(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let instance = create_abort_signal(context);
    Ok(JsValue::from(instance))
}

fn js_controller_abort(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(controller) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let signal_val = controller.get(js_string!(CONTROLLER_SIGNAL_PROP), context)?;
    let Some(signal) = signal_val.as_object() else {
        return Ok(JsValue::undefined());
    };

    let already_aborted = signal
        .get(js_string!(SIGNAL_ABORTED_PROP), context)?
        .to_boolean();
    if already_aborted {
        return Ok(JsValue::undefined());
    }

    let default_reason = JsError::from(
        JsNativeError::error().with_message("AbortError: The operation was aborted."),
    )
    .to_opaque(context);

    let reason = args.first().cloned().unwrap_or(default_reason);

    let _ = signal.set(
        js_string!(SIGNAL_ABORTED_PROP),
        JsValue::from(true),
        false,
        context,
    );
    let _ = signal.set(js_string!(SIGNAL_REASON_PROP), reason, false, context);

    // Trigger registered abort listeners
    let listeners_val = signal.get(js_string!(SIGNAL_LISTENERS_PROP), context)?;
    if let Some(arr) = listeners_val.as_object() {
        let len = arr
            .get(js_string!("length"), context)?
            .to_u32(context)
            .unwrap_or(0);
        for i in 0..len {
            if let Ok(callback_val) = arr.get(i, context)
                && let Some(func) = callback_val.as_callable()
            {
                let _ = func.call(&signal_val, &[], context);
            }
        }
    }

    Ok(JsValue::undefined())
}

fn js_signal_throw_if_aborted(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    if let Some(signal) = this.as_object() {
        let aborted = signal
            .get(js_string!(SIGNAL_ABORTED_PROP), context)?
            .to_boolean();
        if aborted {
            let reason = signal.get(js_string!(SIGNAL_REASON_PROP), context)?;
            return Err(JsError::from_opaque(reason));
        }
    }
    Ok(JsValue::undefined())
}

fn js_signal_add_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(signal) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let event_type = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    if event_type == "abort" {
        let callback = args.get_or_undefined(1);
        if callback.is_callable() {
            let listeners_val = signal.get(js_string!(SIGNAL_LISTENERS_PROP), context)?;
            if let Some(arr) = listeners_val.as_object() {
                let len = arr
                    .get(js_string!("length"), context)?
                    .to_u32(context)
                    .unwrap_or(0);
                let _ = arr.set(len, callback.clone(), false, context);
            }
        }
    }

    Ok(JsValue::undefined())
}

const fn js_signal_remove_event_listener(
    _this: &JsValue,
    _args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

fn js_abort_signal_static_abort(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let signal = create_abort_signal(context);
    let default_reason = JsError::from(
        JsNativeError::error().with_message("AbortError: The operation was aborted."),
    )
    .to_opaque(context);

    let reason = args.first().cloned().unwrap_or(default_reason);

    let _ = signal.set(
        js_string!(SIGNAL_ABORTED_PROP),
        JsValue::from(true),
        false,
        context,
    );
    let _ = signal.set(js_string!(SIGNAL_REASON_PROP), reason, false, context);

    Ok(JsValue::from(signal))
}

fn js_abort_signal_static_timeout(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let signal = create_abort_signal(context);
    Ok(JsValue::from(signal))
}
