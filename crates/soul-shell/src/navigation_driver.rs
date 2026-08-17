//! Single-owner navigation command driver for the live Soul window.

use crate::engine::{RenderOptions, render_active_navigation};
use soul_backend_gpui::SoulBackendHandle;
use soul_core::NavigationController;
use soul_ui::{SoulError, ViewportFrame, WindowId};
use std::sync::mpsc::{self, Sender};
use std::thread;
use tokio::runtime::Runtime;

/// Commands accepted from the Soul toolbar and omnibox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationCommand {
    /// Navigate to user-entered URL or search text.
    Navigate(String),
    /// Traverse one entry backward.
    Back,
    /// Traverse one entry forward.
    Forward,
    /// Re-fetch current URL.
    Reload,
}

/// Handle for sending navigation commands to one controller-owning worker.
#[derive(Clone)]
pub struct NavigationDriver {
    sender: Sender<NavigationCommand>,
}

impl NavigationDriver {
    /// Starts a driver thread owning navigation state and a Tokio runtime.
    #[must_use]
    pub fn spawn(backend: SoulBackendHandle, window_id: WindowId, options: RenderOptions) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let Ok(runtime) = Runtime::new() else {
                tracing::error!("Failed to create navigation runtime");
                return;
            };
            let mut controller = NavigationController::new();

            while let Ok(command) = receiver.recv() {
                if !start_command(&mut controller, command) {
                    continue;
                }
                let result = runtime.block_on(render_active_navigation(&mut controller, options));
                match result {
                    Ok(result) => {
                        let frame = ViewportFrame::SoftwareRgba {
                            width: options.width,
                            height: options.height,
                            pixels: result.pixel_buffer.data,
                        };
                        if let Err(error) = backend.update_viewport(window_id, frame) {
                            log_backend_error(&error);
                        } else if let Err(error) =
                            backend.update_hit_test_map(window_id, result.hit_test_map)
                        {
                            log_backend_error(&error);
                        } else {
                            tracing::info!(url = %result.url, "Navigation frame updated");
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
    }
}

fn log_backend_error(error: &SoulError) {
    tracing::warn!(%error, "Failed to publish navigation frame");
}
