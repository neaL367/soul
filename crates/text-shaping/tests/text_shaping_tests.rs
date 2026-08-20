//! Integration tests for font discovery, text shaping, glyph layout, and line breaking.

use text_shaping::{FontDatabase, FontMetrics, TextShaper, break_lines};

#[test]
fn test_font_database_and_metrics() {
    let db = FontDatabase::global();
    assert!(db.face_count() > 0 || db.query_font("sans-serif").is_none());

    let metrics = FontMetrics::for_size(16.0);
    assert!((metrics.ascent - 12.8).abs() < f32::EPSILON);
    assert!((metrics.descent - 3.2).abs() < f32::EPSILON);
    assert!((metrics.line_height - 19.2).abs() < f32::EPSILON);
}

#[test]
fn test_text_shaper_glyph_advances() {
    let shaper = TextShaper::new();
    let normal_run = shaper.shape_text("Hello World", "sans-serif", 16.0, false);
    let bold_run = shaper.shape_text("Hello World", "sans-serif", 16.0, true);

    assert_eq!(normal_run.glyphs.len(), 11);
    assert_eq!(bold_run.glyphs.len(), 11);

    assert!(normal_run.advance_width > 0.0);
    assert!(bold_run.advance_width > normal_run.advance_width);
}

#[test]
fn test_unicode_line_breaking_at_width_boundary() {
    let shaper = TextShaper::new();
    let text = "The quick brown fox jumps over the lazy dog";

    // Break lines with max width 120px
    let lines = break_lines(text, 120.0, |segment| {
        shaper
            .shape_text(segment, "sans-serif", 16.0, false)
            .advance_width
    });

    assert!(lines.len() >= 3);
    for line in &lines {
        assert!(line.width <= 140.0);
    }
}

#[test]
fn test_unicode_line_breaking_hard_newline() {
    let shaper = TextShaper::new();
    let text = "First Line\nSecond Line\nThird Line";

    let lines = break_lines(text, 500.0, |segment| {
        shaper
            .shape_text(segment, "sans-serif", 16.0, false)
            .advance_width
    });

    assert_eq!(lines.len(), 3);
    assert!(lines[0].is_hard_break);
    assert_eq!(lines[0].text, "First Line");
    assert!(lines[1].is_hard_break);
    assert_eq!(lines[1].text, "Second Line");
    assert_eq!(lines[2].text, "Third Line");
}

#[test]
fn test_text_shaper_letter_and_word_spacing() {
    let shaper = TextShaper::new();
    let base_run = shaper.shape_text("Hello World", "sans-serif", 16.0, false);
    let spaced_run =
        shaper.shape_text_with_spacing("Hello World", "sans-serif", 16.0, false, 2.0, 5.0);

    // 11 characters * 2px letter spacing + 1 space * 5px word spacing = 27px added
    let expected_diff = 27.0;
    let actual_diff = spaced_run.advance_width - base_run.advance_width;
    assert!(
        (actual_diff - expected_diff).abs() < 0.1,
        "expected diff {expected_diff}, got {actual_diff}"
    );
}

#[test]
fn test_non_finite_shaping_inputs_are_sanitized() {
    let shaper = TextShaper::new();

    // NaN font size clamps to zero: zero-width run, no NaN propagates.
    let nan_run = shaper.shape_text("Hello", "sans-serif", f32::NAN, false);
    assert!((nan_run.advance_width - 0.0).abs() < f32::EPSILON);
    assert!(nan_run.advance_width.is_finite());
    assert!(nan_run.metrics.line_height.is_finite());

    // Negative font size clamps to zero.
    let negative_run = shaper.shape_text("Hello", "sans-serif", -16.0, false);
    assert!((negative_run.advance_width - 0.0).abs() < f32::EPSILON);

    // NaN spacing does not poison per-character advances.
    let spaced = shaper.shape_text_with_spacing(
        "Hello World",
        "sans-serif",
        16.0,
        false,
        f32::NAN,
        f32::NAN,
    );
    assert!(spaced.advance_width.is_finite());
    assert!(spaced.advance_width > 0.0);
}

#[test]
fn test_non_finite_max_width_does_not_break_soft_wrapping() {
    let shaper = TextShaper::new();
    let text = "A short line\nwith a hard break";

    // NaN max width disables soft wrapping; hard breaks still apply and all
    // widths stay finite.
    let lines = break_lines(text, f32::NAN, |segment| {
        shaper
            .shape_text(segment, "sans-serif", 16.0, false)
            .advance_width
    });

    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert!(line.width.is_finite());
    }
}
