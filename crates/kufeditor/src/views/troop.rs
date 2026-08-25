use std::borrow::Cow;

use gpui::{AnyElement, Div, ElementId, Stateful, div, prelude::*, px};
use kufeditor_workspace::Severity;

use crate::{components, theme::Theme};

const TROOP_NAMES: [&str; 43] = [
    "Archer",
    "Longbows",
    "Infantry",
    "Spearman",
    "Heavy Infantry",
    "Knight",
    "Paladin",
    "Cavalry",
    "Heavy Cavalry",
    "Storm Riders",
    "Sappers",
    "Pyro Techs",
    "Bomber Wings",
    "Mortar",
    "Ballista",
    "Harpoon",
    "Catapult",
    "Battaloon",
    "Dark Elves Archer",
    "Dark Elves Cavalry Archers",
    "Dark Elves Infantry",
    "Dark Elves Knights",
    "Dark Elves Cavalry",
    "Orc Infantry",
    "Orc Riders",
    "Orc Heavy Riders",
    "Orc Axe Man",
    "Orc Heavy Infantry",
    "Orc Sappers",
    "Orc Scorpion",
    "Orc Swamp Mammoth",
    "Orc Dirigible",
    "Orc Black Wyverns",
    "Orc Ghouls",
    "Orc Bone Dragon",
    "Wall Archers (Humans)",
    "Scouts",
    "Ghoul Selfdestruct",
    "Encablossa Monster (Melee)",
    "Encablossa Flying Monster",
    "Encablossa Monster (Ranged)",
    "Wall Archers (Elves)",
    "Encablossa Main",
];

pub(crate) fn troop_name(index: usize) -> Cow<'static, str> {
    TROOP_NAMES.get(index).map_or_else(
        || Cow::Owned(format!("Troop {}", index + 1)),
        |name| Cow::Borrowed(*name),
    )
}

pub fn render(
    theme: &Theme,
    records: Vec<AnyElement>,
    groups: Vec<AnyElement>,
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
                .child(column_heading(theme, "TROOPS"))
                .child(
                    div()
                        .id("troop-record-scroll")
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
                .child(column_heading(theme, "TROOP PROPERTIES"))
                .child(
                    div()
                        .id("troop-form-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(px(18.0))
                        .flex()
                        .flex_col()
                        .gap(px(14.0))
                        .children(groups),
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
                        .id("troop-diagnostics-scroll")
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
                .child(troop_name(index).into_owned()),
        )
}

pub fn group(
    theme: &Theme,
    label: &'static str,
    fields: Vec<AnyElement>,
    help: Option<&'static str>,
    derived: Option<(&'static str, i32)>,
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
                .child(label),
        )
        .children(help.map(|text| {
            div()
                .px(px(14.0))
                .pt(px(10.0))
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(text)
        }))
        .child(
            div()
                .p(px(10.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(fields)
                .children(derived.map(|(name, value)| read_only_row(theme, name, value))),
        )
}

pub fn field_row(
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
                    "Invalid signed 32-bit value · Enter to retry · Esc to cancel"
                } else {
                    "Enter to apply · Esc to cancel"
                })
        }))
}

pub fn diagnostic_row(
    theme: &Theme,
    severity: Severity,
    title: String,
    message: &'static str,
) -> Div {
    let severity = match severity {
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
        .child(div().text_color(theme.text).child(title))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(message),
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

fn read_only_row(theme: &Theme, label: &'static str, value: i32) -> Div {
    div()
        .flex()
        .items_center()
        .min_h(px(38.0))
        .px(px(10.0))
        .rounded_md()
        .bg(theme.background)
        .text_color(theme.text_dim)
        .child(div().flex_1().child(label))
        .child(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::troop_name;

    #[test]
    fn known_and_extra_records_have_stable_names() {
        assert_eq!(troop_name(0), "Archer");
        assert_eq!(troop_name(42), "Encablossa Main");
        assert_eq!(troop_name(43), "Troop 44");
    }
}
