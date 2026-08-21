//! RFC 6797 HTTP Strict Transport Security (HSTS) persistent policy store.

#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::significant_drop_tightening
)]

use crate::error::StorageError;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Persistent `SQLite` store tracking RFC 6797 HSTS security policies.
pub struct HstsStore {
    conn: Mutex<Connection>,
}

impl HstsStore {
    /// Opens or creates an HSTS `SQLite` database at the specified path.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if `SQLite` connection or table schema creation fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hsts_policies (
                host TEXT PRIMARY KEY,
                max_age INTEGER NOT NULL,
                include_subdomains BOOLEAN NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_hsts_host ON hsts_policies(host);",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Creates an in-memory HSTS store, useful for tests and ephemeral profiles.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` on `SQLite` initialization failure.
    pub fn in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hsts_policies (
                host TEXT PRIMARY KEY,
                max_age INTEGER NOT NULL,
                include_subdomains BOOLEAN NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_hsts_host ON hsts_policies(host);",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Records or updates an HSTS policy for a given host.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` on `SQLite` query failure.
    pub fn record_hsts(
        &self,
        host: &str,
        max_age_secs: u64,
        include_subdomains: bool,
    ) -> Result<(), StorageError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let host_lower = host.trim().to_ascii_lowercase();

        let lock = self.conn.lock().unwrap();
        if max_age_secs == 0 {
            // max-age=0 explicitly removes the HSTS pin
            lock.execute(
                "DELETE FROM hsts_policies WHERE host = ?1",
                params![host_lower],
            )?;
        } else {
            lock.execute(
                "INSERT INTO hsts_policies (host, max_age, include_subdomains, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(host) DO UPDATE SET
                     max_age = excluded.max_age,
                     include_subdomains = excluded.include_subdomains,
                     created_at = excluded.created_at",
                params![
                    host_lower,
                    max_age_secs as i64,
                    include_subdomains,
                    now as i64
                ],
            )?;
        }

        Ok(())
    }

    /// Checks whether an HTTP connection to `host` must be upgraded to HTTPS per HSTS rules.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` on database failure.
    pub fn is_hsts_enforced(&self, host: &str) -> Result<bool, StorageError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let host_lower = host.trim().to_ascii_lowercase();

        let lock = self.conn.lock().unwrap();

        // 1. Direct host check
        let mut stmt =
            lock.prepare("SELECT max_age, created_at FROM hsts_policies WHERE host = ?1")?;
        let mut rows = stmt.query(params![host_lower])?;
        if let Some(row) = rows.next()? {
            let max_age: i64 = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            if (created_at + max_age) as u64 > now {
                return Ok(true);
            }
        }

        // 2. Parent domains check with includeSubDomains
        let mut domain_parts: Vec<&str> = host_lower.split('.').collect();
        while domain_parts.len() > 1 {
            domain_parts.remove(0);
            let parent_domain = domain_parts.join(".");
            let mut parent_stmt = lock.prepare(
                "SELECT max_age, created_at, include_subdomains FROM hsts_policies WHERE host = ?1",
            )?;
            let mut parent_rows = parent_stmt.query(params![parent_domain])?;
            if let Some(row) = parent_rows.next()? {
                let max_age: i64 = row.get(0)?;
                let created_at: i64 = row.get(1)?;
                let include_subdomains: bool = row.get(2)?;
                if include_subdomains && (created_at + max_age) as u64 > now {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Parses an RFC 6797 `Strict-Transport-Security` header value.
    #[must_use]
    pub fn parse_hsts_header(header_val: &str) -> Option<(u64, bool)> {
        let mut max_age = None;
        let mut include_subdomains = false;

        for directive in header_val.split(';') {
            let trimmed = directive.trim();
            if let Some(stripped) = trimmed.strip_prefix("max-age=") {
                let num_str = stripped.trim_matches('"');
                if let Ok(secs) = num_str.parse::<u64>() {
                    max_age = Some(secs);
                }
            } else if trimmed.eq_ignore_ascii_case("includesubdomains") {
                include_subdomains = true;
            }
        }

        max_age.map(|age| (age, include_subdomains))
    }
}
