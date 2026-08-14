//! Inline formatting context layout, line breaking, and baseline alignment.

use crate::box_tree::{BoxType, LayoutBox};
use crate::geometry::Rect;
use css::FontWeight;
use text_shaping::{TextShaper, break_lines};

/// Single styled fragment of text positioned within a line box.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineFragment {
    /// Text slice in this fragment.
    pub text: String,
    /// Geometry rectangle of this fragment.
    pub bounds: Rect,
    /// Distance from the top of the fragment to its baseline.
    pub baseline_offset: f32,
    /// Font size in pixels.
    pub font_size: f32,
}

/// Horizontal line box containing positioned inline fragments with a common baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct LineBox {
    /// Bounding rectangle encompassing all fragments in this line.
    pub bounds: Rect,
    /// Baseline offset from the top of this line box.
    pub baseline: f32,
    /// Ordered list of inline fragments in this line.
    pub fragments: Vec<InlineFragment>,
}

impl LineBox {
    /// Creates a new empty `LineBox` at vertical offset `y`.
    #[must_use]
    pub const fn new(y: f32) -> Self {
        Self {
            bounds: Rect::new(0.0, y, 0.0, 0.0),
            baseline: 0.0,
            fragments: Vec::new(),
        }
    }
}

/// Lays out inline and text child boxes of a block container within an inline formatting context.
#[allow(clippy::too_many_lines)]
pub fn layout_inline_context(parent_box: &mut LayoutBox, max_width: f32) -> f32 {
    let shaper = TextShaper::new();
    let default_font_size = parent_box.style.as_ref().map_or(16.0, |s| s.font_size);
    let default_font_family = parent_box
        .style
        .as_ref()
        .map_or_else(|| "sans-serif".to_string(), |s| s.font_family.clone());

    let mut current_line = LineBox::new(0.0);
    let mut cursor_x = 0.0;
    let mut cursor_y = 0.0;

    for child in &mut parent_box.children {
        let (text, font_size, font_family, is_bold) = match &child.box_type {
            BoxType::TextNode(_, text) => {
                let font_size = child
                    .style
                    .as_ref()
                    .map_or(default_font_size, |s| s.font_size);
                let font_family = child
                    .style
                    .as_ref()
                    .map_or(&default_font_family, |s| &s.font_family);
                let is_bold = child
                    .style
                    .as_ref()
                    .is_some_and(|s| s.font_weight == FontWeight::Bold);
                (text.clone(), font_size, font_family.clone(), is_bold)
            }
            BoxType::InlineNode(_) => {
                let text = child
                    .children
                    .iter()
                    .filter_map(|c| {
                        if let BoxType::TextNode(_, t) = &c.box_type {
                            Some(t.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let font_size = child
                    .style
                    .as_ref()
                    .map_or(default_font_size, |s| s.font_size);
                let font_family = child
                    .style
                    .as_ref()
                    .map_or(&default_font_family, |s| &s.font_family);
                let is_bold = child
                    .style
                    .as_ref()
                    .is_some_and(|s| s.font_weight == FontWeight::Bold);
                (text, font_size, font_family.clone(), is_bold)
            }
            _ => continue,
        };

        if text.is_empty() {
            continue;
        }

        let spans = break_lines(&text, max_width, |seg| {
            shaper
                .shape_text(seg, &font_family, font_size, is_bold)
                .advance_width
        });

        for span in spans {
            if cursor_x > 0.0 && cursor_x + span.width > max_width {
                cursor_y += current_line.bounds.height.max(font_size * 1.2);
                current_line = LineBox::new(cursor_y);
                cursor_x = 0.0;
            }

            let line_h = font_size * 1.2;
            let baseline = font_size * 0.8;

            current_line.fragments.push(InlineFragment {
                text: span.text,
                bounds: Rect::new(cursor_x, cursor_y, span.width, line_h),
                baseline_offset: baseline,
                font_size,
            });

            cursor_x += span.width;
            current_line.bounds.width = cursor_x;
            current_line.bounds.height = current_line.bounds.height.max(line_h);
            current_line.baseline = current_line.baseline.max(baseline);

            if span.is_hard_break {
                cursor_y += current_line.bounds.height.max(font_size * 1.2);
                current_line = LineBox::new(cursor_y);
                cursor_x = 0.0;
            }
        }
    }

    if !current_line.fragments.is_empty() {
        cursor_y += current_line.bounds.height;
    }

    // Update child bounding positions
    let parent_x = parent_box.dimensions.content.x;
    let parent_y = parent_box.dimensions.content.y;
    for child in &mut parent_box.children {
        child.dimensions.content.x = parent_x;
        child.dimensions.content.y = parent_y;
        child.dimensions.content.width = max_width;
        child.dimensions.content.height = cursor_y;
    }

    cursor_y
}
