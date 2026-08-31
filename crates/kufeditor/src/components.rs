use gpui::{Div, ElementId, Stateful, div, prelude::*, px};

use crate::theme::Theme;

pub fn rail_item(
    theme: &Theme,
    id: &'static str,
    label: &'static str,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    let accent = theme.accent;
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(36.0))
        .px(px(12.0))
        .border_l_2()
        .border_color(if selected {
            theme.accent
        } else {
            theme.surface
        })
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.surface
        })
        .text_color(if selected { theme.text } else { theme.text_dim })
        .cursor_pointer()
        .hover(move |style| style.bg(hover).text_color(accent))
        .active(move |style| style.border_color(accent))
        .child(label)
}

pub fn disabled_rail_item(theme: &Theme, id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(36.0))
        .px(px(12.0))
        .border_l_2()
        .border_color(theme.surface)
        .text_color(theme.text_dim)
        .opacity(0.45)
        .child(label)
}

pub fn toolbar_button(
    theme: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    let accent = theme.accent;
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(30.0))
        .px(px(10.0))
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .text_color(if enabled { theme.text } else { theme.text_dim })
        .when(enabled, move |button| {
            button
                .cursor_pointer()
                .hover(move |style| style.bg(hover).border_color(accent))
                .active(move |style| style.bg(theme.accent_dim))
        })
        .when(!enabled, |button| button.opacity(0.45))
        .child(label)
}

pub fn primary_button(theme: &Theme, id: &'static str, label: &'static str) -> Stateful<Div> {
    let raised = theme.raised;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(34.0))
        .px(px(14.0))
        .rounded_md()
        .border_1()
        .border_color(theme.accent)
        .bg(theme.accent_dim)
        .text_color(theme.text)
        .cursor_pointer()
        .hover(move |style| style.bg(raised))
        .active(move |style| style.bg(theme.accent_dim))
        .child(label)
}

pub fn choice_button(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<String>,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.0))
        .px(px(10.0))
        .rounded_md()
        .border_1()
        .border_color(if selected { theme.accent } else { theme.border })
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.surface
        })
        .text_color(if selected { theme.text } else { theme.text_dim })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(label)
}

pub fn surface(theme: &Theme) -> Div {
    div()
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
}

pub fn document_tab(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: String,
    active: bool,
    dirty: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(7.0))
        .h(px(40.0))
        .px(px(14.0))
        .border_b_1()
        .border_color(if active { theme.accent } else { theme.border })
        .bg(if active { theme.raised } else { theme.surface })
        .text_color(if active { theme.text } else { theme.text_dim })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(label)
        .children(dirty.then(|| {
            div()
                .text_color(theme.accent)
                .text_size(px(16.0))
                .child("•")
        }))
}
