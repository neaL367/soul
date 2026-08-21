//! CSS transform, gradient, and transition property parsing.

use crate::properties::{Color, ColorStop, Gradient, TimingFunction, TransformOp, Transition};

/// Parses a CSS `transform` value into a list of [`TransformOp`].
#[must_use]
pub fn parse_transform_list(input: &str) -> Vec<TransformOp> {
    let mut ops = Vec::new();
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return ops;
    }

    let mut rest = trimmed;
    while let Some(open_idx) = rest.find('(') {
        let name = rest[..open_idx].trim();
        let Some(close_idx) = rest.find(')') else {
            break;
        };
        let args_str = &rest[open_idx + 1..close_idx];

        if let Some(op) = parse_single_transform_op(name, args_str) {
            ops.push(op);
        }

        rest = rest[close_idx + 1..].trim();
    }

    ops
}

#[allow(clippy::many_single_char_names)]
fn parse_single_transform_op(name: &str, args_str: &str) -> Option<TransformOp> {
    let args: Vec<&str> = args_str.split(',').map(str::trim).collect();
    match name.to_ascii_lowercase().as_str() {
        "translate" => {
            let tx = parse_length_val(args.first()?)?;
            let ty = args.get(1).and_then(|a| parse_length_val(a)).unwrap_or(0.0);
            Some(TransformOp::Translate(tx, ty))
        }
        "translatex" => {
            let tx = parse_length_val(args.first()?)?;
            Some(TransformOp::Translate(tx, 0.0))
        }
        "translatey" => {
            let ty = parse_length_val(args.first()?)?;
            Some(TransformOp::Translate(0.0, ty))
        }
        "scale" => {
            let sx = args.first()?.parse::<f32>().ok()?;
            let sy = args
                .get(1)
                .and_then(|a| a.parse::<f32>().ok())
                .unwrap_or(sx);
            Some(TransformOp::Scale(sx, sy))
        }
        "scalex" => {
            let sx = args.first()?.parse::<f32>().ok()?;
            Some(TransformOp::Scale(sx, 1.0))
        }
        "scaley" => {
            let sy = args.first()?.parse::<f32>().ok()?;
            Some(TransformOp::Scale(1.0, sy))
        }
        "rotate" => {
            let rad = parse_angle_val(args.first()?)?;
            Some(TransformOp::Rotate(rad))
        }
        "skew" => {
            let ax = parse_angle_val(args.first()?)?;
            let ay = args.get(1).and_then(|a| parse_angle_val(a)).unwrap_or(0.0);
            Some(TransformOp::Skew(ax, ay))
        }
        "skewx" => {
            let ax = parse_angle_val(args.first()?)?;
            Some(TransformOp::Skew(ax, 0.0))
        }
        "skewy" => {
            let ay = parse_angle_val(args.first()?)?;
            Some(TransformOp::Skew(0.0, ay))
        }
        "matrix" if args.len() >= 6 => {
            let a = args[0].parse::<f32>().ok()?;
            let b = args[1].parse::<f32>().ok()?;
            let c = args[2].parse::<f32>().ok()?;
            let d = args[3].parse::<f32>().ok()?;
            let e = args[4].parse::<f32>().ok()?;
            let f = args[5].parse::<f32>().ok()?;
            Some(TransformOp::Matrix(a, b, c, d, e, f))
        }
        _ => None,
    }
}

/// Parses a CSS `transform-origin` value into normalized `(x, y)` percentages.
#[must_use]
pub fn parse_transform_origin(input: &str) -> (f32, f32) {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let x = tokens.first().map_or(0.5, |t| parse_origin_coord(t));
    let y = tokens.get(1).map_or(0.5, |t| parse_origin_coord(t));
    (x, y)
}

fn parse_origin_coord(token: &str) -> f32 {
    let t = token.to_ascii_lowercase();
    match t.as_str() {
        "left" | "top" => 0.0,
        "right" | "bottom" => 1.0,
        "center" => 0.5,
        _ => t
            .strip_suffix('%')
            .and_then(|p| p.parse::<f32>().ok())
            .map_or(0.5, |pct| pct / 100.0),
    }
}

/// Parses a CSS linear or radial gradient function.
#[must_use]
pub fn parse_gradient(input: &str) -> Option<Gradient> {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("linear-gradient(") {
        let inner = rest.strip_suffix(')')?.trim();
        parse_linear_gradient_body(inner)
    } else if let Some(rest) = trimmed.strip_prefix("radial-gradient(") {
        let inner = rest.strip_suffix(')')?.trim();
        parse_radial_gradient_body(inner)
    } else {
        None
    }
}

fn parse_linear_gradient_body(inner: &str) -> Option<Gradient> {
    let mut parts: Vec<&str> = split_gradient_args(inner);
    if parts.is_empty() {
        return None;
    }

    let first = parts[0].trim();
    let angle_deg = parse_linear_direction(first).map_or(180.0, |deg| {
        parts.remove(0);
        deg
    });

    let stops = parse_color_stops(&parts);
    if stops.is_empty() {
        return None;
    }

    Some(Gradient::Linear { angle_deg, stops })
}

