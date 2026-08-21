//! WHATWG `URLSearchParams` ECMAScript bindings.

#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::significant_drop_tightening,
    clippy::cast_possible_truncation,
    clippy::assigning_clones,
    clippy::if_not_else
)]

use super::encoding::{urlencoding_decode, urlencoding_encode};
use boa_engine::gc::{Finalize, Trace};
use boa_engine::object::builtins::JsArray;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsObject, JsResult, JsValue, NativeFunction, js_string};
use std::sync::{Arc, Mutex};
use url::Url;

/// Traceable wrapper holding a shared reference to search parameter pairs.
#[derive(Clone, Trace, Finalize)]
pub struct SearchParamsHolder {
    /// In-memory list of query key-value pairs.
    #[unsafe_ignore_trace]
    pub params: Arc<Mutex<Vec<(String, String)>>>,
    /// Optional parent URL instance for two-way query string synchronization.
    #[unsafe_ignore_trace]
    pub linked_url: Option<Arc<Mutex<Url>>>,
}

/// Constructs a new `URLSearchParams` JavaScript object instance.
///
/// # Errors
///
/// Returns `JsResult` on object allocation failure.
pub fn create_url_search_params_object(
    ctx: &mut Context,
    initial_pairs: Vec<(String, String)>,
    linked_url: Option<Arc<Mutex<Url>>>,
) -> JsResult<JsObject> {
    let holder = SearchParamsHolder {
        params: Arc::new(Mutex::new(initial_pairs)),
        linked_url,
    };

    let get_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let key = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let lock = captures.params.lock().unwrap();
                for (k, v) in lock.iter() {
                    if k == &key {
                        return Ok(JsValue::from(js_string!(v.clone())));
                    }
                }
                Ok(JsValue::null())
            },
            holder.clone(),
        ),
    )
    .build();

    let get_all_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let key = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let lock = captures.params.lock().unwrap();
                let matches: Vec<JsValue> = lock
                    .iter()
                    .filter(|(k, _)| k == &key)
                    .map(|(_, v)| JsValue::from(js_string!(v.clone())))
                    .collect();
                Ok(JsValue::from(JsArray::from_iter(matches, ctx)))
            },
            holder.clone(),
        ),
    )
    .build();

    let set_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let key = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let val = args
                    .get_or_undefined(1)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let mut lock = captures.params.lock().unwrap();
                let mut found = false;
                lock.retain_mut(|(k, v)| {
                    if k == &key {
                        if !found {
                            *v = val.clone();
                            found = true;
                            true
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                });
                if !found {
                    lock.push((key, val));
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let append_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let key = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let val = args
                    .get_or_undefined(1)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let mut lock = captures.params.lock().unwrap();
                lock.push((key, val));
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let has_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let key = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let lock = captures.params.lock().unwrap();
                let has = lock.iter().any(|(k, _)| k == &key);
                Ok(JsValue::from(has))
            },
            holder.clone(),
        ),
    )
    .build();

    let delete_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let key = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let mut lock = captures.params.lock().unwrap();
                lock.retain(|(k, _)| k != &key);
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let to_string_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.params.lock().unwrap();
                let mut out = String::new();
                for (i, (k, v)) in lock.iter().enumerate() {
                    if i > 0 {
                        out.push('&');
                    }
                    out.push_str(&urlencoding_encode(k));
                    out.push('=');
                    out.push_str(&urlencoding_encode(v));
                }
                Ok(JsValue::from(js_string!(out)))
            },
            holder.clone(),
        ),
    )
    .build();

    let sort_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let mut lock = captures.params.lock().unwrap();
                lock.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let size_getter = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, _args, captures, _ctx| {
                let lock = captures.params.lock().unwrap();
                Ok(JsValue::from(lock.len() as u32))
            },
            holder,
        ),
    )
    .build();

    let obj = ObjectInitializer::new(ctx)
        .property(js_string!("get"), get_fn, Attribute::all())
        .property(js_string!("getAll"), get_all_fn, Attribute::all())
        .property(js_string!("set"), set_fn, Attribute::all())
        .property(js_string!("append"), append_fn, Attribute::all())
        .property(js_string!("has"), has_fn, Attribute::all())
        .property(js_string!("delete"), delete_fn, Attribute::all())
        .property(js_string!("toString"), to_string_fn, Attribute::all())
        .property(js_string!("sort"), sort_fn, Attribute::all())
        .accessor(
            js_string!("size"),
            Some(size_getter),
            None,
            Attribute::all(),
        )
        .build();

    Ok(obj)
}

/// Constructs the global `URLSearchParams` function constructor.
pub fn create_url_search_params_constructor(ctx: &mut Context) -> JsObject {
    let params_constructor = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let init_val = args.get_or_undefined(0);
        let mut initial_pairs = Vec::new();

        if !init_val.is_undefined() && !init_val.is_null() {
            let init_str = init_val.to_string(ctx)?.to_std_string_escaped();
            let query = init_str.strip_prefix('?').unwrap_or(&init_str);
            for pair in query.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let mut split = pair.splitn(2, '=');
                let key = split.next().unwrap_or("");
                let val = split.next().unwrap_or("");
                let dec_key = urlencoding_decode(key);
                let dec_val = urlencoding_decode(val);
                initial_pairs.push((dec_key, dec_val));
            }
        }

        let obj = create_url_search_params_object(ctx, initial_pairs, None)?;
        Ok(JsValue::from(obj))
    });

    FunctionObjectBuilder::new(ctx.realm(), params_constructor)
        .constructor(true)
        .name(js_string!("URLSearchParams"))
        .length(0)
        .build()
        .into()
}
