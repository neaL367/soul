//! Windows DPAPI-encrypted persistent credential and secret vault backed by `SQLite`.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use platform_windows::Dpapi;
use rusqlite::params;

/// `SQLite`-backed encrypted credential vault utilizing Windows DPAPI encryption.
#[derive(Debug, Clone)]
pub struct DpapiVault {
    db: StorageDatabase,
}

impl DpapiVault {
    /// Creates a new `DpapiVault` and ensures the database table exists.
    ///
    /// # Errors
    /// Returns `StorageError` if table initialization fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn new(db: StorageDatabase) -> Result<Self, StorageError> {
        {
            let conn = db.conn()?;
            conn.execute(
                r"
                CREATE TABLE IF NOT EXISTS dpapi_vault (
                    domain TEXT NOT NULL,
                    key TEXT NOT NULL,
                    ciphertext BLOB NOT NULL,
                    PRIMARY KEY (domain, key)
                );
                ",
                [],
            )?;
        }
        Ok(Self { db })
    }

    /// Encrypts and persists a secret value using DPAPI.
    ///
    /// # Errors
    /// Returns `StorageError` if encryption or database insertion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn store_secret(
        &self,
        domain: &str,
        key: &str,
        plaintext: &str,
    ) -> Result<(), StorageError> {
        let ciphertext = Dpapi::protect(plaintext.as_bytes()).map_err(StorageError::Encryption)?;

        let conn = self.db.conn()?;
        conn.execute(
            r"
            INSERT INTO dpapi_vault (domain, key, ciphertext)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(domain, key) DO UPDATE SET ciphertext = excluded.ciphertext
            ",
            params![domain, key, ciphertext],
        )?;
        Ok(())
    }

    /// Retrieves and decrypts a secret value using DPAPI.
    ///
    /// # Errors
    /// Returns `StorageError` if database query or decryption fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get_secret(&self, domain: &str, key: &str) -> Result<Option<String>, StorageError> {
        let conn = self.db.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT ciphertext FROM dpapi_vault WHERE domain = ?1 AND key = ?2 LIMIT 1",
        )?;

        let ciphertext: Option<Vec<u8>> =
            stmt.query_row(params![domain, key], |row| row.get(0)).ok();

        let Some(bytes) = ciphertext else {
            return Ok(None);
        };

        let decrypted = Dpapi::unprotect(&bytes).map_err(StorageError::Encryption)?;

        let text =
            String::from_utf8(decrypted).map_err(|e| StorageError::InvalidData(e.to_string()))?;

        Ok(Some(text))
    }

    /// Deletes a stored secret.
    ///
    /// # Errors
    /// Returns `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn delete_secret(&self, domain: &str, key: &str) -> Result<bool, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute(
            "DELETE FROM dpapi_vault WHERE domain = ?1 AND key = ?2",
            params![domain, key],
        )?;
        Ok(count > 0)
    }
}
