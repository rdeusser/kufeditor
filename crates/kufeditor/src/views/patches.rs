use gpui::{Div, div, prelude::*, px};

use crate::{components, theme::Theme};

pub fn render(theme: &Theme) -> Div {
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
                    .child("Patches"),
            )
            .child(
                div()
                    .text_color(theme.text_dim)
                    .child("The patch engine arrives in a later migration stage."),
            ),
    )
}
