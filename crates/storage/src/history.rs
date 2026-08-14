//! Navigation history tracking, visit frequency, and search indexing.

use crate::db::StorageDatabase;
use crate::error::StorageError;
use rusqlite::params;

/// Record representing a visited URL in the browser history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Unique database record identifier.
    pub id: i64,
    /// Target web page URL string.
    pub url: String,
    /// Page title extracted from `<title>` tag if available.
    pub title: Option<String>,
    /// Number of times this URL has been visited.
    pub visit_count: u32,
    /// Unix timestamp in seconds of the most recent visit.
    pub last_visited_at: i64,
}

/// Persistent store managing history records.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    db: StorageDatabase,
}

impl HistoryStore {
    /// Creates a new `HistoryStore` using the provided database connection.
    #[must_use]
    pub const fn new(db: StorageDatabase) -> Self {
        Self { db }
    }

    /// Records a page visit, incrementing `visit_count` if the URL exists or creating a new entry.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if `SQLite` query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn record_visit(
        &self,
        url: &str,
        title: Option<&str>,
        timestamp: i64,
    ) -> Result<(), StorageError> {
        let conn = self.db.conn()?;

        let mut stmt =
            conn.prepare_cached("SELECT id, visit_count FROM history WHERE url = ?1 LIMIT 1")?;
        let existing: Option<(i64, u32)> = stmt
            .query_row(params![url], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok();

        if let Some((id, count)) = existing {
            conn.execute(
                "UPDATE history SET visit_count = ?1, last_visited_at = ?2, title = COALESCE(?3, title) WHERE id = ?4",
                params![count + 1, timestamp, title, id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO history (url, title, visit_count, last_visited_at) VALUES (?1, ?2, 1, ?3)",
                params![url, title, timestamp],
            )?;
        }

        Ok(())
    }

    /// Queries history entries matching an optional search pattern ordered by visit timestamp.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening, clippy::cast_possible_wrap)]
    pub fn query_history(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HistoryEntry>, StorageError> {
        let conn = self.db.conn()?;
        let trimmed = query.trim();

        if trimmed.is_empty() {
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, title, visit_count, last_visited_at FROM history ORDER BY last_visited_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visit_count: row.get(3)?,
                    last_visited_at: row.get(4)?,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        } else {
            let pattern = format!("%{trimmed}%");
            let mut stmt = conn.prepare_cached(
                "SELECT id, url, title, visit_count, last_visited_at FROM history WHERE url LIKE ?1 OR title LIKE ?1 ORDER BY last_visited_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![pattern, limit as i64], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visit_count: row.get(3)?,
                    last_visited_at: row.get(4)?,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        }
    }

    /// Deletes a specific history record by ID.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if query execution fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn delete_history(&self, id: i64) -> Result<bool, StorageError> {
        let conn = self.db.conn()?;
        let count = conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    /// Clears all history entries.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if table deletion fails.
    #[allow(clippy::significant_drop_tightening)]
    pub fn clear_all(&self) -> Result<(), StorageError> {
        let conn = self.db.conn()?;
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    }
}
