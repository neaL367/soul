//! Display list builder converting layout boxes and stacking contexts into draw commands.

use crate::display_item::{DisplayItem, DisplayList};
use crate::stacking::{StackingContext, build_stacking_tree};
use css::{Color, FontWeight};
use layout::{BoxType, LayoutBox, Rect};

/// Builder converting a positioned layout box tree into an ordered `DisplayList`.
pub struct DisplayListBuilder;

impl DisplayListBuilder {
    /// Constructs a complete `DisplayList` from the root of a laid-out document.
    #[must_use]
    pub fn build(root_box: &LayoutBox) -> DisplayList {
        let mut list = DisplayList::new();
        list.bounds = Rect::new(
            root_box.dimensions.content.x,
            root_box.dimensions.content.y,
            root_box.dimensions.content.width,
            root_box.dimensions.content.height,
        );

        let root_stacking = build_stacking_tree(root_box);
        Self::paint_stacking_context(&mut list, &root_stacking);

        list
    }

    /// Paints a single stacking context according to the CSS 2.1 Appendix E order.
    fn paint_stacking_context(list: &mut DisplayList, context: &StackingContext) {
        let has_opacity = context.opacity < 1.0 - f32::EPSILON;
        if has_opacity {
            list.push(DisplayItem::PushOpacity {
                opacity: context.opacity,
            });
        }

        // 1. Background and borders of the root element
        Self::paint_box_background_and_borders(list, context.root);

        // 2. Child stacking contexts with negative z-index
        for child_context in &context.negative_z_children {
            Self::paint_stacking_context(list, child_context);
        }

        // 3. In-flow, non-inline-level descendant boxes
        for block in &context.in_flow_blocks {
            Self::paint_box_background_and_borders(list, block);
        }

        // 4. In-flow, inline-level descendant boxes and text
        for inline in &context.in_flow_inlines {
            Self::paint_inline_or_text(list, inline);
        }

        // 5. Child stacking contexts with zero / auto z-index
        for child_context in &context.zero_z_children {
            Self::paint_stacking_context(list, child_context);
        }

        // 6. Child stacking contexts with positive z-index
        for child_context in &context.positive_z_children {
            Self::paint_stacking_context(list, child_context);
        }

        if has_opacity {
            list.push(DisplayItem::PopOpacity);
        }
    }

    fn paint_box_background_and_borders(list: &mut DisplayList, layout_box: &LayoutBox) {
        let Some(ref style) = layout_box.style else {
            return;
        };

        // Paint background rectangle
        if style.background_color != Color::TRANSPARENT {
            list.push(DisplayItem::DrawRect {
                rect: layout_box.dimensions.padding_box(),
                color: style.background_color,
            });
        }

        // Paint borders if non-zero
        let border_widths = layout_box.dimensions.border;
        if border_widths.horizontal_total() > 0.0 || border_widths.vertical_total() > 0.0 {
            list.push(DisplayItem::DrawBorder {
                rect: layout_box.dimensions.border_box(),
                widths: border_widths,
                color: style.color,
            });
        }
    }

    fn paint_inline_or_text(list: &mut DisplayList, layout_box: &LayoutBox) {
        match &layout_box.box_type {
            BoxType::TextNode(_, text) => {
                let default_color = Color::BLACK;
                let color = layout_box.style.as_ref().map_or(default_color, |s| s.color);
                let font_size = layout_box.style.as_ref().map_or(16.0, |s| s.font_size);
                let font_family = layout_box
                    .style
                    .as_ref()
                    .map_or_else(|| "sans-serif".to_string(), |s| s.font_family.clone());
                let is_bold = layout_box
                    .style
                    .as_ref()
                    .is_some_and(|s| s.font_weight == FontWeight::Bold);

                list.push(DisplayItem::DrawText {
                    rect: layout_box.dimensions.content,
                    text: text.clone(),
                    color,
                    font_size,
                    font_family,
                    is_bold,
                });
            }
            BoxType::InlineNode(_) => {
                Self::paint_box_background_and_borders(list, layout_box);
            }
            _ => {}
        }
    }
}
