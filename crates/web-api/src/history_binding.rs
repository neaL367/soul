//! WHATWG HTML specification `window.history` ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsArray;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue};
use url::Url;

const HISTORY_STACK_PROP: &str = "__soul_history_stack__";
const HISTORY_INDEX_PROP: &str = "__soul_history_index__";

/// Registers `window.history` into the JS context and links it to `window.location`.
pub fn register_history(context: &mut Context, initial_url: &str) {
    let history_obj = create_history_object(context, initial_url);
    let global = context.global_object();
    let _ = global.set(js_string!("history"), history_obj, false, context);
}

/// Creates a `History` JS object instance initialized with the document URL.
#[must_use]
pub fn create_history_object(context: &mut Context, initial_url: &str) -> JsObject {
    let stack = JsArray::new(context);

    // Initial entry
    let initial_entry = ObjectInitializer::new(context)
        .property(js_string!("state"), JsValue::null(), Attribute::all())
        .property(
            js_string!("title"),
            JsValue::from(js_string!("")),
            Attribute::all(),
        )
        .property(
            js_string!("url"),
            JsValue::from(JsString::from(initial_url)),
            Attribute::all(),
        )
        .build();

    let _ = stack.set(0, JsValue::from(initial_entry), false, context);

    let get_length = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            if let Some(hist) = this.as_object() {
                let stack_val = hist.get(js_string!(HISTORY_STACK_PROP), ctx)?;
                if let Some(arr) = stack_val.as_object() {
                    return arr.get(js_string!("length"), ctx);
                }
            }
            Ok(JsValue::from(1))
        }),
    )
    .build();

    let get_state = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(|this, _args, ctx| {
            if let Some(hist) = this.as_object() {
                let idx_val = hist.get(js_string!(HISTORY_INDEX_PROP), ctx)?;
                let idx = idx_val.to_u32(ctx).unwrap_or(0);
                let stack_val = hist.get(js_string!(HISTORY_STACK_PROP), ctx)?;
                if let Some(arr) = stack_val.as_object() {
                    let entry_val = arr.get(idx, ctx)?;
                    if let Some(entry_obj) = entry_val.as_object() {
                        return entry_obj.get(js_string!("state"), ctx);
                    }
                }
            }
            Ok(JsValue::null())
        }),
    )
    .build();

    ObjectInitializer::new(context)
        .property(
            js_string!(HISTORY_STACK_PROP),
            JsValue::from(stack),
            Attribute::all(),
        )
        .property(
            js_string!(HISTORY_INDEX_PROP),
            JsValue::from(0),
            Attribute::all(),
        )
        .accessor(
            js_string!("length"),
            Some(get_length),
            None,
            Attribute::all(),
        )
        .accessor(js_string!("state"), Some(get_state), None, Attribute::all())
        .function(
            NativeFunction::from_fn_ptr(js_history_push_state),
            js_string!("pushState"),
            3,
        )
        .function(
            NativeFunction::from_fn_ptr(js_history_replace_state),
            js_string!("replaceState"),
            3,
        )
        .function(
            NativeFunction::from_fn_ptr(js_history_back),
            js_string!("back"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_history_forward),
            js_string!("forward"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(js_history_go),
            js_string!("go"),
            1,
        )
        .build()
}

fn js_history_push_state(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(hist) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let state = args.get_or_undefined(0);
    let title = args.get_or_undefined(1).to_string(context)?;
    let url_arg = args.get(2);

    let stack_val = hist.get(js_string!(HISTORY_STACK_PROP), context)?;
    let Some(arr) = stack_val.as_object() else {
        return Ok(JsValue::undefined());
    };

    let idx_val = hist.get(js_string!(HISTORY_INDEX_PROP), context)?;
    let mut current_idx = idx_val.to_u32(context).unwrap_or(0);

    let mut resolved_url_str = String::new();
    if let Some(u_val) = url_arg
        && !u_val.is_undefined()
    {
        let raw_url = u_val.to_string(context)?.to_std_string_escaped();
        let global = context.global_object();
        let loc_val = global.get(js_string!("location"), context)?;
        if let Some(loc_obj) = loc_val.as_object() {
            let curr_href = loc_obj
                .get(js_string!("href"), context)?
                .to_string(context)?
                .to_std_string_escaped();

            if let Ok(base_url) = Url::parse(&curr_href) {
                let target_url = base_url.join(&raw_url).map_err(|_| {
                    JsError::from(
                        JsNativeError::typ()
                            .with_message("SecurityError: Invalid URL path in pushState"),
                    )
                })?;

                if target_url.origin() != base_url.origin() {
                    return Err(JsError::from(JsNativeError::typ().with_message(
                        "SecurityError: Cannot pushState to a cross-origin target URL",
                    )));
                }
                resolved_url_str = target_url.to_string();
                update_window_location(context, &target_url)?;
            }
        }
    }

    let new_entry = ObjectInitializer::new(context)
        .property(js_string!("state"), state.clone(), Attribute::all())
        .property(js_string!("title"), JsValue::from(title), Attribute::all())
        .property(
            js_string!("url"),
            JsValue::from(JsString::from(resolved_url_str)),
            Attribute::all(),
        )
        .build();

    current_idx += 1;
    let _ = arr.set(current_idx, JsValue::from(new_entry), false, context);
    let _ = hist.set(
        js_string!(HISTORY_INDEX_PROP),
        JsValue::from(current_idx),
        false,
        context,
    );

    Ok(JsValue::undefined())
}

