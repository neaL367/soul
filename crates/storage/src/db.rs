//! `SQLite` connection management, WAL configuration, and schema migrations.

use crate::error::StorageError;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Thread-safe `SQLite` database manager for browser storage.
#[derive(Debug, Clone)]
pub struct StorageDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl StorageDatabase {
    /// Opens an in-memory `SQLite` database instance (useful for tests and private browsing).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if in-memory database creation or schema migrations fail.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_pragmas(false)?;
        db.run_migrations()?;
        Ok(db)
    }

    /// Opens an on-disk `SQLite` database file with WAL mode and synchronous pragmas.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if file creation, pragma configuration, or migrations fail.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_pragmas(true)?;
        db.run_migrations()?;
        Ok(db)
    }

    /// Returns a locked RAII guard for executing raw queries against the `SQLite` connection.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::LockError` if the mutex lock is poisoned.
    pub fn conn(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))
    }

    #[allow(clippy::significant_drop_tightening)]
    fn init_pragmas(&self, on_disk: bool) -> Result<(), StorageError> {
        let conn = self.conn()?;
        if on_disk {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    fn run_migrations(&self) -> Result<(), StorageError> {
        let conn = self.conn()?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version < 1 {
            conn.execute_batch(SCHEMA_V1)?;
            // v1 databases created before the `host_only` column existed need
            // it added in place; fresh databases already include it.
            let has_host_only: bool = {
                let mut stmt = conn.prepare_cached(
                    "SELECT COUNT(*) FROM pragma_table_info('cookies') WHERE name = 'host_only'",
                )?;
                stmt.query_row([], |row| row.get(0))?
            };
            if !has_host_only {
                conn.execute_batch(
                    "ALTER TABLE cookies ADD COLUMN host_only INTEGER NOT NULL DEFAULT 0;",
                )?;
            }
            conn.pragma_update(None, "user_version", 1)?;
        }

        if version < 2 {
            // IndexedDB tables are rebuilt with origin partitioning so
            // different sites can never observe each other's databases.
            conn.execute_batch(SCHEMA_V2)?;
            conn.pragma_update(None, "user_version", 2)?;
        }

        Ok(())
    }
}

/// Schema version 1: core tables. `cookies` includes `host_only` so cookies
/// set without a `Domain` attribute are never shared with sibling hosts.
const SCHEMA_V1: &str = r"
    CREATE TABLE IF NOT EXISTS history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        url TEXT NOT NULL,
        title TEXT,
        visit_count INTEGER NOT NULL DEFAULT 1,
        last_visited_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);
    CREATE INDEX IF NOT EXISTS idx_history_last_visited ON history(last_visited_at);

    CREATE TABLE IF NOT EXISTS bookmarks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        folder TEXT,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder);

    CREATE TABLE IF NOT EXISTS cookies (
        name TEXT NOT NULL,
        domain TEXT NOT NULL,
        path TEXT NOT NULL,
        value TEXT NOT NULL,
        expires_at INTEGER,
        is_secure INTEGER NOT NULL DEFAULT 0,
        is_http_only INTEGER NOT NULL DEFAULT 0,
        same_site TEXT NOT NULL DEFAULT 'Lax',
        host_only INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (name, domain, path)
    );
    CREATE INDEX IF NOT EXISTS idx_cookies_domain ON cookies(domain);

    CREATE TABLE IF NOT EXISTS local_storage (
        origin TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        PRIMARY KEY (origin, key)
    );
";

/// Schema version 2: `IndexedDB` tables keyed by origin so databases of
/// different sites are fully partitioned.
const SCHEMA_V2: &str = r"
    DROP TABLE IF EXISTS idb_records;
    DROP TABLE IF EXISTS idb_databases;

    CREATE TABLE idb_databases (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        origin TEXT NOT NULL,
        name TEXT NOT NULL,
        version INTEGER NOT NULL,
        UNIQUE (origin, name)
    );

    CREATE TABLE idb_records (
        origin TEXT NOT NULL,
        db_name TEXT NOT NULL,
        store_name TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        PRIMARY KEY (origin, db_name, store_name, key)
    );
    CREATE INDEX IF NOT EXISTS idx_idb_lookup ON idb_records(origin, db_name, store_name);
";

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::significant_drop_tightening)]
    #[test]
    fn migration_brings_v0_databases_to_latest_schema() {
        let dir = std::env::temp_dir().join("soul_migration_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v0.db");
        let _ = std::fs::remove_file(&path);

        // Simulate a v0 database written by the pre-migration code: cookies
        // without host_only, idb tables without origin, user_version = 0.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r"
                CREATE TABLE cookies (
                    name TEXT NOT NULL,
                    domain TEXT NOT NULL,
                    path TEXT NOT NULL,
                    value TEXT NOT NULL,
                    expires_at INTEGER,
                    is_secure INTEGER NOT NULL DEFAULT 0,
                    is_http_only INTEGER NOT NULL DEFAULT 0,
                    same_site TEXT NOT NULL DEFAULT 'Lax',
                    PRIMARY KEY (name, domain, path)
                );
                CREATE TABLE idb_records (
                    db_name TEXT NOT NULL,
                    store_name TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    PRIMARY KEY (db_name, store_name, key)
                );
                INSERT INTO cookies (name, domain, path, value)
                VALUES ('legacy', 'example.com', '/', 'v');
                ",
            )
            .unwrap();
        }

        let db = StorageDatabase::open(&path).unwrap();
        let conn = db.conn().unwrap();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);

        let has_host_only: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('cookies') WHERE name = 'host_only'")
            .unwrap()
            .query_row([], |row| row.get(0))
            .unwrap();
        assert!(has_host_only, "host_only column must exist after migration");

        let has_origin: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('idb_records') WHERE name = 'origin'")
            .unwrap()
            .query_row([], |row| row.get(0))
            .unwrap();
        assert!(has_origin, "origin column must exist after migration");

        // The legacy row survived and defaults to a domain cookie, so the
        // pre-migration behavior (subdomain delivery) still applies to it.
        drop(conn);
        let jar = crate::CookieJar::new(db);
        let cookies = jar
            .get_cookies_for_url("https://sub.example.com/", 100)
            .unwrap();
        assert_eq!(cookies.len(), 1);
        assert!(!cookies[0].host_only);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
