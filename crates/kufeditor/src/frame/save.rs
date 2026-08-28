use gpui::{
    AnyElement, ClickEvent, Context, Div, ScrollStrategy, SharedString, Stateful, div, prelude::*,
    px,
};
use kufeditor_game::NameDictionary;
use kufeditor_workspace::{
    DocumentID, SaveEditor, SaveEquipmentGroup, SaveEquipmentSlot, SaveNumberTarget,
    SaveRosterField, SaveUnitGroup,
};

use super::AppFrame;
use crate::{
    actions::{
        MoveSaveListDown, MoveSaveListEnd, MoveSaveListHome, MoveSaveListLeft,
        MoveSaveListPageDown, MoveSaveListPageUp, MoveSaveListRight, MoveSaveListUp, SetSaveChoice,
    },
    components,
    crusaders_catalog_status::CrusadersCatalogStatus,
    state::{
        SaveListCursor, SaveListKind, SavePresentationState, SavePresentationTransition,
        SaveSection, SaveUnitVisibility,
    },
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

const PLAYER_ONLY_FILTER_LABEL: &str = "Player only";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SaveListMovement {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum SaveNumberRowKeyboard {
    Normal,
    Virtual { cursor: SaveListCursor },
}

impl AppFrame {
    pub(super) fn activate_save_presentation(
        &mut self,
        document: DocumentID,
        cx: &mut Context<Self>,
    ) {
        let player_only = self
            .save_presentations
            .get(document)
            .is_some_and(SavePresentationState::player_only);
        let rows = self.save_unit_rows(document, player_only);
        let roster_count = self.workspace.save_roster_count(document).unwrap_or(0);
        let second_array_count = self
            .workspace
            .save_second_array_count(document)
            .unwrap_or(0);
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
                let mut transition = states.activate_document(document, visibility, draft_active);
                let section = states
                    .get(document)
                    .map_or(SaveSection::Summary, SavePresentationState::section);
                transition = merge_save_presentation_transition(
                    transition,
                    states.reconcile_list_cursor(
                        document,
                        SaveListKind::Roster,
                        SaveUnitVisibility::All {
                            unit_count: roster_count,
                        },
                        draft_active && section == SaveSection::Roster,
                    ),
                );
                transition = merge_save_presentation_transition(
                    transition,
                    states.reconcile_list_cursor(
                        document,
                        SaveListKind::SecondArray,
                        SaveUnitVisibility::All {
                            unit_count: second_array_count,
                        },
                        draft_active && section == SaveSection::Missions,
                    ),
                );
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
            self.crusaders_catalog_status_element(),
            content,
        )
        .tab_group()
        .key_context("SaveEditor")
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
                    let click_value = value.clone();
                    row.debug_selector(move || save_text_selector(field.field).to_owned())
                        .tab_index(0)
                        .cursor_pointer()
                        .on_click(cx.listener(move |frame, _, window, cx| {
                            frame.start_text_edit(target, click_value.clone(), window, cx);
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

    fn uniform_save_list<R>(
        &self,
        id: impl Into<gpui::ElementId>,
        document: DocumentID,
        kind: SaveListKind,
        rows: SaveRows,
        render: impl 'static + Fn(SaveRowLocation, &mut gpui::Window, &mut gpui::App) -> R,
        cx: &mut Context<Self>,
    ) -> Stateful<Div>
    where
        R: IntoElement,
    {
        let state = self
            .save_presentations
            .get(document)
            .cloned()
            .unwrap_or_default();
        let cursor = reconcile_save_list_cursor(state.list_cursor(kind), &rows);
        let position = rows.position_of(cursor.source_index()).unwrap_or(0);
        let control = self.save_lists.get(kind);
        let binding = super::SaveListBinding {
            document,
            cursor,
            position,
            row_count: rows.len(),
        };
        if control.binding.get() != Some(binding) {
            control
                .scroll
                .scroll_to_item(position, ScrollStrategy::Center);
            control.binding.set(Some(binding));
        }

        let root_name = match kind {
            SaveListKind::Units => "units",
            SaveListKind::Roster => "roster",
            SaveListKind::SecondArray => "second-array",
        };
        let root_selector = format!("save-virtual-list-root-{root_name}");
        let list = save::uniform_save_rows(id, rows, render)
            .track_scroll(control.scroll.clone())
            .size_full();

        div()
            .id(save_local_id(
                "save-virtual-list-root",
                document,
                kind as usize,
            ))
            .debug_selector(move || root_selector.clone())
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .track_focus(&control.focus)
            .tab_index(0)
            .tab_stop(true)
            .key_context("SaveVirtualList")
            .on_click(cx.listener(move |frame, event: &ClickEvent, window, cx| {
                if matches!(event, ClickEvent::Keyboard(_)) {
                    frame.activate_save_list_cursor(document, kind, window, cx);
                }
            }))
            .on_action(cx.listener(move |frame, _: &MoveSaveListUp, window, cx| {
                frame.move_save_list_cursor(document, kind, SaveListMovement::Up, window, cx);
            }))
            .on_action(cx.listener(move |frame, _: &MoveSaveListDown, window, cx| {
                frame.move_save_list_cursor(document, kind, SaveListMovement::Down, window, cx);
            }))
            .on_action(cx.listener(move |frame, _: &MoveSaveListHome, window, cx| {
                frame.move_save_list_cursor(document, kind, SaveListMovement::Home, window, cx);
            }))
            .on_action(cx.listener(move |frame, _: &MoveSaveListEnd, window, cx| {
                frame.move_save_list_cursor(document, kind, SaveListMovement::End, window, cx);
            }))
            .on_action(
                cx.listener(move |frame, _: &MoveSaveListPageUp, window, cx| {
                    frame.move_save_list_cursor(
                        document,
                        kind,
                        SaveListMovement::PageUp,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(
                cx.listener(move |frame, _: &MoveSaveListPageDown, window, cx| {
                    frame.move_save_list_cursor(
                        document,
                        kind,
                        SaveListMovement::PageDown,
                        window,
                        cx,
                    );
                }),
            )
            .on_action(cx.listener(move |frame, _: &MoveSaveListLeft, window, cx| {
                frame.move_save_list_cursor(document, kind, SaveListMovement::Left, window, cx);
            }))
            .on_action(
                cx.listener(move |frame, _: &MoveSaveListRight, window, cx| {
                    frame.move_save_list_cursor(
                        document,
                        kind,
                        SaveListMovement::Right,
                        window,
                        cx,
                    );
                }),
            )
            .child(list)
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
        let player_only = state.player_only();
        let filter = components::choice_button(
            &self.theme,
            "save-unit-filter-player-only-control",
            PLAYER_ONLY_FILTER_LABEL,
            player_only,
        )
        .child(if player_only { " ✓" } else { "" })
        .debug_selector(|| "save-unit-filter-player-only".to_owned())
        .tab_index(0)
        .on_click(cx.listener(move |frame, _, window, cx| {
            frame.set_save_player_only(document, !player_only, cx);
            frame.restore_property_or_frame_focus(window, cx);
        }))
        .into_any_element();
        let list = if rows.is_empty() {
            save::empty_state(
                &self.theme,
                if state.player_only() {
                    "This save has no player units."
                } else {
                    "This save has no unit records."
                },
            )
            .id("save-unit-empty")
            .debug_selector(|| "save-unit-empty".to_owned())
            .size_full()
            .into_any_element()
        } else {
            self.uniform_save_list(
                save_local_id("save-unit-list", document, 0),
                document,
                SaveListKind::Units,
                rows,
                cx.processor(move |frame, location, _, cx| {
                    frame.save_virtual_unit_row(document, location, cx)
                }),
                cx,
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
                    .child(filter),
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
                let item = save::unit_row(
                    &self.theme,
                    projection_element_id("save-unit-row", row.id),
                    &row,
                    selected,
                )
                .debug_selector(move || selector.clone())
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.inspect_save_unit(document, location.source_index, cx);
                    window.focus(&frame.focus);
                }));
                item.into_any_element()
            }
            Ok(_) => save::empty_state(&self.theme, "KufEditor could not display this save row.")
                .into_any_element(),
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
                "UNIT IDENTITY",
                vec![
                    save::value_row(
                        &self.theme,
                        save_local_id("save-unit-name", document, unit.row.source_index),
                        "Unit Name",
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
            let (selector, message) = if state.player_only() {
                (
                    "save-equipment-filter-empty",
                    "This save has no player units. No equipment is available.",
                )
            } else {
                (
                    "save-equipment-save-empty",
                    "This save has no units. No equipment is available.",
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
            "Six equipment slots for the selected unit".to_owned(),
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
                        .tab_index(0)
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
                "SELECTED UNIT",
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
                            format!("{} · ID {}", attribute.name, attribute.raw_index),
                            attribute.effect.clone().unwrap_or_else(|| {
                                "No catalog effect matches this attribute. KufEditor will write the original value."
                                    .to_owned()
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
            self.uniform_save_list(
                save_local_id("save-roster-list", document, 0),
                document,
                SaveListKind::Roster,
                world_map_rows,
                cx.processor(move |frame, location, _, cx| {
                    frame.save_virtual_roster_row(document, location, cx)
                }),
                cx,
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
                    format!("{leader_count} player leaders · {row_count} world-map rows"),
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
            Ok(_) => save::empty_state(&self.theme, "KufEditor could not display this save row.")
                .into_any_element(),
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
                let active_cursor = self.save_presentations.get(document).map_or_else(
                    || SavePresentationState::default().list_cursor(SaveListKind::Roster),
                    |state| state.list_cursor(SaveListKind::Roster),
                );
                let fields = row
                    .fields
                    .iter()
                    .map(|field| {
                        let SaveNumberTarget::Roster {
                            record,
                            field: kind,
                        } = field.target
                        else {
                            return self.save_number_row(field, cx);
                        };
                        let cursor = SaveListCursor::Roster {
                            record,
                            field: kind,
                        };
                        self.save_virtual_number_row(
                            field,
                            field.label.clone(),
                            cursor,
                            cursor == active_cursor,
                            cx,
                        )
                    })
                    .collect();
                save::roster_row_with_fields(
                    &self.theme,
                    projection_element_id("save-roster-row", row.id),
                    &row,
                    fields,
                )
                .into_any_element()
            }
            Ok(_) => save::empty_state(&self.theme, "KufEditor could not display this save row.")
                .into_any_element(),
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
            self.uniform_save_list(
                save_local_id("save-second-array-list", document, 0),
                document,
                SaveListKind::SecondArray,
                second_array_rows,
                cx.processor(move |frame, location, _, cx| {
                    frame.save_virtual_second_array_row(document, location, cx)
                }),
                cx,
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
            Ok(SaveRowProjection::SecondArray(field)) => {
                let cursor = SaveListCursor::SecondArray {
                    record: location.source_index,
                };
                let active = self
                    .save_presentations
                    .get(document)
                    .is_some_and(|state| state.list_cursor(SaveListKind::SecondArray) == cursor);
                self.save_virtual_number_row(
                    &field,
                    format!("Second Array {}", location.source_index + 1),
                    cursor,
                    active,
                    cx,
                )
            }
            Ok(_) => save::empty_state(&self.theme, "KufEditor could not display this save row.")
                .into_any_element(),
            Err(error) => save::empty_state(&self.theme, format!("Could not read row: {error}"))
                .into_any_element(),
        }
    }

    fn save_number_row(&self, field: &SaveNumberProjection, cx: &mut Context<Self>) -> AnyElement {
        self.save_number_row_with_label(field, field.label.clone(), cx)
    }

    fn save_virtual_number_row(
        &self,
        field: &SaveNumberProjection,
        label: String,
        cursor: SaveListCursor,
        cursor_active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.save_number_row_with_label_and_keyboard(
            field,
            label,
            SaveNumberRowKeyboard::Virtual { cursor },
            cursor_active,
            cx,
        )
    }

    fn start_save_number_edit(
        &mut self,
        document: DocumentID,
        target: SaveNumberTarget,
        raw_value: i64,
        editor: SaveEditor,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        let Some(edit) = super::ActiveNumberEdit::save(document, target, raw_value, editor) else {
            return;
        };
        self.begin_number_edit(edit);
        window.focus(&self.focus);
        cx.notify();
    }

    fn save_number_row_with_label(
        &self,
        field: &SaveNumberProjection,
        label: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.save_number_row_with_label_and_keyboard(
            field,
            label,
            SaveNumberRowKeyboard::Normal,
            false,
            cx,
        )
    }

    fn save_number_row_with_label_and_keyboard(
        &self,
        field: &SaveNumberProjection,
        label: String,
        keyboard: SaveNumberRowKeyboard,
        cursor_active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = field.id.document;
        let target = field.target;
        let raw_value = field.raw_value;
        let editor = field.editor;
        let selector = save_number_selector(target);
        let virtual_cursor = match keyboard {
            SaveNumberRowKeyboard::Normal => None,
            SaveNumberRowKeyboard::Virtual { cursor } => Some(cursor),
        };
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
                let row = save::editable_value_row(
                    &self.theme,
                    projection_element_id("save-number", field.id),
                    label,
                    display,
                    active_edit.is_some(),
                    active_edit
                        .is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
                )
                .when(cursor_active, |row| row.border_color(self.theme.accent))
                .debug_selector(move || selector.clone())
                .on_click(cx.listener(move |frame, _, window, cx| {
                    if let Some(cursor) = virtual_cursor {
                        frame.set_save_list_cursor(document, cursor, cx);
                    }
                    frame.start_save_number_edit(document, target, raw_value, editor, window, cx);
                }));
                match keyboard {
                    SaveNumberRowKeyboard::Normal => row.tab_index(0).into_any_element(),
                    SaveNumberRowKeyboard::Virtual { .. } => row.into_any_element(),
                }
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
                        .when(
                            matches!(keyboard, SaveNumberRowKeyboard::Normal),
                            |button| button.tab_index(0),
                        )
                        .on_click(cx.listener(move |frame, _, window, cx| {
                            if let Some(cursor) = virtual_cursor {
                                frame.set_save_list_cursor(document, cursor, cx);
                            }
                            window.dispatch_action(Box::new(action), cx);
                        }))
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
                .tab_index(0)
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_save_section(document, section, cx);
                    window.focus(&frame.focus);
                }))
                .into_any_element()
            })
            .collect()
    }

    fn crusaders_catalog_status_element(&self) -> Option<AnyElement> {
        match self.crusaders_catalog.status() {
            CrusadersCatalogStatus::NotConfigured => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-not-configured",
                    "Crusaders installation is not configured",
                    Some("Numeric IDs remain editable without game names.".to_owned()),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Dormant => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-dormant",
                    "Crusaders names are unavailable",
                    Some("Numeric IDs remain editable.".to_owned()),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Loading { .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-loading",
                    "Loading Crusaders names",
                    Some("Numeric IDs remain editable while names load.".to_owned()),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Failed { error, .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-failed",
                    "Could not load Crusaders names",
                    Some(format!("{error}. Numeric IDs remain editable.")),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Ready { issue_count: 0, .. } => None,
            CrusadersCatalogStatus::Ready { issue_count, .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "save-catalog-ready-issues",
                    format!("Loaded names with {issue_count} catalog issues"),
                    Some(
                        "Some records do not have game names. Their numeric IDs are shown instead."
                            .to_owned(),
                    ),
                )
                .into_any_element(),
            ),
        }
    }

    fn save_dictionary(&self) -> Option<&NameDictionary> {
        match self.crusaders_catalog.status() {
            CrusadersCatalogStatus::Ready { dictionary, .. } => Some(dictionary.as_ref()),
            CrusadersCatalogStatus::NotConfigured
            | CrusadersCatalogStatus::Dormant
            | CrusadersCatalogStatus::Loading { .. }
            | CrusadersCatalogStatus::Failed { .. } => None,
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
        self.save_lists.invalidate_all();
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.select_section(document, section, draft_active),
            cx,
        );
    }

    fn inspect_save_unit(&mut self, document: DocumentID, unit: usize, cx: &mut Context<Self>) {
        self.set_save_list_cursor(document, SaveListCursor::Unit { source_index: unit }, cx);
    }

    fn set_save_list_cursor(
        &mut self,
        document: DocumentID,
        cursor: SaveListCursor,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        let kind = cursor.kind();
        if self
            .save_presentations
            .get(document)
            .is_none_or(|state| state.section() != kind.section())
        {
            return;
        }
        let Ok(rows) = self.save_rows(document, kind) else {
            return;
        };
        if rows.position_of(cursor.source_index()).is_none() {
            return;
        }
        self.save_lists.get(kind).invalidate();
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| states.set_list_cursor(document, cursor, draft_active),
            cx,
        );
    }

    fn move_save_list_cursor(
        &mut self,
        document: DocumentID,
        kind: SaveListKind,
        movement: SaveListMovement,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document)
            || self
                .save_presentations
                .get(document)
                .is_none_or(|state| state.section() != kind.section())
        {
            return;
        }
        let Ok(rows) = self.save_rows(document, kind) else {
            return;
        };
        if rows.is_empty() {
            self.save_lists.get(kind).invalidate();
            return;
        }
        let cursor = self.save_presentations.get(document).map_or_else(
            || SavePresentationState::default().list_cursor(kind),
            |state| state.list_cursor(kind),
        );
        let cursor = reconcile_save_list_cursor(cursor, &rows);
        let current_position = rows.position_of(cursor.source_index()).unwrap_or(0);
        let page = self.save_list_page_size(kind, rows.len());
        let (position, roster_field) = match movement {
            SaveListMovement::Up => (current_position.saturating_sub(1), None),
            SaveListMovement::Down => (
                current_position
                    .saturating_add(1)
                    .min(rows.len().saturating_sub(1)),
                None,
            ),
            SaveListMovement::Home => (0, None),
            SaveListMovement::End => (rows.len().saturating_sub(1), None),
            SaveListMovement::PageUp => (current_position.saturating_sub(page), None),
            SaveListMovement::PageDown => (
                current_position
                    .saturating_add(page)
                    .min(rows.len().saturating_sub(1)),
                None,
            ),
            SaveListMovement::Left => {
                let SaveListCursor::Roster { field, .. } = cursor else {
                    return;
                };
                (current_position, Some(adjacent_roster_field(field, false)))
            }
            SaveListMovement::Right => {
                let SaveListCursor::Roster { field, .. } = cursor else {
                    return;
                };
                (current_position, Some(adjacent_roster_field(field, true)))
            }
        };
        let Some(source_index) = rows.source_index(position) else {
            return;
        };
        let target = save_list_cursor_at(cursor, source_index, roster_field);
        if target != cursor {
            self.save_lists.get(kind).invalidate();
            let draft_active = self.save_draft_active();
            self.apply_save_presentation_transition(
                draft_active,
                |states, draft_active| states.set_list_cursor(document, target, draft_active),
                cx,
            );
        }

        let strategy = match movement {
            SaveListMovement::Up | SaveListMovement::Home | SaveListMovement::PageUp => {
                ScrollStrategy::Top
            }
            SaveListMovement::Down | SaveListMovement::End | SaveListMovement::PageDown => {
                ScrollStrategy::Bottom
            }
            SaveListMovement::Left | SaveListMovement::Right => ScrollStrategy::Center,
        };
        let control = self.save_lists.get(kind);
        let generation = control.next_generation();
        let binding = super::SaveListBinding {
            document,
            cursor: target,
            position,
            row_count: rows.len(),
        };
        control.binding.set(Some(binding));
        control.scroll.scroll_to_item(position, strategy);
        cx.notify();
        cx.on_next_frame(window, move |frame, window, _| {
            if frame.save_list_focus_request_is_current(kind, binding, generation) {
                window.focus(&frame.save_lists.get(kind).focus);
            }
        });
    }

    fn activate_save_list_cursor(
        &mut self,
        document: DocumentID,
        kind: SaveListKind,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document)
            || self
                .save_presentations
                .get(document)
                .is_none_or(|state| state.section() != kind.section())
        {
            return;
        }
        let Ok(rows) = self.save_rows(document, kind) else {
            return;
        };
        if rows.is_empty() {
            return;
        }
        let stored_cursor = self.save_presentations.get(document).map_or_else(
            || SavePresentationState::default().list_cursor(kind),
            |state| state.list_cursor(kind),
        );
        let cursor = reconcile_save_list_cursor(stored_cursor, &rows);
        let Some(position) = rows.position_of(cursor.source_index()) else {
            return;
        };
        if cursor != stored_cursor {
            self.set_save_list_cursor(document, cursor, cx);
        }
        self.reveal_save_list_cursor(kind, cx);

        if let SaveListCursor::Unit { source_index } = cursor {
            self.inspect_save_unit(document, source_index, cx);
            return;
        }

        let Some(location) = rows
            .locations(position..position.saturating_add(1))
            .first()
            .copied()
        else {
            return;
        };
        let Ok(projection) =
            save::row_projection(&self.workspace, document, self.save_dictionary(), location)
        else {
            return;
        };
        let field = match (cursor, projection) {
            (SaveListCursor::Roster { record, field }, SaveRowProjection::Roster(row)) => row
                .fields
                .into_iter()
                .find(|projection| projection.target == SaveNumberTarget::Roster { record, field }),
            (
                SaveListCursor::SecondArray { record },
                SaveRowProjection::SecondArray(projection),
            ) if projection.target == SaveNumberTarget::SecondArray { record } => Some(projection),
            _ => None,
        };
        let Some(field) = field else {
            return;
        };
        match field.editor {
            SaveEditor::Number { .. } => self.start_save_number_edit(
                document,
                field.target,
                field.raw_value,
                field.editor,
                window,
                cx,
            ),
            SaveEditor::Choice { choices } => {
                let choice = choices
                    .iter()
                    .position(|choice| choice.value == field.raw_value)
                    .and_then(|index| choices.get((index + 1) % choices.len()))
                    .or_else(|| choices.first());
                if let Some(choice) = choice {
                    window.dispatch_action(
                        Box::new(SetSaveChoice {
                            document,
                            target: field.target,
                            value: choice.value,
                        }),
                        cx,
                    );
                }
            }
        }
    }

    fn save_list_focus_request_is_current(
        &self,
        kind: SaveListKind,
        binding: super::SaveListBinding,
        generation: u64,
    ) -> bool {
        if !self.save_editor_is_visible()
            || self.active_document != Some(binding.document)
            || self.save_lists.get(kind).generation.get() != generation
            || self.save_lists.get(kind).binding.get() != Some(binding)
        {
            return false;
        }
        let Some(state) = self.save_presentations.get(binding.document) else {
            return false;
        };
        if state.section() != kind.section() || state.list_cursor(kind) != binding.cursor {
            return false;
        }
        self.save_rows(binding.document, kind).is_ok_and(|rows| {
            rows.len() == binding.row_count
                && rows.position_of(binding.cursor.source_index()) == Some(binding.position)
        })
    }

    pub(super) fn reveal_focused_save_list_cursor(
        &mut self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) {
        for kind in [
            SaveListKind::Units,
            SaveListKind::Roster,
            SaveListKind::SecondArray,
        ] {
            if self.save_lists.get(kind).focus.is_focused(window) {
                self.reveal_save_list_cursor(kind, cx);
                return;
            }
        }
    }

    fn reveal_save_list_cursor(&mut self, kind: SaveListKind, cx: &mut Context<Self>) {
        if !self.save_editor_is_visible() {
            return;
        }
        let Some(document) = self.active_document else {
            return;
        };
        let Some(state) = self.save_presentations.get(document) else {
            return;
        };
        if state.section() != kind.section() {
            return;
        }
        let stored_cursor = state.list_cursor(kind);
        let Ok(rows) = self.save_rows(document, kind) else {
            return;
        };
        if rows.is_empty() {
            self.save_lists.get(kind).invalidate();
            return;
        }
        let cursor = reconcile_save_list_cursor(stored_cursor, &rows);
        let Some(position) = rows.position_of(cursor.source_index()) else {
            self.save_lists.get(kind).invalidate();
            return;
        };
        if cursor != stored_cursor {
            self.set_save_list_cursor(document, cursor, cx);
        }

        let control = self.save_lists.get(kind);
        control.next_generation();
        control.binding.set(Some(super::SaveListBinding {
            document,
            cursor,
            position,
            row_count: rows.len(),
        }));
        control
            .scroll
            .scroll_to_item(position, ScrollStrategy::Center);
        cx.notify();
    }

    fn save_list_page_size(&self, kind: SaveListKind, row_count: usize) -> usize {
        let fallback = 8.min(row_count).max(1);
        let Some(size) = self.save_lists.get(kind).scroll.0.borrow().last_item_size else {
            return fallback;
        };
        let row_height = match kind {
            SaveListKind::Units => px(54.0),
            SaveListKind::Roster => px(64.0),
            SaveListKind::SecondArray => px(36.0),
        };
        let mut visible = 0;
        while visible < row_count && row_height * (visible + 1) <= size.item.height {
            visible += 1;
        }
        visible.saturating_sub(1).max(1)
    }

    fn save_rows(
        &self,
        document: DocumentID,
        kind: SaveListKind,
    ) -> save::SaveProjectionResult<SaveRows> {
        match kind {
            SaveListKind::Units => {
                let player_only = self
                    .save_presentations
                    .get(document)
                    .is_some_and(SavePresentationState::player_only);
                self.save_unit_rows(document, player_only)
            }
            SaveListKind::Roster => SaveRows::roster(&self.workspace, document),
            SaveListKind::SecondArray => SaveRows::second_array(&self.workspace, document),
        }
    }

    fn restore_property_or_frame_focus(&self, window: &mut gpui::Window, cx: &gpui::App) {
        if let Some(edit) = self.text_edit.as_ref() {
            window.focus(&edit.input.read(cx).focus_handle());
        } else {
            window.focus(&self.focus);
        }
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

    fn set_save_player_only(
        &mut self,
        document: DocumentID,
        player_only: bool,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        self.save_lists.units.invalidate();
        let Ok(rows) = self.save_unit_rows(document, player_only) else {
            return;
        };
        let visibility = rows
            .unit_visibility()
            .unwrap_or(SaveUnitVisibility::All { unit_count: 0 });
        let draft_active = self.save_draft_active();
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| {
                states.set_player_only(document, player_only, visibility, draft_active)
            },
            cx,
        );
    }

    fn save_unit_rows(
        &self,
        document: DocumentID,
        player_only: bool,
    ) -> save::SaveProjectionResult<SaveRows> {
        SaveRows::units(&self.workspace, document, player_only)
    }

    pub(super) fn reconcile_save_presentation(
        &mut self,
        document: DocumentID,
        cx: &mut Context<Self>,
    ) {
        let player_only = self
            .save_presentations
            .get(document)
            .is_some_and(SavePresentationState::player_only);
        let Ok(rows) = self.save_unit_rows(document, player_only) else {
            return;
        };
        self.save_lists.invalidate_all();
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
        let roster_count = self.workspace.save_roster_count(document).unwrap_or(0);
        let second_array_count = self
            .workspace
            .save_second_array_count(document)
            .unwrap_or(0);
        self.apply_save_presentation_transition(
            draft_active,
            |states, draft_active| {
                let section = states
                    .get(document)
                    .map_or(SaveSection::Summary, SavePresentationState::section);
                let unit_draft_active =
                    draft_active && matches!(section, SaveSection::Units | SaveSection::Equipment);
                let mut transition =
                    states.reconcile_document(document, visibility, unit_draft_active);
                transition = merge_save_presentation_transition(
                    transition,
                    states.reconcile_list_cursor(
                        document,
                        SaveListKind::Roster,
                        SaveUnitVisibility::All {
                            unit_count: roster_count,
                        },
                        draft_active && section == SaveSection::Roster,
                    ),
                );
                merge_save_presentation_transition(
                    transition,
                    states.reconcile_list_cursor(
                        document,
                        SaveListKind::SecondArray,
                        SaveUnitVisibility::All {
                            unit_count: second_array_count,
                        },
                        draft_active && section == SaveSection::Missions,
                    ),
                )
            },
            cx,
        );
    }

    const fn save_draft_active(&self) -> bool {
        self.number_edit.is_some() || self.text_edit.is_some()
    }

    #[cfg(test)]
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

fn reconcile_save_list_cursor(cursor: SaveListCursor, rows: &SaveRows) -> SaveListCursor {
    rows.reconciled_source_index(cursor.source_index())
        .map_or(cursor, |source_index| {
            save_list_cursor_at(cursor, source_index, None)
        })
}

const fn merge_save_presentation_transition(
    first: SavePresentationTransition,
    second: SavePresentationTransition,
) -> SavePresentationTransition {
    if first.cancels_draft() || second.cancels_draft() {
        SavePresentationTransition::ChangedAndCancelDraft
    } else if first.changed() || second.changed() {
        SavePresentationTransition::Changed
    } else {
        SavePresentationTransition::Unchanged
    }
}

const fn save_list_cursor_at(
    cursor: SaveListCursor,
    source_index: usize,
    roster_field: Option<SaveRosterField>,
) -> SaveListCursor {
    match cursor {
        SaveListCursor::Unit { .. } => SaveListCursor::Unit { source_index },
        SaveListCursor::Roster { field, .. } => SaveListCursor::Roster {
            record: source_index,
            field: match roster_field {
                Some(field) => field,
                None => field,
            },
        },
        SaveListCursor::SecondArray { .. } => SaveListCursor::SecondArray {
            record: source_index,
        },
    }
}

fn adjacent_roster_field(field: SaveRosterField, forward: bool) -> SaveRosterField {
    let position = SaveRosterField::ALL
        .iter()
        .position(|candidate| *candidate == field)
        .unwrap_or(0);
    let target = if forward {
        position
            .saturating_add(1)
            .min(SaveRosterField::ALL.len().saturating_sub(1))
    } else {
        position.saturating_sub(1)
    };
    SaveRosterField::ALL.get(target).copied().unwrap_or(field)
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
        AppContext, ClipboardItem, Entity, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
        TestAppContext, VisualTestContext, point, px, size,
    };
    use kufeditor_game::{CatalogRole, Game, InstallationError, load_name_dictionary};
    use kufeditor_workspace::{
        ApplyOutcome, Document, DocumentEdit, DocumentID, SaveDocument, SaveEquipmentField,
        SaveEquipmentSlot, SaveMainField, SaveNumberTarget, SaveRosterField, SaveTextField,
        SaveUnitField,
    };

    use super::{
        AppFrame, PLAYER_ONLY_FILTER_LABEL, SAVE_SECTIONS, SaveListMovement, save_section_id,
        save_section_label,
    };
    use crate::{
        actions::{Redo, Undo},
        catalog_status::CatalogRequestError,
        crusaders_catalog_status::CrusadersCatalogKey,
        notices::{Notice, NoticeSource},
        settings::SettingsStartup,
        state::{Area, SaveListCursor, SaveListKind, SaveSection},
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
    fn save_view_player_only_filter_keeps_its_stable_label() {
        assert_eq!(PLAYER_ONLY_FILTER_LABEL, "Player only");
    }

    #[gpui::test]
    fn save_keyboard_tab_enters_the_section_rail_and_enter_activates(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(1, 0, 0).build());

        cx.run_until_parked();
        focus_frame(&frame, cx);
        cx.simulate_keystrokes("tab tab");
        key_cycle(cx, "enter");

        assert_eq!(
            save_state(&frame, cx, document).section(),
            SaveSection::Units,
        );
    }

    #[gpui::test]
    fn save_keyboard_activates_player_filter_unit_and_equipment_slot(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(4, 0, 0)
                .with_unit_roles([99, 0, 1, 3])
                .build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        key_cycle(cx, "space");
        cx.run_until_parked();
        assert!(save_state(&frame, cx, document).player_only());
        assert_eq!(visible_save_unit_indices(&frame, cx, document), [1, 2, 3]);
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 1);

        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        cx.simulate_keystrokes("down");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 2);

        select_and_draw(&frame, cx, document, SaveSection::Equipment);
        focus_frame(&frame, cx);
        press_tabs(cx, 11);
        key_cycle(cx, "space");
        assert_eq!(
            save_state(&frame, cx, document).equipment_slot(),
            SaveEquipmentSlot::TroopArmor,
        );
    }

    #[gpui::test]
    fn save_keyboard_skips_disabled_equipment_slots(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Equipment);
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        key_cycle(cx, "enter");

        assert_eq!(
            save_state(&frame, cx, document).section(),
            SaveSection::Summary,
        );
    }

    #[gpui::test]
    fn save_keyboard_activates_choice_number_and_fixed_text_controls(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
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
        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        key_cycle(cx, "space");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::CampaignIndex)
                    .unwrap(),
                1,
            );
        });

        focus_frame(&frame, cx);
        press_tabs(cx, 10);
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target
                    .is_save(document, SaveNumberTarget::Main(SaveMainField::Field00))
            }));
        });
        cx.simulate_keystrokes("escape");

        focus_frame(&frame, cx);
        cx.simulate_keystrokes("shift-tab");
        key_cycle(cx, "enter");
        frame.update_in(cx, |frame, window, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(
                edit.target,
                crate::frame::TextEditTarget::save(document, SaveTextField::SkyEffects),
            );
            assert!(edit.input.read(cx).focus_handle().is_focused(window));
        });
    }

    #[gpui::test]
    fn save_native_filter_key_cycle_activates_once_on_key_up(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 99]).build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        key_down(cx, "space");
        assert!(!save_state(&frame, cx, document).player_only());

        key_up(cx, "space");
        assert!(save_state(&frame, cx, document).player_only());
    }

    #[gpui::test]
    fn save_native_choice_key_cycle_applies_once_on_key_up(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        cx.run_until_parked();
        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        key_down(cx, "space");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::CampaignIndex)
                    .unwrap(),
                0,
            );
            assert!(!frame.workspace.can_undo(document).unwrap());
        });

        key_up(cx, "space");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::CampaignIndex)
                    .unwrap(),
                1,
            );
            assert!(frame.workspace.can_undo(document).unwrap());
        });
    }

    #[gpui::test]
    fn save_tab_cancels_number_draft_before_foreign_choice_enter_cycle(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        cx.simulate_keystrokes("9");
        press_tabs(cx, 7);
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::Main(SaveMainField::Field00))
                    .unwrap(),
                0,
            );
            assert!(!frame.workspace.can_undo(document).unwrap());
        });

        key_down(cx, "enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::CampaignIndex)
                    .unwrap(),
                0,
            );
            assert!(!frame.workspace.can_undo(document).unwrap());
        });

        key_up(cx, "enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::CampaignIndex)
                    .unwrap(),
                1,
            );
            assert!(frame.workspace.can_undo(document).unwrap());
            assert!(frame.workspace.undo(document).unwrap());
            assert!(!frame.workspace.undo(document).unwrap());
        });
    }

    #[gpui::test]
    fn save_tab_cancels_visible_unit_number_draft_before_platform_space_filter_cycle(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 99]).build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        click(cx, "save-number-unit-0-troop-info-index");
        cx.simulate_keystrokes("9");
        press_tabs(cx, 6);
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert!(!frame.workspace.can_undo(document).unwrap());
        });

        platform_space_key_down(cx);
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert!(!frame.workspace.can_undo(document).unwrap());
            assert!(
                !frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .player_only()
            );
        });

        key_up(cx, "space");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .player_only()
            );
            assert!(!frame.workspace.can_undo(document).unwrap());
        });
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

        click(cx, "save-unit-filter-player-only");
        let state = save_state(&frame, cx, document);
        assert!(state.player_only());
        assert_eq!(state.inspected_unit(), 0);
        assert!(cx.debug_bounds("save-unit-master-row-0").is_some());

        click(cx, "save-unit-filter-player-only");
        assert!(!save_state(&frame, cx, document).player_only());
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
    fn save_player_only_filter_uses_raw_ucd_grouping_and_indents_attached_officers(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(10, 0, 0)
                .with_unit_roles([0, 1, 2, 3, 1, 99, 2, 0, 2, 3])
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-unit-filter-player-only");
        cx.run_until_parked();
        assert!(save_state(&frame, cx, document).player_only());
        draw_frame(cx, &frame);

        assert_eq!(
            visible_save_unit_indices(&frame, cx, document),
            [0, 1, 2, 3, 7, 8, 9],
        );

        let leader = cx.debug_bounds("save-unit-row-content-0").unwrap();
        let officer = cx.debug_bounds("save-unit-row-content-1").unwrap();
        let troop = cx.debug_bounds("save-unit-row-content-3").unwrap();
        assert!(officer.origin.x > leader.origin.x);
        assert_eq!(troop.origin.x, leader.origin.x);

        click(cx, "save-unit-filter-player-only");
        cx.run_until_parked();
        assert!(!save_state(&frame, cx, document).player_only());
        draw_frame(cx, &frame);
        assert_eq!(
            visible_save_unit_indices(&frame, cx, document),
            (0..10).collect::<Vec<_>>(),
        );
        let detached_officer = cx.debug_bounds("save-unit-row-content-4").unwrap();
        assert!(detached_officer.origin.x > leader.origin.x);
    }

    #[gpui::test]
    fn save_player_only_filter_reconciles_first_visible_and_cancels_hidden_draft(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(4, 0, 0)
                .with_unit_roles([99, 0, 1, 3])
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-number-unit-0-troop-info-index");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-unit-filter-player-only");
        cx.run_until_parked();
        draw_frame(cx, &frame);

        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert_eq!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                1,
            );
        });
        assert_eq!(visible_save_unit_indices(&frame, cx, document), [1, 2, 3]);
        let inspected = frame.update(cx, |frame, _| {
            let state = frame.save_presentations.get(document).unwrap();
            let save::SaveSectionModel::Units { inspected, .. } = save::save_section_model(
                &frame.workspace,
                document,
                state,
                frame.save_dictionary(),
            )
            .unwrap() else {
                panic!("units section must remain active");
            };
            inspected.unwrap().row.source_index
        });
        assert_eq!(inspected, 1);
    }

    #[gpui::test]
    fn save_player_only_filter_reconciles_to_empty_and_cancels_the_draft(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(3, 0, 0)
                .with_unit_roles([1, 2, 99])
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-number-unit-0-troop-info-index");
        click(cx, "save-unit-filter-player-only");
        cx.run_until_parked();
        draw_frame(cx, &frame);

        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .player_only()
            );
        });
        assert!(cx.debug_bounds("save-unit-empty").is_some());
        assert!(visible_save_unit_indices(&frame, cx, document).is_empty());
        frame.update(cx, |frame, _| {
            let state = frame.save_presentations.get(document).unwrap();
            let save::SaveSectionModel::Units { inspected, .. } = save::save_section_model(
                &frame.workspace,
                document,
                state,
                frame.save_dictionary(),
            )
            .unwrap() else {
                panic!("units section must remain active");
            };
            assert!(inspected.is_none());
        });
    }

    #[gpui::test]
    fn save_player_only_filter_keeps_a_visible_unit_number_draft(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(2, 0, 0).with_unit_roles([3, 99]).build(),
        );

        cx.run_until_parked();
        click(cx, "save-section-units");
        click(cx, "save-number-unit-0-troop-info-index");
        cx.simulate_keystrokes("9");
        click(cx, "save-unit-filter-player-only");

        frame.update_in(cx, |frame, window, _| {
            let edit = frame.number_edit.as_ref().unwrap();
            assert!(edit.target.is_save(
                document,
                SaveNumberTarget::Unit {
                    unit: 0,
                    field: SaveUnitField::TroopInfoIndex,
                },
            ));
            assert_eq!(edit.editor.draft(), "9");
            assert_eq!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                0,
            );
            assert!(frame.focus.is_focused(window));
        });
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
    fn save_virtual_unit_keyboard_navigation_reaches_an_offscreen_typed_target(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(96, 0, 0)
                .with_unit_roles(std::iter::repeat_n(3, 96))
                .build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        assert!(cx.debug_bounds("save-unit-master-row-95").is_none());
        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);

        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 95);
        assert!(cx.debug_bounds("save-unit-master-row-95").is_some());
        assert!(rendered_unit_count(cx, 96) < 96);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.units.focus.is_focused(window));
        });

        cx.simulate_keystrokes("tab");
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target.is_save(
                    document,
                    SaveNumberTarget::Unit {
                        unit: 95,
                        field: SaveUnitField::TroopInfoIndex,
                    },
                )
            }));
        });
        cx.simulate_keystrokes("9 enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Unit {
                            unit: 95,
                            field: SaveUnitField::TroopInfoIndex,
                        },
                    )
                    .unwrap(),
                9,
            );
        });
    }

    #[gpui::test]
    fn save_virtual_unit_keyboard_navigation_maps_filtered_sources_and_clamps_boundaries(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let roles = (0..96).map(|index| if index % 12 == 0 { 3 } else { 99 });
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(96, 0, 0).with_unit_roles(roles).build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        click(cx, "save-unit-filter-player-only");
        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        cx.simulate_keystrokes("end down");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 84);
        cx.simulate_keystrokes("up");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 72);
        cx.simulate_keystrokes("home up");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 0);
        cx.simulate_keystrokes("pagedown");
        let paged = save_state(&frame, cx, document).inspected_unit();
        assert!(paged > 0 && paged.is_multiple_of(12));
        cx.simulate_keystrokes("pageup");
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 0);
    }

    #[gpui::test]
    fn save_virtual_unit_root_recovers_filtered_cursor_after_scroll_eviction(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let roles = (0..96).map(|index| if index % 2 == 0 { 3 } else { 99 });
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(96, 0, 0).with_unit_roles(roles).build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        click(cx, "save-unit-filter-player-only");
        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);
        assert_eq!(save_state(&frame, cx, document).inspected_unit(), 94);
        assert!(has_debug_bounds(cx, "save-unit-master-row-94"));

        focus_frame(&frame, cx);
        let binding_before_scroll =
            frame.update(cx, |frame, _| frame.save_lists.units.binding.get());
        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::Units, 0);
        assert_eq!(
            frame.update(cx, |frame, _| frame.save_lists.units.binding.get()),
            binding_before_scroll,
        );
        assert_eq!(
            frame.update(cx, |frame, _| frame
                .save_lists
                .units
                .scroll
                .logical_scroll_top_index()),
            0,
        );
        assert!(has_debug_bounds(cx, "save-unit-master-row-0"));
        let generation_before_recovery =
            frame.update(cx, |frame, _| frame.save_lists.units.generation.get());

        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        draw_frame(cx, &frame);
        cx.run_until_parked();
        frame.update_in(cx, |frame, window, _| {
            assert!(
                frame.save_lists.units.focus.is_focused(window),
                "expected Units root focus after recovery Tab; AppFrame focused: {}",
                frame.focus.is_focused(window),
            );
        });
        let (cursor, position, row_count, generation_after_recovery, offset_after_recovery) =
            save_list_control_state(&frame, cx, SaveListKind::Units);
        assert_eq!(cursor, SaveListCursor::Unit { source_index: 94 });
        assert_eq!(position, 47);
        assert_eq!(row_count, 48);
        assert!(generation_after_recovery > generation_before_recovery);
        assert!(offset_after_recovery < px(0.0));
        draw_frame(cx, &frame);
        assert!(has_debug_bounds(cx, "save-unit-master-row-94"));
        assert!(rendered_unit_count(cx, 96) < 96);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.units.focus.is_focused(window));
        });

        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::Units, 0);
        assert_eq!(
            frame.update(cx, |frame, _| frame
                .save_lists
                .units
                .scroll
                .logical_scroll_top_index()),
            0,
        );
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.units.focus.is_focused(window));
        });
        let generation_before_reveal =
            frame.update(cx, |frame, _| frame.save_lists.units.generation.get());
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);
        let (generation_after_reveal, offset_after_reveal) = frame.update(cx, |frame, _| {
            let control = &frame.save_lists.units;
            (
                control.generation.get(),
                control.scroll.0.borrow().base_handle.offset().y,
            )
        });
        assert!(generation_after_reveal > generation_before_reveal);
        assert!(offset_after_reveal < px(0.0));

        key_down(cx, "enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));
        key_up(cx, "enter");
        cx.simulate_keystrokes("tab");
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target.is_save(
                    document,
                    SaveNumberTarget::Unit {
                        unit: 94,
                        field: SaveUnitField::TroopInfoIndex,
                    },
                )
            }));
        });
    }

    #[gpui::test]
    fn save_virtual_unit_native_activation_reveals_an_evicted_cursor(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(
            &frame,
            cx,
            SaveFixture::new(96, 0, 0)
                .with_unit_roles(std::iter::repeat_n(3, 96))
                .build(),
        );

        select_and_draw(&frame, cx, document, SaveSection::Units);
        focus_frame(&frame, cx);
        press_tabs(cx, 7);
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);
        let binding_before_scroll =
            frame.update(cx, |frame, _| frame.save_lists.units.binding.get());

        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::Units, 0);
        assert_eq!(
            frame.update(cx, |frame, _| frame.save_lists.units.binding.get()),
            binding_before_scroll,
        );
        assert!(has_debug_bounds(cx, "save-unit-master-row-0"));
        let generation_before_activation =
            assert_save_list_cursor_is_evicted(&frame, cx, SaveListKind::Units);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.units.focus.is_focused(window));
        });

        key_cycle(cx, "enter");
        draw_frame(cx, &frame);

        assert!(has_debug_bounds(cx, "save-unit-master-row-95"));
        assert!(rendered_unit_count(cx, 96) < 96);
        let (cursor, position, row_count, generation_after_activation, offset) =
            save_list_control_state(&frame, cx, SaveListKind::Units);
        assert_eq!(cursor, SaveListCursor::Unit { source_index: 95 });
        assert_eq!(position, 95);
        assert_eq!(row_count, 96);
        assert!(generation_after_activation > generation_before_activation);
        assert!(offset < px(0.0));
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.units.focus.is_focused(window));
            assert_eq!(
                frame
                    .save_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                95,
            );
            assert!(!frame.workspace.can_undo(document).unwrap());
        });
    }

    #[gpui::test]
    fn save_virtual_roster_keyboard_navigation_reaches_an_offscreen_nonfirst_field(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 96, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        assert!(!has_debug_bounds(cx, "save-roster-95-field-value-64"));
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        cx.simulate_keystrokes("end right right right right");
        draw_frame(cx, &frame);

        assert!(has_debug_bounds(cx, "save-roster-95-field-value-64"));
        assert!(rendered_roster_count(cx, 96) < 96);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.roster.focus.is_focused(window));
        });
        cx.simulate_keystrokes("right");
        assert_eq!(
            save_state(&frame, cx, document).list_cursor(SaveListKind::Roster),
            SaveListCursor::Roster {
                record: 95,
                field: SaveRosterField::Value64,
            },
        );
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target.is_save(
                    document,
                    SaveNumberTarget::Roster {
                        record: 95,
                        field: SaveRosterField::Value64,
                    },
                )
            }));
        });
        cx.simulate_keystrokes("9 enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Roster {
                            record: 95,
                            field: SaveRosterField::Value64,
                        },
                    )
                    .unwrap(),
                9,
            );
        });
    }

    #[gpui::test]
    fn save_virtual_roster_root_recovers_nonfirst_field_after_scroll_eviction(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 96, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        cx.simulate_keystrokes("end right right right right");
        draw_frame(cx, &frame);
        assert!(has_debug_bounds(cx, "save-roster-95-field-value-64"));

        focus_frame(&frame, cx);
        let binding_before_scroll =
            frame.update(cx, |frame, _| frame.save_lists.roster.binding.get());
        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::Roster, 0);
        assert_eq!(
            frame.update(cx, |frame, _| frame.save_lists.roster.binding.get()),
            binding_before_scroll,
        );
        assert_eq!(
            frame.update(cx, |frame, _| frame
                .save_lists
                .roster
                .scroll
                .logical_scroll_top_index()),
            0,
        );
        assert!(has_debug_bounds(cx, "save-roster-field-byte-60"));
        let generation_before_recovery =
            frame.update(cx, |frame, _| frame.save_lists.roster.generation.get());

        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        draw_frame(cx, &frame);
        cx.run_until_parked();
        let (cursor, position, row_count, generation_after_recovery, offset_after_recovery) =
            save_list_control_state(&frame, cx, SaveListKind::Roster);
        assert_eq!(
            cursor,
            SaveListCursor::Roster {
                record: 95,
                field: SaveRosterField::Value64,
            },
        );
        assert_eq!(position, 95);
        assert_eq!(row_count, 96);
        assert!(generation_after_recovery > generation_before_recovery);
        assert!(offset_after_recovery < px(0.0));
        assert!(has_debug_bounds(cx, "save-roster-95-field-value-64"));
        assert!(rendered_roster_count(cx, 96) < 96);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.roster.focus.is_focused(window));
        });

        cx.simulate_keystrokes("tab");
        frame.update_in(cx, |frame, window, _| {
            assert!(!frame.save_lists.roster.focus.is_focused(window));
        });
        cx.simulate_keystrokes("shift-tab");
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.roster.focus.is_focused(window));
        });

        key_down(cx, "enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));
        key_up(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target.is_save(
                    document,
                    SaveNumberTarget::Roster {
                        record: 95,
                        field: SaveRosterField::Value64,
                    },
                )
            }));
        });
    }

    #[gpui::test]
    fn save_virtual_roster_native_enter_reveals_an_evicted_nonfirst_field_before_editing(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 96, 0).build());
        let target = SaveNumberTarget::Roster {
            record: 95,
            field: SaveRosterField::Value64,
        };

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        cx.simulate_keystrokes("end right right right right");
        draw_frame(cx, &frame);
        let binding_before_scroll =
            frame.update(cx, |frame, _| frame.save_lists.roster.binding.get());

        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::Roster, 0);
        assert_eq!(
            frame.update(cx, |frame, _| frame.save_lists.roster.binding.get()),
            binding_before_scroll,
        );
        assert!(has_debug_bounds(cx, "save-roster-field-value-64"));
        let generation_before_activation =
            assert_save_list_cursor_is_evicted(&frame, cx, SaveListKind::Roster);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.roster.focus.is_focused(window));
        });

        key_down(cx, "enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));
        key_up(cx, "enter");
        draw_frame(cx, &frame);

        assert!(has_debug_bounds(cx, "save-roster-95-field-value-64"));
        assert!(rendered_roster_count(cx, 96) < 96);
        let (cursor, position, row_count, generation_after_activation, offset) =
            save_list_control_state(&frame, cx, SaveListKind::Roster);
        assert_eq!(
            cursor,
            SaveListCursor::Roster {
                record: 95,
                field: SaveRosterField::Value64,
            },
        );
        assert_eq!(position, 95);
        assert_eq!(row_count, 96);
        assert!(generation_after_activation > generation_before_activation);
        assert!(offset < px(0.0));
        assert_save_number_draft(&frame, cx, document, target);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.focus.is_focused(window));
            assert!(!frame.workspace.can_undo(document).unwrap());
        });

        cx.simulate_keystrokes("9 enter");
        assert_single_save_number_edit(&frame, cx, document, target, 0, 9);
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(
                        document,
                        SaveNumberTarget::Roster {
                            record: 0,
                            field: SaveRosterField::Value64,
                        },
                    )
                    .unwrap(),
                0,
            );
        });
    }

    #[gpui::test]
    fn save_virtual_second_array_keyboard_navigation_reaches_an_offscreen_typed_target(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 96).build());

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        assert!(cx.debug_bounds("save-number-second-array-95").is_none());
        focus_frame(&frame, cx);
        press_tabs(cx, 27);
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);

        assert!(cx.debug_bounds("save-number-second-array-95").is_some());
        assert!(rendered_second_array_count(cx, 96) < 96);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.second_array.focus.is_focused(window));
        });
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target
                    .is_save(document, SaveNumberTarget::SecondArray { record: 95 })
            }));
        });
        cx.simulate_keystrokes("9 enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::SecondArray { record: 95 })
                    .unwrap(),
                9,
            );
        });
    }

    #[gpui::test]
    fn save_virtual_second_array_root_recovers_cursor_after_scroll_eviction(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 96).build());

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        focus_frame(&frame, cx);
        press_tabs(cx, 27);
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);
        assert!(has_debug_bounds(cx, "save-number-second-array-95"));

        focus_frame(&frame, cx);
        let binding_before_scroll =
            frame.update(cx, |frame, _| frame.save_lists.second_array.binding.get());
        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::SecondArray, 0);
        assert_eq!(
            frame.update(cx, |frame, _| {
                frame.save_lists.second_array.binding.get()
            }),
            binding_before_scroll,
        );
        assert_eq!(
            frame.update(cx, |frame, _| frame
                .save_lists
                .second_array
                .scroll
                .logical_scroll_top_index()),
            0,
        );
        assert!(has_debug_bounds(cx, "save-number-second-array-0"));
        let generation_before_recovery = frame.update(cx, |frame, _| {
            frame.save_lists.second_array.generation.get()
        });

        focus_frame(&frame, cx);
        press_tabs(cx, 27);
        draw_frame(cx, &frame);
        cx.run_until_parked();
        let (cursor, position, row_count, generation_after_recovery, offset_after_recovery) =
            save_list_control_state(&frame, cx, SaveListKind::SecondArray);
        assert_eq!(cursor, SaveListCursor::SecondArray { record: 95 });
        assert_eq!(position, 95);
        assert_eq!(row_count, 96);
        assert!(generation_after_recovery > generation_before_recovery);
        assert!(offset_after_recovery < px(0.0));
        assert!(has_debug_bounds(cx, "save-number-second-array-95"));
        assert!(rendered_second_array_count(cx, 96) < 96);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.second_array.focus.is_focused(window));
        });

        key_down(cx, "enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));
        key_up(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target
                    .is_save(document, SaveNumberTarget::SecondArray { record: 95 })
            }));
        });
    }

    #[gpui::test]
    fn save_virtual_second_array_native_space_reveals_an_evicted_cursor_before_editing(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 96).build());
        let target = SaveNumberTarget::SecondArray { record: 95 };

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        focus_frame(&frame, cx);
        press_tabs(cx, 27);
        cx.simulate_keystrokes("end");
        draw_frame(cx, &frame);
        let binding_before_scroll =
            frame.update(cx, |frame, _| frame.save_lists.second_array.binding.get());

        scroll_save_list_away_from_cursor(&frame, cx, SaveListKind::SecondArray, 0);
        assert_eq!(
            frame.update(cx, |frame, _| {
                frame.save_lists.second_array.binding.get()
            }),
            binding_before_scroll,
        );
        assert!(has_debug_bounds(cx, "save-number-second-array-0"));
        let generation_before_activation =
            assert_save_list_cursor_is_evicted(&frame, cx, SaveListKind::SecondArray);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.save_lists.second_array.focus.is_focused(window));
        });

        platform_space_key_down(cx);
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));
        key_up(cx, "space");
        draw_frame(cx, &frame);

        assert!(has_debug_bounds(cx, "save-number-second-array-95"));
        assert!(rendered_second_array_count(cx, 96) < 96);
        let (cursor, position, row_count, generation_after_activation, offset) =
            save_list_control_state(&frame, cx, SaveListKind::SecondArray);
        assert_eq!(cursor, SaveListCursor::SecondArray { record: 95 });
        assert_eq!(position, 95);
        assert_eq!(row_count, 96);
        assert!(generation_after_activation > generation_before_activation);
        assert!(offset < px(0.0));
        assert_save_number_draft(&frame, cx, document, target);
        frame.update_in(cx, |frame, window, _| {
            assert!(frame.focus.is_focused(window));
            assert!(!frame.workspace.can_undo(document).unwrap());
        });

        cx.simulate_keystrokes("7 enter");
        assert_single_save_number_edit(&frame, cx, document, target, 95, 7);
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_number(document, SaveNumberTarget::SecondArray { record: 0 })
                    .unwrap(),
                0,
            );
        });
    }

    #[gpui::test]
    fn save_virtual_focus_guard_rejects_stale_generation_and_source_binding(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 4, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        frame.update_in(cx, |frame, window, cx| {
            frame.move_save_list_cursor(
                document,
                SaveListKind::Roster,
                SaveListMovement::End,
                window,
                cx,
            );
            let control = &frame.save_lists.roster;
            let binding = control.binding.get().unwrap();
            let generation = control.generation.get();
            control.next_generation();
            assert!(!frame.save_list_focus_request_is_current(
                SaveListKind::Roster,
                binding,
                generation,
            ));

            let stale_source = super::super::SaveListBinding {
                position: 0,
                ..binding
            };
            let generation = control.generation.get();
            control.binding.set(Some(stale_source));
            assert!(!frame.save_list_focus_request_is_current(
                SaveListKind::Roster,
                stale_source,
                generation,
            ));
        });
    }

    #[gpui::test]
    fn save_virtual_empty_lists_do_not_add_actionable_tab_stops(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 0, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        frame.update_in(cx, |frame, window, _| {
            assert!(!frame.save_lists.roster.focus.is_focused(window));
        });
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        focus_frame(&frame, cx);
        press_tabs(cx, 27);
        frame.update_in(cx, |frame, window, _| {
            assert!(!frame.save_lists.second_array.focus.is_focused(window));
        });
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));
    }

    #[gpui::test]
    fn save_virtual_list_root_ignores_pointer_clicks_on_container_whitespace(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 1, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        let bounds = cx
            .debug_bounds("save-virtual-list-root-roster")
            .expect("roster list root must always be painted");
        cx.simulate_click(
            point(bounds.center().x, bounds.bottom() - px(4.0)),
            Modifiers::none(),
        );

        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.is_none());
            assert!(!frame.workspace.can_undo(document).unwrap());
        });
    }

    #[gpui::test]
    fn save_virtual_deferred_focus_cannot_escape_after_a_section_change(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(96, 0, 0).build());

        select_and_draw(&frame, cx, document, SaveSection::Units);
        let (binding, generation) = frame.update_in(cx, |frame, window, cx| {
            frame.move_save_list_cursor(
                document,
                SaveListKind::Units,
                SaveListMovement::End,
                window,
                cx,
            );
            let control = &frame.save_lists.units;
            let request = (control.binding.get().unwrap(), control.generation.get());
            frame.select_save_section(document, SaveSection::Summary, cx);
            window.focus(&frame.focus);
            request
        });
        frame.update(cx, |frame, _| {
            assert!(!frame.save_list_focus_request_is_current(
                SaveListKind::Units,
                binding,
                generation,
            ));
        });
        draw_frame(cx, &frame);
        cx.run_until_parked();

        frame.update_in(cx, |frame, window, _| {
            assert_eq!(
                frame.save_presentations.get(document).unwrap().section(),
                SaveSection::Summary,
            );
            assert!(frame.focus.is_focused(window));
            assert!(!frame.save_lists.units.focus.is_focused(window));
        });
    }

    #[gpui::test]
    fn save_virtual_mouse_edits_update_roster_and_second_array_cursors(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_save(&frame, cx, SaveFixture::new(0, 8, 8).build());

        select_and_draw(&frame, cx, document, SaveSection::Roster);
        click(cx, "save-roster-3-field-byte-62");
        cx.simulate_keystrokes("escape");
        focus_frame(&frame, cx);
        press_tabs(cx, 6);
        cx.simulate_keystrokes("up");
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target.is_save(
                    document,
                    SaveNumberTarget::Roster {
                        record: 2,
                        field: SaveRosterField::Byte62,
                    },
                )
            }));
        });
        cx.simulate_keystrokes("escape");

        select_and_draw(&frame, cx, document, SaveSection::Missions);
        click(cx, "save-number-second-array-3");
        cx.simulate_keystrokes("escape");
        focus_frame(&frame, cx);
        press_tabs(cx, 27);
        cx.simulate_keystrokes("up");
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert!(frame.number_edit.as_ref().is_some_and(|edit| {
                edit.target
                    .is_save(document, SaveNumberTarget::SecondArray { record: 2 })
            }));
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
        click(cx, "save-unit-filter-player-only");
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
        click(cx, "save-unit-filter-player-only");
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
    fn save_text_commit_restores_app_frame_focus(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-text-set-file");
        cx.simulate_keystrokes("B r a v o enter");

        frame.update_in(cx, |frame, window, _| {
            assert!(frame.text_edit.is_none());
            assert!(frame.focus.is_focused(window));
        });
    }

    #[gpui::test]
    fn save_text_cancel_restores_app_frame_focus(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        activate_save(
            &frame,
            cx,
            SaveFixture::new(0, 0, 0)
                .with_save_file_name(b"Alpha")
                .build(),
        );

        cx.run_until_parked();
        click(cx, "save-text-set-file");
        cx.simulate_keystrokes("escape");

        frame.update_in(cx, |frame, window, _| {
            assert!(frame.text_edit.is_none());
            assert!(frame.focus.is_focused(window));
        });
    }

    #[gpui::test]
    fn save_text_input_tab_reenters_save_controls_without_committing(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
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
        cx.simulate_keystrokes("B r a v o");
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        key_cycle(cx, "enter");

        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .save_text(document, SaveTextField::SetFile)
                    .unwrap(),
                "Alpha",
            );
            assert!(frame.text_edit.is_none());
            assert_eq!(
                frame.save_presentations.get(document).unwrap().section(),
                SaveSection::Units,
            );
        });
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
        let (global_request, crusaders_key, settings_revision, settings_settled, task_launches) =
            frame.update(cx, |frame, cx| {
                let global_request = frame.shell.begin_catalog();
                let crusaders_key = CrusadersCatalogKey::new(
                    frame.shell.begin_crusaders_catalog(),
                    PathBuf::from("/catalog/editing"),
                );
                frame.crusaders_catalog.begin(crusaders_key.clone());
                frame.notices.replace(
                    NoticeSource::Workspace,
                    Notice::info("Persistent workspace notice"),
                );
                cx.notify();
                (
                    global_request,
                    crusaders_key,
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
            assert!(
                frame
                    .shell
                    .accepts_crusaders_catalog(crusaders_key.request())
            );
            assert_eq!(frame.shell.game(), Game::Crusaders);
            assert_eq!(frame.shell.area(), Area::Files);
            assert_eq!(frame.settings.latest_revision_for_test(), settings_revision);
            assert_eq!(frame.settings.is_settled(), settings_settled);
            assert_eq!(frame.task_launches, task_launches);
            assert_eq!(
                frame.notices.current().map(Notice::summary),
                Some("Persistent workspace notice"),
            );
            assert!(frame.crusaders_catalog.finish_failed(
                crusaders_key.clone(),
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
            assert!(
                frame
                    .shell
                    .accepts_crusaders_catalog(crusaders_key.request())
            );
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
            SaveFixture::new(2, 0, 0).with_unit_roles([99, 0]).build(),
        );

        cx.run_until_parked();
        click(cx, "save-number-main-field-00");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-section-units");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_none()));

        click(cx, "save-number-unit-0-troop-info-index");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
        click(cx, "save-unit-filter-player-only");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_none()));

        click(cx, "save-unit-filter-player-only");
        click(cx, "save-unit-master-row-0");
        click(cx, "save-number-unit-0-troop-info-index");
        assert!(frame.update(cx, |frame, _| frame.number_edit.is_some()));
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
                .with_unit_roles([99, 0])
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
        click(cx, "save-unit-filter-player-only");
        assert_hidden_text_commit_is_ignored(&frame, cx, first, &stale);

        click(cx, "save-unit-filter-player-only");
        click(cx, "save-unit-master-row-0");
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
            SaveFixture::new(1, 0, 0).with_unit_roles([99]).build(),
        );
        frame.update(cx, |frame, cx| {
            frame.set_save_player_only(document, true, cx);
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
            let key =
                CrusadersCatalogKey::new(frame.shell.begin_crusaders_catalog(), "/catalog/loading");
            frame.crusaders_catalog.begin(key.clone());
            cx.notify();
            key
        });
        draw_frame(cx, &frame);
        assert!(cx.debug_bounds("save-catalog-loading").is_some());

        frame.update(cx, |frame, cx| {
            assert!(frame.crusaders_catalog.finish_failed(
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
            let key =
                CrusadersCatalogKey::new(frame.shell.begin_crusaders_catalog(), "/catalog/ready");
            frame.crusaders_catalog.begin(key.clone());
            assert!(frame.crusaders_catalog.finish_ready(
                key,
                Arc::new(missing_name_dictionary()),
                2,
            ));
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

    #[track_caller]
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
        cx.run_until_parked();
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

    fn focus_frame(frame: &Entity<AppFrame>, cx: &mut VisualTestContext) {
        frame.update_in(cx, |frame, window, _| window.focus(&frame.focus));
    }

    fn press_tabs(cx: &mut VisualTestContext, count: usize) {
        for _ in 0..count {
            cx.simulate_keystrokes("tab");
        }
    }

    fn key_down(cx: &mut VisualTestContext, key: &str) {
        cx.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
        });
    }

    fn key_up(cx: &mut VisualTestContext, key: &str) {
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
    }

    fn key_cycle(cx: &mut VisualTestContext, key: &str) {
        key_down(cx, key);
        key_up(cx, key);
    }

    fn platform_space_key_down(cx: &mut VisualTestContext) {
        let mut keystroke = Keystroke::parse("space").unwrap();
        keystroke.key_char = Some(" ".to_owned());
        cx.simulate_event(KeyDownEvent {
            keystroke,
            is_held: false,
        });
    }

    fn scroll_save_list_away_from_cursor(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        kind: SaveListKind,
        position: usize,
    ) {
        frame.update(cx, |frame, cx| {
            frame
                .save_lists
                .get(kind)
                .scroll
                .scroll_to_item_strict(position, gpui::ScrollStrategy::Top);
            cx.notify();
        });
        draw_frame(cx, frame);
    }

    fn save_list_control_state(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        kind: SaveListKind,
    ) -> (SaveListCursor, usize, usize, u64, gpui::Pixels) {
        frame.update(cx, |frame, _| {
            let control = frame.save_lists.get(kind);
            let binding = control.binding.get().unwrap();
            (
                binding.cursor,
                binding.position,
                binding.row_count,
                control.generation.get(),
                control.scroll.0.borrow().base_handle.offset().y,
            )
        })
    }

    #[track_caller]
    fn assert_save_list_cursor_is_evicted(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        kind: SaveListKind,
    ) -> u64 {
        frame.update(cx, |frame, _| {
            let control = frame.save_lists.get(kind);
            let binding = control.binding.get().unwrap();
            assert_eq!(control.scroll.logical_scroll_top_index(), 0);
            assert_eq!(control.scroll.0.borrow().base_handle.offset().y, px(0.0));
            assert!(
                binding.position >= frame.save_list_page_size(kind, binding.row_count),
                "typed cursor must be outside the direct-scroll viewport",
            );
            control.generation.get()
        })
    }

    #[track_caller]
    fn assert_save_number_draft(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
        target: SaveNumberTarget,
    ) {
        frame.update(cx, |frame, _| {
            assert!(
                frame
                    .number_edit
                    .as_ref()
                    .is_some_and(|edit| edit.target.is_save(document, target)),
                "expected save number draft for {target:?}",
            );
        });
    }

    #[track_caller]
    fn assert_single_save_number_edit(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
        target: SaveNumberTarget,
        original: i64,
        edited: i64,
    ) {
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.workspace.save_number(document, target).unwrap(),
                edited,
            );
            assert!(frame.workspace.undo(document).unwrap());
            assert_eq!(
                frame.workspace.save_number(document, target).unwrap(),
                original,
            );
            assert!(!frame.workspace.undo(document).unwrap());
        });
    }

    fn has_debug_bounds(cx: &mut VisualTestContext, selector: &str) -> bool {
        let selector = Box::leak(selector.to_owned().into_boxed_str());
        cx.debug_bounds(selector).is_some()
    }

    fn rendered_unit_count(cx: &mut VisualTestContext, count: usize) -> usize {
        (0..count)
            .filter(|unit| has_debug_bounds(cx, &format!("save-unit-master-row-{unit}")))
            .count()
    }

    fn rendered_roster_count(cx: &mut VisualTestContext, count: usize) -> usize {
        (0..count)
            .filter(|record| {
                let selector = if *record == 0 {
                    "save-roster-field-byte-60".to_owned()
                } else {
                    format!("save-roster-{record}-field-byte-60")
                };
                has_debug_bounds(cx, &selector)
            })
            .count()
    }

    fn rendered_second_array_count(cx: &mut VisualTestContext, count: usize) -> usize {
        (0..count)
            .filter(|record| has_debug_bounds(cx, &format!("save-number-second-array-{record}")))
            .count()
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

    fn visible_save_unit_indices(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
    ) -> Vec<usize> {
        frame.update(cx, |frame, _| {
            let player_only = frame
                .save_presentations
                .get(document)
                .is_some_and(crate::state::SavePresentationState::player_only);
            save::SaveRows::units(&frame.workspace, document, player_only)
                .unwrap()
                .locations(0..usize::MAX)
                .into_iter()
                .map(|location| location.source_index)
                .collect()
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
