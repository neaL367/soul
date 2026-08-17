//! Single-owner navigation command driver for the live Soul window.

use crate::engine::{RenderOptions, RenderResult, render_active_navigation};
use soul_backend_gpui::SoulBackendHandle;
use soul_core::{NavigationController, PageScrollState};
use soul_ui::{SoulError, ViewportFrame, WindowId};
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
    pub fn spawn(backend: SoulBackendHandle, window_id: WindowId, options: RenderOptions) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let Ok(runtime) = Runtime::new() else {
                tracing::error!("Failed to create navigation runtime");
                return;
            };
            let mut controller = NavigationController::new();
            let mut active_result: Option<RenderResult> = None;
            let mut scroll = PageScrollState::default();

            while let Ok(command) = receiver.recv() {
                if let NavigationCommand::Scroll { delta_y } = command {
                    if let Some(result) = active_result.as_mut() {
                        scroll.set_bounds(result.document_height, options.height as f32);
                        scroll.scroll_by(delta_y);
                        result.scroll_by(delta_y, options.height);
                        publish_result(&backend, window_id, options, result);
                    }
                    continue;
                }
                if !start_command(&mut controller, command) {
                    continue;
                }
                let result = runtime.block_on(render_active_navigation(&mut controller, options));
                match result {
                    Ok(result) => {
                        scroll = PageScrollState::default();
                        scroll.set_bounds(result.document_height, options.height as f32);
                        active_result = Some(result);
                        if let Some(result) = active_result.as_ref() {
                            publish_result(&backend, window_id, options, result);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Navigation command failed");
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
        NavigationCommand::Scroll { .. } => false,
    }
}

fn log_backend_error(error: &SoulError) {
    tracing::warn!(%error, "Failed to publish navigation frame");
}