fn js_history_replace_state(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(hist) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let state = args.get_or_undefined(0);
    let title = args.get_or_undefined(1).to_string(context)?;
    let url_arg = args.get(2);

    let stack_val = hist.get(js_string!(HISTORY_STACK_PROP), context)?;
    let Some(arr) = stack_val.as_object() else {
        return Ok(JsValue::undefined());
    };

    let idx_val = hist.get(js_string!(HISTORY_INDEX_PROP), context)?;
    let current_idx = idx_val.to_u32(context).unwrap_or(0);

    let mut resolved_url_str = String::new();
    if let Some(u_val) = url_arg
        && !u_val.is_undefined()
    {
        let raw_url = u_val.to_string(context)?.to_std_string_escaped();
        let global = context.global_object();
        let loc_val = global.get(js_string!("location"), context)?;
        if let Some(loc_obj) = loc_val.as_object() {
            let curr_href = loc_obj
                .get(js_string!("href"), context)?
                .to_string(context)?
                .to_std_string_escaped();

            if let Ok(base_url) = Url::parse(&curr_href) {
                let target_url = base_url.join(&raw_url).map_err(|_| {
                    JsError::from(
                        JsNativeError::typ()
                            .with_message("SecurityError: Invalid URL path in replaceState"),
                    )
                })?;

                if target_url.origin() != base_url.origin() {
                    return Err(JsError::from(JsNativeError::typ().with_message(
                        "SecurityError: Cannot replaceState to a cross-origin target URL",
                    )));
                }
                resolved_url_str = target_url.to_string();
                update_window_location(context, &target_url)?;
            }
        }
    }

    let updated_entry = ObjectInitializer::new(context)
        .property(js_string!("state"), state.clone(), Attribute::all())
        .property(js_string!("title"), JsValue::from(title), Attribute::all())
        .property(
            js_string!("url"),
            JsValue::from(JsString::from(resolved_url_str)),
            Attribute::all(),
        )
        .build();

    let _ = arr.set(current_idx, JsValue::from(updated_entry), false, context);
    Ok(JsValue::undefined())
}

fn js_history_back(this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    js_history_go(this, &[JsValue::from(-1)], context)
}

fn js_history_forward(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    js_history_go(this, &[JsValue::from(1)], context)
}

fn js_history_go(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(hist) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let delta = args.get_or_undefined(0).to_i32(context).unwrap_or(0);
    if delta == 0 {
        return Ok(JsValue::undefined());
    }

    let stack_val = hist.get(js_string!(HISTORY_STACK_PROP), context)?;
    let Some(arr) = stack_val.as_object() else {
        return Ok(JsValue::undefined());
    };

    let len = arr
        .get(js_string!("length"), context)?
        .to_i32(context)
        .unwrap_or(1);
    let idx_val = hist.get(js_string!(HISTORY_INDEX_PROP), context)?;
    let current_idx = idx_val.to_i32(context).unwrap_or(0);

    let new_idx = (current_idx + delta).clamp(0, len - 1);
    if new_idx != current_idx {
        #[allow(clippy::cast_sign_loss)]
        let _ = hist.set(
            js_string!(HISTORY_INDEX_PROP),
            JsValue::from(new_idx),
            false,
            context,
        );

        #[allow(clippy::cast_sign_loss)]
        if let Ok(entry_val) = arr.get(new_idx as u32, context)
            && let Some(entry_obj) = entry_val.as_object()
        {
            let u_val = entry_obj.get(js_string!("url"), context)?;
            let u_str = u_val.to_string(context)?.to_std_string_escaped();
            if let Ok(target_url) = Url::parse(&u_str) {
                let _ = update_window_location(context, &target_url);
            }
        }
    }

    Ok(JsValue::undefined())
}

fn update_window_location(context: &mut Context, url: &Url) -> JsResult<()> {
    let global = context.global_object();
    let loc_val = global.get(js_string!("location"), context)?;
    if let Some(loc) = loc_val.as_object() {
        let _ = loc.set(
            js_string!("href"),
            JsValue::from(JsString::from(url.to_string())),
            false,
            context,
        );
        let _ = loc.set(
            js_string!("pathname"),
            JsValue::from(JsString::from(url.path())),
            false,
            context,
        );
        let search = url.query().map_or_else(String::new, |q| format!("?{q}"));
        let _ = loc.set(
            js_string!("search"),
            JsValue::from(JsString::from(search)),
            false,
            context,
        );
        let hash = url.fragment().map_or_else(String::new, |f| format!("#{f}"));
        let _ = loc.set(
            js_string!("hash"),
            JsValue::from(JsString::from(hash)),
            false,
            context,
        );
    }
    Ok(())
}
