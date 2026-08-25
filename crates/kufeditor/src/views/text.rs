use gpui::{AnyElement, Div, ElementId, Stateful, div, prelude::*, px};
use kufeditor_workspace::{Diagnostic, Severity};

use crate::{components, theme::Theme};

pub(crate) fn preview(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\t' => '⇥',
            '\n' => '↵',
            '\r' => '␍',
            _ => character,
        })
        .collect()
}

pub(crate) fn entry_metadata(record: usize, wire_index: u32, used: usize, maximum: u16) -> String {
    format!(
        "Entry {:02} · Wire {wire_index} · {used} / {maximum} bytes",
        record + 1
    )
}

pub(crate) fn diagnostic_title(wire_index: u32, field_label: &str) -> String {
    format!("Wire {wire_index} · {field_label}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiagnosticItem {
    diagnostic: Diagnostic,
    wire_index: u32,
}

impl DiagnosticItem {
    pub(crate) const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub(crate) fn title(&self) -> String {
        diagnostic_title(self.wire_index, self.diagnostic.field.label())
    }
}

pub(crate) const fn diagnostic_item(diagnostic: Diagnostic, wire_index: u32) -> DiagnosticItem {
    DiagnosticItem {
        diagnostic,
        wire_index,
    }
}

pub(crate) fn render(
    theme: &Theme,
    records: Vec<AnyElement>,
    details: Vec<AnyElement>,
    diagnostics: Vec<AnyElement>,
) -> Div {
    let records_are_empty = records.is_empty();
    div()
        .size_full()
        .flex()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(240.0))
                .min_h_0()
                .bg(theme.surface)
                .border_r_1()
                .border_color(theme.border)
                .child(column_heading(theme, "TEXT ENTRIES"))
                .child(
                    div()
                        .id("text-record-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .children(records)
                        .children(records_are_empty.then(|| {
                            div()
                                .p(px(12.0))
                                .text_color(theme.text_dim)
                                .child("This file has no text records.")
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(column_heading(theme, "TEXT PROPERTIES"))
                .child(
                    div()
                        .id("text-form-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(px(18.0))
                        .flex()
                        .flex_col()
                        .gap(px(14.0))
                        .children(details),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(280.0))
                .min_h_0()
                .bg(theme.surface)
                .border_l_1()
                .border_color(theme.border)
                .child(column_heading(theme, "VALIDATION"))
                .child(
                    div()
                        .id("text-diagnostics-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(px(12.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .children(diagnostics),
                ),
        )
}

pub(crate) fn record_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    metadata: String,
    text_preview: String,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(4.0))
        .min_h(px(58.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded_md()
        .border_1()
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
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.text_dim)
                .child(metadata),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(if selected { theme.text } else { theme.text_dim })
                .child(text_preview),
        )
}

pub(crate) fn property_group(
    theme: &Theme,
    wire_index: u32,
    maximum: u16,
    text_field: AnyElement,
) -> Div {
    components::surface(theme)
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .h(px(38.0))
                .px(px(14.0))
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(12.0))
                .text_color(theme.accent)
                .child(format!("Entry · Wire {wire_index}")),
        )
        .child(
            div()
                .p(px(10.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(read_only_row(theme, "Wire Index", wire_index.to_string()))
                .child(read_only_row(
                    theme,
                    "Byte Budget",
                    format!("1..={maximum} bytes"),
                ))
                .child(text_field),
        )
}

pub(crate) fn text_field_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    text_preview: String,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(5.0))
        .min_h(px(52.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child("Text"),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(theme.text)
                .child(text_preview),
        )
}

pub(crate) fn text_editor_row(
    theme: &Theme,
    editor: AnyElement,
    current: usize,
    maximum: u16,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .min_h(px(70.0))
        .child(
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(theme.text_dim)
                        .child("Text"),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(theme.text_dim)
                        .child(format!("{current} / {maximum} bytes")),
                ),
        )
        .child(editor)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child("Enter to apply · Esc to cancel"),
        )
}

pub(crate) fn empty_properties(theme: &Theme) -> Div {
    div()
        .text_color(theme.text_dim)
        .child("Select a text entry to view its properties.")
}

pub(crate) fn diagnostic_row(theme: &Theme, item: &DiagnosticItem) -> Div {
    let diagnostic = item.diagnostic();
    let severity = match diagnostic.severity {
        Severity::Info => "INFO",
        Severity::Warning => "WARNING",
        Severity::Error => "ERROR",
    };
    components::surface(theme)
        .p(px(10.0))
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.accent)
                .child(severity),
        )
        .child(div().text_color(theme.text).child(item.title()))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(diagnostic.message),
        )
}

pub(crate) fn no_diagnostics(theme: &Theme) -> Div {
    div()
        .p(px(12.0))
        .text_color(theme.text_dim)
        .child("No validation issues")
}

fn read_only_row(theme: &Theme, label: &'static str, value: String) -> Div {
    div()
        .flex()
        .items_center()
        .min_h(px(38.0))
        .px(px(10.0))
        .rounded_md()
        .bg(theme.background)
        .text_color(theme.text_dim)
        .child(div().flex_1().child(label))
        .child(value)
}

fn column_heading(theme: &Theme, label: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .flex_none()
        .h(px(42.0))
        .px(px(14.0))
        .border_b_1()
        .border_color(theme.border)
        .text_size(px(11.0))
        .text_color(theme.text_dim)
        .child(label)
}

#[cfg(test)]
mod tests {
    use kufeditor_workspace::{TextSoxDocument, TextSoxField};

    use super::{diagnostic_item, diagnostic_title, entry_metadata, preview};

    #[test]
    fn text_sox_preview_makes_allowed_control_bytes_visible() {
        assert_eq!(
            preview("Alpha\tBeta\nGamma\rDelta"),
            "Alpha⇥Beta↵Gamma␍Delta"
        );
    }

    #[test]
    fn text_sox_preview_returns_long_utf8_content_for_gpui_to_truncate() {
        let value = format!("{}💡tail", "a".repeat(300));

        assert_eq!(preview(&value), value);
    }

    #[test]
    fn text_sox_diagnostic_titles_keep_wire_index_and_field_identity() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        for text in [b"Alpha".as_slice(), b"Bravo".as_slice()] {
            bytes.extend_from_slice(&9001_u32.to_le_bytes());
            bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(text);
        }
        let document = TextSoxDocument::parse(bytes).unwrap();
        let diagnostic = document.diagnostics().remove(0);

        assert_eq!(
            diagnostic_item(diagnostic, 9001).title(),
            "Wire 9001 · Index"
        );
        assert_eq!(
            diagnostic_title(9001, TextSoxField::Text.label()),
            "Wire 9001 · Text"
        );
    }

    #[test]
    fn text_sox_entry_metadata_formats_wire_identity_and_byte_budget() {
        assert_eq!(
            entry_metadata(2, 9001, 4, 12),
            "Entry 03 · Wire 9001 · 4 / 12 bytes"
        );
    }
}
