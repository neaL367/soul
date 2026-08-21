//! CSS Custom Properties variable substitution and fallback resolution.

use std::collections::HashMap;

/// Resolves `var(--name)` and `var(--name, fallback)` occurrences in a CSS value string.
#[must_use]
pub fn resolve_var_references<S: std::hash::BuildHasher>(
    value: &str,
    vars: &HashMap<String, String, S>,
) -> String {
    if !value.contains("var(") {
        return value.to_string();
    }

    let mut result = String::with_capacity(value.len());
    let mut remaining = value;

    while let Some(start_idx) = remaining.find("var(") {
        result.push_str(&remaining[..start_idx]);
        let after_var = &remaining[start_idx + 4..];

        // Find matching closing parenthesis accounting for nested parens
        let mut depth = 1usize;
        let mut end_idx = None;
        for (i, ch) in after_var.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(close_pos) = end_idx {
            let var_body = &after_var[..close_pos];
            remaining = &after_var[close_pos + 1..];

            let (var_name, fallback) = split_var_fallback(var_body);

            if let Some(val) = vars.get(var_name) {
                let resolved = resolve_var_references(val, vars);
                result.push_str(&resolved);
            } else if let Some(fb) = fallback {
                let resolved_fallback = resolve_var_references(fb, vars);
                result.push_str(&resolved_fallback);
            }
        } else {
            result.push_str("var(");
            remaining = after_var;
        }
    }

    result.push_str(remaining);
    result
}

fn split_var_fallback(body: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    for (i, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                return (body[..i].trim(), Some(body[i + 1..].trim()));
            }
            _ => {}
        }
    }
    (body.trim(), None)
}
