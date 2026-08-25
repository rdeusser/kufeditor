use gpui::{AnyElement, Div, div, prelude::*, px};

use crate::{components, theme::Theme};

pub fn render(theme: &Theme, tabs: Vec<AnyElement>, editor: Option<Div>) -> Div {
    let body = editor.unwrap_or_else(|| {
        div().size_full().p(px(28.0)).child(
            components::surface(theme)
                .size_full()
                .p(px(28.0))
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(22.0))
                        .text_color(theme.text)
                        .child("No file is open"),
                )
                .child(
                    div()
                        .text_color(theme.text_dim)
                        .child("Open a TroopInfo.sox or SkillInfo.sox file to begin editing."),
                ),
        )
    });

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_none()
                .min_w_0()
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .children(tabs),
        )
        .child(div().flex_1().min_h_0().child(body))
}
