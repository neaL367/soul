//! JavaScript wrapper builder for DOM `Element` nodes.

use boa_engine::{
    Context, JsArgs, JsObject, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::{FunctionObjectBuilder, ObjectInitializer},
    property::Attribute,
};
use dom::{Document, NodeData, NodeId};
use std::sync::{Arc, Mutex};

/// Thread-safe wrapper holding a `Document` pointer and a specific `NodeId`.
#[derive(Clone, Trace, Finalize)]
pub struct NodeHolder(
    #[unsafe_ignore_trace] pub Arc<Mutex<Document>>,
    #[unsafe_ignore_trace] pub NodeId,
);

/// Constructs a rich JavaScript `Element` wrapper around a specific `NodeId`.
#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
pub fn create_element_wrapper(
    ctx: &mut Context,
    document: Arc<Mutex<Document>>,
    node_id: NodeId,
) -> JsObject {
    let holder = NodeHolder(document, node_id);

    let get_attr_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let name = args.get_or_undefined(0).to_string(ctx)?;
                let name_str = name.to_std_string_escaped();
                if let Ok(doc) = captures.0.lock()
                    && let Some(node) = doc.get_node(captures.1)
                    && let NodeData::Element(ref elem) = node.data
                    && let Some(val) = elem.attr(&name_str)
                {
                    return Ok(JsValue::from(js_string!(val)));
                }
                Ok(JsValue::null())
            },
            holder.clone(),
        ),
    )
    .build();

    let set_attr_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let name = args.get_or_undefined(0).to_string(ctx)?;
                let val = args.get_or_undefined(1).to_string(ctx)?;
                let name_str = name.to_std_string_escaped();
                let val_str = val.to_std_string_escaped();
                if let Ok(mut doc) = captures.0.lock() {
                    doc.set_attribute(captures.1, &name_str, &val_str);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let remove_attr_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let name = args.get_or_undefined(0).to_string(ctx)?;
                let name_str = name.to_std_string_escaped();
                if let Ok(mut doc) = captures.0.lock() {
                    doc.remove_attribute(captures.1, &name_str);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let set_text_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let new_text = args.get_or_undefined(0).to_string(ctx)?;
                let text_str = new_text.to_std_string_escaped();
                if let Ok(mut doc) = captures.0.lock() {
                    doc.set_text_content(captures.1, &text_str);
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let append_child_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let child_val = args.get_or_undefined(0);
                if let Some(child_obj) = child_val.as_object() {
                    let id_val = child_obj.get(js_string!("__soul_node_id__"), ctx)?;
                    let child_id = NodeId(id_val.to_u32(ctx).unwrap_or(0) as usize);
                    if let Ok(mut doc) = captures.0.lock() {
                        doc.append_child(captures.1, child_id);
                    }
                }
                Ok(child_val.clone())
            },
            holder.clone(),
        ),
    )
    .build();

    let remove_child_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let child_val = args.get_or_undefined(0);
                if let Some(child_obj) = child_val.as_object() {
                    let id_val = child_obj.get(js_string!("__soul_node_id__"), ctx)?;
                    let child_id = NodeId(id_val.to_u32(ctx).unwrap_or(0) as usize);
                    if let Ok(mut doc) = captures.0.lock() {
                        doc.remove_child(captures.1, child_id);
                    }
                }
                Ok(child_val.clone())
            },
            holder.clone(),
        ),
    )
    .build();

    let replace_child_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let new_child_val = args.get_or_undefined(0);
                let old_child_val = args.get_or_undefined(1);
                if let (Some(new_obj), Some(old_obj)) =
                    (new_child_val.as_object(), old_child_val.as_object())
                {
                    let new_id = NodeId(
                        new_obj
                            .get(js_string!("__soul_node_id__"), ctx)?
                            .to_u32(ctx)
                            .unwrap_or(0) as usize,
                    );
                    let old_id = NodeId(
                        old_obj
                            .get(js_string!("__soul_node_id__"), ctx)?
                            .to_u32(ctx)
                            .unwrap_or(0) as usize,
                    );
                    if let Ok(mut doc) = captures.0.lock() {
                        doc.replace_child(captures.1, new_id, old_id);
                    }
                }
                Ok(old_child_val.clone())
            },
            holder.clone(),
        ),
    )
    .build();

    let clone_node_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let deep = args.get_or_undefined(0).to_boolean();
                let cloned_id = captures
                    .0
                    .lock()
                    .map_or(NodeId(0), |mut doc| doc.clone_node(captures.1, deep));
                let clone_wrapper = create_element_wrapper(ctx, captures.0.clone(), cloned_id);
                Ok(JsValue::from(clone_wrapper))
            },
            holder.clone(),
        ),
    )
    .build();

    let contains_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let other_val = args.get_or_undefined(0);
                if let Some(other_obj) = other_val.as_object() {
                    let other_id = NodeId(
                        other_obj
                            .get(js_string!("__soul_node_id__"), ctx)?
                            .to_u32(ctx)
                            .unwrap_or(0) as usize,
                    );
                    let is_contained = captures
                        .0
                        .lock()
                        .is_ok_and(|doc| doc.contains(captures.1, other_id));
                    return Ok(JsValue::from(is_contained));
                }
                Ok(JsValue::from(false))
            },
            holder.clone(),
        ),
    )
    .build();

    let matches_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let sel = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let is_match = captures
                    .0
                    .lock()
                    .is_ok_and(|doc| doc.matches(captures.1, &sel));
                Ok(JsValue::from(is_match))
            },
            holder.clone(),
        ),
    )
    .build();

    let closest_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let sel = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_escaped();
                let target_id = captures.0.lock().ok().and_then(|doc| doc.closest(captures.1, &sel));
                target_id.map_or_else(
                    || Ok(JsValue::null()),
                    |tid| {
                        let wrapper = create_element_wrapper(ctx, captures.0.clone(), tid);
                        Ok(JsValue::from(wrapper))
                    },
                )
            },
            holder.clone(),
        ),
    )
    .build();

    let class_list_obj = create_class_list_wrapper(ctx, holder.clone());

    let (tag_name, text_content) = holder.0.lock().map_or_else(
        |_| ("DIV".to_string(), String::new()),
        |doc| {
            let tag = doc
                .get_node(node_id)
                .and_then(|n| n.as_element())
                .map_or_else(|| "DIV".to_string(), |e| e.tag_name.to_uppercase());
            let text = doc.text_content(node_id);
            (tag, text)
        },
    );

    let maybe_canvas_get_context = if tag_name == "CANVAS" {
        let canvas_ctx = media::Canvas2DContext::new(300, 150)
            .unwrap_or_else(|_| media::Canvas2DContext::new(1, 1).expect("1x1 fallback canvas"));
        let shared_ctx = Arc::new(Mutex::new(canvas_ctx));
        let canvas_holder = crate::canvas_binding::CanvasHolder(shared_ctx);

        let get_context_fn = FunctionObjectBuilder::new(
            ctx.realm(),
            NativeFunction::from_copy_closure_with_captures(
                |_this, args, caps, ctx| {
                    let context_id = args
                        .get_or_undefined(0)
                        .to_string(ctx)?
                        .to_std_string_escaped();
                    if context_id.eq_ignore_ascii_case("2d") {
                        let ctx2d_obj =
                            crate::canvas_binding::create_canvas_context_2d(ctx, caps.clone())?;
                        Ok(JsValue::from(ctx2d_obj))
                    } else {
                        Ok(JsValue::null())
                    }
                },
                canvas_holder,
            ),
        )
        .build();
        Some(get_context_fn)
    } else {
        None
    };

    #[allow(clippy::cast_possible_truncation)]
    let mut builder = ObjectInitializer::new(ctx);
    builder
        .property(
            js_string!("__soul_node_id__"),
            JsValue::from(node_id.0 as u32),
            Attribute::all(),
        )
        .property(
            js_string!("tagName"),
            js_string!(tag_name),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
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
        .property(
            js_string!("getAttribute"),
            JsValue::from(get_attr_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("setAttribute"),
            JsValue::from(set_attr_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("removeAttribute"),
            JsValue::from(remove_attr_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("appendChild"),
            JsValue::from(append_child_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("removeChild"),
            JsValue::from(remove_child_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("replaceChild"),
            JsValue::from(replace_child_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("cloneNode"),
            JsValue::from(clone_node_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("contains"),
            JsValue::from(contains_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("matches"),
            JsValue::from(matches_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("closest"),
            JsValue::from(closest_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("classList"),
            JsValue::from(class_list_obj),
            Attribute::READONLY | Attribute::ENUMERABLE,
        );

    if let Some(get_context_fn) = maybe_canvas_get_context {
        builder.property(
            js_string!("getContext"),
            JsValue::from(get_context_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        );
        builder.property(js_string!("width"), 300.0, Attribute::all());
        builder.property(js_string!("height"), 150.0, Attribute::all());
    }

    builder.build()
}

#[allow(clippy::too_many_lines)]
fn create_class_list_wrapper(ctx: &mut Context, holder: NodeHolder) -> JsObject {
    let add_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let cls = args.get_or_undefined(0).to_string(ctx)?;
                let cls_str = cls.to_std_string_escaped();
                if let Ok(mut doc) = captures.0.lock()
                    && let Some(node) = doc.get_node_mut(captures.1)
                    && let NodeData::Element(ref mut elem) = node.data
                {
                    elem.add_class(&cls_str);
                    node.dirty_flags.style = true;
                    node.dirty_flags.layout = true;
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let remove_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let cls = args.get_or_undefined(0).to_string(ctx)?;
                let cls_str = cls.to_std_string_escaped();
                if let Ok(mut doc) = captures.0.lock()
                    && let Some(node) = doc.get_node_mut(captures.1)
                    && let NodeData::Element(ref mut elem) = node.data
                {
                    elem.remove_class(&cls_str);
                    node.dirty_flags.style = true;
                    node.dirty_flags.layout = true;
                }
                Ok(JsValue::undefined())
            },
            holder.clone(),
        ),
    )
    .build();

    let toggle_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let cls = args.get_or_undefined(0).to_string(ctx)?;
                let cls_str = cls.to_std_string_escaped();
                let mut toggled = false;
                if let Ok(mut doc) = captures.0.lock()
                    && let Some(node) = doc.get_node_mut(captures.1)
                    && let NodeData::Element(ref mut elem) = node.data
                {
                    toggled = elem.toggle_class(&cls_str);
                    node.dirty_flags.style = true;
                    node.dirty_flags.layout = true;
                }
                Ok(JsValue::from(toggled))
            },
            holder.clone(),
        ),
    )
    .build();

    let contains_fn = FunctionObjectBuilder::new(
        ctx.realm(),
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, captures, ctx| {
                let cls = args.get_or_undefined(0).to_string(ctx)?;
                let cls_str = cls.to_std_string_escaped();
                let has = if let Ok(doc) = captures.0.lock()
                    && let Some(node) = doc.get_node(captures.1)
                    && let NodeData::Element(ref elem) = node.data
                {
                    elem.has_class(&cls_str)
                } else {
                    false
                };
                Ok(JsValue::from(has))
            },
            holder,
        ),
    )
    .build();

    ObjectInitializer::new(ctx)
        .property(
            js_string!("add"),
            JsValue::from(add_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("remove"),
            JsValue::from(remove_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("toggle"),
            JsValue::from(toggle_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("contains"),
            JsValue::from(contains_fn),
            Attribute::WRITABLE | Attribute::CONFIGURABLE | Attribute::ENUMERABLE,
        )
        .build()
}
