//! Font loading, system font discovery, and typographic metrics.

#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::significant_drop_tightening)]

use cosmic_text::FontSystem;
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

    /// Calculates precise typographic metrics for a font family at a given size.
    ///
    /// Attempts to read real font metrics via `swash`; falls back to synthetic
    /// ratios when the font is not found or metrics cannot be read.
    #[must_use]
    pub fn from_database(font_db: &FontDatabase, family_name: &str, font_size: f32) -> Self {
        let font_size = if font_size.is_finite() {
            font_size.max(0.0)
        } else {
            0.0
        };

        if font_size <= 0.0 {
            return Self {
                ascent: 0.0,
                descent: 0.0,
                line_height: 0.0,
            };
        }

        if let Some(metrics) = Self::read_real_metrics(font_db, family_name, font_size) {
            return metrics;
        }

        // Fallback to synthetic ratios tuned per generic family.
        let lower = family_name.to_ascii_lowercase();
        let (ascent_ratio, descent_ratio, line_height_ratio) = match lower.as_str() {
            "monospace" | "courier new" | "consolas" => (0.75, 0.25, 1.25),
            "serif" | "times new roman" | "georgia" => (0.82, 0.22, 1.22),
            _ => (0.80, 0.20, 1.20),
        };

        Self {
            ascent: font_size * ascent_ratio,
            descent: font_size * descent_ratio,
            line_height: font_size * line_height_ratio,
        }
    }

    fn read_real_metrics(
        font_db: &FontDatabase,
        family_name: &str,
        font_size: f32,
    ) -> Option<Self> {
        let face_id = font_db.query_font(family_name)?;
        let db = font_db.inner.lock().ok()?;
        let face = db.face(face_id)?;
        let data = Self::load_face_data(face)?;
        let font = swash::FontRef::from_index(&data, face.index as usize)?;
        let metrics = font.metrics(&[]);

        let upem = metrics.units_per_em as f32;
        if upem <= 0.0 {
            return None;
        }
        // `ascent` is positive, `descent` is positive after handling sign, `leading` is line gap.
        let ascent = (metrics.ascent / upem) * font_size;
        let descent = (metrics.descent / upem) * font_size;
        let leading = (metrics.leading / upem) * font_size;

        // Guard against degenerate metrics.
        if !ascent.is_finite() || !descent.is_finite() || !leading.is_finite() {
            return None;
        }
        let line_height = ascent + descent + leading.max(0.0);
        if line_height <= 0.0 {
            return None;
        }

        Some(Self {
            ascent,
            descent: descent.max(0.0),
            line_height,
        })
    }

    fn load_face_data(face: &fontdb::FaceInfo) -> Option<Vec<u8>> {
        match &face.source {
            Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
            Source::File(path) | Source::SharedFile(path, _) => std::fs::read(path).ok(),
        }
    }
}

/// Thread-safe font database managing system fonts and family resolution.
#[derive(Clone)]
pub struct FontDatabase {
    pub(crate) inner: Arc<Mutex<Database>>,
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

    /// Returns the global shared `cosmic_text::FontSystem` instance.
    #[must_use]
    pub fn global_font_system() -> &'static Mutex<FontSystem> {
        static GLOBAL_FS: OnceLock<Mutex<FontSystem>> = OnceLock::new();
        GLOBAL_FS.get_or_init(|| Mutex::new(FontSystem::new()))
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
