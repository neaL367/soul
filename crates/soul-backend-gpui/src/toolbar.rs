//! Raw GPUI Soul toolbar button primitives.

use crate::state::SharedEventHandler;
use gpui::{
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, rgb,
};
use soul_ui::SoulEvent;

/// Builds a small raw GPUI action button for Soul toolbar actions.
pub fn action_button(
    window_id: u64,
    event_handler: SharedEventHandler,
    label: &'static str,
    event: SoulEvent,
) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(rgb(0x0045_475a))
        .text_color(rgb(0x00cd_d6f4))
        .cursor_pointer()
        .id(label)
        .on_click(move |_, _, _| {
            if let Ok(handler) = event_handler.lock()
                && let Some(handler) = handler.as_deref()
            {
                let event = match event.clone() {
                    SoulEvent::NavigateBack { .. } => SoulEvent::NavigateBack { window_id },
                    SoulEvent::NavigateForward { .. } => SoulEvent::NavigateForward { window_id },
                    SoulEvent::Reload { bypass_cache, .. } => SoulEvent::Reload {
                        window_id,
                        bypass_cache,
                    },
                    other => other,
                };
                handler(event);
            }
        })
        .child(label)
}
