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
    /// True when the cookie was set without a `Domain` attribute; such cookies
    /// are only ever sent to the exact host that set them.
    pub host_only: bool,
}

impl Cookie {
    /// Parses a `Set-Cookie` header value string into a `Cookie` scoped to `request_url`.
    #[must_use]
    pub fn parse(header_val: &str, request_url: &Url) -> Option<Self> {
        let mut parts = header_val.split(';');
        let first = parts.next()?.trim();
        let (name, value) = first.split_once('=')?;
        if name.is_empty() {
            return None;
        }

        let default_domain = request_url.host_str().unwrap_or("").to_ascii_lowercase();
        let mut domain_attr: Option<String> = None;
        let mut path = "/".to_string();
        let mut expires_at = None;
        let mut is_secure = false;
        let mut is_http_only = false;
        let mut same_site = "Lax".to_string();

        for part in parts {
            let part = part.trim();
            if part.eq_ignore_ascii_case("secure") {
                is_secure = true;
            } else if part.eq_ignore_ascii_case("httponly") {
                is_http_only = true;
            } else if let Some((attr_name, attr_val)) = part.split_once('=') {
                let attr_name = attr_name.trim();
                let attr_val = attr_val.trim();
                if attr_name.eq_ignore_ascii_case("domain") && !attr_val.is_empty() {
                    domain_attr = Some(attr_val.trim_start_matches('.').to_ascii_lowercase());
                } else if attr_name.eq_ignore_ascii_case("path") && !attr_val.is_empty() {
                    path = attr_val.to_string();
                } else if attr_name.eq_ignore_ascii_case("samesite") && !attr_val.is_empty() {
                    same_site = attr_val.to_string();
                } else if attr_name.eq_ignore_ascii_case("max-age")
                    && let Ok(delta) = attr_val.parse::<i64>()
                {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let now_i64 = i64::try_from(now).unwrap_or(i64::MAX);
                    expires_at = Some(now_i64.saturating_add(delta));
                }
            }
        }

        // RFC 6265bis §5.3 steps 3-5: a `Domain` attribute is only honored when
        // it is a suffix of the request host, is not itself an IP address, and
        // is not a public suffix (e.g. `com` or `co.uk`, which would otherwise
        // let an attacker poison cookies across all subdomains). Any other
        // value means the whole cookie must be ignored, never silently downgraded
        // to host-only.
        let (domain, host_only) = match domain_attr {
            Some(attr_domain) => {
                let host_is_ip = request_url
                    .host_str()
                    .is_some_and(|h| h.parse::<std::net::IpAddr>().is_ok());
                let domain_is_ip = attr_domain.parse::<std::net::IpAddr>().is_ok();
                if host_is_ip
                    || domain_is_ip
                    || Self::is_public_suffix(&attr_domain)
                    || !Self::is_domain_suffix_of(&default_domain, &attr_domain)
                {
                    return None;
                }
                (attr_domain, false)
            }
            None => (default_domain, true),
        };

        // RFC 6265bis §5.4: Cookies with SameSite=None must specify the Secure attribute.
        if same_site.eq_ignore_ascii_case("None") && !is_secure {
            return None;
        }

        Some(Self {
            name: name.trim().to_string(),
            domain,
            path,
            value: value.trim().to_string(),
            expires_at,
            is_secure,
            is_http_only,
            same_site,
            host_only,
        })
    }

    /// Returns true when `domain` is a public suffix (e.g. `com`, `co.uk`, `org.uk`)
    /// under which cookies must not be set across all sites.
    fn is_public_suffix(domain: &str) -> bool {
        let lower = domain.trim_start_matches('.').to_ascii_lowercase();
        if !lower.contains('.') {
            return true;
        }
        matches!(
            lower.as_str(),
            "co.uk"
                | "ac.uk"
                | "gov.uk"
                | "org.uk"
                | "net.uk"
                | "com.au"
                | "net.au"
                | "org.au"
                | "edu.au"
                | "gov.au"
                | "co.jp"
                | "ne.jp"
                | "or.jp"
                | "ac.jp"
                | "go.jp"
                | "co.nz"
                | "net.nz"
                | "org.nz"
                | "com.br"
                | "net.br"
                | "org.br"
                | "com.sg"
                | "edu.sg"
                | "gov.sg"
                | "com.tw"
                | "org.tw"
                | "edu.tw"
                | "co.in"
                | "net.in"
                | "org.in"
                | "gov.in"
                | "co.za"
                | "org.za"
                | "gov.za"
                | "co.kr"
                | "ne.kr"
                | "or.kr"
                | "go.kr"
        )
    }

