//! Font loading, text shaping, glyph positioning, and Unicode line breaking.

pub mod font;
pub mod line_break;
pub mod shaper;

pub use font::{FontDatabase, FontMetrics};
pub use line_break::{TextLineSpan, break_lines};
pub use shaper::{
    GlyphPosition, ShapedRun, TextShaper, global_swash_cache, rasterize_text_to_callback,
};
