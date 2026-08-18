//! `IndexedDB` JavaScript bindings exposing client-side database storage to Boa.

use boa_engine::{
    Context, JsArgs, JsResult, JsValue,
    gc::{Finalize, Trace},
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
};
use std::sync::Arc;
use storage::IndexedDbStore;

#[derive(Clone, Trace, Finalize)]
struct IdbHolder {
    #[unsafe_ignore_trace]
    store: Arc<IndexedDbStore>,
}

/// Registers the global `window.indexedDB` interface into the JavaScript context.
///
/// # Errors
/// Returns `JsResult` if object initialization fails.
pub fn register_indexeddb(
    context: &mut Context,
    idb_store: Arc<IndexedDbStore>,
) -> JsResult<()> {
    let holder = IdbHolder { store: idb_store };

    let put_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let db = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
            let store = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
            let key = args.get_or_undefined(2).to_string(ctx)?.to_std_string_escaped();
            let val = args.get_or_undefined(3).to_string(ctx)?.to_std_string_escaped();

            let _ = captures.store.put(&db, &store, &key, &val);
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let get_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let db = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
            let store = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
            let key = args.get_or_undefined(2).to_string(ctx)?.to_std_string_escaped();

            match captures.store.get(&db, &store, &key) {
                Ok(Some(val)) => Ok(JsValue::from(js_string!(val))),
                _ => Ok(JsValue::null()),
            }
        },
        holder.clone(),
    );

    let delete_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let db = args.get_or_undefined(0).to_string(ctx)?.to_std_string_escaped();
            let store = args.get_or_undefined(1).to_string(ctx)?.to_std_string_escaped();
            let key = args.get_or_undefined(2).to_string(ctx)?.to_std_string_escaped();

            let _ = captures.store.delete(&db, &store, &key);
            Ok(JsValue::undefined())
        },
        holder.clone(),
    );

    let open_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, captures, ctx| {
            let name_arg = args.get_or_undefined(0).to_string(ctx)?;
            let db_name = name_arg.to_std_string_escaped();
            let version = args.get_or_undefined(1).to_u32(ctx).unwrap_or(1);

            let _ = captures.store.open_or_create_db(&db_name, version);

            let db_obj = ObjectInitializer::new(ctx)
                .property(js_string!("name"), js_string!(db_name), Attribute::READONLY)
                .property(js_string!("version"), version, Attribute::READONLY)
                .build();

            Ok(db_obj.into())
        },
        holder,
    );

    let idb_obj = ObjectInitializer::new(context)
        .function(open_fn, js_string!("open"), 2)
        .function(put_fn, js_string!("put"), 4)
        .function(get_fn, js_string!("get"), 3)
        .function(delete_fn, js_string!("delete"), 3)
        .build();

    context.register_global_property(js_string!("indexedDB"), idb_obj, Attribute::all())?;
    Ok(())
}
