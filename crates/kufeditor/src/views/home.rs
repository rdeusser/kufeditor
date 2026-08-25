use gpui::{Div, Window, div, prelude::*, px};
use kufeditor_game::Game;

use crate::{actions::OpenFile, components, theme::Theme};

pub fn render(theme: &Theme, game: Game) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(40.0))
        .child(
            components::surface(theme)
                .w_full()
                .max_w(px(720.0))
                .p(px(36.0))
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.accent)
                        .child("FORGED STEEL WORKSPACE"),
                )
                .child(
                    div()
                        .text_size(px(30.0))
                        .text_color(theme.text)
                        .child("Kingdom Under Fire tools, reforged."),
                )
                .child(
                    div()
                        .max_w(px(560.0))
                        .text_size(px(15.0))
                        .text_color(theme.text_dim)
                        .child(format!(
                            "Editing {} data with source-preserving Rust codecs.",
                            game.label()
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            components::primary_button(theme, "home-open-file", "Open file")
                                .on_click(|_, window: &mut Window, cx| {
                                    window.dispatch_action(Box::new(OpenFile), cx);
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.text_dim)
                                .child("TroopInfo.sox is available in this first slice."),
                        ),
                ),
        )
}
