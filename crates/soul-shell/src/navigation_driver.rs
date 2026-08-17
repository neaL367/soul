//! Single-owner navigation command driver for the live Soul window.

use crate::engine::{RenderOptions, RenderResult, render_active_navigation};
use crate::local_page::render_new_tab_frame;
use soul_backend_gpui::SoulBackendHandle;
use soul_core::{NavigationController, PageScrollState, TabId, TabManager};
use soul_ui::{HitTestMap, SoulError, TabStripModel, ViewportFrame, WindowId};
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::thread;
use tokio::runtime::Runtime;

/// Commands accepted from the Soul toolbar and omnibox.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationCommand {
    /// Navigate to user-entered URL or search text.
    Navigate(String),
    /// Traverse one entry backward.
    Back,
    /// Traverse one entry forward.
    Forward,
    /// Re-fetch current URL.
    Reload,
    /// Scroll the active page without refetching it.
    Scroll {
        /// Vertical document-space delta in logical pixels.
        delta_y: f32,
    },
    /// Resize the active viewport dimensions.
    Resize {
        /// New window/viewport width.
        width: u32,
        /// New window/viewport height.
        height: u32,
    },
    /// Open a new blank tab and make it active.
    NewTab,
    /// Select a tab by its current tab-strip index.
    SelectTab {
        /// Zero-based tab-strip index.
        tab_index: usize,
    },
    /// Close a tab by its current tab-strip index.
    CloseTab {
        /// Zero-based tab-strip index.
        tab_index: usize,
    },
}

/// Handle for sending navigation commands to one controller-owning worker.
#[derive(Clone)]
pub struct NavigationDriver {
    sender: Sender<NavigationCommand>,
}

