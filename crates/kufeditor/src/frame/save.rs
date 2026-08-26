use gpui::{AnyElement, Context, Div, SharedString, div, prelude::*, px};
use kufeditor_game::NameDictionary;
use kufeditor_workspace::{
    DocumentID, SaveEditor, SaveEquipmentGroup, SaveEquipmentSlot, SaveNumberTarget, SaveUnitGroup,
};

use super::AppFrame;
use crate::{
    actions::SetSaveChoice,
    components,
    save_catalog_status::SaveCatalogStatus,
    state::{SavePresentationState, SavePresentationTransition, SaveSection, SaveUnitVisibility},
    views::save::{
        self, SaveNumberProjection, SaveProjectionID, SaveRowLocation, SaveRowProjection, SaveRows,
        SaveSectionModel, SaveUnitProjection,
    },
};

const SAVE_SECTIONS: [SaveSection; 5] = [
    SaveSection::Summary,
    SaveSection::Units,
    SaveSection::Equipment,
    SaveSection::Roster,
    SaveSection::Missions,
];

const UNIT_FILTERS: [(&str, &str); 4] = [
    ("All", ""),
    ("Leaders", "leader"),
    ("Officers", "officer"),
    ("Troops", "troop"),
];

impl AppFrame {
    pub(super) fn activate_save_presentation(
        &mut self,
        document: DocumentID,
        cx: &mut Context<Self>,
    ) {
        let filter = self
            .save_presentations
            .get(document)
            .map_or_else(String::new, |state| state.unit_filter().to_owned());
        let rows = self.save_unit_rows(document, &filter);
        let draft_active = self.save_draft_active();
        let document_changed = self.active_document != Some(document);
        let visibility = rows
            .as_ref()
            .ok()
            .and_then(SaveRows::unit_visibility)
            .unwrap_or(SaveUnitVisibility::All { unit_count: 0 });
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| {
                let transition = states.activate_document(document, visibility, draft_active);
                if document_changed && transition == SavePresentationTransition::Unchanged {
                    if draft_active {
                        SavePresentationTransition::ChangedAndCancelDraft
                    } else {
                        SavePresentationTransition::Changed
                    }
                } else {
                    transition
                }
            },
            cx,
        );
    }

    pub(super) fn save_editor(&self, document: DocumentID, cx: &mut Context<Self>) -> Div {
        let state = self
            .save_presentations
            .get(document)
            .cloned()
            .unwrap_or_default();
        let dictionary = self.save_dictionary();
        let content = match save::save_section_model(&self.workspace, document, &state, dictionary)
        {
            Ok(model) => self.render_save_section(document, &state, model, cx),
            Err(error) => save::empty_state(
                &self.theme,
                format!("Could not read this Crusaders save: {error}"),
            )
            .size_full()
            .into_any_element(),
        };

        save::render_editor(
            &self.theme,
            self.save_section_rail(document, state.section(), cx),
            self.save_catalog_status_element(),
            content,
        )
    }

    fn render_save_section(
        &self,
        document: DocumentID,
        state: &SavePresentationState,
        model: SaveSectionModel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match model {
            SaveSectionModel::Summary(summary) => self
                .save_summary_view(document, &summary, cx)
                .into_any_element(),
            SaveSectionModel::Units { rows, inspected } => self
                .save_units_view(document, state, &rows, inspected, cx)
                .into_any_element(),
            SaveSectionModel::Equipment {
                slots,
                inspected_unit,
                selected,
            } => self
                .save_equipment_view(
                    document,
                    state,
                    slots,
                    inspected_unit.as_ref(),
                    selected.as_ref(),
                    cx,
                )
                .into_any_element(),
            SaveSectionModel::Roster {
                player_leaders,
                world_map_rows,
            } => self
                .save_roster_view(document, player_leaders, world_map_rows, cx)
                .into_any_element(),
            SaveSectionModel::Missions {
                mission,
                second_array_rows,
            } => self
                .save_missions_view(document, &mission, second_array_rows, cx)
                .into_any_element(),
        }
    }

    fn save_summary_view(
        &self,
        document: DocumentID,
        summary: &save::SaveSummaryProjection,
        cx: &mut Context<Self>,
    ) -> Div {
        save::scrolling_section(
            &self.theme,
            "save-summary",
            "Summary",
            "Envelope, campaign, fixed strings, and record counts".to_owned(),
            vec![
                self.save_summary_envelope(document, summary),
                self.save_summary_values(summary, cx),
                self.save_summary_text(document, summary, cx),
                self.save_summary_counts(document, summary),
                self.save_summary_context(document, summary),
            ],
        )
    }

    fn save_summary_envelope(
        &self,
        document: DocumentID,
        summary: &save::SaveSummaryProjection,
    ) -> AnyElement {
        let envelope = vec![
            save::value_row(
                &self.theme,
                save_local_id("save-envelope-prefix", document, 0),
                "Size Prefix",
                yes_no(summary.has_size_prefix),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-envelope-context", document, 0),
                "Context Block",
                yes_no(summary.has_context),
            )
            .into_any_element(),
        ];
        save::group(&self.theme, "ENVELOPE", envelope)
            .id("save-summary-envelope")
            .debug_selector(|| "save-summary-envelope".to_owned())
            .into_any_element()
    }

    fn save_summary_values(
        &self,
        summary: &save::SaveSummaryProjection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut values = vec![self.save_number_row(&summary.campaign, cx)];
        values.extend(
            summary
                .main_fields
                .iter()
                .map(|field| self.save_number_row(field, cx)),
        );
        values.push(self.save_number_row(&summary.saved_unit_reference, cx));
        save::group(&self.theme, "SAVE VALUES", values)
            .id("save-summary-values")
            .debug_selector(|| "save-summary-values".to_owned())
            .into_any_element()
    }

    fn save_summary_text(
        &self,
        document: DocumentID,
        summary: &save::SaveSummaryProjection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = summary
            .text_fields
            .iter()
            .map(|field| {
                let target = super::TextEditTarget::save(document, field.field);
                if let Some(edit) = self.text_edit.as_ref().filter(|edit| edit.target == target) {
                    return save::text_editor_row(
                        &self.theme,
                        projection_element_id("save-text-editor", field.id),
                        field.label.clone(),
                        edit.input.clone().into_any_element(),
                        edit.validation_error.clone(),
                    )
                    .debug_selector(move || save_text_editor_selector(field.field).to_owned())
                    .into_any_element();
                }
                let row = save::text_value_row(
                    &self.theme,
                    projection_element_id("save-text", field.id),
                    field.label.clone(),
                    match &field.value {
                        Ok(value) => empty_label(value),
                        Err(error) => error.workspace_error().to_string(),
                    },
                );
                if let Ok(value) = &field.value {
                    let value = value.clone();
                    row.debug_selector(move || save_text_selector(field.field).to_owned())
                        .tab_stop(true)
                        .cursor_pointer()
                        .on_click(cx.listener(move |frame, _, window, cx| {
                            frame.start_text_edit(target, value.clone(), window, cx);
                        }))
                        .into_any_element()
                } else {
                    row.debug_selector(|| "save-fixed-text-error".to_owned())
                        .into_any_element()
                }
            })
            .collect();
        save::group(&self.theme, "FIXED STRINGS", text).into_any_element()
    }

    fn save_summary_counts(
        &self,
        document: DocumentID,
        summary: &save::SaveSummaryProjection,
    ) -> AnyElement {
        let counts = vec![
            save::value_row(
                &self.theme,
                save_local_id("save-count-units", document, 0),
                "Units",
                summary.unit_count.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-count-roster", document, 0),
                "World Map Rows",
                summary.roster_count.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-count-second", document, 0),
                "Second Array Rows",
                summary.second_array_count.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-role-leader", document, 0),
                "Leaders",
                summary.role_counts.leader.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-role-officer-1", document, 0),
                "Officer 1",
                summary.role_counts.officer_1.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-role-officer-2", document, 0),
                "Officer 2",
                summary.role_counts.officer_2.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-role-troop", document, 0),
                "Troops",
                summary.role_counts.troop.to_string(),
            )
            .into_any_element(),
            save::value_row(
                &self.theme,
                save_local_id("save-role-unknown", document, 0),
                "Unknown Roles",
                summary.role_counts.unknown.to_string(),
            )
            .into_any_element(),
        ];
        save::group(&self.theme, "COUNTS AND ROLES", counts)
            .id("save-summary-counts")
            .debug_selector(|| "save-summary-counts".to_owned())
            .into_any_element()
    }

    fn save_summary_context(
        &self,
        document: DocumentID,
        summary: &save::SaveSummaryProjection,
    ) -> AnyElement {
        let context = if summary.context_text.is_empty() {
            vec![
                save::empty_state(&self.theme, "No readable context strings were found.")
                    .into_any_element(),
            ]
        } else {
            summary
                .context_text
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    save::text_value_row(
                        &self.theme,
                        save_local_id("save-context", document, index),
                        format!("Context {}", index + 1),
                        value.clone(),
                    )
                    .into_any_element()
                })
                .collect()
        };
        save::group(&self.theme, "CONTEXT TEXT", context).into_any_element()
    }

    fn save_units_view(
        &self,
        document: DocumentID,
        state: &SavePresentationState,
        rows: &SaveRows,
        inspected: Option<SaveUnitProjection>,
        cx: &mut Context<Self>,
    ) -> Div {
        let list = self.save_unit_list(document, state, rows.clone(), cx);
        let details =
            inspected.map_or_else(Vec::new, |unit| self.save_unit_details(document, &unit, cx));
        save::split_section(
            &self.theme,
            "save-units",
            "Units",
            format!("{} visible unit records", rows.len()),
            list,
            details,
        )
    }

    fn save_unit_list(
        &self,
        document: DocumentID,
        state: &SavePresentationState,
        rows: SaveRows,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = UNIT_FILTERS
            .into_iter()
            .enumerate()
            .map(|(index, (label, filter))| {
                let selected = state.unit_filter() == filter;
                let selector = format!("save-unit-filter-{}", label.to_lowercase());
                components::choice_button(&self.theme, ("save-unit-filter", index), label, selected)
                    .child(if selected { " ✓" } else { "" })
                    .debug_selector(move || selector.clone())
                    .tab_stop(true)
                    .on_click(cx.listener(move |frame, _, window, cx| {
                        frame.set_save_unit_filter(document, filter, cx);
                        window.focus(&frame.focus);
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let list = if rows.is_empty() {
            save::empty_state(
                &self.theme,
                if state.unit_filter().is_empty() {
                    "This save has no unit records."
                } else {
                    "No units match the current filter."
                },
            )
            .id("save-unit-empty")
            .debug_selector(|| "save-unit-empty".to_owned())
            .size_full()
            .into_any_element()
        } else {
            save::uniform_save_rows(
                save_local_id("save-unit-list", document, 0),
                rows,
                cx.processor(move |frame, location, _, cx| {
                    frame.save_virtual_unit_row(document, location, cx)
                }),
            )
            .size_full()
            .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .flex_none()
                    .p(px(8.0))
                    .flex()
                    .flex_wrap()
                    .gap(px(5.0))
                    .border_b_1()
                    .border_color(self.theme.border)
                    .children(filters),
            )
            .child(div().flex_1().min_h_0().overflow_hidden().child(list))
            .into_any_element()
    }

    fn save_virtual_unit_row(
        &mut self,
        document: DocumentID,
        location: SaveRowLocation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let projection =
            save::row_projection(&self.workspace, document, self.save_dictionary(), location);
        match projection {
            Ok(SaveRowProjection::Unit(row)) => {
                let selected = self
                    .save_presentations
                    .get(document)
                    .is_some_and(|state| state.inspected_unit() == row.source_index);
                let selector = format!("save-unit-master-row-{}", row.source_index);
                save::unit_row(
                    &self.theme,
                    projection_element_id("save-unit-row", row.id),
                    &row,
                    selected,
                )
                .debug_selector(move || selector.clone())
                .tab_stop(true)
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.inspect_save_unit(document, location.source_index, cx);
                    window.focus(&frame.focus);
                }))
                .into_any_element()
            }
            Ok(_) => save::empty_state(&self.theme, "Unexpected save row kind.").into_any_element(),
            Err(error) => save::empty_state(&self.theme, format!("Could not read unit: {error}"))
                .into_any_element(),
        }
    }

    fn save_unit_details(
        &self,
        document: DocumentID,
        unit: &SaveUnitProjection,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut details = Vec::new();
        if self.save_dictionary().is_some()
            && unit.row.name_availability == save::SaveNameAvailability::Unavailable
        {
            details.push(save::inline_name_unavailable(&self.theme, "Unit").into_any_element());
        }
        details.push(
            save::group(
                &self.theme,
                "RAW IDENTITY",
                vec![
                    save::value_row(
                        &self.theme,
                        save_local_id("save-unit-name", document, unit.row.source_index),
                        "Resolved Name",
                        unit.row.label.clone(),
                    )
                    .into_any_element(),
                    save::value_row(
                        &self.theme,
                        save_local_id("save-unit-role", document, unit.row.source_index),
                        "UCD Role",
                        unit.row.role.clone(),
                    )
                    .into_any_element(),
                    save::value_row(
                        &self.theme,
                        save_local_id("save-unit-character", document, unit.row.source_index),
                        "Character ID",
                        unit.row.character_id.to_string(),
                    )
                    .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        for group in [
            SaveUnitGroup::Core,
            SaveUnitGroup::Formation,
            SaveUnitGroup::Advanced,
        ] {
            let fields = unit
                .fields
                .iter()
                .filter(|field| unit_field_group(field.target) == Some(group))
                .map(|field| self.save_number_row(field, cx))
                .collect();
            details.push(save::group(&self.theme, group.label(), fields).into_any_element());
        }
        details.push(save::skill_bytes(&self.theme, &unit.skill_data).into_any_element());
        details
    }

    fn save_equipment_view(
        &self,
        document: DocumentID,
        state: &SavePresentationState,
        slots: [SaveEquipmentSlot; 6],
        inspected_unit: Option<&save::SaveUnitRowProjection>,
        selected: Option<&save::SaveEquipmentProjection>,
        cx: &mut Context<Self>,
    ) -> Div {
        let slots_enabled = inspected_unit.is_some();
        let mut content =
            vec![self.save_equipment_slot_bar(document, state, slots, slots_enabled, cx)];
        if let (Some(unit), Some(equipment)) = (inspected_unit, selected) {
            content.extend(self.save_equipment_details(document, unit, equipment, cx));
        } else {
            let (selector, message) = if state.unit_filter().is_empty() {
                (
                    "save-equipment-save-empty",
                    "This save has no units, so there is no equipment to inspect.",
                )
            } else {
                (
                    "save-equipment-filter-empty",
                    "No units match the current filter, so there is no equipment to inspect.",
                )
            };
            content.push(
                save::empty_state(&self.theme, message)
                    .id(selector)
                    .debug_selector(move || selector.to_owned())
                    .into_any_element(),
            );
        }

        save::scrolling_section(
            &self.theme,
            "save-equipment",
            "Equipment",
            "Six stable equipment slots for the inspected unit".to_owned(),
            content,
        )
    }

    fn save_equipment_slot_bar(
        &self,
        document: DocumentID,
        state: &SavePresentationState,
        slots: [SaveEquipmentSlot; 6],
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let slot_buttons = slots
            .into_iter()
            .enumerate()
            .map(|(index, slot)| {
                let selector = equipment_slot_selector(slot, enabled);
                let button = save::equipment_slot_button(
                    &self.theme,
                    ("save-equipment-slot", index),
                    slot.label(),
                    state.equipment_slot() == slot,
                    enabled,
                )
                .debug_selector(move || selector.to_owned());
                if enabled {
                    button
                        .tab_stop(true)
                        .on_click(cx.listener(move |frame, _, window, cx| {
                            frame.select_save_equipment_slot(document, slot, cx);
                            window.focus(&frame.focus);
                        }))
                        .into_any_element()
                } else {
                    button.into_any_element()
                }
            })
            .collect::<Vec<_>>();
        components::surface(&self.theme)
            .p(px(10.0))
            .flex()
            .flex_wrap()
            .gap(px(7.0))
            .children(slot_buttons)
            .into_any_element()
    }

    fn save_equipment_details(
        &self,
        document: DocumentID,
        unit: &save::SaveUnitRowProjection,
        equipment: &save::SaveEquipmentProjection,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut content = vec![
            save::group(
                &self.theme,
                "INSPECTED UNIT",
                vec![
                    save::value_row(
                        &self.theme,
                        save_local_id("save-equipment-unit", document, unit.source_index),
                        "Unit",
                        unit.label.clone(),
                    )
                    .into_any_element(),
                    save::value_row(
                        &self.theme,
                        save_local_id("save-equipment-item", document, unit.source_index),
                        equipment.slot_label.clone(),
                        equipment.item_name.clone(),
                    )
                    .into_any_element(),
                ],
            )
            .into_any_element(),
        ];
        if self.save_dictionary().is_some()
            && equipment.name_availability == save::SaveNameAvailability::Unavailable
        {
            content
                .push(save::inline_name_unavailable(&self.theme, "Equipment").into_any_element());
        }
        content.push(
            save::group(
                &self.theme,
                "ATTRIBUTES",
                equipment
                    .attributes
                    .iter()
                    .map(|attribute| {
                        save::text_value_row(
                            &self.theme,
                            projection_element_id("save-equipment-attribute", attribute.id),
                            format!("{} · raw {}", attribute.name, attribute.raw_index),
                            attribute.effect.clone().unwrap_or_else(|| {
                                "No catalog effect is available; raw value preserved.".to_owned()
                            }),
                        )
                        .into_any_element()
                    })
                    .collect(),
            )
            .into_any_element(),
        );
        for group in [
            SaveEquipmentGroup::Core,
            SaveEquipmentGroup::Skills,
            SaveEquipmentGroup::Resistances,
            SaveEquipmentGroup::Advanced,
        ] {
            let fields = equipment
                .fields
                .iter()
                .filter(|field| equipment_field_group(field.target) == Some(group))
                .map(|field| self.save_number_row(field, cx))
                .collect();
            content.push(save::group(&self.theme, group.label(), fields).into_any_element());
        }
        content
    }

    fn save_roster_view(
        &self,
        document: DocumentID,
        player_leaders: SaveRows,
        world_map_rows: SaveRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let leader_count = player_leaders.len();
        let leader_summary = if player_leaders.is_empty() {
            save::empty_state(&self.theme, "No player leaders were found.")
                .id("save-player-leaders-empty")
                .debug_selector(|| "save-player-leaders-empty".to_owned())
                .into_any_element()
        } else {
            save::uniform_save_rows(
                save_local_id("save-player-leader-list", document, 0),
                player_leaders,
                cx.processor(move |frame, location, _, _| {
                    frame.save_virtual_player_leader_row(document, location)
                }),
            )
            .size_full()
            .into_any_element()
        };
        let row_count = world_map_rows.len();
        let list = if world_map_rows.is_empty() {
            save::empty_state(&self.theme, "This save has no world-map rows.")
                .id("save-roster-empty")
                .debug_selector(|| "save-roster-empty".to_owned())
                .size_full()
                .into_any_element()
        } else {
            save::uniform_save_rows(
                save_local_id("save-roster-list", document, 0),
                world_map_rows,
                cx.processor(move |frame, location, _, cx| {
                    frame.save_virtual_roster_row(document, location, cx)
                }),
            )
            .size_full()
            .into_any_element()
        };

        div().size_full().child(
            div()
                .id("save-roster")
                .debug_selector(|| "save-roster".to_owned())
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .child(save::section_header(
                    &self.theme,
                    "Roster",
                    format!("{leader_count} player leaders · {row_count} virtual world-map rows"),
                ))
                .child(
                    div()
                        .id("save-player-leader-list-panel")
                        .flex_none()
                        .h(px(180.0))
                        .min_h_0()
                        .overflow_hidden()
                        .child(leader_summary),
                )
                .child(
                    div()
                        .flex_none()
                        .h(px(34.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .bg(self.theme.surface)
                        .border_y_1()
                        .border_color(self.theme.border)
                        .text_size(px(11.0))
                        .text_color(self.theme.text_dim)
                        .child("WORLD MAP STATE"),
                )
                .child(div().flex_1().min_h_0().overflow_hidden().child(list)),
        )
    }

    fn save_virtual_player_leader_row(
        &self,
        document: DocumentID,
        location: SaveRowLocation,
    ) -> AnyElement {
        match save::row_projection(&self.workspace, document, self.save_dictionary(), location) {
            Ok(SaveRowProjection::Unit(row)) => save::player_leader_row(
                &self.theme,
                projection_element_id("save-player-leader", row.id),
                &row,
            )
            .debug_selector(|| "save-player-leader-row".to_owned())
            .into_any_element(),
            Ok(_) => save::empty_state(&self.theme, "Unexpected save row kind.").into_any_element(),
            Err(error) => save::empty_state(&self.theme, format!("Could not read leader: {error}"))
                .into_any_element(),
        }
    }

    fn save_virtual_roster_row(
        &self,
        document: DocumentID,
        location: SaveRowLocation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match save::row_projection(&self.workspace, document, self.save_dictionary(), location) {
            Ok(SaveRowProjection::Roster(row)) => {
                let fields = row
                    .fields
                    .iter()
                    .map(|field| self.save_number_row(field, cx))
                    .collect();
                save::roster_row_with_fields(
                    &self.theme,
                    projection_element_id("save-roster-row", row.id),
                    &row,
                    fields,
                )
                .into_any_element()
            }
            Ok(_) => save::empty_state(&self.theme, "Unexpected save row kind.").into_any_element(),
            Err(error) => save::empty_state(&self.theme, format!("Could not read row: {error}"))
                .into_any_element(),
        }
    }

    fn save_missions_view(
        &self,
        document: DocumentID,
        mission: &save::SaveMissionProjection,
        second_array_rows: SaveRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let second_count = second_array_rows.len();
        let second_panel = if second_array_rows.is_empty() {
            None
        } else {
            let second_list = self.save_second_array_list(document, second_array_rows, cx);
            Some(self.save_second_array_panel(second_list))
        };
        let fixed_scroll = div()
            .id("save-mission-fixed-scroll")
            .min_h_0()
            .overflow_y_scroll()
            .p(px(14.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .children(self.save_mission_fixed(mission, cx));
        let fixed_scroll = if second_count == 0 {
            fixed_scroll.flex_1()
        } else {
            fixed_scroll.flex_none().w(px(430.0))
        };

        div().size_full().child(
            div()
                .id("save-missions")
                .debug_selector(|| "save-missions".to_owned())
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .child(save::section_header(
                    &self.theme,
                    "Missions",
                    format!("20 completion slots · {second_count} second-array rows"),
                ))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .child(fixed_scroll)
                        .children(second_panel),
                ),
        )
    }

    fn save_second_array_list(
        &self,
        document: DocumentID,
        second_array_rows: SaveRows,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if second_array_rows.is_empty() {
            save::empty_state(&self.theme, "This save has no second-array rows.")
                .size_full()
                .into_any_element()
        } else {
            save::uniform_save_rows(
                save_local_id("save-second-array-list", document, 0),
                second_array_rows,
                cx.processor(move |frame, location, _, cx| {
                    frame.save_virtual_second_array_row(document, location, cx)
                }),
            )
            .size_full()
            .into_any_element()
        }
    }

    fn save_mission_fixed(
        &self,
        mission: &save::SaveMissionProjection,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut fixed = vec![
            save::group(
                &self.theme,
                "CURRENT MISSION",
                vec![self.save_number_row(&mission.current_mission, cx)],
            )
            .into_any_element(),
        ];
        fixed.push(
            save::mission_completion_group(
                &self.theme,
                mission
                    .completions
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        self.save_number_row_with_label(field, format!("Mission {}", index + 1), cx)
                    })
                    .collect(),
            )
            .into_any_element(),
        );
        fixed
    }

    fn save_second_array_panel(&self, second_list: AnyElement) -> gpui::Stateful<Div> {
        div()
            .id("save-second-array-panel")
            .debug_selector(|| "save-second-array-panel".to_owned())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_l_1()
            .border_color(self.theme.border)
            .child(
                div()
                    .flex_none()
                    .h(px(36.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .bg(self.theme.surface)
                    .text_size(px(11.0))
                    .text_color(self.theme.text_dim)
                    .child("SECOND ARRAY"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(second_list),
            )
    }

    fn save_virtual_second_array_row(
        &self,
        document: DocumentID,
        location: SaveRowLocation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match save::row_projection(&self.workspace, document, self.save_dictionary(), location) {
            Ok(SaveRowProjection::SecondArray(field)) => self.save_number_row_with_label(
                &field,
                format!("Second Array {}", location.source_index + 1),
                cx,
            ),
            Ok(_) => save::empty_state(&self.theme, "Unexpected save row kind.").into_any_element(),
            Err(error) => save::empty_state(&self.theme, format!("Could not read row: {error}"))
                .into_any_element(),
        }
    }

    fn save_number_row(&self, field: &SaveNumberProjection, cx: &mut Context<Self>) -> AnyElement {
        self.save_number_row_with_label(field, field.label.clone(), cx)
    }

    fn save_number_row_with_label(
        &self,
        field: &SaveNumberProjection,
        label: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = field.id.document;
        let target = field.target;
        let raw_value = field.raw_value;
        let editor = field.editor;
        let selector = save_number_selector(target);
        match editor {
            SaveEditor::Number { .. } => {
                let active_edit = self
                    .number_edit
                    .as_ref()
                    .filter(|edit| edit.target.is_save(document, target));
                let display = active_edit.map_or_else(
                    || field.display_value.clone(),
                    |edit| edit.editor.draft().to_owned(),
                );
                save::editable_value_row(
                    &self.theme,
                    projection_element_id("save-number", field.id),
                    label,
                    display,
                    active_edit.is_some(),
                    active_edit
                        .is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
                )
                .debug_selector(move || selector.clone())
                .tab_stop(true)
                .on_click(cx.listener(move |frame, _, window, cx| {
                    if frame.active_document != Some(document) {
                        return;
                    }
                    let Some(edit) =
                        super::ActiveNumberEdit::save(document, target, raw_value, editor)
                    else {
                        return;
                    };
                    frame.begin_number_edit(edit);
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element()
            }
            SaveEditor::Choice { choices } => {
                let known_current = choices.iter().any(|choice| choice.value == raw_value);
                let buttons = choices
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, choice)| {
                        let action = SetSaveChoice {
                            document,
                            target,
                            value: choice.value,
                        };
                        let choice_selector = save_choice_selector(target, choice.value);
                        components::choice_button(
                            &self.theme,
                            projection_choice_element_id(field.id, index),
                            choice.label,
                            choice.value == raw_value,
                        )
                        .debug_selector(move || choice_selector.clone())
                        .tab_stop(true)
                        .on_click(move |_, window, cx| {
                            window.dispatch_action(Box::new(action), cx);
                        })
                        .into_any_element()
                    })
                    .collect();
                save::choice_value_row(
                    &self.theme,
                    projection_element_id("save-choice", field.id),
                    label,
                    field.display_value.clone(),
                    (!known_current).then(|| unknown_choice_selector(target)),
                    buttons,
                )
                .debug_selector(move || selector.clone())
                .into_any_element()
            }
        }
    }

    fn save_section_rail(
        &self,
        document: DocumentID,
        selected: SaveSection,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let unit_count = self.workspace.save_unit_count(document).unwrap_or(0);
        let roster_count = self.workspace.save_roster_count(document).unwrap_or(0);
        SAVE_SECTIONS
            .into_iter()
            .map(|section| {
                save::section_rail_item(
                    &self.theme,
                    save_section_id(section),
                    save_section_label(section, unit_count, roster_count),
                    selected == section,
                )
                .debug_selector(move || save_section_id(section).to_owned())
                .tab_stop(true)
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_save_section(document, section, cx);
                    window.focus(&frame.focus);
                }))
                .into_any_element()
            })
            .collect()
    }

    fn save_catalog_status_element(&self) -> Option<AnyElement> {
        match self.save_catalog.status() {
            SaveCatalogStatus::NotConfigured => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-not-configured",
                    "Crusaders installation is not configured",
                    Some("Raw IDs remain available without game names.".to_owned()),
                )
                .into_any_element(),
            ),
            SaveCatalogStatus::Dormant => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-dormant",
                    "Crusaders names are unavailable",
                    Some("Raw IDs remain available.".to_owned()),
                )
                .into_any_element(),
            ),
            SaveCatalogStatus::Loading { .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-loading",
                    "Loading Crusaders names",
                    Some("The save remains readable as raw values.".to_owned()),
                )
                .into_any_element(),
            ),
            SaveCatalogStatus::Failed { error, .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-failed",
                    "Could not load Crusaders names",
                    Some(format!("{error}. Raw IDs remain available.")),
                )
                .into_any_element(),
            ),
            SaveCatalogStatus::Ready { issue_count: 0, .. } => None,
            SaveCatalogStatus::Ready { issue_count, .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-ready-issues",
                    format!("Loaded names with {issue_count} catalog issues"),
                    Some("Some records can use raw fallback labels.".to_owned()),
                )
                .into_any_element(),
            ),
        }
    }

    fn save_dictionary(&self) -> Option<&NameDictionary> {
        match self.save_catalog.status() {
            SaveCatalogStatus::Ready { dictionary, .. } => Some(dictionary.as_ref()),
            SaveCatalogStatus::NotConfigured
            | SaveCatalogStatus::Dormant
            | SaveCatalogStatus::Loading { .. }
            | SaveCatalogStatus::Failed { .. } => None,
        }
    }

    fn select_save_section(
        &mut self,
        document: DocumentID,
        section: SaveSection,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.select_section(document, section, draft_active),
            cx,
        );
    }

    fn inspect_save_unit(&mut self, document: DocumentID, unit: usize, cx: &mut Context<Self>) {
        if self.active_document != Some(document) {
            return;
        }
        let filter = self
            .save_presentations
            .get(document)
            .map_or("", SavePresentationState::unit_filter)
            .to_owned();
        let Ok(rows) = self.save_unit_rows(document, &filter) else {
            return;
        };
        let visibility = rows
            .unit_visibility()
            .unwrap_or(SaveUnitVisibility::All { unit_count: 0 });
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.inspect_unit(document, unit, visibility, draft_active),
            cx,
        );
    }

    fn select_save_equipment_slot(
        &mut self,
        document: DocumentID,
        slot: SaveEquipmentSlot,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.select_equipment_slot(document, slot, draft_active),
            cx,
        );
    }

    fn set_save_unit_filter(&mut self, document: DocumentID, filter: &str, cx: &mut Context<Self>) {
        if self.active_document != Some(document) {
            return;
        }
        let Ok(rows) = self.save_unit_rows(document, filter) else {
            return;
        };
        let visibility = rows
            .unit_visibility()
            .unwrap_or(SaveUnitVisibility::All { unit_count: 0 });
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| {
                states.set_unit_filter(document, filter.to_owned(), visibility, draft_active)
            },
            cx,
        );
    }

    fn save_unit_rows(
        &self,
        document: DocumentID,
        filter: &str,
    ) -> save::SaveProjectionResult<SaveRows> {
        SaveRows::units(&self.workspace, document, self.save_dictionary(), filter)
    }

    pub(super) fn reconcile_save_presentation(
        &mut self,
        document: DocumentID,
        cx: &mut Context<Self>,
    ) {
        let filter = self
            .save_presentations
            .get(document)
            .map_or("", SavePresentationState::unit_filter)
            .to_owned();
        let Ok(rows) = self.save_unit_rows(document, &filter) else {
            return;
        };
        let visibility = rows
            .unit_visibility()
            .unwrap_or(SaveUnitVisibility::All { unit_count: 0 });
        let draft_active = self
            .number_edit
            .as_ref()
            .is_some_and(|edit| edit.target.document() == document)
            || self
                .text_edit
                .as_ref()
                .is_some_and(|edit| edit.target.document() == document);
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.reconcile_document(document, visibility, draft_active),
            cx,
        );
    }

    const fn save_draft_active(&self) -> bool {
        self.number_edit.is_some() || self.text_edit.is_some()
    }

    #[allow(
        dead_code,
        reason = "Task 12 covers document-close cancellation before tab close controls are exposed"
    )]
    fn remove_save_presentation(&mut self, document: DocumentID, cx: &mut Context<Self>) {
        let draft_active = self
            .number_edit
            .as_ref()
            .is_some_and(|edit| edit.target.document() == document)
            || self
                .text_edit
                .as_ref()
                .is_some_and(|edit| edit.target.document() == document);
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.remove_document(document, draft_active),
            cx,
        );
        if self.active_document == Some(document) {
            self.active_document = None;
            cx.notify();
        }
    }

    fn apply_save_presentation_transition(
        &mut self,
        draft_active: bool,
        mut apply: impl FnMut(
            &mut crate::state::SavePresentationStates,
            bool,
        ) -> SavePresentationTransition,
        cx: &mut Context<Self>,
    ) {
        let mut preview = self.save_presentations.clone();
        let preview_transition = apply(&mut preview, draft_active);
        if preview_transition.cancels_draft() {
            self.cancel_property_edit();
        }

        let transition = apply(&mut self.save_presentations, false);
        if transition.changed() {
            cx.notify();
        }
    }
}

