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
        conn.execute_batch(
            r"
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
                PRIMARY KEY (name, domain, path)
            );
            CREATE INDEX IF NOT EXISTS idx_cookies_domain ON cookies(domain);

            CREATE TABLE IF NOT EXISTS local_storage (
                origin TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY (origin, key)
            );
            ",
        )?;
        Ok(())
    }
}
