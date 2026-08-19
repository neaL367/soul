//! Tab strip and individual tab element layout for `PageView`.

use super::PageView;
use crate::layout::TAB_STRIP_HEIGHT;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px, rgb,
};
use soul_ui::{SoulEvent, TabItem};

impl PageView {
    pub(super) fn tab_strip_element(&self, cx: &Context<Self>) -> impl IntoElement {
        let tab_items = self.current_tabs().tabs().to_vec();
        div()
            .w_full()
            .h(px(TAB_STRIP_HEIGHT))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .bg(rgb(0x0018_1825))
            .children(
                tab_items
                    .into_iter()
                    .enumerate()
                    .map(|(tab_index, tab)| self.tab_element(tab_index, tab)),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x0045_475a))
                    .text_color(rgb(0x00cd_d6f4))
                    .cursor_pointer()
                    .id("new-tab")
                    .on_click(cx.listener(|this, _, _, _| {
                        this.emit_event(SoulEvent::NewTabRequested {
                            window_id: this.window_id.0,
                        });
                    }))
                    .child("+"),
            )
    }

    fn tab_element(&self, tab_index: usize, tab: TabItem) -> impl IntoElement {
        let event_handler = self.event_handler.clone();
        let close_event_handler = self.event_handler.clone();
        let window_id = self.window_id.0;
        let background = if tab.is_active {
            0x0031_3244
        } else {
            0x0024_2636
        };
        div()
            .px_3()
            .py_1()
            .rounded_sm()
            .bg(rgb(background))
            .text_color(rgb(0x00cd_d6f4))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_1()
            .id(format!("tab-{tab_index}"))
            .on_click(move |_, _, _| {
                if let Ok(handler) = event_handler.lock()
                    && let Some(handler) = handler.as_deref()
                {
                    handler(SoulEvent::TabSelected {
                        window_id,
                        tab_index,
                    });
                }
            })
            .child(if tab.is_loading {
                format!("{} …", tab.title)
            } else {
                tab.title
            })
            .child(
                div()
                    .px_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .id(format!("close-tab-{tab_index}"))
                    .on_click(move |_, _, _| {
                        if let Ok(handler) = close_event_handler.lock()
                            && let Some(handler) = handler.as_deref()
                        {
                            handler(SoulEvent::TabCloseRequested {
                                window_id,
                                tab_index,
                            });
                        }
                    })
                    .child("×"),
            )
    }
}