fn unit_field_group(target: SaveNumberTarget) -> Option<SaveUnitGroup> {
    match target {
        SaveNumberTarget::Unit { field, .. } => Some(field.group()),
        _ => None,
    }
}

fn equipment_field_group(target: SaveNumberTarget) -> Option<SaveEquipmentGroup> {
    match target {
        SaveNumberTarget::Equipment { field, .. } => Some(field.group()),
        _ => None,
    }
}

const fn equipment_slot_selector(slot: SaveEquipmentSlot, enabled: bool) -> &'static str {
    match (slot, enabled) {
        (SaveEquipmentSlot::LeaderWeapon, true) => "save-equipment-slot-leader-weapon",
        (SaveEquipmentSlot::LeaderAccessory, true) => "save-equipment-slot-leader-accessory",
        (SaveEquipmentSlot::LeaderArmor, true) => "save-equipment-slot-leader-armor",
        (SaveEquipmentSlot::TroopWeapon, true) => "save-equipment-slot-troop-weapon",
        (SaveEquipmentSlot::TroopAccessory, true) => "save-equipment-slot-troop-accessory",
        (SaveEquipmentSlot::TroopArmor, true) => "save-equipment-slot-troop-armor",
        (SaveEquipmentSlot::LeaderWeapon, false) => "save-equipment-slot-leader-weapon-disabled",
        (SaveEquipmentSlot::LeaderAccessory, false) => {
            "save-equipment-slot-leader-accessory-disabled"
        }
        (SaveEquipmentSlot::LeaderArmor, false) => "save-equipment-slot-leader-armor-disabled",
        (SaveEquipmentSlot::TroopWeapon, false) => "save-equipment-slot-troop-weapon-disabled",
        (SaveEquipmentSlot::TroopAccessory, false) => {
            "save-equipment-slot-troop-accessory-disabled"
        }
        (SaveEquipmentSlot::TroopArmor, false) => "save-equipment-slot-troop-armor-disabled",
    }
}

