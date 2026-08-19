//! CSS tokenization and stylesheet parser.

use crate::rule::{Combinator, Declaration, Origin, Rule, Selector, SimpleSelector, StyleSheet};

/// Parses a CSS string into a structured `StyleSheet`.
#[must_use]
pub fn parse_stylesheet(css: &str, origin: Origin) -> StyleSheet {
    let (clean_css, spans) = scan_css(css);
    let mut sheet = StyleSheet::new(origin);

    for (selector_start, brace_idx, body_end) in spans {
        let selector_text = clean_css[selector_start..brace_idx].trim();
        // At-rules (`@media`, `@import`, ...) are not supported yet: skip their
        // bodies instead of misparsing their contents as style rules.
        if selector_text.starts_with('@') {
            continue;
        }
        let body_text = &clean_css[brace_idx + 1..body_end];

        let selectors = parse_selectors(selector_text);
        let declarations = parse_declarations(body_text);

        if !selectors.is_empty() && !declarations.is_empty() {
            sheet.rules.push(Rule {
                selectors,
                declarations,
                origin,
            });
        }
    }

    sheet
}

/// Single-pass scanner producing the comment-stripped stylesheet plus the byte
/// spans of every top-level rule as `(selector_start, brace_idx, body_end)`.
///
/// The scan is string-, comment-, and parenthesis-aware: braces and
/// `/* ... */` sequences inside quoted strings or `url(...)` values do not
/// terminate rules, and comments inside string values are preserved.
fn scan_css(css: &str) -> (String, Vec<(usize, usize, usize)>) {
    let mut cleaned = String::with_capacity(css.len());
    let mut spans = Vec::new();
    let mut chars = css.char_indices().peekable();
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let mut in_comment = false;
    let mut paren_depth = 0u32;
    let mut depth = 0u32;
    let mut selector_start: Option<usize> = Some(0);
    let mut brace_idx = 0;

    while let Some((_, ch)) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek().is_some_and(|(_, c)| *c == '/') {
                chars.next();
                in_comment = false;
            }
            continue;
        }
        if let Some(quote) = in_string {
            cleaned.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '/' if chars.peek().is_some_and(|(_, c)| *c == '*') => {
                chars.next();
                in_comment = true;
            }
            '"' | '\'' => {
                in_string = Some(ch);
                cleaned.push(ch);
            }
            '(' => {
                paren_depth += 1;
                cleaned.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                cleaned.push(ch);
            }
            '{' if paren_depth == 0 => {
                if depth == 0 {
                    selector_start = Some(selector_start.unwrap_or(cleaned.len()));
                    brace_idx = cleaned.len();
                }
                depth += 1;
                cleaned.push(ch);
            }
            '}' if paren_depth == 0 => {
                if depth > 0 {
                    depth -= 1;
                    cleaned.push(ch);
                    if depth == 0 {
                        if let Some(start) = selector_start {
                            spans.push((start, brace_idx, cleaned.len() - 1));
                        }
                        selector_start = Some(cleaned.len());
                    }
                }
                // Stray '}' outside any rule is ignored.
            }
            _ => cleaned.push(ch),
        }
    }

    (cleaned, spans)
}

fn parse_selectors(input: &str) -> Vec<Selector> {
    input
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                parse_single_selector(trimmed)
            }
        })
        .collect()
}

fn parse_single_selector(input: &str) -> Option<Selector> {
    let mut sequence = Vec::new();
    let tokens = tokenize_selector(input);

    for (combinator, token) in tokens {
        let simple = if token == "*" {
            SimpleSelector::Universal
        } else if let Some(id) = token.strip_prefix('#') {
            SimpleSelector::Id(id.to_string())
        } else if let Some(class) = token.strip_prefix('.') {
            SimpleSelector::Class(class.to_string())
        } else {
            SimpleSelector::Tag(token.to_ascii_lowercase())
        };
        sequence.push((combinator, simple));
    }

    if sequence.is_empty() {
        None
    } else {
        Some(Selector { sequence })
    }
}

fn tokenize_selector(input: &str) -> Vec<(Option<Combinator>, String)> {
    let mut tokens = Vec::new();

    // Split on whitespace, then on embedded '>' so that "div>p" and "div > p"
    // both tokenize as `div` CHILD `p`.
    let mut raw_tokens: Vec<&str> = Vec::new();
    for part in input.split_whitespace() {
        let mut rest = part;
        while let Some(idx) = rest.find('>') {
            let before = &rest[..idx];
            if !before.is_empty() {
                raw_tokens.push(before);
            }
            raw_tokens.push(">");
            rest = &rest[idx + 1..];
        }
        if !rest.is_empty() {
            raw_tokens.push(rest);
        }
    }

    let mut prev_was_child = false;
    for (i, part) in raw_tokens.iter().enumerate() {
        if *part == ">" {
            prev_was_child = true;
            continue;
        }

        let combinator = if i == 0 {
            None
        } else if prev_was_child {
            prev_was_child = false;
            Some(Combinator::Child)
        } else {
            Some(Combinator::Descendant)
        };

        tokens.push((combinator, (*part).to_string()));
    }

    tokens
}

fn parse_declarations(input: &str) -> Vec<Declaration> {
    let mut decls = Vec::new();
    let mut start = 0;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let mut paren_depth = 0u32;

    // Split on ';' only outside strings and url(...) parentheses so that values
    // like `content: "a;b"` or `background: url(data:...;base64,...)` survive.
    for (idx, ch) in input.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ';' if paren_depth == 0 => {
                push_declaration(&mut decls, &input[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    push_declaration(&mut decls, &input[start..]);
    decls
}

fn push_declaration(decls: &mut Vec<Declaration>, statement: &str) {
    let trimmed = statement.trim();
    if trimmed.is_empty() {
        return;
    }

    if let Some((prop, val)) = trimmed.split_once(':') {
        let val_trimmed = val.trim();
        // `!important` may be written with or without whitespace before `!` and
        // is case-insensitive per CSS Syntax Level 3.
        let lowered = val_trimmed.to_ascii_lowercase();
        let marker_len = if lowered.ends_with("!important") {
            Some("!important".len())
        } else if lowered.ends_with("! important") {
            Some("! important".len())
        } else {
            None
        };

        let (value, important) = marker_len.map_or((val_trimmed, false), |len| {
            (val_trimmed[..val_trimmed.len() - len].trim(), true)
        });

        decls.push(Declaration::new(prop, value, important));
    }
}
