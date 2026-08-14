//! Font loading, system font discovery, and typographic metrics.

use fontdb::{Database, Family, Query, Source};
use std::sync::{Arc, Mutex, OnceLock};

/// Vertical typographic metrics for a font at a specific pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// Distance in pixels from the baseline to the top of the highest glyphs.
    pub ascent: f32,
    /// Distance in pixels from the baseline to the bottom of the lowest descenders (positive value).
    pub descent: f32,
    /// Suggested line height in pixels (ascent + descent + line gap).
    pub line_height: f32,
}

impl FontMetrics {
    /// Creates synthetic standard metrics for a given font size in pixels.
    #[must_use]
    pub fn for_size(font_size: f32) -> Self {
        let ascent = font_size * 0.8;
        let descent = font_size * 0.2;
        let line_height = font_size * 1.2;
        Self {
            ascent,
            descent,
            line_height,
        }
    }
}

/// Thread-safe font database managing system fonts and family resolution.
#[derive(Clone)]
pub struct FontDatabase {
    inner: Arc<Mutex<Database>>,
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDatabase {
    /// Returns the global shared `FontDatabase` instance initialized with system fonts.
    #[must_use]
    pub fn global() -> &'static Self {
        static GLOBAL_DB: OnceLock<FontDatabase> = OnceLock::new();
        GLOBAL_DB.get_or_init(|| {
            let db = Self::new();
            db.load_system_fonts();
            db
        })
    }

    /// Creates a new empty `FontDatabase`.
    #[must_use]
    pub fn new() -> Self {
        let mut db = Database::new();
        db.set_sans_serif_family("Arial");
        db.set_serif_family("Times New Roman");
        db.set_monospace_family("Courier New");

        Self {
            inner: Arc::new(Mutex::new(db)),
        }
    }

    /// Loads system-installed fonts into the database.
    pub fn load_system_fonts(&self) {
        let mut db = self.inner.lock().expect("font database lock poisoned");
        db.load_system_fonts();
        tracing::debug!(loaded_faces = db.len(), "System fonts loaded into database");
    }

    /// Loads a font from in-memory byte slice.
    pub fn load_font_data(&self, data: Vec<u8>) {
        let mut db = self.inner.lock().expect("font database lock poisoned");
        db.load_font_source(Source::Binary(Arc::new(data)));
    }

    /// Queries for a font ID matching family and fallback requirements.
    #[must_use]
    pub fn query_font(&self, family_name: &str) -> Option<fontdb::ID> {
        let db = self.inner.lock().expect("font database lock poisoned");
        let lower = family_name.to_ascii_lowercase();
        let family = match lower.as_str() {
            "sans-serif" => Family::SansSerif,
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            name => Family::Name(name),
        };

        db.query(&Query {
            families: &[family, Family::SansSerif],
            ..Query::default()
        })
    }

    /// Returns the total number of font faces registered in the database.
    #[must_use]
    pub fn face_count(&self) -> usize {
        let db = self.inner.lock().expect("font database lock poisoned");
        db.len()
    }
}
