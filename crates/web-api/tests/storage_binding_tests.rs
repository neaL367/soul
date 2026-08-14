//! Integration tests for JavaScript Web Storage (`localStorage`) bindings.

use javascript::JsRuntime;
use std::sync::Arc;
use storage::{LocalStorage, StorageDatabase};
use web_api::register_storage;

#[test]
fn test_js_local_storage_crud() {
    let mut runtime = JsRuntime::new();
    let temp_db = std::env::temp_dir().join(format!("soul_js_storage_{}.db", std::process::id()));
    let db = StorageDatabase::open(&temp_db).expect("Open SQLite failed");
    let local_storage = Arc::new(LocalStorage::new(db));

    register_storage(
        &mut runtime.context,
        local_storage.clone(),
        "https://example.com",
    )
    .expect("Register storage failed");

    // Execute script writing to localStorage
    let script = r#"
        localStorage.setItem("user_theme", "dark_mode");
        localStorage.getItem("user_theme");
    "#;

    let res = runtime.eval(script).expect("Eval failed");
    assert_eq!(res, "\"dark_mode\"");

    // Verify written to backing SQLite directly
    let db_val = local_storage
        .get_item("https://example.com", "user_theme")
        .unwrap();
    assert_eq!(db_val, Some("dark_mode".to_string()));

    let _ = std::fs::remove_file(temp_db);
}
