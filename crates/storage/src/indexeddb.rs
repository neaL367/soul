//! SQLite-backed `IndexedDB` database and object store persistence engine.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use rusqlite::params;

/// SQLite-backed persistent `IndexedDB` engine for web applications.
#[derive(Debug, Clone)]
pub struct IndexedDbStore {
    db: StorageDatabase,
}

impl IndexedDbStore {
    /// Creates a new `IndexedDbStore` and ensures table schemas exist.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if table creation fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn new(db: StorageDatabase) -> Result<Self, StorageError> {
        {
            let conn = db.conn()?;
            conn.execute_batch(
                r"
                CREATE TABLE IF NOT EXISTS idb_databases (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    version INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS idb_records (
                    db_name TEXT NOT NULL,
                    store_name TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    PRIMARY KEY (db_name, store_name, key)
                );
                CREATE INDEX IF NOT EXISTS idx_idb_lookup ON idb_records(db_name, store_name);
                ",
            )?;
        }
        Ok(Self { db })
    }

    /// Creates or upgrades an `IndexedDB` database version.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn open_or_create_db(&self, name: &str, version: u32) -> Result<u32, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt =
            conn.prepare_cached("SELECT version FROM idb_databases WHERE name = ?1 LIMIT 1")?;
        let existing: Option<u32> = stmt.query_row(params![name], |row| row.get(0)).ok();

        if let Some(cur_ver) = existing {
            if version > cur_ver {
                conn.execute(
                    "UPDATE idb_databases SET version = ?1 WHERE name = ?2",
                    params![version, name],
                )?;
                Ok(version)
            } else {
                Ok(cur_ver)
            }
        } else {
            conn.execute(
                "INSERT INTO idb_databases (name, version) VALUES (?1, ?2)",
                params![name, version],
            )?;
            Ok(version)
        }
    }

    /// Inserts or updates an object record in an object store.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn put(
        &self,
        db_name: &str,
        store_name: &str,
        key: &str,
        value: &str,
    ) -> Result<(), StorageError> {
        let conn = self.db.conn()?;
        conn.execute(
            r"
            INSERT INTO idb_records (db_name, store_name, key, value)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(db_name, store_name, key) DO UPDATE SET value = excluded.value
            ",
            params![db_name, store_name, key, value],
        )?;
        Ok(())
    }

    /// Retrieves an object record from an object store by key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get(
        &self,
        db_name: &str,
        store_name: &str,
        key: &str,
    ) -> Result<Option<String>, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT value FROM idb_records WHERE db_name = ?1 AND store_name = ?2 AND key = ?3 LIMIT 1",
        )?;
        let result = stmt
            .query_row(params![db_name, store_name, key], |row| row.get(0))
            .ok();
        Ok(result)
    }

    /// Deletes an object record by key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn delete(&self, db_name: &str, store_name: &str, key: &str) -> Result<bool, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute(
            "DELETE FROM idb_records WHERE db_name = ?1 AND store_name = ?2 AND key = ?3",
            params![db_name, store_name, key],
        )?;
        Ok(count > 0)
    }

    /// Returns all records inside an object store.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get_all(
        &self,
        db_name: &str,
        store_name: &str,
    ) -> Result<Vec<(String, String)>, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT key, value FROM idb_records WHERE db_name = ?1 AND store_name = ?2 ORDER BY key ASC",
        )?;
        let rows = stmt.query_map(params![db_name, store_name], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }
}
