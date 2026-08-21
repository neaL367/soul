//! Display list builder converting layout boxes and stacking contexts into draw commands.

use crate::display_item::{DisplayItem, DisplayList};
use crate::stacking::{StackingContext, build_stacking_tree};
use css::{Color, FontWeight};
use dom::NodeId;
use image_decode::DecodedImage;
use layout::{BoxType, LayoutBox, Rect};
use std::collections::HashMap;

/// Builder converting a positioned layout box tree into an ordered `DisplayList`.
pub struct DisplayListBuilder;

impl DisplayListBuilder {
    /// Constructs a complete `DisplayList` from the root of a laid-out document.
    ///
    /// `images` maps DOM node ids of `<img>` elements to their decoded bitmaps;
    /// absent entries render as empty boxes.
    #[must_use]
    pub fn build(root_box: &LayoutBox, images: &HashMap<NodeId, DecodedImage>) -> DisplayList {
        let mut list = DisplayList::new();
        if root_box.dimensions.content.is_finite() {
            list.bounds = Rect::new(
                root_box.dimensions.content.x,
                root_box.dimensions.content.y,
                root_box.dimensions.content.width,
                root_box.dimensions.content.height,
            );
        }

        let root_stacking = build_stacking_tree(root_box);
        Self::paint_stacking_context(&mut list, &root_stacking, images);

        list
    }

    /// Paints a single stacking context according to the CSS 2.1 Appendix E order.
    fn paint_stacking_context(
        list: &mut DisplayList,
        context: &StackingContext,
        images: &HashMap<NodeId, DecodedImage>,
    ) {
        let has_opacity = context.opacity.is_finite() && context.opacity < 1.0 - f32::EPSILON;
        if has_opacity {
            list.push(DisplayItem::PushOpacity {
                opacity: context.opacity,
            });
        }

        // 1. Background and borders of the root element
        Self::paint_box_background_and_borders(list, context.root, images);

        // 2. Child stacking contexts with negative z-index
        for child_context in &context.negative_z_children {
            Self::paint_stacking_context(list, child_context, images);
        }

        // 3. In-flow, non-inline-level descendant boxes
        for block in &context.in_flow_blocks {
            Self::paint_box_background_and_borders(list, block, images);
        }

        // 4. In-flow, inline-level descendant boxes and text
        for inline in &context.in_flow_inlines {
            Self::paint_inline_or_text(list, inline, images);
        }

        // 5. Child stacking contexts with zero / auto z-index
        for child_context in &context.zero_z_children {
            Self::paint_stacking_context(list, child_context, images);
        }

        // 6. Child stacking contexts with positive z-index
        for child_context in &context.positive_z_children {
            Self::paint_stacking_context(list, child_context, images);
        }

        if has_opacity {
            list.push(DisplayItem::PopOpacity);
        }
    }

    fn paint_box_background_and_borders(
        list: &mut DisplayList,
        layout_box: &LayoutBox,
        images: &HashMap<NodeId, DecodedImage>,
    ) {
        let Some(ref style) = layout_box.style else {
            return;
        };

        // Paint box shadows (behind background)
        if !style.box_shadow.is_empty() && layout_box.dimensions.border_box().is_finite() {
            list.push(DisplayItem::DrawBoxShadow {
                rect: layout_box.dimensions.border_box(),
                shadows: style.box_shadow.clone(),
            });
        }

        // Paint background rectangle
        if style.background_color != Color::TRANSPARENT
            && layout_box.dimensions.padding_box().is_finite()
        {
            list.push(DisplayItem::DrawRect {
                rect: layout_box.dimensions.padding_box(),
                color: style.background_color,
            });
        }

        // Paint borders if non-zero
        let border_widths = layout_box.dimensions.border;
        if (border_widths.horizontal_total() > 0.0 || border_widths.vertical_total() > 0.0)
            && layout_box.dimensions.border_box().is_finite()
        {
            list.push(DisplayItem::DrawBorder {
                rect: layout_box.dimensions.border_box(),
                widths: border_widths,
                color: style.color,
            });
        }

        // Paint decoded `<img>` content if available for this element.
        if let BoxType::BlockNode(id) | BoxType::InlineNode(id) = layout_box.box_type
            && let Some(image) = images.get(&id)
            && layout_box.dimensions.content.is_finite()
        {
            list.push(DisplayItem::DrawImage {
                rect: layout_box.dimensions.content,
                width: image.width,
                height: image.height,
                pixels: image.rgba_pixels.clone(),
            });
        }
    }

    fn paint_inline_or_text(
        list: &mut DisplayList,
        layout_box: &LayoutBox,
        images: &HashMap<NodeId, DecodedImage>,
    ) {
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

                if layout_box.dimensions.content.is_finite() && font_size.is_finite() {
                    list.push(DisplayItem::DrawText {
                        rect: layout_box.dimensions.content,
                        text: text.clone(),
                        color,
                        font_size,
                        font_family,
                        is_bold,
                    });
                }
            }
            BoxType::InlineNode(_) => {
                Self::paint_box_background_and_borders(list, layout_box, images);
            }
            _ => {}
        }
    }
}
