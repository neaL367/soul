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
#[allow(clippy::too_many_lines)]
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

    ObjectInitializer::new(ctx)
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
            js_string!("classList"),
            JsValue::from(class_list_obj),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .build()
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
