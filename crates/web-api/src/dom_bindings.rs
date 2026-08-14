//! DOM element and document JavaScript bindings.

use boa_engine::{
    Context, JsArgs, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::Attribute,
};
use dom::{Document, InvalidationFlags, NodeData, NodeId};
use std::sync::{Arc, Mutex};

#[derive(Clone, Trace, Finalize)]
struct DocHolder(#[unsafe_ignore_trace] Arc<Mutex<Document>>);

#[derive(Clone, Trace, Finalize)]
struct NodeSetterHolder(
    #[unsafe_ignore_trace] Arc<Mutex<Document>>,
    #[unsafe_ignore_trace] NodeId,
);

/// Registers the global `document` object with `getElementById` and element property accessors.
///
/// # Errors
///
/// Returns a `JsResult` error if DOM property registration fails.
pub fn register_dom(context: &mut Context, document: Arc<Mutex<Document>>) -> JsResult<()> {
    let get_element_by_id_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let id_arg = args.get_or_undefined(0).to_string(ctx)?;
            let id_str = id_arg.to_std_string_escaped();

            let Ok(doc_guard) = captures.0.lock() else {
                return Ok(JsValue::null());
            };

            let Some(node_id) = doc_guard.get_element_by_id(&id_str) else {
                return Ok(JsValue::null());
            };

            let text_content = doc_guard.text_content(node_id);
            drop(doc_guard);

            // Build JS element object wrapper
            let setter_holder = NodeSetterHolder(captures.0.clone(), node_id);
            let set_text_fn = FunctionObjectBuilder::new(
                ctx.realm(),
                NativeFunction::from_copy_closure_with_captures(
                    |_this, args, setter_captures, ctx| {
                        let new_text = args.get_or_undefined(0).to_string(ctx)?;
                        let text_str = new_text.to_std_string_escaped();

                        if let Ok(mut doc) = setter_captures.0.lock() {
                            let target_node = setter_captures.1;
                            let children = doc.children(target_node);
                            let mut found_text = false;
                            for child_id in children {
                                if let Some(node) = doc.get_node_mut(child_id)
                                    && let NodeData::Text(ref mut t) = node.data
                                {
                                    t.clone_from(&text_str);
                                    found_text = true;
                                    node.dirty_flags = InvalidationFlags::all();
                                }
                            }

                            if !found_text {
                                let new_node_id = doc.alloc_node(NodeData::Text(text_str));
                                doc.append_child(target_node, new_node_id);
                            }

                            if let Some(node) = doc.get_node_mut(target_node) {
                                node.dirty_flags = InvalidationFlags::all();
                            }
                        }
                        Ok(JsValue::undefined())
                    },
                    setter_holder,
                ),
            )
            .build();

            let elem_obj = ObjectInitializer::new(ctx)
                .property(
                    js_string!("textContent"),
                    js_string!(text_content),
                    Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .property(
                    js_string!("setTextContent"),
                    JsValue::from(set_text_fn),
                    Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
                )
                .build();

            Ok(JsValue::from(elem_obj))
        },
        DocHolder(document),
    );

    let document_obj = ObjectInitializer::new(context)
        .function(get_element_by_id_fn, js_string!("getElementById"), 1)
        .build();

    context.register_global_property(
        js_string!("document"),
        document_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    )?;

    Ok(())
}
