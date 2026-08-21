//! WHATWG `XMLHttpRequest` and Fetch specification `FormData` ECMAScript bindings.

#![allow(clippy::unnecessary_wraps)]

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsArray;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsString, JsValue};

const FORM_DATA_ENTRIES_PROP: &str = "__soul_form_data_entries__";

/// Registers the standard `FormData` constructor class into the JS context.
pub fn register_form_data(context: &mut Context) {
    let form_data_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_form_data_constructor),
    )
    .constructor(true)
    .name(js_string!("FormData"))
    .length(0)
    .build();

    let proto = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(js_form_data_append),
            JsString::from("append"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_set),
            JsString::from("set"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_get),
            JsString::from("get"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_get_all),
            JsString::from("getAll"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_has),
            JsString::from("has"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_delete),
            JsString::from("delete"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_entries),
            JsString::from("entries"),
            0,
        )
        .build();

    let _ = form_data_ctor.set(JsString::from("prototype"), proto, false, context);
    let global = context.global_object();
    let _ = global.set(JsString::from("FormData"), form_data_ctor, false, context);
}

/// Creates a new `FormData` JS instance object.
#[must_use]
pub fn create_form_data_instance(context: &mut Context) -> JsObject {
    let entries_arr = JsArray::new(context);

    ObjectInitializer::new(context)
        .property(
            JsString::from(FORM_DATA_ENTRIES_PROP),
            JsValue::from(entries_arr),
            boa_engine::property::Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_append),
            JsString::from("append"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_set),
            JsString::from("set"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_get),
            JsString::from("get"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_get_all),
            JsString::from("getAll"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_has),
            JsString::from("has"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_delete),
            JsString::from("delete"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(js_form_data_entries),
            JsString::from("entries"),
            0,
        )
        .build()
}

fn js_form_data_constructor(
    _this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let instance = create_form_data_instance(context);
    Ok(JsValue::from(instance))
}

fn get_entries_array(this: &JsValue, context: &mut Context) -> JsResult<JsObject> {
    let Some(obj) = this.as_object() else {
        return Err(JsError::from(
            JsNativeError::typ().with_message("Value of 'this' must be a FormData instance"),
        ));
    };

    let entries_val = obj.get(JsString::from(FORM_DATA_ENTRIES_PROP), context)?;
    entries_val.as_object().map_or_else(
        || {
            let arr = JsArray::new(context);
            let _ = obj.set(
                JsString::from(FORM_DATA_ENTRIES_PROP),
                JsValue::from(arr.clone()),
                false,
                context,
            );
            Ok(arr.into())
        },
        Ok,
    )
}

fn js_form_data_append(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let name = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let value = args
        .get_or_undefined(1)
        .to_string(context)?
        .to_std_string_escaped();

    let entries = get_entries_array(this, context)?;
    let len = entries
        .get(JsString::from("length"), context)?
        .to_u32(context)
        .unwrap_or(0);

    let pair = JsArray::new(context);
    let _ = pair.set(0, JsValue::from(JsString::from(name)), false, context);
    let _ = pair.set(1, JsValue::from(JsString::from(value)), false, context);

    let _ = entries.set(len, JsValue::from(pair), false, context);
    Ok(JsValue::undefined())
}

fn js_form_data_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let value = args
        .get_or_undefined(1)
        .to_string(context)?
        .to_std_string_escaped();

    let _ = js_form_data_delete(
        this,
        &[JsValue::from(JsString::from(name.clone()))],
        context,
    )?;
    let _ = js_form_data_append(
        this,
        &[
            JsValue::from(JsString::from(name)),
            JsValue::from(JsString::from(value)),
        ],
        context,
    )?;

    Ok(JsValue::undefined())
}

fn js_form_data_get(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let target_name = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let entries = get_entries_array(this, context)?;
    let len = entries
        .get(JsString::from("length"), context)?
        .to_u32(context)
        .unwrap_or(0);

    for i in 0..len {
        let pair_val = entries.get(i, context)?;
        if let Some(pair) = pair_val.as_object() {
            let key = pair
                .get(0, context)?
                .to_string(context)?
                .to_std_string_escaped();
            if key == target_name {
                let val = pair.get(1, context)?;
                return Ok(val);
            }
        }
    }

    Ok(JsValue::null())
}

fn js_form_data_get_all(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let target_name = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let entries = get_entries_array(this, context)?;
    let len = entries
        .get(JsString::from("length"), context)?
        .to_u32(context)
        .unwrap_or(0);

    let result_arr = JsArray::new(context);
    let mut out_idx = 0;

    for i in 0..len {
        let pair_val = entries.get(i, context)?;
        if let Some(pair) = pair_val.as_object() {
            let key = pair
                .get(0, context)?
                .to_string(context)?
                .to_std_string_escaped();
            if key == target_name {
                let val = pair.get(1, context)?;
                let _ = result_arr.set(out_idx, val, false, context);
                out_idx += 1;
            }
        }
    }

    Ok(JsValue::from(result_arr))
}

fn js_form_data_has(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let target_name = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let entries = get_entries_array(this, context)?;
    let len = entries
        .get(JsString::from("length"), context)?
        .to_u32(context)
        .unwrap_or(0);

    for i in 0..len {
        let pair_val = entries.get(i, context)?;
        if let Some(pair) = pair_val.as_object() {
            let key = pair
                .get(0, context)?
                .to_string(context)?
                .to_std_string_escaped();
            if key == target_name {
                return Ok(JsValue::from(true));
            }
        }
    }

    Ok(JsValue::from(false))
}

fn js_form_data_delete(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let target_name = args
        .get_or_undefined(0)
        .to_string(context)?
        .to_std_string_escaped();
    let entries = get_entries_array(this, context)?;
    let len = entries
        .get(JsString::from("length"), context)?
        .to_u32(context)
        .unwrap_or(0);

    let new_arr = JsArray::new(context);
    let mut out_idx = 0;

    for i in 0..len {
        let pair_val = entries.get(i, context)?;
        if let Some(pair) = pair_val.as_object() {
            let key = pair
                .get(0, context)?
                .to_string(context)?
                .to_std_string_escaped();
            if key != target_name {
                let _ = new_arr.set(out_idx, pair_val, false, context);
                out_idx += 1;
            }
        }
    }

    if let Some(obj) = this.as_object() {
        let _ = obj.set(
            JsString::from(FORM_DATA_ENTRIES_PROP),
            JsValue::from(new_arr),
            false,
            context,
        );
    }

    Ok(JsValue::undefined())
}

fn js_form_data_entries(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let entries = get_entries_array(this, context)?;
    Ok(JsValue::from(entries))
}