    /// Returns true when `host` is `domain` itself or a subdomain of it.
    fn is_domain_suffix_of(host: &str, domain: &str) -> bool {
        host == domain || host.ends_with(&format!(".{domain}"))
    }
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
            INSERT INTO cookies (name, domain, path, value, expires_at, is_secure, is_http_only, same_site, host_only)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(name, domain, path) DO UPDATE SET
                value = excluded.value,
                expires_at = excluded.expires_at,
                is_secure = excluded.is_secure,
                is_http_only = excluded.is_http_only,
                same_site = excluded.same_site,
                host_only = excluded.host_only
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
                i32::from(cookie.host_only),
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
            SELECT name, domain, path, value, expires_at, is_secure, is_http_only, same_site, host_only
            FROM cookies
            WHERE (expires_at IS NULL OR expires_at > ?1)
            ",
        )?;

        let rows = stmt.query_map(params![current_time], |row| {
            let sec_int: i32 = row.get(5)?;
            let http_int: i32 = row.get(6)?;
            let host_only_int: i32 = row.get(8)?;
            Ok(Cookie {
                name: row.get(0)?,
                domain: row.get(1)?,
                path: row.get(2)?,
                value: row.get(3)?,
                expires_at: row.get(4)?,
                is_secure: sec_int != 0,
                is_http_only: http_int != 0,
                same_site: row.get(7)?,
                host_only: host_only_int != 0,
            })
        })?;

        let mut matched = Vec::new();
        for r in rows {
            let cookie = r?;
            if cookie.is_secure && !is_https {
                continue;
            }
            // Host-only cookies are bound to the exact host; only cookies set
            // with an explicit Domain attribute may follow the suffix rule.
            if cookie.host_only {
                if cookie.domain != host {
                    continue;
                }
            } else if !Self::domain_matches(&host, &cookie.domain) {
                continue;
            }
            if !Self::path_matches(req_path, &cookie.path) {
                continue;
            }
            matched.push(cookie);
        }

        Ok(matched)
    }

    /// Retrieves active cookies matching the given URL taking `SameSite` security policy into account.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if URL parsing or database querying fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get_cookies_for_request(
        &self,
        url_str: &str,
        current_time: i64,
        top_origin: Option<&str>,
        is_safe_method: bool,
    ) -> Result<Vec<Cookie>, StorageError> {
        let all_matched = self.get_cookies_for_url(url_str, current_time)?;
        let parsed_target =
            Url::parse(url_str).map_err(|e| StorageError::InvalidUrl(e.to_string()))?;
        let target_host = parsed_target.host_str().unwrap_or("").to_ascii_lowercase();

        let same_site_context =
            top_origin
                .and_then(|to| Url::parse(to).ok())
                .is_some_and(|to_url| {
                    let to_host = to_url.host_str().unwrap_or("").to_ascii_lowercase();
                    to_host == target_host
                        || Self::domain_matches(&target_host, &to_host)
                        || Self::domain_matches(&to_host, &target_host)
                });

        let mut filtered = Vec::new();
        for cookie in all_matched {
            let same_site_lower = cookie.same_site.to_ascii_lowercase();
            match same_site_lower.as_str() {
                "strict" => {
                    if same_site_context {
                        filtered.push(cookie);
                    }
                }
                "none" => {
                    // SameSite=None cookies MUST have Secure attribute set
                    if cookie.is_secure {
                        filtered.push(cookie);
                    }
                }
                _ => {
                    // Default behavior (Lax)
                    if same_site_context || is_safe_method {
                        filtered.push(cookie);
                    }
                }
            }
        }

        Ok(filtered)
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
