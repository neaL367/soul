//! Individual rendering pipeline execution stages: subresource fetching, layout, paint, and rasterization.

use super::pipeline_types::{PipelineTimings, RenderOptions, crop_viewport};
use crate::hit_testing::build_hit_test_map;
use dom::{Document, NodeData, NodeId};
use image_decode::{DecodedImage, ImageDecoder};
use layout::{
    A11yNode, Dimensions, IntrinsicSize, Rect, build_box_tree_with_intrinsics, layout_block,
};
use networking::{HttpClient, HttpRequest};
use paint::DisplayListBuilder;
use raster::{CpuRasterizer, PixelBuffer};
use soul_core::NavigationError;
use soul_ui::HitTestMap;
use std::collections::HashMap;
use std::time::Instant;
use url::Url;

pub(super) fn document_title(doc: &Document, url: &Url) -> String {
    let title = doc
        .get_elements_by_tag_name("title")
        .first()
        .map(|id| doc.text_content(*id).trim().to_string())
        .filter(|title| !title.is_empty());
    title
        .or_else(|| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "New Tab".to_string())
}

/// Fetches and decodes every `<img>` subresource through the security-checked
/// client path (mixed content + CORS enforced against the document origin).
/// Individual failures are non-fatal: the image is skipped and logged.
pub(super) async fn load_subresource_images(
    client: &HttpClient,
    document_url: &Url,
    doc: &Document,
) -> HashMap<NodeId, DecodedImage> {
    let mut images = HashMap::new();

    for img_id in doc.get_elements_by_tag_name("img") {
        let Some(node) = doc.get_node(img_id) else {
            continue;
        };
        let NodeData::Element(element) = &node.data else {
            continue;
        };
        let Some(src) = element.attr("src") else {
            continue;
        };
        let Ok(url) = document_url.join(src) else {
            tracing::warn!(src, "Skipping image with unresolvable src");
            continue;
        };

        let request = HttpRequest::get(url.clone());
        match client
            .fetch_with_security_context(&request, Some(document_url))
            .await
        {
            Ok(response) => {
                let decoded = if response.mime_type.contains("svg") {
                    ImageDecoder::decode_svg(&response.body, 0, 0)
                } else {
                    ImageDecoder::decode_raster(&response.body)
                };
                match decoded {
                    Ok(image) => {
                        tracing::debug!(
                            url = %url,
                            width = image.width,
                            height = image.height,
                            "Decoded image"
                        );
                        images.insert(img_id, image);
                    }
                    Err(err) => {
                        tracing::warn!(url = %url, %err, "Skipping undecodable image");
                    }
                }
            }
            Err(err) => {
                tracing::warn!(url = %url, %err, "Blocked image subresource (CORS/mixed content)");
            }
        }
    }

    images
}

/// Shared layout → paint → raster core with accessibility extraction.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::type_complexity
)]
pub(super) fn layout_paint_raster(
    doc: &Document,
    styles: &HashMap<NodeId, css::ComputedStyle>,
    images: &HashMap<NodeId, DecodedImage>,
    options: RenderOptions,
) -> Result<
    (
        PixelBuffer,
        PixelBuffer,
        f32,
        Option<A11yNode>,
        HitTestMap,
        PipelineTimings,
    ),
    NavigationError,
> {
    let mut timings = PipelineTimings::default();

    let layout_start = Instant::now();
    let intrinsics: HashMap<NodeId, IntrinsicSize> = images
        .iter()
        .map(|(id, img)| (*id, IntrinsicSize::new(img.width, img.height)))
        .collect();
    let mut layout_box = build_box_tree_with_intrinsics(doc, doc.root_id(), styles, &intrinsics)
        .ok_or_else(|| {
            NavigationError::Other("box tree construction failed for document".to_string())
        })?;
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, options.width as f32, options.height as f32),
        ..Default::default()
    };
    layout_block(&mut layout_box, &viewport);
    timings.layout = layout_start.elapsed();

    let paint_start = Instant::now();
    let display_list = DisplayListBuilder::build(&layout_box, images);
    timings.paint = paint_start.elapsed();

    let raster_start = Instant::now();
    let document_height = (layout_box.dimensions.content.y + layout_box.dimensions.content.height)
        .ceil()
        .max(options.height as f32);
    let document_buffer =
        CpuRasterizer::rasterize(&display_list, options.width, document_height as u32)
            .map_err(|e| NavigationError::Other(format!("rasterization failed: {e}")))?;
    let pixel_buffer = crop_viewport(&document_buffer, options.height, 0.0);
    timings.raster = raster_start.elapsed();

    let a11y_tree = A11yNode::from_layout_box(doc, &layout_box);
    let hit_test_map = build_hit_test_map(doc, &layout_box);

    Ok((
        pixel_buffer,
        document_buffer,
        document_height,
        a11y_tree,
        hit_test_map,
        timings,
    ))
}
