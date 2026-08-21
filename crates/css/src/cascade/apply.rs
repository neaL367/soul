//! CSS property declaration parsing and application to `ComputedStyle`.

pub mod shadow;
pub mod transform;
pub mod values;
pub mod var;

pub use var::resolve_var_references;

use self::shadow::parse_box_shadows;
use self::transform::{
    parse_gradient, parse_transform_list, parse_transform_origin, parse_transitions,
};
use self::values::{
    apply_border_shorthand, parse_4_edges, parse_4_edges_non_negative, parse_font_family,
    parse_grid_tracks, parse_length, parse_non_negative_length, parse_non_negative_px, parse_px,
};
use crate::properties::{
    AlignItems, AlignSelf, BoxSizing, Color, ComputedStyle, Display, FlexDirection, FlexWrap,
    FontStyle, FontWeight, GridTrack, JustifyContent, Position, TextAlign, TextDecoration,
    Visibility,
};
use crate::rule::Declaration;

/// Applies a parsed CSS `Declaration` to a `ComputedStyle`.
#[allow(clippy::too_many_lines)]
pub fn apply_declaration(style: &mut ComputedStyle, decl: &Declaration) {
    if decl.property.starts_with("--") {
        let resolved_value = resolve_var_references(&decl.value, &style.custom_properties);
        style
            .custom_properties
            .insert(decl.property.clone(), resolved_value);
        return;
    }

    let resolved_val = if decl.value.contains("var(") {
        resolve_var_references(&decl.value, &style.custom_properties)
    } else {
        decl.value.clone()
    };
    let value = resolved_val.as_str();

    match decl.property.as_str() {
        "display" => match value {
            "block" => style.display = Display::Block,
            "inline" => style.display = Display::Inline,
            "inline-block" => style.display = Display::InlineBlock,
            "flex" => style.display = Display::Flex,
            "grid" => style.display = Display::Grid,
            "none" => style.display = Display::None,
            _ => {}
        },
        "position" => match value {
            "static" => style.position = Position::Static,
            "relative" => style.position = Position::Relative,
            "absolute" => style.position = Position::Absolute,
            "fixed" => style.position = Position::Fixed,
            "sticky" => style.position = Position::Sticky,
            _ => {}
        },
        "visibility" => match value {
            "visible" => style.visibility = Visibility::Visible,
            "hidden" => style.visibility = Visibility::Hidden,
            "collapse" => style.visibility = Visibility::Collapse,
            _ => {}
        },
        "box-sizing" => match value {
            "border-box" => style.box_sizing = BoxSizing::BorderBox,
            "content-box" => style.box_sizing = BoxSizing::ContentBox,
            _ => {}
        },
        "color" => {
            if let Some(c) = Color::parse(value) {
                style.color = c;
            }
        }
        "background-color" | "background" => {
            if let Some(g) = parse_gradient(value) {
                style.background_gradient = Some(g);
            } else if let Some(c) = Color::parse(value) {
                style.background_color = c;
            }
        }
        "background-image" => {
            if let Some(g) = parse_gradient(value) {
                style.background_gradient = Some(g);
            }
        }
        "opacity" => {
            if let Ok(val) = value.parse::<f32>()
                && val.is_finite()
            {
                style.opacity = val.clamp(0.0, 1.0);
            }
        }
        "font-size" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.font_size = px;
            }
        }
        "font-family" => {
            if let Some(family) = parse_font_family(value) {
                style.font_family = family;
            }
        }
        "letter-spacing" => {
            if let Some(px) = parse_px(value) {
                style.letter_spacing = px;
            }
        }
        "word-spacing" => {
            if let Some(px) = parse_px(value) {
                style.word_spacing = px;
            }
        }
        "font-weight" => match value {
            "bold" | "700" => style.font_weight = FontWeight::Bold,
            "normal" | "400" => style.font_weight = FontWeight::Normal,
            _ => {
                if let Ok(w) = value.parse::<u16>()
                    && (1..=1000).contains(&w)
                {
                    style.font_weight = FontWeight::Number(w);
                }
            }
        },
        "font-style" => match value {
            "italic" => style.font_style = FontStyle::Italic,
            "oblique" => style.font_style = FontStyle::Oblique,
            "normal" => style.font_style = FontStyle::Normal,
            _ => {}
        },
        "text-decoration" => match value {
            "underline" => style.text_decoration = TextDecoration::Underline,
            "line-through" => style.text_decoration = TextDecoration::LineThrough,
            "none" => style.text_decoration = TextDecoration::None,
            _ => {}
        },
        "text-align" => match value {
            "left" => style.text_align = TextAlign::Left,
            "right" => style.text_align = TextAlign::Right,
            "center" => style.text_align = TextAlign::Center,
            "justify" => style.text_align = TextAlign::Justify,
            _ => {}
        },
        "line-height" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.line_height = Some(px);
            } else if let Ok(factor) = value.trim().parse::<f32>()
                && factor.is_finite()
                && factor > 0.0
            {
                style.line_height = Some(style.font_size * factor);
            }
        }
        "margin" => {
            if let Some((t, r, b, l)) = parse_4_edges(value) {
                style.margin_top = t;
                style.margin_right = r;
                style.margin_bottom = b;
                style.margin_left = l;
            }
        }
        "margin-top" => {
            if let Some(px) = parse_px(value) {
                style.margin_top = px;
            }
        }
        "margin-bottom" => {
            if let Some(px) = parse_px(value) {
                style.margin_bottom = px;
            }
        }
        "margin-left" => {
            if let Some(px) = parse_px(value) {
                style.margin_left = px;
            }
        }
        "margin-right" => {
            if let Some(px) = parse_px(value) {
                style.margin_right = px;
            }
        }
        "padding" => {
            if let Some((t, r, b, l)) = parse_4_edges_non_negative(value) {
                style.padding_top = t;
                style.padding_right = r;
                style.padding_bottom = b;
                style.padding_left = l;
            }
        }
        "padding-top" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.padding_top = px;
            }
        }
        "padding-bottom" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.padding_bottom = px;
            }
        }
        "padding-left" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.padding_left = px;
            }
        }
        "padding-right" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.padding_right = px;
            }
        }
        "border" => {
            apply_border_shorthand(style, value);
        }
        "border-width" => {
            if let Some((t, r, b, l)) = parse_4_edges_non_negative(value) {
                style.border_top_width = t;
                style.border_right_width = r;
                style.border_bottom_width = b;
                style.border_left_width = l;
            }
        }
        "border-color" => {
            if let Some(c) = Color::parse(value) {
                style.border_top_color = c;
                style.border_right_color = c;
                style.border_bottom_color = c;
                style.border_left_color = c;
            }
        }
        "border-radius" => {
            if let Some((tl, tr, br, bl)) = parse_4_edges_non_negative(value) {
                style.border_radius_top_left = tl;
                style.border_radius_top_right = tr;
                style.border_radius_bottom_right = br;
                style.border_radius_bottom_left = bl;
            }
        }
        "border-top-width" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.border_top_width = px;
            }
        }
        "border-right-width" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.border_right_width = px;
            }
        }
        "border-bottom-width" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.border_bottom_width = px;
            }
        }
        "border-left-width" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.border_left_width = px;
            }
        }
        "z-index" => {
            if let Ok(z) = value.trim().parse::<i32>() {
                style.z_index = Some(z);
            }
        }
        "width" => {
            if let Some(len) = parse_non_negative_length(value) {
                style.width = len;
            }
        }
        "height" => {
            if let Some(len) = parse_non_negative_length(value) {
                style.height = len;
            }
        }
        "top" => {
            if let Some(len) = parse_length(value) {
                style.top = len;
            }
        }
        "right" => {
            if let Some(len) = parse_length(value) {
                style.right = len;
            }
        }
        "bottom" => {
            if let Some(len) = parse_length(value) {
                style.bottom = len;
            }
        }
        "left" => {
            if let Some(len) = parse_length(value) {
                style.left = len;
            }
        }
        "flex-direction" => match value {
            "row" => style.flex_direction = FlexDirection::Row,
            "row-reverse" => style.flex_direction = FlexDirection::RowReverse,
            "column" => style.flex_direction = FlexDirection::Column,
            "column-reverse" => style.flex_direction = FlexDirection::ColumnReverse,
            _ => {}
        },
        "flex-wrap" => match value {
            "nowrap" => style.flex_wrap = FlexWrap::NoWrap,
            "wrap" => style.flex_wrap = FlexWrap::Wrap,
            "wrap-reverse" => style.flex_wrap = FlexWrap::WrapReverse,
            _ => {}
        },
        "justify-content" => match value {
            "flex-start" => style.justify_content = JustifyContent::FlexStart,
            "flex-end" => style.justify_content = JustifyContent::FlexEnd,
            "center" => style.justify_content = JustifyContent::Center,
            "space-between" => style.justify_content = JustifyContent::SpaceBetween,
            "space-around" => style.justify_content = JustifyContent::SpaceAround,
            "space-evenly" => style.justify_content = JustifyContent::SpaceEvenly,
            _ => {}
        },
        "align-items" => match value {
            "stretch" => style.align_items = AlignItems::Stretch,
            "flex-start" => style.align_items = AlignItems::FlexStart,
            "flex-end" => style.align_items = AlignItems::FlexEnd,
            "center" => style.align_items = AlignItems::Center,
            "baseline" => style.align_items = AlignItems::Baseline,
            _ => {}
        },
        "align-self" => match value {
            "auto" => style.align_self = AlignSelf::Auto,
            "stretch" => style.align_self = AlignSelf::Stretch,
            "flex-start" => style.align_self = AlignSelf::FlexStart,
            "flex-end" => style.align_self = AlignSelf::FlexEnd,
            "center" => style.align_self = AlignSelf::Center,
            "baseline" => style.align_self = AlignSelf::Baseline,
            _ => {}
        },
        "flex-grow" => {
            if let Ok(v) = value.trim().parse::<f32>()
                && v.is_finite()
            {
                style.flex_grow = v.max(0.0);
            }
        }
        "flex-shrink" => {
            if let Ok(v) = value.trim().parse::<f32>()
                && v.is_finite()
            {
                style.flex_shrink = v.max(0.0);
            }
        }
        "flex-basis" => {
            if let Some(len) = parse_non_negative_length(value) {
                style.flex_basis = len;
            }
        }
        "grid-template-columns" => {
            if let Some(tracks) = parse_grid_tracks(value) {
                style.grid_template_columns = tracks;
            }
        }
        "grid-template-rows" => {
            if let Some(tracks) = parse_grid_tracks(value) {
                style.grid_template_rows = tracks;
            }
        }
        "gap" | "grid-gap" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.grid_gap = px;
            } else if let Some(GridTrack::Px(px)) =
                parse_grid_tracks(value).and_then(|t| t.into_iter().next())
            {
                style.grid_gap = px;
            }
        }
        "row-gap" | "grid-row-gap" | "column-gap" | "grid-column-gap" => {
            if let Some(px) = parse_non_negative_px(value) {
                style.grid_gap = px;
            }
        }
        "box-shadow" => {
            if let Some(shadows) = parse_box_shadows(value) {
                style.box_shadow = shadows;
            }
        }
        "transform" => {
            style.transform = parse_transform_list(value);
        }
        "transform-origin" => {
            style.transform_origin = parse_transform_origin(value);
        }
        "transition" | "transition-property" | "transition-duration" => {
            style.transition_properties = parse_transitions(value);
        }
        "content" => {
            let trimmed = value.trim();
            if trimmed == "none" || trimmed == "normal" {
                style.content = None;
            } else {
                let unquoted = trimmed.trim_matches('"').trim_matches('\'');
                style.content = Some(unquoted.to_string());
            }
        }
        _ => {}
    }
}