fn save_section_label(section: SaveSection, unit_count: usize, roster_count: usize) -> String {
    match section {
        SaveSection::Summary => "Summary".to_owned(),
        SaveSection::Units => format!("Units · {unit_count}"),
        SaveSection::Equipment => "Equipment".to_owned(),
        SaveSection::Roster => format!("Roster · {roster_count}"),
        SaveSection::Missions => "Missions".to_owned(),
    }
}

const fn save_section_id(section: SaveSection) -> &'static str {
    match section {
        SaveSection::Summary => "save-section-summary",
        SaveSection::Units => "save-section-units",
        SaveSection::Equipment => "save-section-equipment",
        SaveSection::Roster => "save-section-roster",
        SaveSection::Missions => "save-section-missions",
    }
}

fn projection_element_id(prefix: &'static str, id: SaveProjectionID) -> SharedString {
    format!("{prefix}:{id:?}").into()
}

fn projection_choice_element_id(id: SaveProjectionID, index: usize) -> SharedString {
    format!("save-choice:{id:?}:{index}").into()
}

fn save_number_selector(target: SaveNumberTarget) -> String {
    match target {
        SaveNumberTarget::CampaignIndex => "save-number-campaign".to_owned(),
        SaveNumberTarget::Main(field) => format!(
            "save-number-main-{}",
            selector_slug(field.label()).replace("0x", "")
        ),
        SaveNumberTarget::SelectedUnit => "save-number-selected-unit-reference".to_owned(),
        SaveNumberTarget::Unit { unit, field } => {
            format!("save-number-unit-{unit}-{}", selector_slug(field.label()))
        }
        SaveNumberTarget::Equipment { unit, slot, field } => format!(
            "save-number-equipment-{unit}-{}-{}",
            selector_slug(slot.label()),
            selector_slug(field.label())
        ),
        SaveNumberTarget::Roster { record: 0, field } => {
            format!("save-roster-field-{}", selector_slug(field.label()))
        }
        SaveNumberTarget::Roster { record, field } => format!(
            "save-roster-{record}-field-{}",
            selector_slug(field.label())
        ),
        SaveNumberTarget::MissionCompletion { slot } => {
            format!("save-mission-completion-{slot}")
        }
        SaveNumberTarget::CurrentMissionIndex => "save-number-current-mission-index".to_owned(),
        SaveNumberTarget::SecondArray { record } => {
            format!("save-number-second-array-{record}")
        }
    }
}

