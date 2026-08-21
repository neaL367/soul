//! CSS value parsers for lengths, percentages, edge boxes, font families, and grid tracks.

use crate::properties::{Color, ComputedStyle, GridTrack};

pub(crate) fn parse_font_family(value: &str) -> Option<String> {
    let first = value.split(',').next()?.trim();
    let unquoted = first
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| first.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(first);
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

pub(crate) fn apply_border_shorthand(style: &mut ComputedStyle, value: &str) {
    for part in value.split_whitespace() {
        if let Some(px) = parse_non_negative_px(part) {
            style.border_top_width = px;
            style.border_right_width = px;
            style.border_bottom_width = px;
            style.border_left_width = px;
        } else if let Some(color) = Color::parse(part) {
            style.border_top_color = color;
            style.border_right_color = color;
            style.border_bottom_color = color;
            style.border_left_color = color;
        }
    }
}

pub(crate) fn parse_percent(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    let num = trimmed.strip_suffix('%')?.trim().parse::<f32>().ok()?;
    if num.is_finite() { Some(num) } else { None }
}

pub(crate) fn parse_px(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    trimmed.strip_suffix("px").map_or_else(
        || {
            if trimmed == "0" || trimmed == "0.0" {
                Some(0.0)
            } else {
                None
            }
        },
        |num| {
            let num = num.trim().parse::<f32>().ok()?;
            if num.is_finite() { Some(num) } else { None }
        },
    )
}

/// Parses a length in `px`, rejecting negative values.
pub(crate) fn parse_non_negative_px(value: &str) -> Option<f32> {
    parse_px(value).filter(|v| *v >= 0.0)
}

/// Parses 1–4 edge lengths, rejecting the whole declaration if any edge is negative.
pub(crate) fn parse_4_edges_non_negative(value: &str) -> Option<(f32, f32, f32, f32)> {
    parse_4_edges(value).filter(|(t, r, b, l)| *t >= 0.0 && *r >= 0.0 && *b >= 0.0 && *l >= 0.0)
}

pub(crate) fn parse_4_edges(value: &str) -> Option<(f32, f32, f32, f32)> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.len() {
        1 => {
            let v = parse_px(parts[0])?;
            Some((v, v, v, v))
        }
        2 => {
            let tb = parse_px(parts[0])?;
            let rl = parse_px(parts[1])?;
            Some((tb, rl, tb, rl))
        }
        3 => {
            let top = parse_px(parts[0])?;
            let rl = parse_px(parts[1])?;
            let bottom = parse_px(parts[2])?;
            Some((top, rl, bottom, rl))
        }
        4 => {
            let top = parse_px(parts[0])?;
            let right = parse_px(parts[1])?;
            let bottom = parse_px(parts[2])?;
            let left = parse_px(parts[3])?;
            Some((top, right, bottom, left))
        }
        _ => None,
    }
}

pub(crate) fn parse_grid_tracks(value: &str) -> Option<Vec<GridTrack>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut tracks = Vec::new();
    for part in trimmed.split_whitespace() {
        if part.contains('(') {
            tracks.push(GridTrack::Auto);
            continue;
        }
        let track = parse_grid_track(part)?;
        tracks.push(track);
    }
    if tracks.is_empty() {
        None
    } else {
        Some(tracks)
    }
}

fn parse_grid_track(value: &str) -> Option<GridTrack> {
    let lower = value.to_ascii_lowercase();
    if lower == "auto" {
        return Some(GridTrack::Auto);
    }
    if let Some(fr_str) = lower.strip_suffix("fr")
        && let Ok(v) = fr_str.trim().parse::<f32>()
        && v.is_finite()
        && v >= 0.0
    {
        return Some(GridTrack::Fr(v));
    }
    if let Some(px) = parse_non_negative_px(value) {
        return Some(GridTrack::Px(px));
    }
    if let Some(pct) = parse_percent(value) {
        return Some(GridTrack::Percent(pct));
    }
    None
}
