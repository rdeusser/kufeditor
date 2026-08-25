use std::borrow::Cow;

use gpui::{AnyElement, Div, ElementId, Stateful, div, prelude::*, px};
use kufeditor_workspace::{Diagnostic, Severity};

use crate::{components, theme::Theme};

const SKILL_NAMES: [&str; 15] = [
    "Melee",
    "Ranged",
    "Frontal",
    "Riding",
    "Teamwork",
    "Scouting",
    "Gunpowder",
    "Beast Mastery",
    "Fire",
    "Lightning",
    "Ice",
    "Holy",
    "Earth",
    "Curse",
    "Any Elemental",
];

pub(crate) fn skill_name(index: usize) -> Cow<'static, str> {
    SKILL_NAMES.get(index).map_or_else(
        || Cow::Owned(format!("Skill {}", index + 1)),
        |name| Cow::Borrowed(*name),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiagnosticItem {
    diagnostic: Diagnostic,
}

impl DiagnosticItem {
    pub(crate) const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub(crate) fn title(&self) -> String {
        let label = self.diagnostic.location.label();
        self.diagnostic.location.record().map_or_else(
            || label.to_owned(),
            |record| format!("{} · {label}", skill_name(record)),
        )
    }
}

pub(crate) const fn diagnostic_item(diagnostic: Diagnostic) -> DiagnosticItem {
    DiagnosticItem { diagnostic }
}

pub fn render(
    theme: &Theme,
    records: Vec<AnyElement>,
    details: Vec<AnyElement>,
    diagnostics: Vec<AnyElement>,
) -> Div {
    div()
        .size_full()
        .flex()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(220.0))
                .min_h_0()
                .bg(theme.surface)
                .border_r_1()
                .border_color(theme.border)
                .child(column_heading(theme, "SKILLS"))
                .child(
                    div()
                        .id("skill-record-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(px(8.0))
                        .children(records),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(column_heading(theme, "SKILL PROPERTIES"))
                .child(
                    div()
                        .id("skill-form-scroll")
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
                        .id("skill-diagnostics-scroll")
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

pub fn record_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    index: usize,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(38.0))
        .px(px(10.0))
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
        .text_color(if selected { theme.text } else { theme.text_dim })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .w(px(24.0))
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child(format!("{:02}", index + 1)),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .child(skill_name(index).into_owned()),
        )
}

pub fn group(theme: &Theme, label: String, fields: Vec<AnyElement>) -> Div {
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
                .child(label),
        )
        .child(
            div()
                .p(px(10.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .children(fields),
        )
}

pub fn number_field_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: &'static str,
    value: String,
    active: bool,
    invalid: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(4.0))
        .min_h(px(38.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded_md()
        .border_1()
        .border_color(if active { theme.accent } else { theme.border })
        .bg(if active { theme.raised } else { theme.surface })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(theme.text_dim)
                        .child(label),
                )
                .child(
                    div()
                        .flex_none()
                        .text_color(if invalid { theme.accent } else { theme.text })
                        .child(value),
                ),
        )
        .children(active.then(|| {
            div()
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child(if invalid {
                    "Enter a value in range · Enter to retry · Esc to cancel"
                } else {
                    "Enter to apply · Esc to cancel"
                })
        }))
}

pub fn text_field_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: &'static str,
    value: String,
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
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(theme.text)
                .child(value),
        )
}

pub fn text_editor_row(theme: &Theme, label: &'static str, editor: AnyElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .min_h(px(58.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child(label),
        )
        .child(editor)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child("Enter to apply · Esc to cancel"),
        )
}

pub fn invalid_text_field(
    theme: &Theme,
    label: &'static str,
    value: &'static str,
    diagnostic: String,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .min_h(px(58.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded_md()
        .border_1()
        .border_color(theme.accent)
        .bg(theme.surface)
        .opacity(0.7)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child(label),
        )
        .child(div().text_color(theme.accent).child(value))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child(diagnostic),
        )
}

pub fn choice_row(theme: &Theme, choices: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .min_h(px(42.0))
        .px(px(10.0))
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(theme.text_dim)
                .child("Skill Type"),
        )
        .child(div().flex().gap(px(6.0)).children(choices))
}

pub fn diagnostic_row(theme: &Theme, item: &DiagnosticItem) -> Div {
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

pub fn no_diagnostics(theme: &Theme) -> Div {
    div()
        .p(px(12.0))
        .text_color(theme.text_dim)
        .child("No validation issues")
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
    #![allow(
        clippy::unwrap_used,
        reason = "the synthetic SkillInfo fixture has checked wire lengths"
    )]

    use kufeditor_workspace::{
        Diagnostic, DiagnosticLocation, SaveNumberTarget, Severity, SkillDocument,
    };

    use super::{diagnostic_item, skill_name};

    fn document_with_bad_type() -> SkillDocument {
        let localization_key = b"@(S_Melee)";
        let icon_path = b"IL_SKL_Melee.tga";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&u16::try_from(localization_key.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(localization_key);
        bytes.extend_from_slice(&u16::try_from(icon_path.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(icon_path);
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        bytes.extend_from_slice(&50_u32.to_le_bytes());
        bytes.resize(bytes.len() + 64, 0);
        SkillDocument::parse(bytes).unwrap()
    }

    #[test]
    fn skill_records_use_legacy_names_and_a_numbered_fallback() {
        let expected = [
            "Melee",
            "Ranged",
            "Frontal",
            "Riding",
            "Teamwork",
            "Scouting",
            "Gunpowder",
            "Beast Mastery",
            "Fire",
            "Lightning",
            "Ice",
            "Holy",
            "Earth",
            "Curse",
            "Any Elemental",
        ];

        for (index, expected_name) in expected.into_iter().enumerate() {
            assert_eq!(skill_name(index), expected_name);
        }
        assert_eq!(skill_name(15), "Skill 16");
    }

    #[test]
    fn skill_diagnostic_item_keeps_record_and_field_identity() {
        let mut diagnostics = document_with_bad_type().diagnostics();
        let diagnostic = diagnostics.remove(0);
        let expected = diagnostic.clone();

        let item = diagnostic_item(diagnostic);

        assert_eq!(item.diagnostic(), &expected);
        assert_eq!(item.title(), "Melee · Skill Type");
    }

    #[test]
    fn skill_diagnostic_title_uses_document_location_without_record_prefix() {
        let diagnostic = Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::Save(SaveNumberTarget::CampaignIndex),
            message: "Campaign warning",
        };

        assert_eq!(diagnostic_item(diagnostic).title(), "Campaign");
    }
}
