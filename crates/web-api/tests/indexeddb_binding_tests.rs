//! Integration tests for Boa JS IndexedDB bindings.

use javascript::JsRuntime;
use std::sync::Arc;
use storage::{IndexedDbStore, StorageDatabase};
use web_api::register_indexeddb;

#[test]
fn test_indexeddb_js_bindings_roundtrip() {
    let db = StorageDatabase::open_in_memory().expect("in-memory database");
    let store = Arc::new(IndexedDbStore::new(db).expect("create indexeddb store"));

    let mut runtime = JsRuntime::new();

    register_indexeddb(&mut runtime.context, store.clone())
        .expect("register indexeddb binding");

    let script = r#"
        var db = indexedDB.open("TestApp", 1);
        indexedDB.put("TestApp", "userData", "user:101", "Alice");
        var readVal = indexedDB.get("TestApp", "userData", "user:101");
        readVal;
    "#;

    let result = runtime.eval(script).expect("eval indexeddb script");
    assert_eq!(result, "\"Alice\"");

    // Check underlying store
    let persisted = store.get("TestApp", "userData", "user:101").unwrap();
    assert_eq!(persisted.as_deref(), Some("Alice"));

    // Delete
    runtime
        .eval(r#"indexedDB.delete("TestApp", "userData", "user:101");"#)
        .expect("eval delete");
    let deleted = store.get("TestApp", "userData", "user:101").unwrap();
    assert_eq!(deleted, None);
}
