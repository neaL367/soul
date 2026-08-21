//! W3C Clipboard API specification `navigator.clipboard` ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::ObjectInitializer;
use boa_engine::object::builtins::JsPromise;
use boa_engine::{Context, JsArgs, JsObject, JsResult, JsString, JsValue};

const CLIPBOARD_BUFFER_PROP: &str = "__soul_clipboard_buffer__";

/// Creates the `navigator.clipboard` JS instance.
#[must_use]
pub fn create_clipboard_object(context: &mut Context) -> JsObject {
    ObjectInitializer::new(context)
        .property(
            js_string!(CLIPBOARD_BUFFER_PROP),
            JsValue::from(js_string!("")),
            boa_engine::property::Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(js_clipboard_write_text),
            js_string!("writeText"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_clipboard_read_text),
            js_string!("readText"),
            0,
        )
        .build()
}

fn js_clipboard_write_text(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let text = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();

    if let Some(clip) = this.as_object() {
        let _ = clip.set(
            js_string!(CLIPBOARD_BUFFER_PROP),
            JsValue::from(JsString::from(text)),
            false,
            context,
        );
    }

    let promise = JsPromise::new(
        |resolvers, ctx| {
            let _ = resolvers.resolve.call(&JsValue::undefined(), &[], ctx);
            Ok(JsValue::undefined())
        },
        context,
    );

    Ok(JsValue::from(promise))
}

fn js_clipboard_read_text(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let current_text = if let Some(clip) = this.as_object() {
        clip.get(js_string!(CLIPBOARD_BUFFER_PROP), context)?
    } else {
        JsValue::from(js_string!(""))
    };

    let promise = JsPromise::new(
        move |resolvers, ctx| {
            let _ = resolvers
                .resolve
                .call(&JsValue::undefined(), &[current_text], ctx);
            Ok(JsValue::undefined())
        },
        context,
    );

    Ok(JsValue::from(promise))
}
