//! Storage engine providing `SQLite` persistence for history, bookmarks, cookies, web storage, and `IndexedDB`.

pub mod bookmarks;
pub mod cookies;
pub mod db;
pub mod error;
pub mod history;
pub mod indexeddb;
pub mod web_storage;

pub use bookmarks::{BookmarkEntry, BookmarkStore};
pub use cookies::{Cookie, CookieJar};
pub use db::StorageDatabase;
pub use error::StorageError;
pub use history::{HistoryEntry, HistoryStore};
pub use indexeddb::IndexedDbStore;
pub use web_storage::{LocalStorage, SessionStorage};
