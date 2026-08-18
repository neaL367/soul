//! Text shaping and glyph advance calculation.

use crate::font::{FontDatabase, FontMetrics};

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
        let metrics = FontMetrics::for_size(font_size);
        let _font_id = self.font_db.query_font(font_family);

        let mut glyphs = Vec::with_capacity(text.len());
        let mut total_advance = 0.0;

        for (idx, ch) in text.char_indices() {
            let base_advance = character_advance_width(ch, font_size, font_family);
            let bold_adjusted = if is_bold {
                base_advance * 1.05
            } else {
                base_advance
            };

            let spacing = if ch == ' ' || ch == '\u{00A0}' {
                letter_spacing + word_spacing
            } else {
                letter_spacing
            };

            let char_advance = (bold_adjusted + spacing).max(0.0);

            #[allow(clippy::cast_possible_truncation)]
            glyphs.push(GlyphPosition {
                glyph_id: ch as u32,
                x_offset: total_advance,
                y_offset: 0.0,
                x_advance: char_advance,
                y_advance: 0.0,
                cluster: idx as u32,
            });

            total_advance += char_advance;
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

fn character_advance_width(ch: char, font_size: f32, family: &str) -> f32 {
    if family.eq_ignore_ascii_case("monospace") || family.eq_ignore_ascii_case("courier new") {
        return font_size * 0.6;
    }

    match ch {
        ' ' | '\t' | '\u{00A0}' | 'i' | 'l' | 'j' | '!' | '.' | ',' | ':' | ';' | '\'' | '|' => {
            font_size * 0.28
        }
        'f' | 'r' | 't' | '(' | ')' | '[' | ']' | '{' | '}' => font_size * 0.35,
        'm' | 'w' | 'M' | 'W' => font_size * 0.85,
        'A'..='Z' => font_size * 0.65,
        '0'..='9' => font_size * 0.55,
        _ => font_size * 0.52,
    }
}
