//! Text shaping and glyph advance calculation using OpenType font engines.

use crate::font::{FontDatabase, FontMetrics};
use cosmic_text::{Attrs, Buffer, Color, Family, Metrics, Shaping, SwashCache, Weight};
use std::sync::{Mutex, OnceLock};

/// Positioned glyph within a shaped text run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphPosition {
    /// Glyph identifier in the underlying font.
    pub glyph_id: u32,
    /// Horizontal offset from origin in layout pixels.
    pub x_offset: f32,
    /// Vertical offset from origin in layout pixels.
    pub y_offset: f32,
    /// Horizontal advance width to the next glyph in pixels.
    pub x_advance: f32,
    /// Vertical advance in pixels (usually 0.0 for horizontal LTR text).
    pub y_advance: f32,
    /// Character cluster index in the source UTF-8 string.
    pub cluster: u32,
}

/// Contiguous run of shaped glyphs with total advance width and typographic metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedRun {
    /// Ordered list of positioned glyphs.
    pub glyphs: Vec<GlyphPosition>,
    /// Total horizontal advance width of the run in pixels.
    pub advance_width: f32,
    /// Applied font size in pixels.
    pub font_size: f32,
    /// Source text string.
    pub text: String,
    /// Typographic vertical metrics for this run.
    pub metrics: FontMetrics,
}

/// Returns the global shared `cosmic_text::SwashCache` instance.
#[must_use]
pub fn global_swash_cache() -> &'static Mutex<SwashCache> {
    static SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();
    SWASH_CACHE.get_or_init(|| Mutex::new(SwashCache::new()))
}

/// Text shaping engine that converts UTF-8 strings into measured, positioned glyph runs.
#[derive(Clone, Default)]
pub struct TextShaper {
    font_db: FontDatabase,
}

impl TextShaper {
    /// Creates a new `TextShaper` using the global font database.
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_db: FontDatabase::global().clone(),
        }
    }

    /// Creates a `TextShaper` with a custom font database.
    #[must_use]
    pub const fn with_database(font_db: FontDatabase) -> Self {
        Self { font_db }
    }

    /// Shapes a text string into a `ShapedRun` with glyph positions and advance metrics.
    #[must_use]
    pub fn shape_text(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        is_bold: bool,
    ) -> ShapedRun {
        self.shape_text_with_spacing(text, font_family, font_size, is_bold, 0.0, 0.0)
    }

    /// Shapes a text string into a `ShapedRun` applying CSS letter-spacing and word-spacing.
    #[allow(clippy::significant_drop_tightening)]
    #[must_use]
    pub fn shape_text_with_spacing(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        is_bold: bool,
        letter_spacing: f32,
        word_spacing: f32,
    ) -> ShapedRun {
        let font_size = if font_size.is_finite() {
            font_size.max(0.0)
        } else {
            0.0
        };
        let letter_spacing = if letter_spacing.is_finite() {
            letter_spacing
        } else {
            0.0
        };
        let word_spacing = if word_spacing.is_finite() {
            word_spacing
        } else {
            0.0
        };

        let metrics = FontMetrics::from_database(&self.font_db, font_family, font_size);

        if text.is_empty() || font_size <= 0.0 {
            return ShapedRun {
                glyphs: Vec::new(),
                advance_width: 0.0,
                font_size,
                text: text.to_string(),
                metrics,
            };
        }

        let family_lower = font_family.to_ascii_lowercase();
        let family = match family_lower.as_str() {
            "sans-serif" => Family::SansSerif,
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            name => Family::Name(name),
        };

        let weight = if is_bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        };

        let mut font_system = FontDatabase::global_font_system()
            .lock()
            .expect("font system lock poisoned");

        let cosmic_metrics = Metrics::new(font_size, metrics.line_height);
        let mut buffer = Buffer::new(&mut font_system, cosmic_metrics);
        let attrs = Attrs::new().family(family).weight(weight);

        buffer.set_text(&mut font_system, text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut font_system, false);

        let mut glyphs = Vec::new();
        let mut total_advance: f32 = 0.0;

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let is_space = text
                    .get(glyph.start..glyph.end)
                    .is_some_and(|s| s.chars().all(|c| c == ' ' || c == '\u{00A0}'));

                let spacing = if is_space {
                    letter_spacing + word_spacing
                } else {
                    letter_spacing
                };

                let advance = (glyph.w + spacing).max(0.0);
                #[allow(clippy::cast_possible_truncation)]
                glyphs.push(GlyphPosition {
                    glyph_id: u32::from(glyph.glyph_id),
                    x_offset: total_advance,
                    y_offset: glyph.y,
                    x_advance: advance,
                    y_advance: 0.0,
                    cluster: glyph.start as u32,
                });
                total_advance += advance;
            }
        }

        // If buffer layout had no glyphs (e.g. whitespace-only), synthesize whitespace advances
        if glyphs.is_empty() {
            let space_advance = (font_size * 0.3 + letter_spacing + word_spacing).max(0.0);
            for (idx, ch) in text.char_indices() {
                #[allow(clippy::cast_possible_truncation)]
                glyphs.push(GlyphPosition {
                    glyph_id: ch as u32,
                    x_offset: total_advance,
                    y_offset: 0.0,
                    x_advance: space_advance,
                    y_advance: 0.0,
                    cluster: idx as u32,
                });
                total_advance += space_advance;
            }
        }

        ShapedRun {
            glyphs,
            advance_width: total_advance,
            font_size,
            text: text.to_string(),
            metrics,
        }
    }
}

/// Rasterizes a text run using the global font system and `SwashCache`, invoking a callback for each pixel/sub-rect.
#[allow(clippy::significant_drop_tightening)]
pub fn rasterize_text_to_callback(
    text: &str,
    font_family: &str,
    font_size: f32,
    is_bold: bool,
    color_rgba: (u8, u8, u8, u8),
    mut draw_fn: impl FnMut(i32, i32, u32, u32, Color),
) {
    if text.is_empty() || font_size <= 0.0 {
        return;
    }

    let family_lower = font_family.to_ascii_lowercase();
    let family = match family_lower.as_str() {
        "sans-serif" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" => Family::Monospace,
        name => Family::Name(name),
    };

    let weight = if is_bold {
        Weight::BOLD
    } else {
        Weight::NORMAL
    };

    let mut font_system = FontDatabase::global_font_system()
        .lock()
        .expect("font system lock poisoned");
    let mut swash_cache = global_swash_cache()
        .lock()
        .expect("swash cache lock poisoned");

    let metrics = Metrics::new(font_size, font_size * 1.2);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    let attrs = Attrs::new().family(family).weight(weight);

    buffer.set_text(&mut font_system, text, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut font_system, false);

    let text_color = Color::rgba(color_rgba.0, color_rgba.1, color_rgba.2, color_rgba.3);
    buffer.draw(
        &mut font_system,
        &mut swash_cache,
        text_color,
        |x, y, w, h, c| {
            draw_fn(x, y, w, h, c);
        },
    );
}