impl NavigationDriver {
    /// Starts a driver thread owning navigation state and a Tokio runtime.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::too_many_lines)]
    pub fn spawn(
        backend: SoulBackendHandle,
        window_id: WindowId,
        mut options: RenderOptions,
        initial_frame: Option<ViewportFrame>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let Ok(runtime) = Runtime::new() else {
                tracing::error!("Failed to create navigation runtime");
                return;
            };
            let mut tabs = TabManager::new();
            tabs.create_tab();
            let mut results: HashMap<TabId, RenderResult> = HashMap::new();
            let mut initial_frames = HashMap::new();
            if let Some(frame) = initial_frame {
                initial_frames.insert(TabId(1), frame);
            } else if let Ok(frame) = render_new_tab_frame(options) {
                initial_frames.insert(TabId(1), frame);
            }
            publish_tab_strip(&backend, window_id, &tabs);
            if let Some(active_id) = tabs.active_tab_id() {
                publish_active_result(
                    &backend,
                    window_id,
                    options,
                    &results,
                    &initial_frames,
                    active_id,
                );
            }

            while let Ok(command) = receiver.recv() {
                match command {
                    NavigationCommand::NewTab => {
                        let tab_id = tabs.create_tab();
                        match render_new_tab_frame(options) {
                            Ok(frame) => {
                                initial_frames.insert(tab_id, frame);
                            }
                            Err(error) => {
                                tracing::warn!(%error, "Failed to render new tab page");
                            }
                        }
                        publish_tab_strip(&backend, window_id, &tabs);
                        publish_active_result(
                            &backend,
                            window_id,
                            options,
                            &results,
                            &initial_frames,
                            tab_id,
                        );
                    }
                    NavigationCommand::SelectTab { tab_index } => {
                        if let Some(tab) = tabs.tabs().get(tab_index) {
                            let tab_id = tab.id;
                            if tabs.select_tab(tab_id) {
                                publish_tab_strip(&backend, window_id, &tabs);
                                publish_active_result(
                                    &backend,
                                    window_id,
                                    options,
                                    &results,
                                    &initial_frames,
                                    tab_id,
                                );
                            }
                        }
                    }
                    NavigationCommand::CloseTab { tab_index } => {
                        if let Some(tab) = tabs.tabs().get(tab_index) {
                            let closed_id = tab.id;
                            if tabs.close_tab(closed_id) {
                                results.remove(&closed_id);
                                publish_tab_strip(&backend, window_id, &tabs);
                                if let Some(active_id) = tabs.active_tab_id() {
                                    publish_active_result(
                                        &backend,
                                        window_id,
                                        options,
                                        &results,
                                        &initial_frames,
                                        active_id,
                                    );
                                } else {
                                    clear_active_page(&backend, window_id);
                                }
                            }
                        }
                    }
                    NavigationCommand::Scroll { delta_y } => {
                        let Some(active_id) = tabs.active_tab_id() else {
                            continue;
                        };
                        let Some(result) = results.get_mut(&active_id) else {
                            continue;
                        };
                        let Some(tab) = tabs.get_tab_mut(active_id) else {
                            continue;
                        };
                        tab.scroll
                            .set_bounds(result.document_height, options.height as f32);
                        tab.scroll.scroll_by(delta_y);
                        result.scroll_by(delta_y, options.height);
                        publish_result(&backend, window_id, options, result);
                    }
                    NavigationCommand::Resize { width, height } => {
                        options.width = width;
                        options.height = height;
                        let Some(active_id) = tabs.active_tab_id() else {
                            continue;
                        };
                        if let Some(result) = results.get_mut(&active_id) {
                            if let Some(tab) = tabs.get_tab_mut(active_id) {
                                tab.scroll
                                    .set_bounds(result.document_height, options.height as f32);
                            }
                            result.scroll_by(0.0, options.height);
                            publish_result(&backend, window_id, options, result);
                        } else if let std::collections::hash_map::Entry::Occupied(mut entry) =
                            initial_frames.entry(active_id)
                            && let Ok(frame) = render_new_tab_frame(options)
                        {
                            entry.insert(frame);
                            publish_active_result(
                                &backend,
                                window_id,
                                options,
                                &results,
                                &initial_frames,
                                active_id,
                            );
                        }
                    }
                    command => {
                        let Some(active_id) = tabs.active_tab_id() else {
                            continue;
                        };
                        let should_render = {
                            let Some(tab) = tabs.get_tab_mut(active_id) else {
                                continue;
                            };
                            start_command(&mut tab.controller, command)
                        };
                        if !should_render {
                            continue;
                        }
                        publish_tab_strip(&backend, window_id, &tabs);
                        let Some(tab) = tabs.get_tab_mut(active_id) else {
                            continue;
                        };
                        let result = runtime
                            .block_on(render_active_navigation(&mut tab.controller, options));
                        match result {
                            Ok(result) => {
                                tab.scroll = PageScrollState::default();
                                tab.scroll
                                    .set_bounds(result.document_height, options.height as f32);
                                tab.title.clone_from(&result.title);
                                results.insert(active_id, result);
                                publish_tab_strip(&backend, window_id, &tabs);
                                publish_active_result(
                                    &backend,
                                    window_id,
                                    options,
                                    &results,
                                    &initial_frames,
                                    active_id,
                                );
                            }
                            Err(error) => {
                                tracing::warn!(%error, "Navigation command failed");
                            }
                        }
                    }
                }
            }
        });
        Self { sender }
    }

    /// Sends a command to the navigation worker.
    ///
    /// # Errors
    ///
    /// Returns `SendError` if the navigation worker has exited.
    pub fn send(
        &self,
        command: NavigationCommand,
    ) -> Result<(), mpsc::SendError<NavigationCommand>> {
        self.sender.send(command)
    }
}

fn publish_result(
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

fn publish_active_result(
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

fn clear_active_page(backend: &SoulBackendHandle, window_id: WindowId) {
    if let Err(error) = backend.clear_page_state(window_id) {
        log_backend_error(&error);
    }
}

fn publish_tab_strip(backend: &SoulBackendHandle, window_id: WindowId, tabs: &TabManager) {
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

fn start_command(controller: &mut NavigationController, command: NavigationCommand) -> bool {
    match command {
        NavigationCommand::Navigate(input) => match controller.navigate(&input) {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!(%error, input, "Invalid navigation input");
                false
            }
        },
        NavigationCommand::Back => controller.go_back().is_some(),
        NavigationCommand::Forward => controller.go_forward().is_some(),
        NavigationCommand::Reload => controller.reload().is_some(),
        NavigationCommand::Scroll { .. }
        | NavigationCommand::Resize { .. }
        | NavigationCommand::NewTab
        | NavigationCommand::SelectTab { .. }
        | NavigationCommand::CloseTab { .. } => false,
    }
}

fn log_backend_error(error: &SoulError) {
    tracing::warn!(%error, "Failed to publish navigation frame");
}
