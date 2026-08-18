//! Integration tests for Windows DPAPI-encrypted storage vault.

use storage::{DpapiVault, StorageDatabase};

#[test]
fn test_dpapi_vault_crud_roundtrip() {
    let db = StorageDatabase::open_in_memory().expect("in-memory db");
    let vault = DpapiVault::new(db).expect("create dpapi vault");

    // Store secret
    vault
        .store_secret("example.com", "session_token", "super_secret_token_12345")
        .expect("store secret");

    // Retrieve secret
    let retrieved = vault
        .get_secret("example.com", "session_token")
        .expect("get secret");
    assert_eq!(retrieved.as_deref(), Some("super_secret_token_12345"));

    // Non-existent key
    let missing = vault
        .get_secret("example.com", "missing_key")
        .expect("query missing");
    assert_eq!(missing, None);

    // Delete secret
    let deleted = vault
        .delete_secret("example.com", "session_token")
        .expect("delete secret");
    assert!(deleted);

    let after_delete = vault
        .get_secret("example.com", "session_token")
        .expect("query deleted");
    assert_eq!(after_delete, None);
}
