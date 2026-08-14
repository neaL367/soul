//! Bookmarks database store for user saved pages and folders.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use rusqlite::params;

/// User saved bookmark record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkEntry {
    /// Unique database record identifier.
    pub id: i64,
    /// Target bookmarked URL string.
    pub url: String,
    /// User-visible title.
    pub title: String,
    /// Folder or tag grouping.
    pub folder: Option<String>,
    /// Creation timestamp in Unix seconds.
    pub created_at: i64,
}

/// Persistent store managing user bookmarks.
#[derive(Debug, Clone)]
pub struct BookmarkStore {
    db: StorageDatabase,
}

impl BookmarkStore {
    /// Creates a new `BookmarkStore` using the provided database connection.
    #[must_use]
    pub const fn new(db: StorageDatabase) -> Self {
        Self { db }
    }

    /// Adds a new bookmark entry and returns its assigned database ID.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if insertion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn add_bookmark(
        &self,
        url: &str,
        title: &str,
        folder: Option<&str>,
        created_at: i64,
    ) -> Result<i64, StorageError> {
        let conn = self.db.conn()?;
        conn.execute(
            "INSERT INTO bookmarks (url, title, folder, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![url, title, folder, created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Lists all bookmarks or bookmarks filtered by folder.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn list_bookmarks(&self, folder: Option<&str>) -> Result<Vec<BookmarkEntry>, StorageError> {
        let conn = self.db.conn()?;

        if let Some(f) = folder {
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, title, folder, created_at FROM bookmarks WHERE folder = ?1 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![f], |row| {
                Ok(BookmarkEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    folder: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        } else {
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, title, folder, created_at FROM bookmarks ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(BookmarkEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    folder: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        }
    }

    /// Deletes a bookmark by ID.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn delete_bookmark(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }
}
