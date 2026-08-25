use gpui::{AnyElement, Context, Div, SharedString, div, prelude::*, px};
use kufeditor_game::NameDictionary;
use kufeditor_workspace::{
    DocumentID, SaveEquipmentGroup, SaveEquipmentSlot, SaveNumberTarget, SaveUnitGroup,
};

use super::AppFrame;
use crate::{
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
        let transition =
            self.save_presentations
                .activate_document(document, visibility, draft_active);
        let transition = if document_changed {
            if draft_active {
                SavePresentationTransition::ChangedAndCancelDraft
            } else if transition == SavePresentationTransition::Unchanged {
                SavePresentationTransition::Changed
            } else {
                transition
            }
        } else {
            transition
        };
        self.finish_save_presentation_transition(transition, cx);
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
                .save_summary_view(document, &summary)
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
    ) -> Div {
        save::scrolling_section(
            &self.theme,
            "save-summary",
            "Summary",
            "Envelope, campaign, fixed strings, and record counts".to_owned(),
            vec![
                self.save_summary_envelope(document, summary),
                self.save_summary_values(summary),
                self.save_summary_text(summary),
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

    fn save_summary_values(&self, summary: &save::SaveSummaryProjection) -> AnyElement {
        let mut values = vec![self.save_number_row(&summary.campaign)];
        values.extend(
            summary
                .main_fields
                .iter()
                .map(|field| self.save_number_row(field)),
        );
        values.push(self.save_number_row(&summary.saved_unit_reference));
        save::group(&self.theme, "SAVE VALUES", values)
            .id("save-summary-values")
            .debug_selector(|| "save-summary-values".to_owned())
            .into_any_element()
    }

    fn save_summary_text(&self, summary: &save::SaveSummaryProjection) -> AnyElement {
        let text = summary
            .text_fields
            .iter()
            .map(|field| {
                let row = save::text_value_row(
                    &self.theme,
                    projection_element_id("save-text", field.id),
                    field.label.clone(),
                    match &field.value {
                        Ok(value) => empty_label(value),
                        Err(error) => error.clone(),
                    },
                );
                if field.value.is_ok() {
                    row.into_any_element()
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
            inspected.map_or_else(Vec::new, |unit| self.save_unit_details(document, &unit));
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
                components::choice_button(&self.theme, ("save-unit-filter", index), label, selected)
                    .child(if selected { " ✓" } else { "" })
                    .tab_stop(true)
                    .on_click(cx.listener(move |frame, _, window, cx| {
                        frame.set_save_unit_filter(document, filter.to_owned(), cx);
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
                save::unit_row(
                    &self.theme,
                    projection_element_id("save-unit-row", row.id),
                    &row,
                    selected,
                )
                .debug_selector(|| "save-unit-master-row".to_owned())
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
    ) -> Vec<AnyElement> {
        let mut details = Vec::new();
        if self.save_dictionary().is_some() && unit.row.label.starts_with("Job ") {
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
                .map(|field| self.save_number_row(field))
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
            content.extend(self.save_equipment_details(document, unit, equipment));
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
        if self.save_dictionary().is_some() && equipment.item_name.starts_with("Item Type ") {
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
                .map(|field| self.save_number_row(field))
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
                cx.processor(move |frame, location, _, _| {
                    frame.save_virtual_roster_row(document, location)
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
    ) -> AnyElement {
        match save::row_projection(&self.workspace, document, self.save_dictionary(), location) {
            Ok(SaveRowProjection::Roster(row)) => save::roster_row(
                &self.theme,
                projection_element_id("save-roster-row", row.id),
                &row,
            )
            .into_any_element(),
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
            .children(self.save_mission_fixed(mission));
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
                cx.processor(move |frame, location, _, _| {
                    frame.save_virtual_second_array_row(document, location)
                }),
            )
            .size_full()
            .into_any_element()
        }
    }

    fn save_mission_fixed(&self, mission: &save::SaveMissionProjection) -> Vec<AnyElement> {
        let mut fixed = vec![
            save::group(
                &self.theme,
                "CURRENT MISSION",
                vec![self.save_number_row(&mission.current_mission)],
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
                        save::value_row(
                            &self.theme,
                            projection_element_id("save-mission", field.id),
                            format!("Mission {}", index + 1),
                            field.display_value.clone(),
                        )
                        .debug_selector(move || format!("save-mission-completion-{index}"))
                        .into_any_element()
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
    ) -> AnyElement {
        match save::row_projection(&self.workspace, document, self.save_dictionary(), location) {
            Ok(SaveRowProjection::SecondArray(field)) => save::second_array_row(
                &self.theme,
                projection_element_id("save-second-array-row", field.id),
                location.source_index,
                field.raw_value,
            )
            .into_any_element(),
            Ok(_) => save::empty_state(&self.theme, "Unexpected save row kind.").into_any_element(),
            Err(error) => save::empty_state(&self.theme, format!("Could not read row: {error}"))
                .into_any_element(),
        }
    }

    fn save_number_row(&self, field: &SaveNumberProjection) -> AnyElement {
        save::value_row(
            &self.theme,
            projection_element_id("save-number", field.id),
            field.label.clone(),
            field.display_value.clone(),
        )
        .into_any_element()
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
        let transition =
            self.save_presentations
                .select_section(document, section, self.save_draft_active());
        self.finish_save_presentation_transition(transition, cx);
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
        let transition = self.save_presentations.inspect_unit(
            document,
            unit,
            rows.unit_visibility()
                .unwrap_or(SaveUnitVisibility::All { unit_count: 0 }),
            self.save_draft_active(),
        );
        self.finish_save_presentation_transition(transition, cx);
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
        let transition =
            self.save_presentations
                .select_equipment_slot(document, slot, self.save_draft_active());
        self.finish_save_presentation_transition(transition, cx);
    }

    fn set_save_unit_filter(
        &mut self,
        document: DocumentID,
        filter: String,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        let Ok(rows) = self.save_unit_rows(document, &filter) else {
            return;
        };
        let transition = self.save_presentations.set_unit_filter(
            document,
            filter,
            rows.unit_visibility()
                .unwrap_or(SaveUnitVisibility::All { unit_count: 0 }),
            self.save_draft_active(),
        );
        self.finish_save_presentation_transition(transition, cx);
    }

    fn save_unit_rows(
        &self,
        document: DocumentID,
        filter: &str,
    ) -> save::SaveProjectionResult<SaveRows> {
        SaveRows::units(&self.workspace, document, self.save_dictionary(), filter)
    }

    const fn save_draft_active(&self) -> bool {
        self.number_edit.is_some() || self.text_edit.is_some()
    }

    fn finish_save_presentation_transition(
        &mut self,
        transition: SavePresentationTransition,
        cx: &mut Context<Self>,
    ) {
        match transition {
            SavePresentationTransition::Unchanged => {}
            SavePresentationTransition::Changed => cx.notify(),
            SavePresentationTransition::ChangedAndCancelDraft => {
                self.cancel_property_edit();
                cx.notify();
            }
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

    use std::{fs, mem::size_of, path::PathBuf, sync::Arc};

    use gpui::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, point, px, size};
    use kufeditor_game::{CatalogRole, Game, InstallationError, load_name_dictionary};
    use kufeditor_workspace::{Document, DocumentID, SaveDocument, SaveEquipmentSlot};

    use super::{AppFrame, SAVE_SECTIONS, UNIT_FILTERS, save_section_id, save_section_label};
    use crate::{
        catalog_status::CatalogRequestError,
        save_catalog_status::SaveCatalogKey,
        settings::SettingsStartup,
        state::{Area, SaveSection},
    };

    const CONTEXT_SIZE: usize = 0x438;
    const MAIN_SIZE: usize = 0x154;
    const UNIT_SIZE: usize = 483;
    const EQUIPMENT_SIZE: usize = 64;

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
        let document = activate_save(&frame, cx, save_fixture(1, 1, 1));

        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-summary").is_some());
        assert!(cx.debug_bounds("save-summary-envelope").is_some());

        select_and_draw(&frame, cx, document, SaveSection::Units);
        assert!(cx.debug_bounds("save-units").is_some());
        assert!(cx.debug_bounds("save-unit-master-row").is_some());

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
    fn save_view_draws_empty_states_without_duplicate_or_inert_panes(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_save(&frame, cx, save_fixture(0, 0, 0));

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
        let document = activate_save(&frame, cx, save_fixture_with_role(3));
        frame.update(cx, |frame, cx| {
            frame.set_save_unit_filter(document, "leader".to_owned(), cx);
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
        let mut bytes = save_fixture(0, 0, 0);
        let main_offset = 2 * size_of::<u32>() + CONTEXT_SIZE + size_of::<u32>();
        *bytes.get_mut(main_offset + 0x20).unwrap() = 0x80;
        activate_save(&frame, cx, bytes);

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
        let document = activate_save(&frame, cx, save_fixture(1, 0, 0));

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

    fn save_fixture(unit_count: usize, roster_count: usize, second_array_count: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0x6e);
        append_u32(&mut bytes, u32::MAX);
        bytes.resize(bytes.len() + CONTEXT_SIZE - size_of::<u32>(), 0);
        append_u32(&mut bytes, 0);
        bytes.resize(bytes.len() + MAIN_SIZE, 0);

        append_u32(&mut bytes, u32::try_from(unit_count).unwrap());
        if unit_count > 0 {
            append_complete_unit(&mut bytes);
            append_zero_records(&mut bytes, unit_count - 1, UNIT_SIZE);
        }

        append_i32(&mut bytes, -1);
        append_u32(&mut bytes, u32::try_from(roster_count).unwrap());
        append_zero_records(&mut bytes, roster_count, 8);

        append_u32(&mut bytes, u32::try_from(second_array_count).unwrap());
        for value in 0..second_array_count {
            append_u32(&mut bytes, u32::try_from(value).unwrap());
        }
        for slot in 0_i32..20 {
            append_i32(&mut bytes, slot - 1);
        }
        append_i32(&mut bytes, -2);

        bytes.resize(0x8000, 0);
        let length = u32::try_from(bytes.len()).unwrap();
        bytes
            .get_mut(..size_of::<u32>())
            .unwrap()
            .copy_from_slice(&length.to_le_bytes());
        bytes
    }

    fn save_fixture_with_role(role: u32) -> Vec<u8> {
        let mut bytes = save_fixture(1, 0, 0);
        let unit_offset =
            2 * size_of::<u32>() + CONTEXT_SIZE + size_of::<u32>() + MAIN_SIZE + size_of::<u32>();
        let ucd_offset = unit_offset + 10 * size_of::<u32>();
        bytes
            .get_mut(ucd_offset..ucd_offset + size_of::<u32>())
            .unwrap()
            .copy_from_slice(&role.to_le_bytes());
        bytes
    }

    fn append_complete_unit(bytes: &mut Vec<u8>) {
        let start = bytes.len();
        append_i32(bytes, -1);
        for value in [2_u32, 2, 4, 0x34, 0x38, 0x3c, 0x40] {
            append_u32(bytes, value);
        }
        append_i32(bytes, -1);
        for value in [5_u32, 0, 6, 7, 8] {
            append_u32(bytes, value);
        }
        bytes.extend_from_slice(&[1, 0, 1]);
        for value in [60_u32, 64, 68] {
            append_u32(bytes, value);
        }
        bytes.extend(0xa0_u8..=0xb7);
        append_zero_records(bytes, 6, EQUIPMENT_SIZE);
        append_u32(bytes, 504);
        assert_eq!(bytes.len() - start, UNIT_SIZE);
    }

    fn append_zero_records(bytes: &mut Vec<u8>, count: usize, record_size: usize) {
        bytes.resize(bytes.len() + count * record_size, 0);
    }

    fn append_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn append_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
