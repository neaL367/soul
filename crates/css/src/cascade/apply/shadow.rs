//! CSS `box-shadow` property parsing and layer tokenization.

use super::values::parse_px;
use crate::properties::{BoxShadow, Color};

pub(crate) fn parse_box_shadows(value: &str) -> Option<Vec<BoxShadow>> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    if trimmed.is_empty() {
        return None;
    }

    let mut shadows = Vec::new();
    for layer_str in split_comma_separated_layers(trimmed) {
        if let Some(shadow) = parse_single_box_shadow(layer_str) {
            shadows.push(shadow);
        }
    }

    if shadows.is_empty() {
        None
    } else {
        Some(shadows)
    }
}

fn split_comma_separated_layers(value: &str) -> Vec<&str> {
    let mut layers = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (i, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let layer = value[start..i].trim();
                if !layer.is_empty() {
                    layers.push(layer);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = value[start..].trim();
    if !last.is_empty() {
        layers.push(last);
    }
    layers
}

fn parse_single_box_shadow(input: &str) -> Option<BoxShadow> {
    let mut inset = false;
    let mut lengths = Vec::new();
    let mut color = None;

    let tokens = tokenize_shadow_layer(input);
    for token in tokens {
        if token.eq_ignore_ascii_case("inset") {
            inset = true;
        } else if let Some(px) = parse_px(&token) {
            lengths.push(px);
        } else if let Some(c) = Color::parse(&token) {
            color = Some(c);
        }
    }

    if lengths.len() < 2 {
        return None;
    }

    let offset_x = lengths[0];
    let offset_y = lengths[1];
    let blur_radius = if lengths.len() >= 3 {
        lengths[2].max(0.0)
    } else {
        0.0
    };
    let spread_radius = if lengths.len() >= 4 { lengths[3] } else { 0.0 };
    let color = color.unwrap_or(Color::BLACK);

    Some(BoxShadow {
        offset_x,
        offset_y,
        blur_radius,
        spread_radius,
        color,
        inset,
    })
}

fn tokenize_shadow_layer(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for ch in input.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ' ' | '\t' | '\n' | '\r' if depth == 0 => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
