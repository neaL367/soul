//! CSS property declaration parsing and application to `ComputedStyle`.

use crate::properties::{Color, ComputedStyle, Display, FontWeight, Length, Position, TextAlign};
use crate::rule::Declaration;

#[allow(clippy::too_many_lines)]
pub(super) fn apply_declaration(style: &mut ComputedStyle, decl: &Declaration) {
    match decl.property.as_str() {
        "display" => match decl.value.as_str() {
            "block" => style.display = Display::Block,
            "inline" => style.display = Display::Inline,
            "inline-block" => style.display = Display::InlineBlock,
            "flex" => style.display = Display::Flex,
            "grid" => style.display = Display::Grid,
            "none" => style.display = Display::None,
            _ => {}
        },
        "position" => match decl.value.as_str() {
            "static" => style.position = Position::Static,
            "relative" => style.position = Position::Relative,
            "absolute" => style.position = Position::Absolute,
            "fixed" => style.position = Position::Fixed,
            "sticky" => style.position = Position::Sticky,
            _ => {}
        },
        "color" => {
            if let Some(c) = Color::parse(&decl.value) {
                style.color = c;
            }
        }
        "background-color" | "background" => {
            if let Some(c) = Color::parse(&decl.value) {
                style.background_color = c;
            }
        }
        "opacity" => {
            if let Ok(val) = decl.value.parse::<f32>() {
                style.opacity = val.clamp(0.0, 1.0);
            }
        }
        "font-size" => {
            if let Some(px) = parse_px(&decl.value) {
                style.font_size = px;
            }
        }
        "font-weight" => match decl.value.as_str() {
            "bold" | "700" => style.font_weight = FontWeight::Bold,
            "normal" | "400" => style.font_weight = FontWeight::Normal,
            _ => {}
        },
        "text-align" => match decl.value.as_str() {
            "left" => style.text_align = TextAlign::Left,
            "right" => style.text_align = TextAlign::Right,
            "center" => style.text_align = TextAlign::Center,
            "justify" => style.text_align = TextAlign::Justify,
            _ => {}
        },
        "margin" => {
            if let Some((t, r, b, l)) = parse_4_edges(&decl.value) {
                style.margin_top = t;
                style.margin_right = r;
                style.margin_bottom = b;
                style.margin_left = l;
            }
        }
        "margin-top" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_top = px;
            }
        }
        "margin-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_bottom = px;
            }
        }
        "margin-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_left = px;
            }
        }
        "margin-right" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin_right = px;
            }
        }
        "padding" => {
            if let Some((t, r, b, l)) = parse_4_edges(&decl.value) {
                style.padding_top = t;
                style.padding_right = r;
                style.padding_bottom = b;
                style.padding_left = l;
            }
        }
        "padding-top" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_top = px;
            }
        }
        "padding-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_bottom = px;
            }
        }
        "padding-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_left = px;
            }
        }
        "padding-right" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding_right = px;
            }
        }
        "border-width" | "border" => {
            if let Some((t, r, b, l)) = parse_4_edges(&decl.value) {
                style.border_top_width = t;
                style.border_right_width = r;
                style.border_bottom_width = b;
                style.border_left_width = l;
            }
        }
        "border-top-width" => {
            if let Some(px) = parse_px(&decl.value) {
                style.border_top_width = px;
            }
        }
        "border-right-width" => {
            if let Some(px) = parse_px(&decl.value) {
                style.border_right_width = px;
            }
        }
        "border-bottom-width" => {
            if let Some(px) = parse_px(&decl.value) {
                style.border_bottom_width = px;
            }
        }
        "border-left-width" => {
            if let Some(px) = parse_px(&decl.value) {
                style.border_left_width = px;
            }
        }
        "z-index" => {
            if let Ok(z) = decl.value.trim().parse::<i32>() {
                style.z_index = Some(z);
            }
        }
        "width" => {
            if let Some(px) = parse_px(&decl.value) {
                style.width = Length::Px(px);
            } else if let Some(pct) = parse_percent(&decl.value) {
                style.width = Length::Percent(pct);
            }
        }
        "height" => {
            if let Some(px) = parse_px(&decl.value) {
                style.height = Length::Px(px);
            } else if let Some(pct) = parse_percent(&decl.value) {
                style.height = Length::Percent(pct);
            }
        }
        _ => {}
    }
}

fn parse_percent(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    trimmed
        .strip_suffix('%')
        .and_then(|num| num.trim().parse::<f32>().ok())
}

fn parse_px(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    trimmed.strip_suffix("px").map_or_else(
        || trimmed.parse::<f32>().ok(),
        |num| num.trim().parse::<f32>().ok(),
    )
}

fn parse_4_edges(value: &str) -> Option<(f32, f32, f32, f32)> {
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