fn parse_radial_gradient_body(inner: &str) -> Option<Gradient> {
    let parts = split_gradient_args(inner);
    let stops = parse_color_stops(&parts);
    if stops.is_empty() {
        return None;
    }

    Some(Gradient::Radial {
        center: (0.5, 0.5),
        radius: 1.0,
        stops,
    })
}

fn parse_linear_direction(s: &str) -> Option<f32> {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "to top" => Some(0.0),
        "to right" => Some(90.0),
        "to bottom" => Some(180.0),
        "to left" => Some(270.0),
        "to top right" | "to right top" => Some(45.0),
        "to bottom right" | "to right bottom" => Some(135.0),
        "to bottom left" | "to left bottom" => Some(225.0),
        "to top left" | "to left top" => Some(315.0),
        _ => lower
            .strip_suffix("deg")
            .and_then(|d| d.parse::<f32>().ok())
            .or_else(|| {
                lower
                    .strip_suffix("rad")
                    .and_then(|r| r.parse::<f32>().ok())
                    .map(f32::to_degrees)
            }),
    }
}

fn parse_color_stops(parts: &[&str]) -> Vec<ColorStop> {
    let mut raw_stops = Vec::new();
    for part in parts {
        let tokens: Vec<&str> = part.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        let color = Color::parse(tokens[0]);
        let pos = tokens.get(1).and_then(|p| {
            p.strip_suffix('%')
                .and_then(|v| v.parse::<f32>().ok())
                .map(|pct| pct / 100.0)
        });
        if let Some(c) = color {
            raw_stops.push((c, pos));
        }
    }

    if raw_stops.is_empty() {
        return Vec::new();
    }

    let n = raw_stops.len();
    let mut stops = Vec::with_capacity(n);
    for (i, (color, explicit_pos)) in raw_stops.into_iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let fallback = if n > 1 {
            i as f32 / (n - 1) as f32
        } else {
            0.0
        };
        let position = explicit_pos.unwrap_or(fallback);
        stops.push(ColorStop { position, color });
    }
    stops
}

fn split_gradient_args(s: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ',' if depth == 0 => {
                args.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < s.len() {
        let tail = s[start..].trim();
        if !tail.is_empty() {
            args.push(tail);
        }
    }
    args
}

fn parse_length_val(s: &str) -> Option<f32> {
    let trimmed = s.trim();
    trimmed
        .strip_suffix("px")
        .unwrap_or(trimmed)
        .parse::<f32>()
        .ok()
}

fn parse_angle_val(s: &str) -> Option<f32> {
    let trimmed = s.trim().to_ascii_lowercase();
    trimmed
        .strip_suffix("deg")
        .and_then(|d| d.parse::<f32>().ok())
        .map(f32::to_radians)
        .or_else(|| {
            trimmed
                .strip_suffix("rad")
                .and_then(|r| r.parse::<f32>().ok())
        })
        .or_else(|| {
            trimmed
                .strip_suffix("turn")
                .and_then(|t| t.parse::<f32>().ok())
                .map(|t| t * std::f32::consts::TAU)
        })
        .or_else(|| trimmed.parse::<f32>().ok().map(f32::to_radians))
}

/// Parses a CSS `transition` shorthand or longhand value.
#[must_use]
pub fn parse_transitions(input: &str) -> Vec<Transition> {
    let mut result = Vec::new();
    for part in input.split(',') {
        let tokens: Vec<&str> = part.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let mut property = "all".to_string();
        let mut duration_ms = 0.0f32;
        let mut timing_function = TimingFunction::Ease;
        let mut delay_ms = 0.0f32;
        let mut time_count = 0;

        for token in tokens {
            if let Some(ms) = parse_time_ms(token) {
                if time_count == 0 {
                    duration_ms = ms;
                } else {
                    delay_ms = ms;
                }
                time_count += 1;
            } else if let Some(tf) = parse_timing_function(token) {
                timing_function = tf;
            } else {
                property = token.to_string();
            }
        }

        result.push(Transition {
            property,
            duration_ms,
            timing_function,
            delay_ms,
        });
    }
    result
}

fn parse_time_ms(s: &str) -> Option<f32> {
    let lower = s.to_ascii_lowercase();
    lower
        .strip_suffix("ms")
        .and_then(|ms| ms.parse::<f32>().ok())
        .or_else(|| {
            lower
                .strip_suffix('s')
                .and_then(|sec| sec.parse::<f32>().ok())
                .map(|s| s * 1000.0)
        })
}

fn parse_timing_function(s: &str) -> Option<TimingFunction> {
    match s.to_ascii_lowercase().as_str() {
        "linear" => Some(TimingFunction::Linear),
        "ease" => Some(TimingFunction::Ease),
        "ease-in" => Some(TimingFunction::EaseIn),
        "ease-out" => Some(TimingFunction::EaseOut),
        "ease-in-out" => Some(TimingFunction::EaseInOut),
        _ => None,
    }
}
