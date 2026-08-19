//! Integration tests for Boa JS `IndexedDB` bindings.

use javascript::JsRuntime;
use std::sync::Arc;
use storage::{IndexedDbStore, StorageDatabase};
use web_api::register_indexeddb;

const ORIGIN_A: &str = "https://app-a.example";
const ORIGIN_B: &str = "https://app-b.example";

#[test]
fn test_indexeddb_js_bindings_roundtrip() {
    let db = StorageDatabase::open_in_memory().expect("in-memory database");
    let store = Arc::new(IndexedDbStore::new(db).expect("create indexeddb store"));

    let mut runtime = JsRuntime::new();

    register_indexeddb(&mut runtime.context, store.clone(), ORIGIN_A)
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
    let persisted = store
        .get(ORIGIN_A, "TestApp", "userData", "user:101")
        .unwrap();
    assert_eq!(persisted.as_deref(), Some("Alice"));

    // Delete
    runtime
        .eval(r#"indexedDB.delete("TestApp", "userData", "user:101");"#)
        .expect("eval delete");
    let deleted = store
        .get(ORIGIN_A, "TestApp", "userData", "user:101")
        .unwrap();
    assert_eq!(deleted, None);
}

#[test]
fn test_indexeddb_js_bindings_are_scoped_to_the_registered_origin() {
    let db = StorageDatabase::open_in_memory().expect("in-memory database");
    let store = Arc::new(IndexedDbStore::new(db).expect("create indexeddb store"));

    let mut runtime_a = JsRuntime::new();
    register_indexeddb(&mut runtime_a.context, store.clone(), ORIGIN_A)
        .expect("register indexeddb for origin A");
    runtime_a
        .eval(r#"indexedDB.put("Shared", "store", "k", "data-from-a");"#)
        .expect("eval put for A");

    // A second context for a different origin must not see A's data even
    // though both use the same database/store/key names.
    let mut runtime_b = JsRuntime::new();
    register_indexeddb(&mut runtime_b.context, store.clone(), ORIGIN_B)
        .expect("register indexeddb for origin B");
    let result = runtime_b
        .eval(r#"indexedDB.get("Shared", "store", "k");"#)
        .expect("eval get for B");
    assert_eq!(result, "null");

    let from_a = store.get(ORIGIN_A, "Shared", "store", "k").unwrap();
    assert_eq!(from_a.as_deref(), Some("data-from-a"));
}
