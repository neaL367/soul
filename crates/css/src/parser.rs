//! CSS tokenization and stylesheet parser.

use crate::rule::{Combinator, Declaration, Origin, Rule, Selector, SimpleSelector, StyleSheet};

/// Parses a CSS string into a structured `StyleSheet`.
#[must_use]
pub fn parse_stylesheet(css: &str, origin: Origin) -> StyleSheet {
    let clean_css = strip_comments(css);
    let mut sheet = StyleSheet::new(origin);

    let mut rest = clean_css.as_str();
    while let Some(open_brace) = rest.find('{') {
        let selector_text = &rest[..open_brace].trim();
        let after_open = &rest[open_brace + 1..];

        if let Some(close_brace) = after_open.find('}') {
            let body_text = &after_open[..close_brace].trim();
            rest = &after_open[close_brace + 1..];

            let selectors = parse_selectors(selector_text);
            let declarations = parse_declarations(body_text);

            if !selectors.is_empty() && !declarations.is_empty() {
                sheet.rules.push(Rule {
                    selectors,
                    declarations,
                    origin,
                });
            }
        } else {
            break;
        }
    }

    sheet
}

fn strip_comments(css: &str) -> String {
    let mut result = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
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
    let parts: Vec<&str> = input.split_whitespace().collect();

    let mut prev_was_child = false;
    for (i, part) in parts.iter().enumerate() {
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
    for statement in input.split(';') {
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((prop, val)) = trimmed.split_once(':') {
            let (val_str, important) = val
                .trim()
                .strip_suffix("!important")
                .map_or_else(|| (val.trim(), false), |stripped| (stripped.trim(), true));

            decls.push(Declaration::new(prop, val_str, important));
        }
    }
    decls
}
