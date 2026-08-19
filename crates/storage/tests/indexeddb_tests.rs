//! Integration tests for SQLite-backed `IndexedDB` persistence, queries, and origin partitioning.

use storage::{IndexedDbStore, StorageDatabase};

const ORIGIN_A: &str = "https://app-a.example";
const ORIGIN_B: &str = "https://app-b.example";

#[test]
fn test_indexeddb_put_get_delete_all() {
    let db = StorageDatabase::open_in_memory().expect("failed to open db");
    let idb = IndexedDbStore::new(db).expect("failed to init idb");

    let version = idb.open_or_create_db(ORIGIN_A, "notes_app", 1).unwrap();
    assert_eq!(version, 1);

    // Put records
    idb.put(
        ORIGIN_A,
        "notes_app",
        "notes",
        "note_1",
        r#"{"title":"Rust","body":"Awesome"}"#,
    )
    .unwrap();
    idb.put(
        ORIGIN_A,
        "notes_app",
        "notes",
        "note_2",
        r#"{"title":"GPUI","body":"Fast"}"#,
    )
    .unwrap();

    // Get specific record
    let note1 = idb.get(ORIGIN_A, "notes_app", "notes", "note_1").unwrap();
    assert!(note1.is_some());
    assert!(note1.unwrap().contains("Awesome"));

    // Get all records in store
    let all = idb.get_all(ORIGIN_A, "notes_app", "notes").unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0, "note_1");
    assert_eq!(all[1].0, "note_2");

    // Delete record
    let deleted = idb
        .delete(ORIGIN_A, "notes_app", "notes", "note_1")
        .unwrap();
    assert!(deleted);

    let remaining = idb.get_all(ORIGIN_A, "notes_app", "notes").unwrap();
    assert_eq!(remaining.len(), 1);
}

#[test]
fn test_indexeddb_records_are_partitioned_by_origin() {
    let db = StorageDatabase::open_in_memory().expect("failed to open db");
    let idb = IndexedDbStore::new(db).expect("failed to init idb");

    idb.open_or_create_db(ORIGIN_A, "shared_db", 1).unwrap();
    idb.open_or_create_db(ORIGIN_B, "shared_db", 1).unwrap();

    idb.put(ORIGIN_A, "shared_db", "store", "k", "value-from-a")
        .unwrap();

    // Same database name from a different origin must not see A's records.
    let from_b = idb.get(ORIGIN_B, "shared_db", "store", "k").unwrap();
    assert!(from_b.is_none());
    assert!(
        idb.get_all(ORIGIN_B, "shared_db", "store")
            .unwrap()
            .is_empty()
    );

    // Each origin's database version is tracked independently.
    assert_eq!(idb.open_or_create_db(ORIGIN_B, "shared_db", 2).unwrap(), 2);
    assert_eq!(idb.open_or_create_db(ORIGIN_A, "shared_db", 1).unwrap(), 1);

    // A's record is still intact after B's writes.
    let from_a = idb.get(ORIGIN_A, "shared_db", "store", "k").unwrap();
    assert_eq!(from_a.as_deref(), Some("value-from-a"));
}
