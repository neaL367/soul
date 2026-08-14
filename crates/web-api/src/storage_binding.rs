//! Web Storage (`localStorage` and `sessionStorage`) JavaScript bindings for Boa.

use boa_engine::{
    Context, JsArgs, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};
use std::sync::Arc;
use storage::{LocalStorage, SessionStorage};

#[derive(Clone, Trace, Finalize)]
struct LocalStorageHolder {
    #[unsafe_ignore_trace]
    storage: Arc<LocalStorage>,
    #[unsafe_ignore_trace]
    origin: String,
}

#[derive(Clone, Trace, Finalize)]
struct SessionStorageHolder {
    #[unsafe_ignore_trace]
    storage: Arc<SessionStorage>,
    #[unsafe_ignore_trace]
    origin: String,
}

/// Registers the global `window.localStorage` object into the JavaScript context.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn register_local_storage(
    context: &mut Context,
    local_storage: Arc<LocalStorage>,
    origin: &str,
) -> JsResult<()> {
    let holder = LocalStorageHolder {
        storage: local_storage,
        origin: origin.to_string(),
    };

    let get_item_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let key_arg = args.get_or_undefined(0).to_string(ctx)?;
            let key_str = key_arg.to_std_string_escaped();

            match captures.storage.get_item(&captures.origin, &key_str) {
                Ok(Some(val)) => Ok(JsValue::from(js_string!(val))),
                _ => Ok(JsValue::null()),
            }
        },
        holder.clone(),
    );

    let set_item_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let key_arg = args.get_or_undefined(0).to_string(ctx)?;
            let val_arg = args.get_or_undefined(1).to_string(ctx)?;
            let key_str = key_arg.to_std_string_escaped();
            let val_str = val_arg.to_std_string_escaped();

            let _ = captures
                .storage
                .set_item(&captures.origin, &key_str, &val_str);
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let remove_item_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let key_arg = args.get_or_undefined(0).to_string(ctx)?;
            let key_str = key_arg.to_std_string_escaped();

            let _ = captures.storage.remove_item(&captures.origin, &key_str);
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let clear_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, captures, _ctx| {
            let _ = captures.storage.clear_origin(&captures.origin);
            Ok(JsValue::undefined())
        },
        holder,
    );

    let storage_obj = ObjectInitializer::new(context)
        .function(get_item_fn, js_string!("getItem"), 1)
        .function(set_item_fn, js_string!("setItem"), 2)
        .function(remove_item_fn, js_string!("removeItem"), 1)
        .function(clear_fn, js_string!("clear"), 0)
        .build();

    context.register_global_property(
        js_string!("localStorage"),
        storage_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}

/// Registers the global `window.sessionStorage` object into the JavaScript context.
///
/// # Errors
///
/// Returns `JsResult` if registration fails.
pub fn register_session_storage(
    context: &mut Context,
    session_storage: Arc<SessionStorage>,
    origin: &str,
) -> JsResult<()> {
    let holder = SessionStorageHolder {
        storage: session_storage,
        origin: origin.to_string(),
    };

    let get_item_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let key_arg = args.get_or_undefined(0).to_string(ctx)?;
            let key_str = key_arg.to_std_string_escaped();

            captures
                .storage
                .get_item(&captures.origin, &key_str)
                .map_or_else(
                    || Ok(JsValue::null()),
                    |val| Ok(JsValue::from(js_string!(val))),
                )
        },
        holder.clone(),
    );

    let set_item_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let key_arg = args.get_or_undefined(0).to_string(ctx)?;
            let val_arg = args.get_or_undefined(1).to_string(ctx)?;
            let key_str = key_arg.to_std_string_escaped();
            let val_str = val_arg.to_std_string_escaped();

            captures
                .storage
                .set_item(&captures.origin, &key_str, &val_str);
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let remove_item_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let key_arg = args.get_or_undefined(0).to_string(ctx)?;
            let key_str = key_arg.to_std_string_escaped();

            let _ = captures.storage.remove_item(&captures.origin, &key_str);
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let clear_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, _args, captures, _ctx| {
            captures.storage.clear_origin(&captures.origin);
            Ok(JsValue::undefined())
        },
        holder,
    );

    let storage_obj = ObjectInitializer::new(context)
        .function(get_item_fn, js_string!("getItem"), 1)
        .function(set_item_fn, js_string!("setItem"), 2)
        .function(remove_item_fn, js_string!("removeItem"), 1)
        .function(clear_fn, js_string!("clear"), 0)
        .build();

    context.register_global_property(
        js_string!("sessionStorage"),
        storage_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}
