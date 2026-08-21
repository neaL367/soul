//! WHATWG `URL` class object and constructor implementation.

#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::significant_drop_tightening,
    clippy::cast_possible_truncation
)]

use boa_engine::gc::{Finalize, Trace};
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{
    Context, JsArgs, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, js_string,
};
use std::sync::{Arc, Mutex};
use url::Url;

/// Traceable wrapper holding a shared reference to a parsed WHATWG [`Url`].
#[derive(Clone, Trace, Finalize)]
pub struct UrlHolder(#[unsafe_ignore_trace] pub Arc<Mutex<Url>>);

/// Constructs a new `URL` JavaScript object instance.
///
/// # Errors
///
/// Returns `JsResult` on object allocation failure.
pub fn create_url_object(ctx: &mut Context, parsed: Url) -> JsResult<JsObject> {
    let shared = Arc::new(Mutex::new(parsed));
    let holder = UrlHolder(shared);

    let get_href = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                Ok(JsValue::from(js_string!(lock.as_str())))
            },
            holder.clone(),
        ),
    )
    .build();

    let set_href = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let new_href = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let parsed = Url::parse(&new_href).map_err(|e| {
                    JsError::from(JsNativeError::typ().with_message(format!("Invalid URL: {e}")))
                })?;
                let mut lock = captures.0.lock().unwrap();
                *lock = parsed;
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let get_origin = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                Ok(JsValue::from(js_string!(
                    lock.origin().ascii_serialization()
                )))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_protocol = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                Ok(JsValue::from(js_string!(format!("{}:", lock.scheme()))))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_host = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                let host = lock.host_str().unwrap_or("");
                let port = lock.port().map_or_else(String::new, |p| format!(":{p}"));
                Ok(JsValue::from(js_string!(format!("{host}{port}"))))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_hostname = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                Ok(JsValue::from(js_string!(lock.host_str().unwrap_or(""))))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_port = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                let port = lock.port().map_or_else(String::new, |p| p.to_string());
                Ok(JsValue::from(js_string!(port)))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_pathname = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                Ok(JsValue::from(js_string!(lock.path())))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_search = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                let search = lock.query().map_or_else(String::new, |q| format!("?{q}"));
                Ok(JsValue::from(js_string!(search)))
            },
            holder.clone(),
        ),
    )
    .build();

    let get_hash = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                let hash = lock
                    .fragment()
                    .map_or_else(String::new, |f| format!("#{f}"));
                Ok(JsValue::from(js_string!(hash)))
            },
            holder.clone(),
        ),
    )
    .build();

    let to_string_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.0.lock().unwrap();
                Ok(JsValue::from(js_string!(lock.as_str())))
            },
            holder,
        ),
    )
    .build();

    let obj = ObjectInitializer::new(ctx)
        .accessor(
            js_string!("href"),
            Some(get_href),
            Some(set_href),
            Attribute::all(),
        )
        .accessor(
            js_string!("origin"),
            Some(get_origin),
            None,
            Attribute::all(),
        )
        .accessor(
            js_string!("protocol"),
            Some(get_protocol),
            None,
            Attribute::all(),
        )
        .accessor(js_string!("host"), Some(get_host), None, Attribute::all())
        .accessor(
            js_string!("hostname"),
            Some(get_hostname),
            None,
            Attribute::all(),
        )
        .accessor(js_string!("port"), Some(get_port), None, Attribute::all())
        .accessor(
            js_string!("pathname"),
            Some(get_pathname),
            None,
            Attribute::all(),
        )
        .accessor(
            js_string!("search"),
            Some(get_search),
            None,
            Attribute::all(),
        )
        .accessor(js_string!("hash"), Some(get_hash), None, Attribute::all())
        .property(
            js_string!("toString"),
            to_string_fn.clone(),
            Attribute::all(),
        )
        .property(js_string!("toJSON"), to_string_fn, Attribute::all())
        .build();

    Ok(obj)
}

/// Constructs the global `URL` function constructor.
pub fn create_url_constructor(ctx: &mut Context) -> JsObject {
    let url_constructor = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let input_str = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();

        let base_opt = if let Some(base_val) = args.get(1) {
            if base_val.is_undefined() {
                None
            } else {
                Some(base_val.to_string(ctx)?.to_std_string_escaped())
            }
        } else {
            None
        };

        let parsed = match base_opt {
            Some(base_str) => {
                let base = Url::parse(&base_str).map_err(|e| {
                    JsError::from(
                        JsNativeError::typ().with_message(format!("Invalid base URL: {e}")),
                    )
                })?;
                base.join(&input_str).map_err(|e| {
                    JsError::from(JsNativeError::typ().with_message(format!("Invalid URL: {e}")))
                })?
            }
            None => Url::parse(&input_str).map_err(|e| {
                JsError::from(JsNativeError::typ().with_message(format!("Invalid URL: {e}")))
            })?,
        };

        let obj = create_url_object(ctx, parsed)?;
        Ok(JsValue::from(obj))
    });

    FunctionObjectBuilder::new(ctx.realm(), url_constructor)
        .constructor(true)
        .name(js_string!("URL"))
        .length(1)
        .build()
        .into()
}
