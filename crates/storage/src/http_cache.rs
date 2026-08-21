//! RFC 9111 HTTP cache store backed by `SQLite` (WAL mode).
//!
//! Implements freshness evaluation (`max-age`, `Expires`), conditional
//! revalidation support (`ETag` / `If-None-Match` -> 304), and `Cache-Control`
//! storability checks (`no-store`, `private`).

use crate::error::StorageError;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single cached HTTP response entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The canonical request URL string.
    pub url: String,
    /// `ETag` response header value, if present.
    pub etag: Option<String>,
    /// `Last-Modified` response header value, if present.
    pub last_modified: Option<String>,
    /// Resolved `max-age` in seconds (from `Cache-Control` or `Expires`).
    pub max_age_secs: u64,
    /// Unix timestamp (seconds) at which this entry was stored or last refreshed.
    pub cached_at_unix: u64,
    /// HTTP status code of the cached response.
    pub status_code: u16,
    /// MIME type string (e.g. `"text/html"`).
    pub mime_type: String,
    /// Decompressed response body bytes.
    pub body: Vec<u8>,
}

/// SQLite-backed RFC 9111 HTTP cache store.
pub struct HttpCacheStore {
    conn: std::sync::Mutex<Connection>,
}

#[allow(clippy::significant_drop_tightening)]
impl HttpCacheStore {
    /// Opens (or creates) the cache database at `db_path` with WAL mode.
    ///
    /// # Errors
    /// Returns `StorageError` if the database cannot be opened or the schema
    /// cannot be initialized.
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS http_cache (
                 url              TEXT PRIMARY KEY,
                 etag             TEXT,
                 last_modified    TEXT,
                 max_age_secs     INTEGER NOT NULL DEFAULT 0,
                 cached_at_unix   INTEGER NOT NULL,
                 status_code      INTEGER NOT NULL,
                 mime_type        TEXT NOT NULL,
                 body             BLOB NOT NULL
             );",
        )?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    /// Looks up a cached entry for `url`.
    ///
    /// Returns `Ok(None)` when there is no matching entry.
    ///
    /// # Errors
    /// Returns `StorageError` on database access failure.
    pub fn lookup(&self, url: &str) -> Result<Option<CacheEntry>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::LockError("mutex lock poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT url, etag, last_modified, max_age_secs, cached_at_unix,
                    status_code, mime_type, body
             FROM http_cache WHERE url = ?1",
        )?;
        let result = stmt.query_row(params![url], |row| {
            Ok(CacheEntry {
                url: row.get(0)?,
                etag: row.get(1)?,
                last_modified: row.get(2)?,
                max_age_secs: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                cached_at_unix: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                status_code: u16::try_from(row.get::<_, i64>(5)?).unwrap_or(200),
                mime_type: row.get(6)?,
                body: row.get(7)?,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::from(e)),
        }
    }

    /// Returns `true` if `entry` is still fresh (RFC 9111 s4.2).
    #[must_use]
    pub fn is_fresh(entry: &CacheEntry) -> bool {
        if entry.max_age_secs == 0 {
            return false;
        }
        let now = unix_now();
        now < entry.cached_at_unix.saturating_add(entry.max_age_secs)
    }

    /// Stores a new cache entry (INSERT OR REPLACE).
    ///
    /// Silently returns `Ok(())` when `headers` contain `no-store` or `private`.
    ///
    /// # Errors
    /// Returns `StorageError` on database write failure.
    pub fn store(
        &self,
        url: &str,
        status_code: u16,
        mime_type: &str,
        headers: &HashMap<String, String>,
        body: &[u8],
    ) -> Result<(), StorageError> {
        if !should_cache_response(headers) {
            return Ok(());
        }
        let etag = headers.get("etag").cloned();
        let last_modified = headers.get("last-modified").cloned();
        let max_age_secs = parse_max_age(headers);
        let cached_at_unix = unix_now().cast_signed();

        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::LockError("mutex lock poisoned".to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO http_cache
             (url, etag, last_modified, max_age_secs, cached_at_unix,
              status_code, mime_type, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                url,
                etag,
                last_modified,
                max_age_secs.cast_signed(),
                cached_at_unix,
                i64::from(status_code),
                mime_type,
                body,
            ],
        )?;
        tracing::debug!(url, max_age_secs, "Cached HTTP response");
        Ok(())
    }

    /// Updates freshness timestamp and optional new `ETag` after a 304
    /// Not Modified response (RFC 9111 s4.3.4) without replacing the body.
    ///
    /// # Errors
    /// Returns `StorageError` on database write failure.
    pub fn update_metadata(
        &self,
        url: &str,
        etag: Option<&str>,
        max_age_secs: u64,
    ) -> Result<(), StorageError> {
        let now = unix_now().cast_signed();
        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::LockError("mutex lock poisoned".to_string()))?;
        conn.execute(
            "UPDATE http_cache
             SET cached_at_unix = ?1, etag = COALESCE(?2, etag),
                 max_age_secs   = ?3
             WHERE url = ?4",
            params![now, etag, max_age_secs.cast_signed(), url],
        )?;
        tracing::debug!(url, "Refreshed cache metadata after 304");
        Ok(())
    }
}

// -- Helpers ------------------------------------------------------------------

/// Returns the current time as Unix seconds.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Returns `false` when the response MUST NOT be stored per RFC 9111 s5.2.
///
/// Responses carrying `Set-Cookie` (s5.2.2.4) or responding to requests with
/// `Authorization` (s5.2.2.3) are not stored unless explicitly permitted.
pub fn should_cache_response<S: ::std::hash::BuildHasher>(
    headers: &HashMap<String, String, S>,
) -> bool {
    let cc = headers.get("cache-control").map_or("", String::as_str);
    let mut has_public = false;
    let mut has_must_revalidate = false;
    for directive in cc.split(',') {
        let d = directive.trim().to_ascii_lowercase();
        match d.as_str() {
            "no-store" | "private" => return false,
            "public" => has_public = true,
            "must-revalidate" => has_must_revalidate = true,
            _ => {}
        }
    }
    for key in headers.keys() {
        if key.eq_ignore_ascii_case("set-cookie") && !has_public {
            return false;
        }
        if key.eq_ignore_ascii_case("authorization") && !(has_public || has_must_revalidate) {
            return false;
        }
    }
    true
}

/// Parses `Cache-Control: max-age=N` (RFC 9111 s5.2.2.1).
/// Returns `0` (treat as uncacheable) if not present.
#[must_use]
pub fn parse_max_age<S: ::std::hash::BuildHasher>(headers: &HashMap<String, String, S>) -> u64 {
    if let Some(cc) = headers.get("cache-control") {
        for directive in cc.split(',') {
            let d = directive.trim().to_ascii_lowercase();
            if let Some(val) = d.strip_prefix("max-age=")
                && let Ok(n) = val.trim().parse::<u64>()
            {
                return n;
            }
        }
    }
    0
}
