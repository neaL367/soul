//! Cookie jar storage and RFC 6265bis domain/path matching.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use rusqlite::params;
use url::Url;

/// Representation of an HTTP cookie record with RFC 6265bis metadata attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// Cookie name key.
    pub name: String,
    /// Domain scope (e.g. `example.com`).
    pub domain: String,
    /// Path scope (e.g. `/` or `/api`).
    pub path: String,
    /// Cookie payload value.
    pub value: String,
    /// Optional expiration timestamp in Unix seconds.
    pub expires_at: Option<i64>,
    /// Secure flag restricting cookie to HTTPS.
    pub is_secure: bool,
    /// `HttpOnly` flag restricting cookie access from client-side scripts.
    pub is_http_only: bool,
    /// `SameSite` attribute (`Lax`, `Strict`, `None`).
    pub same_site: String,
}

/// Persistent and RFC 6265bis-compliant Cookie Jar.
#[derive(Debug, Clone)]
pub struct CookieJar {
    db: StorageDatabase,
}

impl CookieJar {
    /// Creates a new `CookieJar` using the provided database connection.
    #[must_use]
    pub const fn new(db: StorageDatabase) -> Self {
        Self { db }
    }

    /// Inserts or updates a cookie in the persistent store.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if `SQLite` query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn set_cookie(&self, cookie: &Cookie) -> Result<(), StorageError> {
        let conn = self.db.conn()?;
        conn.execute(
            r"
            INSERT INTO cookies (name, domain, path, value, expires_at, is_secure, is_http_only, same_site)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(name, domain, path) DO UPDATE SET
                value = excluded.value,
                expires_at = excluded.expires_at,
                is_secure = excluded.is_secure,
                is_http_only = excluded.is_http_only,
                same_site = excluded.same_site
            ",
            params![
                cookie.name,
                cookie.domain.to_ascii_lowercase(),
                cookie.path,
                cookie.value,
                cookie.expires_at,
                i32::from(cookie.is_secure),
                i32::from(cookie.is_http_only),
                cookie.same_site,
            ],
        )?;
        Ok(())
    }

    /// Retrieves all active cookies matching the given URL per RFC 6265bis.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if URL parsing or database querying fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get_cookies_for_url(
        &self,
        url_str: &str,
        current_time: i64,
    ) -> Result<Vec<Cookie>, StorageError> {
        let parsed_url =
            Url::parse(url_str).map_err(|e| StorageError::InvalidUrl(e.to_string()))?;

        let host = parsed_url
            .host_str()
            .ok_or_else(|| StorageError::InvalidUrl("Missing host in URL".to_string()))?
            .to_ascii_lowercase();

        let req_path = parsed_url.path();
        let is_https = parsed_url.scheme() == "https";

        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            r"
            SELECT name, domain, path, value, expires_at, is_secure, is_http_only, same_site
            FROM cookies
            WHERE (expires_at IS NULL OR expires_at > ?1)
            ",
        )?;

        let rows = stmt.query_map(params![current_time], |row| {
            let sec_int: i32 = row.get(5)?;
            let http_int: i32 = row.get(6)?;
            Ok(Cookie {
                name: row.get(0)?,
                domain: row.get(1)?,
                path: row.get(2)?,
                value: row.get(3)?,
                expires_at: row.get(4)?,
                is_secure: sec_int != 0,
                is_http_only: http_int != 0,
                same_site: row.get(7)?,
            })
        })?;

        let mut matched = Vec::new();
        for r in rows {
            let cookie = r?;
            if cookie.is_secure && !is_https {
                continue;
            }
            if !Self::domain_matches(&host, &cookie.domain) {
                continue;
            }
            if !Self::path_matches(req_path, &cookie.path) {
                continue;
            }
            matched.push(cookie);
        }

        Ok(matched)
    }

    /// Removes expired cookies from the persistent store.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn clear_expired(&self, current_time: i64) -> Result<usize, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute(
            "DELETE FROM cookies WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            params![current_time],
        )?;
        Ok(count)
    }

    fn domain_matches(host: &str, domain: &str) -> bool {
        let d = domain.trim_start_matches('.').to_ascii_lowercase();
        let h = host.to_ascii_lowercase();

        if h == d {
            return true;
        }

        if h.ends_with(&format!(".{d}")) {
            return true;
        }

        false
    }

    fn path_matches(request_path: &str, cookie_path: &str) -> bool {
        if request_path == cookie_path {
            return true;
        }

        if let Some(stripped) = request_path.strip_prefix(cookie_path)
            && (cookie_path.ends_with('/') || stripped.starts_with('/'))
        {
            return true;
        }

        false
    }
}
