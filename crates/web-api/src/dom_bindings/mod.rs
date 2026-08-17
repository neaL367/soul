//! DOM element and document JavaScript bindings.

pub mod element_wrapper;
pub mod query;

use boa_engine::{
    Context, JsArgs, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};
use dom::{Document, NodeId};
use element_wrapper::create_element_wrapper;
use std::sync::{Arc, Mutex};

#[derive(Clone, Trace, Finalize)]
struct DocHolder(#[unsafe_ignore_trace] Arc<Mutex<Document>>);

/// Registers the global `document` object with query and mutation methods.
///
/// # Errors
///
/// Returns a `JsResult` error if DOM property registration fails.
pub fn register_dom(context: &mut Context, document: Arc<Mutex<Document>>) -> JsResult<()> {
    let doc_clone1 = document.clone();
    let get_element_by_id_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let id_arg = args.get_or_undefined(0).to_string(ctx)?;
            let id_str = id_arg.to_std_string_escaped();

            let node_id = captures
                .0
                .lock()
                .ok()
                .and_then(|doc| doc.get_element_by_id(&id_str));

            let Some(node_id) = node_id else {
                return Ok(JsValue::null());
            };

            let elem_obj = create_element_wrapper(ctx, captures.0.clone(), node_id);
            Ok(JsValue::from(elem_obj))
        },
        DocHolder(doc_clone1),
    );

    let doc_clone2 = document.clone();
    let query_selector_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let sel_arg = args.get_or_undefined(0).to_string(ctx)?;
            let sel_str = sel_arg.to_std_string_escaped();

            let node_id = captures
                .0
                .lock()
                .ok()
                .and_then(|doc| query::query_selector(&doc, &sel_str));

            let Some(node_id) = node_id else {
                return Ok(JsValue::null());
            };

            let elem_obj = create_element_wrapper(ctx, captures.0.clone(), node_id);
            Ok(JsValue::from(elem_obj))
        },
        DocHolder(doc_clone2),
    );

    let create_element_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let tag_arg = args.get_or_undefined(0).to_string(ctx)?;
            let tag_str = tag_arg.to_std_string_escaped();

            let node_id = captures
                .0
                .lock()
                .map_or(NodeId(0), |mut doc| doc.create_element(&tag_str));

            let elem_obj = create_element_wrapper(ctx, captures.0.clone(), node_id);
            Ok(JsValue::from(elem_obj))
        },
        DocHolder(document),
    );

    let document_obj = ObjectInitializer::new(context)
        .function(get_element_by_id_fn, js_string!("getElementById"), 1)
        .function(query_selector_fn, js_string!("querySelector"), 1)
        .function(create_element_fn, js_string!("createElement"), 1)
        .build();

    context.register_global_property(
        js_string!("document"),
        document_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}
