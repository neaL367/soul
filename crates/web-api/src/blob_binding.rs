//! W3C File API specification `Blob` and `File` ECMAScript bindings.

#![allow(
    clippy::unnecessary_wraps,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsArray;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsObject, JsResult, JsString, JsValue};

const BLOB_DATA_PROP: &str = "__soul_blob_data__";

/// Registers the standard `Blob` and `File` constructors into the JS context.
pub fn register_blob(context: &mut Context) {
    let blob_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_blob_constructor),
    )
    .constructor(true)
    .name(js_string!("Blob"))
    .length(0)
    .build();

    let file_ctor = FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_fn_ptr(js_file_constructor),
    )
    .constructor(true)
    .name(js_string!("File"))
    .length(2)
    .build();

    let global = context.global_object();
    let _ = global.set(js_string!("Blob"), blob_ctor, false, context);
    let _ = global.set(js_string!("File"), file_ctor, false, context);
}

/// Creates a `Blob` JS object instance from in-memory bytes and MIME type.
#[must_use]
pub fn create_blob_instance(bytes: &[u8], mime_type: String, context: &mut Context) -> JsObject {
    let size = bytes.len();
    let byte_arr = JsArray::new(context);
    for (i, b) in bytes.iter().enumerate() {
        let _ = byte_arr.set(i, JsValue::from(u32::from(*b)), false, context);
    }

    ObjectInitializer::new(context)
        .property(
            JsString::from(BLOB_DATA_PROP),
            JsValue::from(byte_arr),
            Attribute::all(),
        )
        .property(
            JsString::from("size"),
            JsValue::from(size as u32),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            JsString::from("type"),
            JsValue::from(JsString::from(mime_type)),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .function(
            NativeFunction::from_fn_ptr(js_blob_slice),
            JsString::from("slice"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(js_blob_text),
            JsString::from("text"),
            0,
        )
        .build()
}

fn js_blob_constructor(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parts = args.get_or_undefined(0);
    let mut collected_bytes = Vec::new();

    if let Some(arr) = parts.as_object() {
        let len = arr
            .get(JsString::from("length"), context)?
            .to_u32(context)
            .unwrap_or(0);
        for i in 0..len {
            let item = arr.get(i, context)?;
            let text = item.to_string(context)?.to_std_string_escaped();
            collected_bytes.extend_from_slice(text.as_bytes());
        }
    }

    let mime_type = if let Some(opts) = args.get_or_undefined(1).as_object()
        && let Ok(type_val) = opts.get(JsString::from("type"), context)
        && !type_val.is_undefined()
    {
        type_val.to_string(context)?.to_std_string_escaped()
    } else {
        String::new()
    };

    let instance = create_blob_instance(&collected_bytes, mime_type, context);
    Ok(JsValue::from(instance))
}

fn js_file_constructor(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parts = args.get_or_undefined(0);
    let name = args
        .get_or_undefined(1)
        .to_string(context)?
        .to_std_string_escaped();

    let mut collected_bytes = Vec::new();
    if let Some(arr) = parts.as_object() {
        let len = arr
            .get(JsString::from("length"), context)?
            .to_u32(context)
            .unwrap_or(0);
        for i in 0..len {
            let item = arr.get(i, context)?;
            let text = item.to_string(context)?.to_std_string_escaped();
            collected_bytes.extend_from_slice(text.as_bytes());
        }
    }

    let mut mime_type = String::new();
    let mut last_modified: u64 = 1_700_000_000_000;

    if let Some(opts) = args.get_or_undefined(2).as_object() {
        if let Ok(type_val) = opts.get(JsString::from("type"), context)
            && !type_val.is_undefined()
        {
            mime_type = type_val.to_string(context)?.to_std_string_escaped();
        }
        if let Ok(lm_val) = opts.get(JsString::from("lastModified"), context)
            && let Ok(num) = lm_val.to_number(context)
        {
            #[allow(clippy::cast_sign_loss)]
            {
                last_modified = num.max(0.0) as u64;
            }
        }
    }

    let instance = create_blob_instance(&collected_bytes, mime_type, context);
    let _ = instance.set(
        JsString::from("name"),
        JsValue::from(JsString::from(name)),
        false,
        context,
    );
    let _ = instance.set(
        JsString::from("lastModified"),
        JsValue::from(last_modified as f64),
        false,
        context,
    );

    Ok(JsValue::from(instance))
}

fn js_blob_slice(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(obj) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let start = args.get_or_undefined(0).to_u32(context).unwrap_or(0) as usize;
    let size_val = obj.get(JsString::from("size"), context)?;
    let total_size = size_val.to_u32(context).unwrap_or(0) as usize;

    let end = args.get(1).map_or(total_size, |end_val| {
        (end_val.to_u32(context).unwrap_or(total_size as u32) as usize).min(total_size)
    });

    let blob_data = obj.get(JsString::from(BLOB_DATA_PROP), context)?;
    let mut sliced_bytes = Vec::new();

    if let Some(arr) = blob_data.as_object() {
        for idx in start..end {
            if let Ok(b_val) = arr.get(idx, context)
                && let Ok(b) = b_val.to_u32(context)
            {
                sliced_bytes.push(b as u8);
            }
        }
    }

    let mime_type = if let Some(type_val) = args.get(2) {
        type_val.to_string(context)?.to_std_string_escaped()
    } else {
        String::new()
    };

    let sliced_blob = create_blob_instance(&sliced_bytes, mime_type, context);
    Ok(JsValue::from(sliced_blob))
}

fn js_blob_text(this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(obj) = this.as_object() else {
        return Ok(JsValue::undefined());
    };

    let blob_data = obj.get(JsString::from(BLOB_DATA_PROP), context)?;
    let mut bytes = Vec::new();

    if let Some(arr) = blob_data.as_object() {
        let len = arr
            .get(JsString::from("length"), context)?
            .to_u32(context)
            .unwrap_or(0);
        for idx in 0..len {
            if let Ok(b_val) = arr.get(idx, context)
                && let Ok(b) = b_val.to_u32(context)
            {
                bytes.push(b as u8);
            }
        }
    }

    let text_content = String::from_utf8_lossy(&bytes).to_string();
    Ok(JsValue::from(JsString::from(text_content)))
}
