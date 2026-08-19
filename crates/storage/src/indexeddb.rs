//! SQLite-backed `IndexedDB` database and object store persistence engine.
//!
//! Every record is keyed by the originating site (the page origin), so
//! databases with the same name from different sites never share data.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use rusqlite::params;

/// SQLite-backed persistent `IndexedDB` engine for web applications.
#[derive(Debug, Clone)]
pub struct IndexedDbStore {
    db: StorageDatabase,
}

impl IndexedDbStore {
    /// Creates a new `IndexedDbStore`. The table schemas are owned by the
    /// database migration ladder (`StorageDatabase`), not by this type.
    ///
    /// # Errors
    ///
    /// This constructor cannot fail; the `Result` shape is kept for API
    /// stability and future validation.
    #[allow(clippy::significant_drop_tightening)]
    #[allow(clippy::significant_drop_tightening)]
    pub const fn new(db: StorageDatabase) -> Result<Self, StorageError> {
        Ok(Self { db })
    }

    /// Creates or upgrades an `IndexedDB` database version for `origin`.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn open_or_create_db(
        &self,
        origin: &str,
        name: &str,
        version: u32,
    ) -> Result<u32, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT version FROM idb_databases WHERE origin = ?1 AND name = ?2 LIMIT 1",
        )?;
        let existing: Option<u32> = stmt.query_row(params![origin, name], |row| row.get(0)).ok();

        if let Some(cur_ver) = existing {
            if version > cur_ver {
                conn.execute(
                    "UPDATE idb_databases SET version = ?1 WHERE origin = ?2 AND name = ?3",
                    params![version, origin, name],
                )?;
                Ok(version)
            } else {
                Ok(cur_ver)
            }
        } else {
            conn.execute(
                "INSERT INTO idb_databases (origin, name, version) VALUES (?1, ?2, ?3)",
                params![origin, name, version],
            )?;
            Ok(version)
        }
    }

    /// Inserts or updates an object record in an object store for `origin`.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn put(
        &self,
        origin: &str,
        db_name: &str,
        store_name: &str,
        key: &str,
        value: &str,
    ) -> Result<(), StorageError> {
        let conn = self.db.conn()?;
        conn.execute(
            r"
            INSERT INTO idb_records (origin, db_name, store_name, key, value)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(origin, db_name, store_name, key) DO UPDATE SET value = excluded.value
            ",
            params![origin, db_name, store_name, key, value],
        )?;
        Ok(())
    }

    /// Retrieves an object record from an object store for `origin` by key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get(
        &self,
        origin: &str,
        db_name: &str,
        store_name: &str,
        key: &str,
    ) -> Result<Option<String>, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT value FROM idb_records WHERE origin = ?1 AND db_name = ?2 AND store_name = ?3 AND key = ?4 LIMIT 1",
        )?;
        let result = stmt
            .query_row(params![origin, db_name, store_name, key], |row| row.get(0))
            .ok();
        Ok(result)
    }

    /// Deletes an object record for `origin` by key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn delete(
        &self,
        origin: &str,
        db_name: &str,
        store_name: &str,
        key: &str,
    ) -> Result<bool, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute(
            "DELETE FROM idb_records WHERE origin = ?1 AND db_name = ?2 AND store_name = ?3 AND key = ?4",
            params![origin, db_name, store_name, key],
        )?;
        Ok(count > 0)
    }

    /// Returns all records inside an object store for `origin`.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get_all(
        &self,
        origin: &str,
        db_name: &str,
        store_name: &str,
    ) -> Result<Vec<(String, String)>, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT key, value FROM idb_records WHERE origin = ?1 AND db_name = ?2 AND store_name = ?3 ORDER BY key ASC",
        )?;
        let rows = stmt.query_map(params![origin, db_name, store_name], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }
}
