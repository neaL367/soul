//! Integration tests for JavaScript Web Storage (`localStorage` and `sessionStorage`) bindings.

use javascript::JsRuntime;
use std::sync::Arc;
use storage::{LocalStorage, SessionStorage, StorageDatabase};
use web_api::{register_local_storage, register_session_storage};

#[test]
fn test_js_local_storage_crud() {
    let mut runtime = JsRuntime::new();
    let temp_db = std::env::temp_dir().join(format!("soul_js_storage_{}.db", std::process::id()));
    let db = StorageDatabase::open(&temp_db).expect("Open SQLite failed");
    let local_storage = Arc::new(LocalStorage::new(db));

    register_local_storage(
        &mut runtime.context,
        local_storage.clone(),
        "https://example.com",
    )
    .expect("Register storage failed");

    let script = r#"
        localStorage.setItem("user_theme", "dark_mode");
        localStorage.getItem("user_theme");
    "#;

    let res = runtime.eval(script).expect("Eval failed");
    assert_eq!(res, "\"dark_mode\"");

    let db_val = local_storage
        .get_item("https://example.com", "user_theme")
        .unwrap();
    assert_eq!(db_val, Some("dark_mode".to_string()));

    let _ = std::fs::remove_file(temp_db);
}

#[test]
fn test_js_session_storage_crud() {
    let mut runtime = JsRuntime::new();
    let session_storage = Arc::new(SessionStorage::new());

    register_session_storage(
        &mut runtime.context,
        session_storage.clone(),
        "https://example.com",
    )
    .expect("Register session storage failed");

    let script = r#"
        sessionStorage.setItem("session_token", "xyz-987");
        sessionStorage.getItem("session_token");
    "#;

    let res = runtime.eval(script).expect("Eval failed");
    assert_eq!(res, "\"xyz-987\"");

    assert_eq!(
        session_storage.get_item("https://example.com", "session_token"),
        Some("xyz-987".to_string())
    );
}
