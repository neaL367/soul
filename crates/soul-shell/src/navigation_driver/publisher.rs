//! Publishing helpers for GPUI window state updates.

use crate::engine::{RenderOptions, RenderResult};
use soul_backend_gpui::SoulBackendHandle;
use soul_core::{TabId, TabManager};
use soul_ui::{HitTestMap, SoulError, TabStripModel, ViewportFrame, WindowId};
use std::collections::HashMap;

pub(super) fn publish_result(
    backend: &SoulBackendHandle,
    window_id: WindowId,
    options: RenderOptions,
    result: &RenderResult,
) {
    let frame = ViewportFrame::SoftwareRgba {
        width: options.width,
        height: options.height,
        pixels: result.pixel_buffer.data.clone(),
    };
    if let Err(error) = backend.update_page_state(
        window_id,
        frame,
        result.hit_test_map.clone(),
        result.scroll_y,
    ) {
        log_backend_error(&error);
    } else {
        tracing::info!(url = %result.url, scroll_y = result.scroll_y, "Navigation frame updated");
    }
}

pub(super) fn publish_active_result(
    backend: &SoulBackendHandle,
    window_id: WindowId,
    options: RenderOptions,
    results: &HashMap<TabId, RenderResult>,
    initial_frames: &HashMap<TabId, ViewportFrame>,
    tab_id: TabId,
) {
    if let Some(result) = results.get(&tab_id) {
        publish_result(backend, window_id, options, result);
    } else if let Some(frame) = initial_frames.get(&tab_id) {
        if let Err(error) =
            backend.update_page_state(window_id, frame.clone(), HitTestMap::default(), 0.0)
        {
            log_backend_error(&error);
        }
    } else {
        clear_active_page(backend, window_id);
    }
}

pub(super) fn clear_active_page(backend: &SoulBackendHandle, window_id: WindowId) {
    if let Err(error) = backend.clear_page_state(window_id) {
        log_backend_error(&error);
    }
}

pub(super) fn publish_tab_strip(
    backend: &SoulBackendHandle,
    window_id: WindowId,
    tabs: &TabManager,
) {
    let active_id = tabs.active_tab_id();
    let mut strip = TabStripModel::new();
    for tab in tabs.tabs() {
        strip.add_tab(tab.id, tab.title.clone(), Some(tab.id) == active_id);
        if let Some(item) = strip.tabs().last()
            && tab.controller.state().is_loading()
        {
            strip.set_loading(item.id, true);
        }
    }
    if let Err(error) = backend.update_tab_strip(window_id, strip) {
        log_backend_error(&error);
    }
}

fn log_backend_error(error: &SoulError) {
    tracing::warn!(%error, "Failed to publish navigation frame");
}
