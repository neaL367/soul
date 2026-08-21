//! CSS tokenization and stylesheet parser.

#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

use crate::rule::{Declaration, Origin, Rule, Selector, StyleSheet};
use crate::selector_impl::SoulParser;
use cssparser::{Parser as CssParser, ParserInput, ToCss};
use selectors::parser::{ParseRelative, SelectorList};

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
    // Use `selectors` crate for robust parsing. Handle comma-separated selector list.
    // If the full list fails, try forgiving per-selector fallback to preserve valid selectors.
    let mut input_owned = ParserInput::new(input);
    let mut parser = CssParser::new(&mut input_owned);
    let soul_parser = SoulParser;

    match SelectorList::parse(&soul_parser, &mut parser, ParseRelative::No) {
        Ok(list) => list
            .slice()
            .iter()
            .map(|s| {
                let mut out = String::new();
                s.to_css(&mut out).unwrap();
                Selector {
                    inner: s.clone(),
                    source: out,
                }
            })
            .collect(),
        Err(_) => {
            // Forgiving fallback: split manually on commas and keep individually valid selectors.
            input
                .split(',')
                .filter_map(|part| {
                    let trimmed = part.trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    let mut pi = ParserInput::new(trimmed);
                    let mut p = CssParser::new(&mut pi);
                    match SelectorList::parse(&soul_parser, &mut p, ParseRelative::No) {
                        Ok(list) if !list.slice().is_empty() => Some(Selector {
                            inner: list.slice()[0].clone(),
                            source: trimmed.to_string(),
                        }),
                        _ => None,
                    }
                })
                .collect()
        }
    }
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
