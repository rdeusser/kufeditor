use gpui::{Div, div, prelude::*, px};
use kufeditor_workspace::{DocumentId, Workspace};

use crate::{components, theme::Theme};

pub fn render(
    theme: &Theme,
    workspace: &Workspace,
    active_document: Option<DocumentId>,
    selected_troop: usize,
) -> Div {
    let body = match active_document {
        Some(id) => {
            let title = workspace
                .title(id)
                .unwrap_or_else(|error| format!("Unavailable document: {error}"));
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(22.0))
                        .text_color(theme.text)
                        .child(title),
                )
                .child(
                    div()
                        .text_color(theme.text_dim)
                        .child(format!("Selected troop record: {}", selected_troop + 1)),
                )
        }
        None => div()
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
                    .child("Open a TroopInfo.sox file to begin editing."),
            ),
    };

    div().size_full().p(px(28.0)).child(
        components::surface(theme)
            .size_full()
            .p(px(28.0))
            .child(body),
    )
}
