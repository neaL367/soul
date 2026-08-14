//! Web Storage implementations: persistent `LocalStorage` and in-memory `SessionStorage`.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use rusqlite::params;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Persistent, origin-scoped key-value storage backed by `SQLite`.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    db: StorageDatabase,
}

impl LocalStorage {
    /// Creates a new `LocalStorage` manager with the given database connection.
    #[must_use]
    pub const fn new(db: StorageDatabase) -> Self {
        Self { db }
    }

    /// Retrieves an item value for a specific web origin and key.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get_item(&self, origin: &str, key: &str) -> Result<Option<String>, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT value FROM local_storage WHERE origin = ?1 AND key = ?2 LIMIT 1",
        )?;
        let result = stmt.query_row(params![origin, key], |row| row.get(0)).ok();
        Ok(result)
    }

    /// Inserts or updates an origin-scoped key-value pair.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn set_item(&self, origin: &str, key: &str, value: &str) -> Result<(), StorageError> {
        let conn = self.db.conn()?;
        conn.execute(
            r"
            INSERT INTO local_storage (origin, key, value)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(origin, key) DO UPDATE SET value = excluded.value
            ",
            params![origin, key, value],
        )?;
        Ok(())
    }

    /// Removes an item for the specified origin and key.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn remove_item(&self, origin: &str, key: &str) -> Result<bool, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute(
            "DELETE FROM local_storage WHERE origin = ?1 AND key = ?2",
            params![origin, key],
        )?;
        Ok(count > 0)
    }

    /// Clears all keys for a specific origin.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn clear_origin(&self, origin: &str) -> Result<(), StorageError> {
        let conn = self.db.conn()?;
        conn.execute(
            "DELETE FROM local_storage WHERE origin = ?1",
            params![origin],
        )?;
        Ok(())
    }

    /// Returns the number of items stored for an origin.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query fails.
    #[allow(
        clippy::significant_drop_tightening,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn len(&self, origin: &str) -> Result<usize, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt =
            conn.prepare_cached("SELECT COUNT(*) FROM local_storage WHERE origin = ?1")?;
        let count: i64 = stmt.query_row(params![origin], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Returns `true` if there are no items stored for the origin.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query fails.
    pub fn is_empty(&self, origin: &str) -> Result<bool, StorageError> {
        Ok(self.len(origin)? == 0)
    }
}

/// In-memory, per-tab session storage (strictly never persisted to disk per ADR-8).
#[derive(Debug, Clone, Default)]
pub struct SessionStorage {
    data: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
}

impl SessionStorage {
    /// Creates a new empty in-memory `SessionStorage`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Retrieves an item for the specified origin and key.
    #[must_use]
    pub fn get_item(&self, origin: &str, key: &str) -> Option<String> {
        let guard = self.data.lock().ok()?;
        guard.get(origin).and_then(|map| map.get(key).cloned())
    }

    /// Inserts or updates an item in session storage.
    pub fn set_item(&self, origin: &str, key: &str, value: &str) {
        if let Ok(mut guard) = self.data.lock() {
            let map = guard.entry(origin.to_string()).or_default();
            map.insert(key.to_string(), value.to_string());
        }
    }

    /// Removes an item from session storage.
    #[must_use]
    pub fn remove_item(&self, origin: &str, key: &str) -> bool {
        if let Ok(mut guard) = self.data.lock()
            && let Some(map) = guard.get_mut(origin)
        {
            return map.remove(key).is_some();
        }
        false
    }

    /// Clears all keys for the specified origin.
    pub fn clear_origin(&self, origin: &str) {
        if let Ok(mut guard) = self.data.lock() {
            guard.remove(origin);
        }
    }

    /// Returns the number of items stored for an origin.
    #[must_use]
    pub fn len(&self, origin: &str) -> usize {
        self.data
            .lock()
            .ok()
            .and_then(|g| g.get(origin).map(HashMap::len))
            .unwrap_or(0)
    }

    /// Returns `true` if there are no items stored for the origin.
    #[must_use]
    pub fn is_empty(&self, origin: &str) -> bool {
        self.len(origin) == 0
    }
}