fn save_choice_selector(target: SaveNumberTarget, value: i64) -> String {
    match target {
        SaveNumberTarget::CampaignIndex => format!("save-choice-campaign-{value}"),
        _ => format!("save-choice:{}:{value}", save_number_selector(target)),
    }
}

fn unknown_choice_selector(target: SaveNumberTarget) -> String {
    match target {
        SaveNumberTarget::CampaignIndex => "save-choice-current-unknown".to_owned(),
        _ => format!("save-choice-unknown:{}", save_number_selector(target)),
    }
}

const fn save_text_selector(field: kufeditor_workspace::SaveTextField) -> &'static str {
    match field {
        kufeditor_workspace::SaveTextField::MapName => "save-text-map-name",
        kufeditor_workspace::SaveTextField::SetFile => "save-text-set-file",
        kufeditor_workspace::SaveTextField::SkyEffects => "save-text-sky-effects",
    }
}

const fn save_text_editor_selector(field: kufeditor_workspace::SaveTextField) -> &'static str {
    match field {
        kufeditor_workspace::SaveTextField::MapName => "save-text-editor-map-name",
        kufeditor_workspace::SaveTextField::SetFile => "save-text-editor-set-file",
        kufeditor_workspace::SaveTextField::SkyEffects => "save-text-editor-sky-effects",
    }
}

