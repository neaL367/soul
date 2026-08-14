//! Integration tests for SQLite-backed `IndexedDB` persistence and queries.

use storage::{IndexedDbStore, StorageDatabase};

#[test]
fn test_indexeddb_put_get_delete_all() {
    let db = StorageDatabase::open_in_memory().expect("failed to open db");
    let idb = IndexedDbStore::new(db).expect("failed to init idb");

    let version = idb.open_or_create_db("notes_app", 1).unwrap();
    assert_eq!(version, 1);

    // Put records
    idb.put(
        "notes_app",
        "notes",
        "note_1",
        r#"{"title":"Rust","body":"Awesome"}"#,
    )
    .unwrap();
    idb.put(
        "notes_app",
        "notes",
        "note_2",
        r#"{"title":"GPUI","body":"Fast"}"#,
    )
    .unwrap();

    // Get specific record
    let note1 = idb.get("notes_app", "notes", "note_1").unwrap();
    assert!(note1.is_some());
    assert!(note1.unwrap().contains("Awesome"));

    // Get all records in store
    let all = idb.get_all("notes_app", "notes").unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0, "note_1");
    assert_eq!(all[1].0, "note_2");

    // Delete record
    let deleted = idb.delete("notes_app", "notes", "note_1").unwrap();
    assert!(deleted);

    let remaining = idb.get_all("notes_app", "notes").unwrap();
    assert_eq!(remaining.len(), 1);
}
