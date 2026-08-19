//! CSS property declaration parsing and application to `ComputedStyle`.

use crate::properties::{
    AlignItems, AlignSelf, BoxSizing, Color, ComputedStyle, Display, FlexDirection, FlexWrap,
    FontStyle, FontWeight, JustifyContent, Length, Position, TextAlign, TextDecoration,
};
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
        "box-sizing" => match decl.value.as_str() {
            "border-box" => style.box_sizing = BoxSizing::BorderBox,
            "content-box" => style.box_sizing = BoxSizing::ContentBox,
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
            if let Ok(val) = decl.value.parse::<f32>()
                && val.is_finite()
            {
                style.opacity = val.clamp(0.0, 1.0);
            }
        }
        "font-size" => {
            if let Some(px) = parse_px(&decl.value) {
                style.font_size = px;
            }
        }
        "font-family" => {
            if let Some(family) = parse_font_family(&decl.value) {
                style.font_family = family;
            }
        }
        "letter-spacing" => {
            if let Some(px) = parse_px(&decl.value) {
                style.letter_spacing = px;
            }
        }
        "word-spacing" => {
            if let Some(px) = parse_px(&decl.value) {
                style.word_spacing = px;
            }
        }
        "font-weight" => match decl.value.as_str() {
            "bold" | "700" => style.font_weight = FontWeight::Bold,
            "normal" | "400" => style.font_weight = FontWeight::Normal,
            _ => {
                if let Ok(w) = decl.value.parse::<u16>()
                    && (1..=1000).contains(&w)
                {
                    style.font_weight = FontWeight::Number(w);
                }
            }
        },
        "font-style" => match decl.value.as_str() {
            "italic" => style.font_style = FontStyle::Italic,
            "oblique" => style.font_style = FontStyle::Oblique,
            "normal" => style.font_style = FontStyle::Normal,
            _ => {}
        },
        "text-decoration" => match decl.value.as_str() {
            "underline" => style.text_decoration = TextDecoration::Underline,
            "line-through" => style.text_decoration = TextDecoration::LineThrough,
            "none" => style.text_decoration = TextDecoration::None,
            _ => {}
        },
        "text-align" => match decl.value.as_str() {
            "left" => style.text_align = TextAlign::Left,
            "right" => style.text_align = TextAlign::Right,
            "center" => style.text_align = TextAlign::Center,
            "justify" => style.text_align = TextAlign::Justify,
            _ => {}
        },
        "line-height" => {
            if let Some(px) = parse_px(&decl.value) {
                style.line_height = Some(px);
            } else if let Ok(factor) = decl.value.trim().parse::<f32>()
                && factor.is_finite()
                && factor > 0.0
            {
                style.line_height = Some(style.font_size * factor);
            }
        }
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
        "border" => {
            apply_border_shorthand(style, &decl.value);
        }
        "border-width" => {
            if let Some((t, r, b, l)) = parse_4_edges(&decl.value) {
                style.border_top_width = t;
                style.border_right_width = r;
                style.border_bottom_width = b;
                style.border_left_width = l;
            }
        }
        "border-color" => {
            if let Some(c) = Color::parse(&decl.value) {
                style.border_top_color = c;
                style.border_right_color = c;
                style.border_bottom_color = c;
                style.border_left_color = c;
            }
        }
        "border-radius" => {
            if let Some((tl, tr, br, bl)) = parse_4_edges(&decl.value) {
                style.border_radius_top_left = tl;
                style.border_radius_top_right = tr;
                style.border_radius_bottom_right = br;
                style.border_radius_bottom_left = bl;
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
        "flex-direction" => match decl.value.as_str() {
            "row" => style.flex_direction = FlexDirection::Row,
            "row-reverse" => style.flex_direction = FlexDirection::RowReverse,
            "column" => style.flex_direction = FlexDirection::Column,
            "column-reverse" => style.flex_direction = FlexDirection::ColumnReverse,
            _ => {}
        },
        "flex-wrap" => match decl.value.as_str() {
            "nowrap" => style.flex_wrap = FlexWrap::NoWrap,
            "wrap" => style.flex_wrap = FlexWrap::Wrap,
            "wrap-reverse" => style.flex_wrap = FlexWrap::WrapReverse,
            _ => {}
        },
        "justify-content" => match decl.value.as_str() {
            "flex-start" => style.justify_content = JustifyContent::FlexStart,
            "flex-end" => style.justify_content = JustifyContent::FlexEnd,
            "center" => style.justify_content = JustifyContent::Center,
            "space-between" => style.justify_content = JustifyContent::SpaceBetween,
            "space-around" => style.justify_content = JustifyContent::SpaceAround,
            "space-evenly" => style.justify_content = JustifyContent::SpaceEvenly,
            _ => {}
        },
        "align-items" => match decl.value.as_str() {
            "stretch" => style.align_items = AlignItems::Stretch,
            "flex-start" => style.align_items = AlignItems::FlexStart,
            "flex-end" => style.align_items = AlignItems::FlexEnd,
            "center" => style.align_items = AlignItems::Center,
            "baseline" => style.align_items = AlignItems::Baseline,
            _ => {}
        },
        "align-self" => match decl.value.as_str() {
            "auto" => style.align_self = AlignSelf::Auto,
            "stretch" => style.align_self = AlignSelf::Stretch,
            "flex-start" => style.align_self = AlignSelf::FlexStart,
            "flex-end" => style.align_self = AlignSelf::FlexEnd,
            "center" => style.align_self = AlignSelf::Center,
            "baseline" => style.align_self = AlignSelf::Baseline,
            _ => {}
        },
        "flex-grow" => {
            if let Ok(v) = decl.value.trim().parse::<f32>()
                && v.is_finite()
            {
                style.flex_grow = v.max(0.0);
            }
        }
        "flex-shrink" => {
            if let Ok(v) = decl.value.trim().parse::<f32>()
                && v.is_finite()
            {
                style.flex_shrink = v.max(0.0);
            }
        }
        "flex-basis" => {
            if let Some(px) = parse_px(&decl.value) {
                style.flex_basis = Length::Px(px);
            } else if let Some(pct) = parse_percent(&decl.value) {
                style.flex_basis = Length::Percent(pct);
            }
        }
        _ => {}
    }
}

fn parse_font_family(value: &str) -> Option<String> {
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

fn apply_border_shorthand(style: &mut ComputedStyle, value: &str) {
    for part in value.split_whitespace() {
        if let Some(px) = parse_px(part) {
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

fn parse_percent(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    let num = trimmed.strip_suffix('%')?.trim().parse::<f32>().ok()?;
    if num.is_finite() { Some(num) } else { None }
}

fn parse_px(value: &str) -> Option<f32> {
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
