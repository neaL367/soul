//! Background worker loop and command executor for `NavigationDriver`.

use super::publisher::{
    clear_active_page, publish_active_result, publish_result, publish_tab_strip,
};
use super::types::{NavigationCommand, NavigationDriver};
use crate::engine::{RenderOptions, RenderResult, render_active_navigation};
use crate::local_page::render_new_tab_frame;
use networking::NetworkClient;
use soul_backend_gpui::SoulBackendHandle;
use soul_core::{NavigationController, PageScrollState, TabId, TabManager};
use soul_ui::{ViewportFrame, WindowId};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use tokio::runtime::Runtime;

impl NavigationDriver {
    /// Starts a driver thread owning navigation state and a Tokio runtime.
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
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
            // The live browser routes all network traffic through the IPC contract
            // (in-process transport); the named-pipe transport is the proven
            // cross-process swap, so the M15 network-process split stays a
            // transport change rather than a rewrite (ADR-2/ADR-5).
            let network_client = runtime.block_on(NetworkClient::ipc_in_process());
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
                            tabs.select_tab(tab_id);
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
                    NavigationCommand::CloseTab { tab_index } => {
                        if let Some(tab) = tabs.tabs().get(tab_index) {
                            let tab_id = tab.id;
                            if tabs.close_tab(tab_id) {
                                results.remove(&tab_id);
                                initial_frames.remove(&tab_id);
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
                        let result = runtime.block_on(render_active_navigation(
                            &mut tab.controller,
                            &network_client,
                            options,
                        ));
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
