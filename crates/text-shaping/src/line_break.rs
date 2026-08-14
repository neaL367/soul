//! UAX #14 Unicode line breaking and text segment wrapping.

/// Line segment resulting from breaking text against a maximum width constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLineSpan {
    /// Text slice content of this line.
    pub text: String,
    /// Measured advance width in pixels.
    pub width: f32,
    /// `true` if this line ended with an explicit hard newline character.
    pub is_hard_break: bool,
}

/// Breaks text into lines using Unicode line breaking opportunities and a width constraint.
pub fn break_lines<F>(text: &str, max_width: f32, mut measure_fn: F) -> Vec<TextLineSpan>
where
    F: FnMut(&str) -> f32,
{
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0;

    let breaks: Vec<(usize, unicode_linebreak::BreakOpportunity)> =
        unicode_linebreak::linebreaks(text).collect();

    let mut prev_idx = 0;
    for (idx, opportunity) in breaks {
        let segment = &text[prev_idx..idx];
        prev_idx = idx;

        let seg_width = measure_fn(segment);
        let is_mandatory = opportunity == unicode_linebreak::BreakOpportunity::Mandatory;

        if !current_line.is_empty() && current_width + seg_width > max_width {
            lines.push(TextLineSpan {
                text: current_line.trim_end().to_string(),
                width: current_width,
                is_hard_break: false,
            });
            current_line = segment.trim_start().to_string();
            current_width = measure_fn(&current_line);
        } else {
            current_line.push_str(segment);
            current_width += seg_width;
        }

        if is_mandatory {
            lines.push(TextLineSpan {
                text: current_line.trim_end().to_string(),
                width: current_width,
                is_hard_break: true,
            });
            current_line.clear();
            current_width = 0.0;
        }
    }

    if !current_line.is_empty() {
        lines.push(TextLineSpan {
            text: current_line,
            width: current_width,
            is_hard_break: false,
        });
    }

    lines
}
