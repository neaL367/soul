//! Layout-derived page hit-test map construction.

use dom::{Document, NodeData};
use layout::{BoxType, LayoutBox};
use soul_ui::{HitTestMap, HitTestRegion, HitTestTarget};

/// Builds link hit-test regions from a laid-out document.
#[must_use]
pub fn build_hit_test_map(doc: &Document, root: &LayoutBox) -> HitTestMap {
    let mut regions = Vec::new();
    collect_hit_test_regions(doc, root, &mut regions);
    HitTestMap { regions }
}

fn collect_hit_test_regions(
    doc: &Document,
    layout_box: &LayoutBox,
    regions: &mut Vec<HitTestRegion>,
) {
    if let BoxType::BlockNode(id) | BoxType::InlineNode(id) = layout_box.box_type
        && let Some(node) = doc.get_node(id)
        && let NodeData::Element(element) = &node.data
        && element.tag_name == "a"
        && let Some(href) = element.attr("href")
    {
        let rect = layout_box.dimensions.content;
        if rect.width > 0.0 && rect.height > 0.0 {
            regions.push(HitTestRegion {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                target: HitTestTarget::Link(href.to_string()),
            });
        }
    }

    for child in &layout_box.children {
        collect_hit_test_regions(doc, child, regions);
    }
}