fn selector_slug(label: &str) -> String {
    let mut slug = String::with_capacity(label.len());
    let mut separator = false;
    for character in label.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    slug
}

fn save_local_id(prefix: &'static str, document: DocumentID, index: usize) -> SharedString {
    format!("{prefix}:{document:?}:{index}").into()
}

const fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

fn empty_label(value: &str) -> String {
    if value.is_empty() {
        "—".to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled GPUI and save fixtures make failures fatal"
    )]

    use std::{fs, path::PathBuf, sync::Arc};

    use gpui::{
        AppContext, ClipboardItem, Entity, Modifiers, TestAppContext, VisualTestContext, point, px,
        size,
    };
    use kufeditor_game::{CatalogRole, Game, InstallationError, load_name_dictionary};
    use kufeditor_workspace::{
        ApplyOutcome, Document, DocumentEdit, DocumentID, SaveDocument, SaveEquipmentField,
        SaveEquipmentSlot, SaveMainField, SaveNumberTarget, SaveRosterField, SaveTextField,
        SaveUnitField,
    };

    use super::{AppFrame, SAVE_SECTIONS, UNIT_FILTERS, save_section_id, save_section_label};
    use crate::{
        actions::{Redo, Undo},
        catalog_status::CatalogRequestError,
        notices::{Notice, NoticeSource},
        save_catalog_status::SaveCatalogKey,
        settings::SettingsStartup,
        state::{Area, SaveSection},
        test_support::SaveFixture,
        text_input::{TextInput, TextInputEvent},
        views::save,
    };

    #[test]
    fn save_view_section_rail_has_stable_labels_and_focus_order() {
        assert_eq!(
            SAVE_SECTIONS.map(|section| save_section_label(section, 3, 7)),
            [
                "Summary",
                "Units · 3",
                "Equipment",
                "Roster · 7",
                "Missions",
            ],
        );
        assert_eq!(
            SAVE_SECTIONS.map(save_section_id),
            [
                "save-section-summary",
                "save-section-units",
                "save-section-equipment",
                "save-section-roster",
                "save-section-missions",
            ],
        );
        assert_eq!(SAVE_SECTIONS.first(), Some(&SaveSection::Summary));
    }

    #[test]
    fn save_view_unit_filter_controls_keep_stable_values() {
        assert_eq!(
            UNIT_FILTERS,
            [
                ("All", ""),
                ("Leaders", "leader"),
                ("Officers", "officer"),
                ("Troops", "troop"),
            ],
        );
    }

    #[gpui::test]
    fn save_view_draws_each_real_section_from_the_app_frame(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(1, 1, 1).with_unit_roles([0]).build(),
        );

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-summary").is_some());
        assert!(cx.debug_bounds("save-summary-envelope").is_some());

        select_and_draw(&frame, cx, document, SaveSection::Units);
        assert!(cx.debug_bounds("save-units").is_some());
        assert!(cx.debug_bounds("save-unit-master-row-0").is_some());

        select_and_draw(&frame, cx, document, SaveSection::Equipment);
        assert!(cx.debug_bounds("save-equipment").is_some());
        assert!(
            cx.debug_bounds("save-equipment-slot-leader-weapon")
                .is_some()
        );

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        assert!(cx.debug_bounds("save-roster").is_some());
        assert!(cx.debug_bounds("save-player-leader-row").is_some());
        assert!(cx.debug_bounds("save-roster-field-byte-60").is_some());

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        assert!(cx.debug_bounds("save-missions").is_some());
        let first = cx.debug_bounds("save-mission-completion-0").unwrap();
        let second = cx.debug_bounds("save-mission-completion-1").unwrap();
        assert_eq!(first.origin.y, second.origin.y);
        assert_ne!(first.origin.x, second.origin.x);
        assert!(cx.debug_bounds("save-second-array-panel").is_some());
    }

    #[gpui::test]
    fn save_view_clicks_wire_sections_filters_units_and_equipment_slots(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 0]).build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        assert_eq!(
            save_state(&frame, cx, document).section(),
            SaveSection::Units
        );

        click(cx, "save-unit-filter-troops");
        let state = save_state(&frame, cx, document);
        assert_eq!(state.unit_filter(), "troop");
        assert_eq!(state.inspected_unit(), 0);
        assert!(cx.debug_bounds("save-unit-master-row-0").is_some());

        click(cx, "save-unit-filter-all");
        click(cx, "save-unit-master-row-1");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 1);

        click(cx, "save-section-equipment");
        assert_eq!(
            save_state(&frame, cx, document).section(),
            SaveSection::Equipment
        );
        click(cx, "save-equipment-slot-troop-armor");
        assert_eq!(
            save_state(&frame, cx, document).equipment_slot(),
            SaveEquipmentSlot::TroopArmor
        );
        assert!(cx.debug_bounds("save-equipment").is_some());
    }

    #[gpui::test]
    fn save_edit_numeric_field_opens_with_format_bounds(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        cx.simulate_keystrokes("down");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.number_edit.as_ref().map(|edit| edit.editor.draft()),
                Some("0"),
            );
        });

        cx.simulate_keystrokes("escape");
        frame.update(cx, |frame, cx| {
            assert_eq!(
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetSaveNumber {
                            target: SaveNumberTarget::Main(SaveMainField::Field00),
                            value: i64::from(u32::MAX),
                        },
                    )
                    .unwrap(),
                ApplyOutcome::Changed,
            );
            cx.notify();
        });
        draw_frame(cx, &frame);
        click(cx, "save-number-main-field-00");
        cx.simulate_keystrokes("up");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.number_edit.as_ref().map(|edit| edit.editor.draft()),
                Some("4294967295"),
            );
        });
    }

    #[gpui::test]
    fn save_edit_lazy_numeric_sections_dispatch_their_exact_typed_targets(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(1, 1, 1).build());

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-number-unit-0-troop-info-index");
        cx.simulate_keystrokes("9 enter");

        click(cx, "save-section-equipment");
        click(cx, "save-number-equipment-0-leader-weapon-level");
        cx.simulate_keystrokes("6 enter");

        click(cx, "save-section-roster");
        click(cx, "save-roster-field-byte-60");
        cx.simulate_keystrokes("7 enter");

        click(cx, "save-section-missions");
        click(cx, "save-number-current-mission-index");
        cx.simulate_keystrokes("3 enter");
        click(cx, "save-mission-completion-0");
        cx.simulate_keystrokes("4 enter");
        click(cx, "save-number-second-array-0");
        cx.simulate_keystrokes("5 enter");

        frame.update(cx, |frame, _| {
            for (target, expected) in [
                (
                    SaveNumberTarget::Unit {
                        unit: 0,
                        field: SaveUnitField::TroopInfoIndex,
                    },
                    9,
                ),
                (
                    SaveNumberTarget::Equipment {
                        unit: 0,
                        slot: SaveEquipmentSlot::LeaderWeapon,
                        field: SaveEquipmentField::Level,
                    },
                    6,
                ),
                (
                    SaveNumberTarget::Roster {
                        record: 0,
                        field: SaveRosterField::Byte60,
                    },
                    7,
                ),
                (SaveNumberTarget::CurrentMissionIndex, 3),
                (SaveNumberTarget::MissionCompletion { slot: 0 }, 4),
                (SaveNumberTarget::SecondArray { record: 0 }, 5),
            ] {
                assert_eq!(
                    frame.workspace.save_number(document, target).unwrap(),
                    expected
                );
            }
        });
    }

    #[gpui::test]
    fn save_edit_choice_uses_metadata_and_keeps_unknown_current_value(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(1, 0, 0).with_unit_roles([99]).build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        assert!(
            cx.debug_bounds("save-choice-unknown:save-number-unit-0-ucd")
                .is_some()
        );
        assert!(
            cx.debug_bounds("save-choice:save-number-unit-0-ucd:99")
                .is_none()
        );
        click(cx, "save-choice:save-number-unit-0-ucd:1");

        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Unit {
                            unit: 0,
                            field: SaveUnitField::UCD,
                        },
                    )
                    .unwrap(),
                1,
            );
            assert!(frame.workspace.is_dirty(document).unwrap());
            assert!(frame.workspace.can_undo(document).unwrap());
        });
    }

    #[gpui::test]
    fn save_edit_filtered_choice_reconciles_the_inspected_unit_and_hidden_draft(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 3]).build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-unit-filter-troops");
        click(cx, "save-unit-master-row-0");
        click(cx, "save-number-unit-0-troop-info-index");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_some()));
        draw_frame(cx, &frame);

        click(cx, "save-choice:save-number-unit-0-ucd:1");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Unit {
                            unit: 0,
                            field: SaveUnitField::UCD,
                        },
                    )
                    .unwrap(),
                1,
            );
            assert_eq!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                1,
            );
            assert!(frame.number_edit.is_none());
        });

        cx.run_until_parked();
        assert!(cx.debug_bounds("save-unit-master-row-1").is_some());
        assert!(
            cx.debug_bounds("save-number-unit-1-troop-info-index")
                .is_some()
        );
    }

    #[gpui::test]
    fn save_edit_undo_and_redo_actions_reconcile_filtered_units_and_redraw(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 3]).build(),
        );
        frame.update(cx, |frame, cx| {
            assert_eq!(
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetSaveNumber {
                            target: SaveNumberTarget::Unit {
                                unit: 0,
                                field: SaveUnitField::UCD,
                            },
                            value: 1,
                        },
                    )
                    .unwrap(),
                ApplyOutcome::Changed,
            );
            cx.notify();
        });

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-unit-filter-troops");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 1);
        let row_one_before_undo = cx.debug_bounds("save-unit-master-row-1").unwrap();

        frame.update_in(cx, |_, window, cx| {
            window.dispatch_action(Box::new(Undo), cx);
        });
        cx.run_until_parked();
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Unit {
                            unit: 0,
                            field: SaveUnitField::UCD,
                        },
                    )
                    .unwrap(),
                3,
            );
            assert_eq!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                1,
            );
        });
        let row_one_after_undo = cx.debug_bounds("save-unit-master-row-1").unwrap();
        assert!(row_one_after_undo.origin.y > row_one_before_undo.origin.y);
        click(cx, "save-unit-master-row-0");
        click(cx, "save-number-unit-0-troop-info-index");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_some()));
        let row_one_before_redo = cx.debug_bounds("save-unit-master-row-1").unwrap();

        frame.update_in(cx, |_, window, cx| {
            window.dispatch_action(Box::new(Redo), cx);
        });
        cx.run_until_parked();
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Unit {
                            unit: 0,
                            field: SaveUnitField::UCD,
                        },
                    )
                    .unwrap(),
                1,
            );
            assert_eq!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                1,
            );
            assert!(frame.number_edit.is_none());
        });
        let row_one_after_redo = cx.debug_bounds("save-unit-master-row-1").unwrap();
        assert!(row_one_after_redo.origin.y < row_one_before_redo.origin.y);
        assert_eq!(row_one_after_redo.origin.y, row_one_before_undo.origin.y);
        assert!(
            cx.debug_bounds("save-number-unit-1-troop-info-index")
                .is_some()
        );
    }

    #[gpui::test]
    fn save_edit_fixed_text_commits_and_invalid_draft_stays_visible(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-text-set-file");
        frame.update(cx, |frame, cx| {
            assert_eq!(
                frame
                    .text_edit
                    .as_ref()
                    .map(|edit| edit.input.read(cx).content()),
                Some("Alpha"),
            );
        });
        cx.simulate_keystrokes("B r a v o enter");
        cx.run_until_parked();
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Bravo",
            );
            assert!(frame.text_edit.is_none());
        });

        draw_frame(cx, &frame);
        click(cx, "save-text-set-file");
        cx.simulate_keystrokes(
            "x x x x x x x x x x x x x x x x x x x x x x x x x x x x x x x x enter",
        );
        cx.run_until_parked();
        let input = frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(
                edit.input.read(cx).content(),
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
            );
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Bravo",
            );
            edit.input.clone()
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-text-validation-error").is_some());

        cx.simulate_keystrokes("backspace");
        frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input, input);
            assert_eq!(edit.input.read(cx).content().len(), 31);
            assert!(edit.validation_error.is_none());
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Bravo",
            );
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-text-editor-set-file").is_some());
    }

    #[gpui::test]
    fn save_edit_fixed_text_keeps_ascii_and_zero_errors_beside_the_exact_draft(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-text-set-file");
        frame.update(cx, |_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("é".to_owned()));
        });
        cx.simulate_keystrokes("cmd-v enter");
        let ascii_input = frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input.read(cx).content(), "é");
            assert!(
                edit.validation_error
                    .as_deref()
                    .is_some_and(|error| error.contains("non-ASCII byte"))
            );
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Alpha",
            );
            edit.input.clone()
        });

        cx.simulate_keystrokes("backspace");
        frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input, ascii_input);
            assert_eq!(edit.input.read(cx).content(), "");
            assert!(edit.validation_error.is_none());
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Alpha",
            );
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-text-editor-set-file").is_some());

        cx.simulate_keystrokes("escape");
        draw_frame(cx, &frame);
        click(cx, "save-text-set-file");
        frame.update(cx, |_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("\0".to_owned()));
        });
        cx.simulate_keystrokes("cmd-v enter");
        let zero_input = frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input.read(cx).content(), "\0");
            assert!(
                edit.validation_error
                    .as_deref()
                    .is_some_and(|error| error.contains("zero byte"))
            );
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Alpha",
            );
            edit.input.clone()
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-text-validation-error").is_some());

        cx.simulate_keystrokes("backspace");
        frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input, zero_input);
            assert_eq!(edit.input.read(cx).content(), "");
            assert!(edit.validation_error.is_none());
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Alpha",
            );
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-text-editor-set-file").is_some());
    }

    #[gpui::test]
    fn save_edit_stale_content_change_keeps_the_active_validation(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );
        let stale = start_hidden_save_text(&frame, cx, document);
        frame.update_in(cx, |frame, window, cx| {
            frame.start_text_edit(
                crate::frame::TextEditTarget::save(document, SaveTextField::MapName),
                "Map".to_owned(),
                window,
                cx,
            );
            frame.text_edit.as_mut().unwrap().validation_error =
                Some("active validation".to_owned());
        });

        stale.update(cx, |_, cx| cx.emit(TextInputEvent::ContentChanged));
        cx.run_until_parked();

        frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input.read(cx).content(), "Map");
            assert_eq!(edit.validation_error.as_deref(), Some("active validation"));
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::MapName)
                    .unwrap(),
                "",
            );
            assert!(!frame.workspace.is_dirty(document).unwrap());
        });
    }

    #[gpui::test]
    fn save_edit_preserves_shell_catalog_and_notice_state_while_names_load_or_fail(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());
        let (global_request, save_key, settings_revision, settings_settled, task_launches) = frame
            .update(cx, |frame, cx| {
                let global_request = frame.shell.begin_catalog();
                let save_key = SaveCatalogKey::new(
                    frame.shell.begin_save_catalog(),
                    PathBuf::from("/catalog/editing"),
                );
                frame.save_catalog.begin(save_key.clone());
                frame.notices.replace(
                    NoticeSource::Workspace,
                    Notice::info("Persistent workspace notice"),
                );
                cx.notify();
                (
                    global_request,
                    save_key,
                    frame.settings.latest_revision_for_test(),
                    frame.settings.is_settled(),
                    frame.task_launches,
                )
            });

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-loading").is_some());
        click(cx, "save-number-main-field-00");
        cx.simulate_keystrokes("enter");
        frame.update(cx, |frame, cx| {
            assert!(frame.shell.accepts_catalog(global_request));
            assert!(frame.shell.accepts_save_catalog(save_key.request()));
            assert_eq!(frame.shell.game(), Game::Crusaders);
            assert_eq!(frame.shell.area(), Area::Files);
            assert_eq!(frame.settings.latest_revision_for_test(), settings_revision);
            assert_eq!(frame.settings.is_settled(), settings_settled);
            assert_eq!(frame.task_launches, task_launches);
            assert_eq!(
                frame.notices.current().map(Notice::summary),
                Some("Persistent workspace notice"),
            );
            assert!(frame.save_catalog.finish_failed(
                save_key.clone(),
                CatalogRequestError::Installation(InstallationError::RootMissing {
                    game: Game::Crusaders,
                    root: PathBuf::from("/catalog/editing"),
                }),
            ));
            cx.notify();
        });

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-failed").is_some());
        click(cx, "save-number-main-field-00");
        cx.simulate_keystrokes("7 enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::Main(SaveMainField::Field00))
                    .unwrap(),
                7,
            );
            assert!(frame.shell.accepts_catalog(global_request));
            assert!(frame.shell.accepts_save_catalog(save_key.request()));
            assert_eq!(frame.shell.game(), Game::Crusaders);
            assert_eq!(frame.shell.area(), Area::Files);
            assert_eq!(frame.settings.latest_revision_for_test(), settings_revision);
            assert_eq!(frame.settings.is_settled(), settings_settled);
            assert_eq!(frame.task_launches, task_launches);
            assert_eq!(
                frame.notices.current().map(Notice::summary),
                Some("Persistent workspace notice"),
            );
        });
    }

    #[gpui::test]
    fn save_edit_save_and_save_as_keep_the_sav_extension_policy(cx: &mut TestAppContext) {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source.sav");
        let copy_without_extension = temporary.path().join("copy");
        let copy = temporary.path().join("copy.sav");
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = frame.update(cx, |frame, cx| {
            let document = frame.workspace.open_loaded(
                source.clone(),
                Document::Save(SaveDocument::parse(SaveFixture::new(0, 0, 0).build()).unwrap()),
            );
            frame.activate_document(document, cx);
            frame.shell.select_area(Area::Files);
            assert_eq!(
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetSaveNumber {
                            target: SaveNumberTarget::Main(SaveMainField::Field00),
                            value: 1,
                        },
                    )
                    .unwrap(),
                ApplyOutcome::Changed,
            );
            assert!(frame.start_save(document, None, cx));
            document
        });
        cx.run_until_parked();

        frame.update(cx, |frame, cx| {
            assert_eq!(frame.workspace.path(document).unwrap(), source);
            assert!(!frame.workspace.is_dirty(document).unwrap());
            assert!(source.is_file());
            assert_eq!(
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetSaveNumber {
                            target: SaveNumberTarget::Main(SaveMainField::Field00),
                            value: 2,
                        },
                    )
                    .unwrap(),
                ApplyOutcome::Changed,
            );
            let notice = frame.allocate_workspace_notice_identity();
            frame.notices.begin_pending(NoticeSource::Workspace, notice);
            frame.finish_save_as_prompt(
                notice,
                document,
                crate::frame::SaveAsPromptResult::Selected(copy_without_extension.clone()),
                cx,
            );
        });
        cx.run_until_parked();

        frame.update(cx, |frame, _| {
            assert_eq!(frame.workspace.path(document).unwrap(), copy);
            assert!(!frame.workspace.is_dirty(document).unwrap());
            assert!(copy.is_file());
        });
    }

    #[gpui::test]
    fn save_edit_equal_number_text_and_choice_values_are_history_neutral(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );
        let before = frame.update(cx, |frame, _| {
            frame.close_documents = crate::frame::CloseDocuments::Discard;
            frame.close_pending = true;
            frame.close_armed = true;
            frame.workspace.state_id(document).unwrap()
        });

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        cx.simulate_keystrokes("enter");
        draw_frame(cx, &frame);
        click(cx, "save-text-set-file");
        cx.simulate_keystrokes("enter");
        draw_frame(cx, &frame);
        click(cx, "save-choice-campaign-0");
        cx.run_until_parked();

        frame.update(cx, |frame, _| {
            assert_eq!(frame.workspace.state_id(document).unwrap(), before);
            assert!(!frame.workspace.is_dirty(document).unwrap());
            assert!(!frame.workspace.can_undo(document).unwrap());
            assert!(frame.number_edit.is_none());
            assert!(frame.text_edit.is_none());
            assert_eq!(frame.close_documents, crate::frame::CloseDocuments::Discard);
            assert!(frame.close_pending);
            assert!(frame.close_armed);
        });
    }

    #[gpui::test]
    fn save_edit_undo_and_redo_refresh_projected_values(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        cx.run_until_parked();
        click(cx, "save-choice-campaign-2");
        frame.update(cx, |frame, cx| {
            assert_eq!(
                save::summary_projection(&frame.workspace, document)
                    .unwrap()
                    .campaign
                    .display_value,
                "Ecclesia (Kendal) (2)",
            );
            frame.move_history(false, cx);
            assert_eq!(
                save::summary_projection(&frame.workspace, document)
                    .unwrap()
                    .campaign
                    .display_value,
                "Hironeiden (Gerald) (0)",
            );
            frame.move_history(true, cx);
            assert_eq!(
                save::summary_projection(&frame.workspace, document)
                    .unwrap()
                    .campaign
                    .display_value,
                "Ecclesia (Kendal) (2)",
            );
        });
    }

    #[gpui::test]
    fn hidden_save_draft_closes_before_section_filter_unit_and_slot_changes(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 0]).build(),
        );

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-section-units");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_none()));

        click(cx, "save-number-unit-0-troop-info-index");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-unit-filter-troops");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_none()));

        click(cx, "save-number-unit-0-troop-info-index");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-unit-filter-all");
        click(cx, "save-unit-master-row-1");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_none()));

        click(cx, "save-section-equipment");
        click(cx, "save-number-equipment-1-leader-weapon-level");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-equipment-slot-troop-armor");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Equipment {
                            unit: 1,
                            slot: SaveEquipmentSlot::LeaderWeapon,
                            field: kufeditor_workspace::SaveEquipmentField::Level,
                        },
                    )
                    .unwrap(),
                0,
            );
        });
    }

    #[gpui::test]
    fn hidden_save_draft_closes_before_active_document_and_window_close(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let first = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());
        let second = frame.update(cx, |frame, _| {
            frame.workspace.open_loaded(
                PathBuf::from("second.sav"),
                Document::Save(SaveDocument::parse(SaveFixture::new(0, 0, 0).build()).unwrap()),
            )
        });

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        frame.update(cx, |frame, cx| frame.activate_document(second, cx));
        frame.update(cx, |frame, _| {
            assert_eq!(frame.active_document, Some(second));
            assert!(frame.number_edit.is_none());
            assert!(!frame.workspace.is_dirty(first).unwrap());
        });

        frame.update(cx, |frame, cx| frame.activate_document(first, cx));
        draw_frame(cx, &frame);
        click(cx, "save-number-main-field-00");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_some()));
        assert!(frame.update_in(cx, |frame, window, cx| {
            frame.window_should_close(window, cx)
        }));
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert!(!frame.workspace.is_dirty(first).unwrap());
        });
    }

    #[gpui::test]
    fn hidden_save_draft_closes_before_document_presentation_is_removed(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        frame.update(cx, |frame, cx| {
            assert!(frame.number_edit.is_some());
            frame.remove_save_presentation(document, cx);
            assert!(frame.number_edit.is_none());
            assert!(frame.save_presentations.get(document).is_none());
            assert!(!frame.workspace.is_dirty(document).unwrap());
        });

        frame.update(cx, |frame, cx| frame.activate_document(document, cx));
        let stale = start_hidden_save_text(&frame, cx, document);
        frame.update(cx, |frame, cx| {
            frame.remove_save_presentation(document, cx);
        });
        assert_hidden_text_commit_is_ignored(&frame, cx, document, &stale);
        frame.update(cx, |frame, _| {
            assert!(frame.save_presentations.get(document).is_none());
        });
    }

    #[gpui::test]
    fn hidden_save_draft_text_input_cancels_before_every_context_transition(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let first = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0)
                .with_unit_roles([3, 0])
                .with_save_file_name(b"Alpha")
                .build(),
        );
        let second = frame.update(cx, |frame, _| {
            frame.workspace.open_loaded(
                PathBuf::from("second-text.sav"),
                Document::Save(SaveDocument::parse(SaveFixture::new(0, 0, 0).build()).unwrap()),
            )
        });

        cx.run_until_parked();
        click(cx, "save-text-set-file");
        let stale = active_text_input(&frame, cx);
        click(cx, "save-section-units");
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);

        let stale = start_hidden_save_text(&frame, cx, first);
        click(cx, "save-unit-filter-troops");
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);

        click(cx, "save-unit-filter-all");
        let stale = start_hidden_save_text(&frame, cx, first);
        click(cx, "save-unit-master-row-1");
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);

        click(cx, "save-section-equipment");
        let stale = start_hidden_save_text(&frame, cx, first);
        click(cx, "save-equipment-slot-troop-armor");
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);

        let stale = start_hidden_save_text(&frame, cx, first);
        frame.update(cx, |frame, cx| frame.activate_document(second, cx));
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);

        frame.update(cx, |frame, cx| frame.activate_document(first, cx));
        let stale = start_hidden_save_text(&frame, cx, first);
        assert!(frame.update_in(cx, |frame, window, cx| {
            frame.window_should_close(window, cx)
        }));
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);
    }

    #[gpui::test]
    fn save_view_draws_empty_states_without_duplicate_or_inert_panes(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Units);
        assert!(cx.debug_bounds("save-unit-empty").is_some());
        assert!(cx.debug_bounds("save-detail-panel:save-units").is_some());
        assert!(cx.debug_bounds("save-unit-detail-empty").is_none());

        select_and_draw(&frame, cx, document, SaveSection::Equipment);
        let disabled = cx
            .debug_bounds("save-equipment-slot-troop-armor-disabled")
            .unwrap();
        cx.simulate_click(disabled.center(), Modifiers::none());
        let equipment_slot = frame.update(cx, |frame, _| {
            frame
                .save_presentations
                .get(document)
                .unwrap()
                .equipment_slot()
        });
        assert_eq!(equipment_slot, SaveEquipmentSlot::LeaderWeapon,);

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        assert!(cx.debug_bounds("save-player-leaders-empty").is_some());
        assert!(cx.debug_bounds("save-roster-empty").is_some());

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        assert!(cx.debug_bounds("save-missions").is_some());
        assert!(cx.debug_bounds("save-second-array-panel").is_none());
    }

    #[gpui::test]
    fn save_view_draws_filtered_equipment_as_no_match_not_empty_save(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(1, 0, 0).with_unit_roles([3]).build(),
        );
        frame.update(cx, |frame, cx| {
            frame.set_save_unit_filter(document, "leader", cx);
            frame.select_save_section(document, SaveSection::Equipment, cx);
        });

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-equipment-filter-empty").is_some());
        assert!(cx.debug_bounds("save-equipment-save-empty").is_none());
    }

    #[gpui::test]
    fn save_view_draws_one_fixed_text_error_without_hiding_summary(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_invalid_map_name_byte(0x80)
                .build(),
        );

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-summary").is_some());
        assert!(cx.debug_bounds("save-fixed-text-error").is_some());
        assert!(cx.debug_bounds("save-summary-values").is_some());
        assert!(cx.debug_bounds("save-summary-counts").is_some());
    }

    #[gpui::test]
    fn save_view_draws_catalog_states_inline(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_save(&frame, cx, SaveFixture::new(1, 0, 0).build());

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-not-configured").is_some());

        let loading_key = frame.update(cx, |frame, cx| {
            let key = SaveCatalogKey::new(frame.shell.begin_save_catalog(), "/catalog/loading");
            frame.save_catalog.begin(key.clone());
            cx.notify();
            key
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-loading").is_some());

        frame.update(cx, |frame, cx| {
            assert!(frame.save_catalog.finish_failed(
                loading_key,
                CatalogRequestError::Installation(InstallationError::RootMissing {
                    game: Game::Crusaders,
                    root: PathBuf::from("/catalog/loading"),
                }),
            ));
            cx.notify();
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-failed").is_some());

        frame.update(cx, |frame, cx| {
            let key = SaveCatalogKey::new(frame.shell.begin_save_catalog(), "/catalog/ready");
            frame.save_catalog.begin(key.clone());
            assert!(
                frame
                    .save_catalog
                    .finish_ready(key, Arc::new(missing_name_dictionary()), 2,)
            );
            frame.select_save_section(document, SaveSection::Units, cx);
            cx.notify();
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-ready-issues").is_some());
        assert!(cx.debug_bounds("save-name-unavailable").is_some());
    }

    fn test_startup() -> SettingsStartup {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        drop(file);
        SettingsStartup::load(path)
    }

    fn active_text_input(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
    ) -> Entity<TextInput> {
        frame.update(cx, |frame, _| {
            frame.text_edit.as_ref().unwrap().input.clone()
        })
    }

    fn start_hidden_save_text(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
    ) -> Entity<TextInput> {
        frame.update_in(cx, |frame, window, cx| {
            frame.start_text_edit(
                crate::frame::TextEditTarget::save(document, SaveTextField::SetFile),
                "Alpha".to_owned(),
                window,
                cx,
            );
            frame.text_edit.as_ref().unwrap().input.clone()
        })
    }

    fn assert_hidden_text_commit_is_ignored(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
        stale: &Entity<TextInput>,
    ) {
        frame.update(cx, |frame, _| assert!(frame.text_edit.is_none()));
        stale.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("Hidden mutation".to_owned()));
        });
        cx.run_until_parked();
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Alpha",
            );
            assert!(!frame.workspace.is_dirty(document).unwrap());
        });
    }

    fn activate_save(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        bytes: Vec<u8>,
    ) -> DocumentID {
        frame.update(cx, |frame, cx| {
            let document = frame.workspace.open_loaded(
                PathBuf::from("campaign.sav"),
                Document::Save(SaveDocument::parse(bytes).unwrap()),
            );
            frame.activate_document(document, cx);
            frame.shell.select_area(Area::Files);
            cx.notify();
            document
        })
    }

    fn select_and_draw(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
        section: SaveSection,
    ) {
        frame.update(cx, |frame, cx| {
            frame.select_save_section(document, section, cx);
        });
        draw_frame(cx, frame);
    }

    fn draw_frame(cx: &mut VisualTestContext, frame: &Entity<AppFrame>) {
        let frame = frame.clone();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(1180.0), px(780.0)),
            move |_, _| frame,
        );
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing click target {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
    }

    fn save_state(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
    ) -> crate::state::SavePresentationState {
        frame.update(cx, |frame, _| {
            frame.save_presentations.get(document).unwrap().clone()
        })
    }

    fn missing_name_dictionary() -> kufeditor_game::NameDictionary {
        let temporary = tempfile::tempdir().unwrap();
        let sox = temporary.path().join("Data/SOX");
        let path = sox.join(CatalogRole::TroopNames.relative_path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = 100_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&999_u32.to_le_bytes());
        bytes.extend_from_slice(&7_u16.to_le_bytes());
        bytes.extend_from_slice(b"Missing");
        fs::write(path, bytes).unwrap();
        load_name_dictionary(&sox).unwrap().dictionary
    }
}
