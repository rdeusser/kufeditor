use gpui::{Context, Div, div, prelude::*, px};
use kufeditor_workspace::{
    DocumentID, STGAreaField, STGEventTarget, STGNumberTarget, STGReferenceKind, STGTailStatus,
    STGTextTarget, STGUnitField, STGValue, WorkspaceError,
};

use super::AppFrame;
use crate::{
    state::{
        STGDocumentTransition, STGIndexVisibility, STGReferenceVisibility, STGVisibleSelections,
    },
    views::stg::{
        STGDocumentProjection, STGEventBlockProjection, STGEventRows, STGIndexRows,
        STGReferenceRows, STGSearchQuery, STGSearchRecord, STGTailProjection,
    },
};

type STGProjectionResult<T> = Result<T, Box<WorkspaceError>>;

struct STGPresentationProjection {
    document: STGDocumentProjection,
    units: STGIndexRows,
    events: STGEventRows,
}

impl STGPresentationProjection {
    fn visibility(&self) -> STGVisibleSelections<'_> {
        STGVisibleSelections::new(
            self.units.visibility(),
            STGIndexVisibility::Range(0..self.document.areas().map_or(0, STGIndexRows::len)),
            STGIndexVisibility::Range(0..self.document.variables().map_or(0, STGIndexRows::len)),
            self.events.visibility(),
            STGIndexVisibility::Range(0..self.document.footer().map_or(0, STGIndexRows::len)),
        )
    }
}

impl AppFrame {
    pub(super) fn stg_projection(
        &self,
        document: DocumentID,
    ) -> STGProjectionResult<STGDocumentProjection> {
        let units = self.workspace.stg_unit_count(document)?;
        let areas = self.workspace.stg_area_count(document)?;
        let variables = self.workspace.stg_variable_count(document)?;
        let footer = self.workspace.stg_footer_count(document)?;
        let event_blocks = self
            .workspace
            .stg_event_block_count(document)?
            .map(|count| {
                (0..count)
                    .map(|block| {
                        let projection = self.workspace.stg_event_block(document, block)?;
                        Ok(STGEventBlockProjection::new(
                            block,
                            projection.header,
                            projection.event_count,
                        ))
                    })
                    .collect::<STGProjectionResult<Vec<_>>>()
            })
            .transpose()?;
        let tail = match self.workspace.stg_tail_status(document)? {
            STGTailStatus::Parsed { suffix } => STGTailProjection::Parsed {
                suffix_bytes: suffix.len(),
            },
            STGTailStatus::Raw { bytes, failure } => STGTailProjection::Raw {
                bytes: bytes.len(),
                region: failure.region().to_string(),
                offset: failure.offset(),
            },
        };
        Ok(STGDocumentProjection::new(
            units,
            areas,
            variables,
            event_blocks,
            footer,
            tail,
        ))
    }

    pub(super) fn activate_stg_presentation(
        &mut self,
        document: DocumentID,
        cx: &mut Context<Self>,
    ) {
        let Ok(projection) = self.stg_presentation_projection(document) else {
            return;
        };
        let transition =
            self.stg_presentations
                .activate_document(document, &projection.visibility(), None);
        if transition.changed() {
            cx.notify();
        }
    }

    pub(super) fn deactivate_stg_presentation(&mut self, cx: &mut Context<Self>) {
        if self
            .stg_presentations
            .deactivate_active_document(None)
            .changed()
        {
            cx.notify();
        }
    }

    pub(super) fn reconcile_stg_presentation(
        &mut self,
        document: DocumentID,
        cause: STGDocumentTransition,
        cx: &mut Context<Self>,
    ) {
        let Ok(projection) = self.stg_presentation_projection(document) else {
            return;
        };
        let (expanded, picker) =
            self.stg_presentations
                .get(document)
                .map_or((None, None), |state| {
                    (
                        state.expanded_script(),
                        state.reference_picker().map(|picker| {
                            (picker.target(), picker.kind(), picker.query().to_owned())
                        }),
                    )
                });
        let projected_expanded = expanded.and_then(|target| cause.remap_script_target(target));
        let projected_picker_target = picker.as_ref().and_then(|(target, _, _)| {
            cause.remap_script_target(target.script).map(|script| {
                kufeditor_workspace::STGParameterTarget {
                    script,
                    parameter: target.parameter,
                }
            })
        });
        let visible_scripts = projected_expanded
            .filter(|target| self.workspace.stg_script(document, *target).is_ok())
            .into_iter()
            .collect::<Vec<_>>();
        let picker_visible =
            picker
                .as_ref()
                .zip(projected_picker_target)
                .is_some_and(|((_, kind, _), target)| {
                    projected_expanded == Some(target.script)
                        && self
                            .workspace
                            .stg_parameter(document, target)
                            .is_ok_and(|parameter| {
                                parameter.reference == Some(*kind)
                                    && matches!(
                                        parameter.value,
                                        STGValue::Integer(_) | STGValue::Enum(_)
                                    )
                            })
                });
        let reference_rows = match picker.as_ref().filter(|_| picker_visible) {
            Some((_, kind, query)) => {
                let Ok(rows) = self.stg_reference_rows(document, *kind, query, &projection) else {
                    return;
                };
                Some(rows)
            }
            None => None,
        };
        let reference_visibility = match reference_rows.as_ref() {
            Some(rows) => rows.visibility(),
            None => STGReferenceVisibility::empty(),
        };
        let transition = self.stg_presentations.reconcile_document(
            document,
            &projection.visibility(),
            &visible_scripts,
            picker_visible,
            &reference_visibility,
            cause,
            None,
        );
        if transition.changed() {
            cx.notify();
        }
    }

    fn stg_presentation_projection(
        &self,
        document: DocumentID,
    ) -> STGProjectionResult<STGPresentationProjection> {
        let projection = self.stg_projection(document)?;
        let (unit_query, event_query) = self.stg_presentations.get(document).map_or_else(
            || (String::new(), String::new()),
            |state| {
                (
                    state.unit_query().to_owned(),
                    state.event_query().to_owned(),
                )
            },
        );
        let unit_query = STGSearchQuery::new(&unit_query);
        let event_query = STGSearchQuery::new(&event_query);
        let units = self.stg_unit_rows(document, &unit_query, projection.units().len())?;
        let base_events = projection
            .events()
            .cloned()
            .unwrap_or_else(|| STGEventRows::from_blocks(Vec::new()));
        let events = self.stg_event_rows(document, &event_query, base_events)?;
        Ok(STGPresentationProjection {
            document: projection,
            units,
            events,
        })
    }

    fn stg_unit_rows(
        &self,
        document: DocumentID,
        query: &STGSearchQuery,
        count: usize,
    ) -> STGProjectionResult<STGIndexRows> {
        if query.is_empty() {
            return Ok(STGIndexRows::range(count));
        }

        let dictionary = self.visible_crusaders_dictionary();
        let mut visible = Vec::new();
        for unit in 0..count {
            let name = self
                .workspace
                .stg_text(document, STGTextTarget::UnitName { unit })?;
            let source_name = name.decoded();
            let unique_id = self.workspace.stg_number(
                document,
                STGNumberTarget::Unit {
                    unit,
                    field: STGUnitField::UniqueID,
                },
            )?;
            let job_type = self
                .workspace
                .stg_number(
                    document,
                    STGNumberTarget::Unit {
                        unit,
                        field: STGUnitField::LeaderJobType,
                    },
                )
                .ok()
                .and_then(|value| u8::try_from(value).ok());
            let model_id = self
                .workspace
                .stg_number(
                    document,
                    STGNumberTarget::Unit {
                        unit,
                        field: STGUnitField::LeaderModelID,
                    },
                )
                .ok()
                .and_then(|value| u8::try_from(value).ok());
            let resolved = match (dictionary.as_ref(), source_name, job_type, model_id) {
                (Some(dictionary), Some(source_name), Some(job_type), Some(model_id)) => {
                    Some(dictionary.stg_unit_name(source_name, job_type, model_id))
                }
                _ => None,
            };
            if STGSearchRecord::new(unit, source_name, resolved.as_deref(), Some(unique_id))
                .matches(query)
            {
                visible.push(unit);
            }
        }
        Ok(STGIndexRows::filtered(visible))
    }

    fn stg_event_rows(
        &self,
        document: DocumentID,
        query: &STGSearchQuery,
        base: STGEventRows,
    ) -> STGProjectionResult<STGEventRows> {
        if query.is_empty() {
            return Ok(base);
        }

        let visible = self.stg_matching_events(document, query, &base, |_, target| target)?;
        Ok(STGEventRows::filtered(visible))
    }

    fn stg_matching_events<T>(
        &self,
        document: DocumentID,
        query: &STGSearchQuery,
        base: &STGEventRows,
        mut project: impl FnMut(usize, STGEventTarget) -> T,
    ) -> STGProjectionResult<Vec<T>> {
        let dictionary = self.visible_crusaders_dictionary();
        let mut visible = Vec::new();
        for (position, target) in base.targets().enumerate() {
            let event = self.workspace.stg_event(document, target)?;
            let source_text = event.description.decoded();
            let translated = dictionary
                .as_ref()
                .and_then(|dictionary| source_text.and_then(|text| dictionary.translate(text)));
            if STGSearchRecord::new(
                position,
                source_text,
                translated.as_deref(),
                Some(i64::from(event.id)),
            )
            .matches(query)
            {
                visible.push(project(position, target));
            }
        }
        Ok(visible)
    }

    fn stg_reference_rows(
        &self,
        document: DocumentID,
        kind: STGReferenceKind,
        query: &str,
        projection: &STGPresentationProjection,
    ) -> STGProjectionResult<STGReferenceRows> {
        let query = STGSearchQuery::new(query);
        if query.is_empty() {
            if matches!(kind, STGReferenceKind::Event | STGReferenceKind::Trigger) {
                return Ok(STGReferenceRows::from_event_rows(
                    kind,
                    projection
                        .document
                        .events()
                        .cloned()
                        .unwrap_or_else(|| STGEventRows::from_blocks(Vec::new())),
                ));
            }
            return Ok(STGReferenceRows::range(
                kind,
                Self::stg_reference_count(kind, projection),
            ));
        }

        match kind {
            STGReferenceKind::Troop => Ok(STGReferenceRows::from_rows(
                kind,
                self.stg_unit_rows(document, &query, projection.document.units().len())?,
            )),
            STGReferenceKind::Area => {
                let count = projection.document.areas().map_or(0, STGIndexRows::len);
                let dictionary = self.visible_crusaders_dictionary();
                let mut visible = Vec::new();
                for area in 0..count {
                    let description = self
                        .workspace
                        .stg_text(document, STGTextTarget::AreaDescription { area })?;
                    let source_text = description.decoded();
                    let translated = dictionary.as_ref().and_then(|dictionary| {
                        source_text.and_then(|text| dictionary.translate(text))
                    });
                    let id = self.workspace.stg_number(
                        document,
                        STGNumberTarget::Area {
                            area,
                            field: STGAreaField::AreaID,
                        },
                    )?;
                    if STGSearchRecord::new(area, source_text, translated.as_deref(), Some(id))
                        .matches(&query)
                    {
                        visible.push(area);
                    }
                }
                Ok(STGReferenceRows::filtered(kind, visible))
            }
            STGReferenceKind::Variable => {
                let count = projection.document.variables().map_or(0, STGIndexRows::len);
                let dictionary = self.visible_crusaders_dictionary();
                let mut visible = Vec::new();
                for variable in 0..count {
                    let name = self
                        .workspace
                        .stg_text(document, STGTextTarget::VariableName { variable })?;
                    let source_text = name.decoded();
                    let translated = dictionary.as_ref().and_then(|dictionary| {
                        source_text.and_then(|text| dictionary.translate(text))
                    });
                    let id = self
                        .workspace
                        .stg_number(document, STGNumberTarget::VariableID { variable })?;
                    if STGSearchRecord::new(variable, source_text, translated.as_deref(), Some(id))
                        .matches(&query)
                    {
                        visible.push(variable);
                    }
                }
                Ok(STGReferenceRows::filtered(kind, visible))
            }
            STGReferenceKind::Event | STGReferenceKind::Trigger => {
                let base = projection
                    .document
                    .events()
                    .cloned()
                    .unwrap_or_else(|| STGEventRows::from_blocks(Vec::new()));
                let visible =
                    self.stg_matching_events(document, &query, &base, |_, target| target)?;
                Ok(STGReferenceRows::from_event_rows(
                    kind,
                    STGEventRows::filtered(visible),
                ))
            }
        }
    }

    fn stg_reference_count(
        kind: STGReferenceKind,
        projection: &STGPresentationProjection,
    ) -> usize {
        match kind {
            STGReferenceKind::Troop => projection.document.units().len(),
            STGReferenceKind::Area => projection.document.areas().map_or(0, STGIndexRows::len),
            STGReferenceKind::Variable => {
                projection.document.variables().map_or(0, STGIndexRows::len)
            }
            STGReferenceKind::Event | STGReferenceKind::Trigger => {
                projection.document.events().map_or(0, STGEventRows::len)
            }
        }
    }

    pub(super) fn stg_editor(&self, document: DocumentID) -> Div {
        let projection = match self.stg_projection(document) {
            Ok(projection) => projection,
            Err(error) => {
                return div().size_full().child(
                    div()
                        .id("stg-editor-error")
                        .debug_selector(|| "stg-editor-error".to_owned())
                        .size_full()
                        .p(px(28.0))
                        .text_color(self.theme.text_dim)
                        .child(format!("Could not read STG: {error}")),
                );
            }
        };
        let section = self
            .stg_presentations
            .get(document)
            .map_or("Header", |state| state.section().label());
        let summary = match projection.tail() {
            STGTailProjection::Parsed { .. } => format!(
                "{} units · {} areas · {} events",
                projection.units().len(),
                projection.areas().map_or(0, STGIndexRows::len),
                projection.events().map_or(0, STGEventRows::len),
            ),
            STGTailProjection::Raw { region, offset, .. } => format!(
                "{} units · {region} tail preserved from byte {offset}",
                projection.units().len(),
            ),
        };
        div().size_full().child(
            div()
                .id("stg-editor")
                .debug_selector(|| "stg-editor".to_owned())
                .size_full()
                .p(px(28.0))
                .text_color(self.theme.text_dim)
                .child("STG")
                .child(section)
                .child(summary)
                .child("Structured controls follow this presentation foundation."),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled GPUI and STG fixtures make failures fatal"
    )]

    use std::{fs, path::PathBuf};

    use gpui::{AppContext, Context, TestAppContext, WindowOptions, point, px, size};
    use kufeditor_game::Game;
    use kufeditor_workspace::{
        Document, DocumentEdit, DocumentID, STGDocument, STGEventTarget, STGNumberTarget,
        STGParameterTarget, STGReferenceKind, STGScriptKind, STGScriptTarget, STGStructuralEdit,
        STGTextTarget, STGValueKind, STGValueTarget, TroopDocument,
    };

    use super::super::{AppFrame, EditorRoute, editor_route};
    use crate::{
        crusaders_catalog_status::CrusadersCatalogStatus,
        settings::SettingsStartup,
        state::{
            Area, STGDocumentTransition, STGReferenceCursor, STGReferencePickerState, STGSection,
            STGSelection,
        },
    };

    fn test_startup() -> SettingsStartup {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        drop(file);
        SettingsStartup::load(path)
    }

    fn test_window(cx: &mut TestAppContext) -> gpui::WindowHandle<AppFrame> {
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        })
    }

    fn stg_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 1_001);
        bytes.resize(bytes.len() + 620, 0);
        append_u32(&mut bytes, 1);
        bytes.resize(bytes.len() + 544, 0);
        append_u32(&mut bytes, 1);
        let area = bytes.len();
        bytes.resize(bytes.len() + 84, 0);
        bytes
            .get_mut(area..area + b"Forest".len())
            .unwrap()
            .copy_from_slice(b"Forest");
        bytes
            .get_mut(area + 64..area + 68)
            .unwrap()
            .copy_from_slice(&22_u32.to_le_bytes());
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 9);
        append_u32(&mut bytes, 1);
        bytes.resize(bytes.len() + 64, 0);
        append_u32(&mut bytes, 500);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 15);
        append_u32(&mut bytes, 2);
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 22);
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 20);
        append_u32(&mut bytes, 21);
        bytes
    }

    fn append_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn apply_and_reconcile_stg_structure(
        frame: &mut AppFrame,
        document: DocumentID,
        edit: STGStructuralEdit,
        cx: &mut Context<AppFrame>,
    ) {
        frame
            .workspace
            .apply(document, DocumentEdit::EditSTGStructure { edit })
            .unwrap();
        frame.reconcile_stg_presentation(
            document,
            STGDocumentTransition::StructuralEdit(Some(edit.change())),
            cx,
        );
    }

    fn assert_script_binding(frame: &AppFrame, document: DocumentID, script: STGScriptTarget) {
        let state = frame.stg_presentations.get(document).unwrap();
        assert_eq!(state.expanded_script(), Some(script));
        assert_eq!(
            state
                .reference_picker()
                .map(STGReferencePickerState::target),
            Some(STGParameterTarget {
                script,
                parameter: 0,
            })
        );
    }

    fn assert_reference_cursor(frame: &AppFrame, document: DocumentID, cursor: STGReferenceCursor) {
        assert_eq!(
            frame
                .stg_presentations
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(cursor)
        );
    }

    fn troop_document() -> TroopDocument {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(..8)
            .unwrap()
            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
        TroopDocument::parse(bytes).unwrap()
    }

    #[gpui::test]
    fn stg_presentation_route_activates_state_and_shared_crusaders_catalog(
        cx: &mut TestAppContext,
    ) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame.shell.select_game(Game::Heroes);
                frame.game_paths.set_root(
                    Game::Crusaders,
                    Some(PathBuf::from("/configured/crusaders")),
                );
                frame.select_area(Area::Files, cx);
                let document = frame.workspace.open_loaded(
                    PathBuf::from("mission.stg"),
                    Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                );

                frame.activate_document(document, cx);

                assert_eq!(
                    editor_route(frame.workspace.document_kind(document).unwrap()),
                    EditorRoute::STG
                );
                assert_eq!(frame.active_document, Some(document));
                assert_eq!(frame.shell.game(), Game::Heroes);
                let state = frame.stg_presentations.get(document).unwrap();
                assert_eq!(state.section(), STGSection::Header);
                assert_eq!(state.inspected_unit(), Some(0));
                assert_eq!(state.inspected_area(), Some(0));
                assert_eq!(state.inspected_variable(), None);
                assert_eq!(
                    state.inspected_event(),
                    Some(STGEventTarget { block: 0, event: 0 })
                );
                assert_eq!(state.inspected_footer(), Some(0));
                assert!(matches!(
                    frame.crusaders_catalog.status(),
                    CrusadersCatalogStatus::Loading { key }
                        if key.root() == std::path::Path::new("/configured/crusaders")
                ));

                let projection = frame.stg_projection(document).unwrap();
                assert_eq!(projection.units().len(), 1);
                assert_eq!(projection.areas().unwrap().len(), 1);
                assert_eq!(projection.variables().unwrap().len(), 0);
                assert_eq!(projection.events().unwrap().stored_block_count(), 1);
                assert_eq!(projection.footer().unwrap().len(), 1);

                let active_generation = frame
                    .stg_presentations
                    .get(document)
                    .unwrap()
                    .binding_generation();
                frame.select_area(Area::Home, cx);
                let hidden_view_generation = frame
                    .stg_presentations
                    .get(document)
                    .unwrap()
                    .binding_generation();
                assert!(hidden_view_generation > active_generation);
                frame.select_area(Area::Files, cx);
                let returned_view_generation = frame
                    .stg_presentations
                    .get(document)
                    .unwrap()
                    .binding_generation();
                assert!(returned_view_generation > hidden_view_generation);
                let troop = frame.workspace.open_loaded(
                    PathBuf::from("TroopInfo.sox"),
                    Document::Troop(troop_document()),
                );
                frame.activate_document(troop, cx);
                let hidden_generation = frame
                    .stg_presentations
                    .get(document)
                    .unwrap()
                    .binding_generation();
                assert!(hidden_generation > returned_view_generation);
                frame.activate_document(document, cx);
                assert!(
                    frame
                        .stg_presentations
                        .get(document)
                        .unwrap()
                        .binding_generation()
                        > hidden_generation
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn stg_route_draws_the_editor_from_the_app_frame(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let editor = frame.update(cx, |frame, cx| {
            let document = frame.workspace.open_loaded(
                PathBuf::from("mission.stg"),
                Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
            );
            frame.activate_document(document, cx);
            frame.document_editor(document, cx)
        });

        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(1180.0), px(780.0)),
            move |_, _| editor,
        );

        assert!(
            cx.debug_bounds("stg-editor").is_some(),
            "error={}",
            cx.debug_bounds("stg-editor-error").is_some(),
        );
    }

    #[gpui::test]
    fn stg_reconciliation_filters_picker_queries_and_revalidates_reference_kinds(
        cx: &mut TestAppContext,
    ) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let document = frame.workspace.open_loaded(
                    PathBuf::from("mission.stg"),
                    Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                );
                frame.activate_document(document, cx);
                let script = STGScriptTarget {
                    block: 0,
                    event: 0,
                    kind: STGScriptKind::Condition,
                    script: 0,
                };
                let parameter = STGParameterTarget {
                    script,
                    parameter: 0,
                };
                frame
                    .stg_presentations
                    .set_expanded_script(document, Some(script), None);
                frame.stg_presentations.set_reference_picker(
                    document,
                    Some(STGReferencePickerState::new(
                        parameter,
                        STGReferenceKind::Area,
                        "missing".to_owned(),
                        Some(STGReferenceCursor::Index(0)),
                    )),
                    None,
                );

                frame.reconcile_stg_presentation(document, STGDocumentTransition::ScalarEdit, cx);
                assert_eq!(
                    frame
                        .stg_presentations
                        .get(document)
                        .unwrap()
                        .reference_picker()
                        .unwrap()
                        .cursor(),
                    None
                );

                frame.stg_presentations.set_reference_picker(
                    document,
                    Some(STGReferencePickerState::new(
                        parameter,
                        STGReferenceKind::Area,
                        "forest".to_owned(),
                        Some(STGReferenceCursor::Index(0)),
                    )),
                    None,
                );
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::EditSTGStructure {
                            edit: STGStructuralEdit::ChangeScriptType {
                                target: script,
                                type_id: 10,
                            },
                        },
                    )
                    .unwrap();
                frame.reconcile_stg_presentation(
                    document,
                    STGDocumentTransition::StructuralEdit(Some(
                        STGStructuralEdit::ChangeScriptType {
                            target: script,
                            type_id: 10,
                        }
                        .change(),
                    )),
                    cx,
                );
                assert!(
                    frame
                        .stg_presentations
                        .get(document)
                        .unwrap()
                        .reference_picker()
                        .is_none()
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn stg_reconciliation_closes_reference_pickers_for_nonnumeric_values(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                for (index, kind) in [STGValueKind::Float, STGValueKind::String]
                    .into_iter()
                    .enumerate()
                {
                    let document = frame.workspace.open_loaded(
                        PathBuf::from(format!("mission-{index}.stg")),
                        Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                    );
                    frame.activate_document(document, cx);
                    let script = STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Condition,
                        script: 0,
                    };
                    let parameter = STGParameterTarget {
                        script,
                        parameter: 0,
                    };
                    frame
                        .stg_presentations
                        .set_expanded_script(document, Some(script), None);
                    frame.stg_presentations.set_reference_picker(
                        document,
                        Some(STGReferencePickerState::new(
                            parameter,
                            STGReferenceKind::Area,
                            String::new(),
                            Some(STGReferenceCursor::Index(0)),
                        )),
                        None,
                    );
                    frame
                        .workspace
                        .apply(
                            document,
                            DocumentEdit::EditSTGStructure {
                                edit: STGStructuralEdit::ChangeValueType {
                                    target: STGValueTarget::ScriptParameter(parameter),
                                    kind,
                                },
                            },
                        )
                        .unwrap();

                    frame.reconcile_stg_presentation(
                        document,
                        STGDocumentTransition::StructuralEdit(Some(
                            STGStructuralEdit::ChangeValueType {
                                target: STGValueTarget::ScriptParameter(parameter),
                                kind,
                            }
                            .change(),
                        )),
                        cx,
                    );
                    assert!(
                        frame
                            .stg_presentations
                            .get(document)
                            .unwrap()
                            .reference_picker()
                            .is_none(),
                        "{kind:?}"
                    );
                }
            })
            .unwrap();
    }

    #[gpui::test]
    fn stg_structural_history_preserves_the_selected_event_identity(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let document = frame.workspace.open_loaded(
                    PathBuf::from("mission.stg"),
                    Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                );
                for event in 1..=2 {
                    frame
                        .workspace
                        .apply(
                            document,
                            DocumentEdit::EditSTGStructure {
                                edit: STGStructuralEdit::InsertEvent {
                                    target: STGEventTarget { block: 0, event },
                                },
                            },
                        )
                        .unwrap();
                }
                frame.activate_document(document, cx);
                frame.stg_presentations.select(
                    document,
                    STGSelection::Event(Some(STGEventTarget { block: 0, event: 2 })),
                    None,
                );

                let remove = STGStructuralEdit::RemoveEvent {
                    target: STGEventTarget { block: 0, event: 0 },
                };
                apply_and_reconcile_stg_structure(frame, document, remove, cx);
                assert_eq!(
                    frame
                        .stg_presentations
                        .get(document)
                        .unwrap()
                        .inspected_event(),
                    Some(STGEventTarget { block: 0, event: 1 })
                );

                frame.move_history(false, cx);
                assert_eq!(
                    frame
                        .stg_presentations
                        .get(document)
                        .unwrap()
                        .inspected_event(),
                    Some(STGEventTarget { block: 0, event: 2 })
                );

                frame.move_history(true, cx);
                assert_eq!(
                    frame
                        .stg_presentations
                        .get(document)
                        .unwrap()
                        .inspected_event(),
                    Some(STGEventTarget { block: 0, event: 1 })
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn stg_structural_history_preserves_filtered_trigger_picker_cursor_identity(
        cx: &mut TestAppContext,
    ) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let document = frame.workspace.open_loaded(
                    PathBuf::from("mission.stg"),
                    Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                );
                let trigger_script = STGScriptTarget {
                    block: 0,
                    event: 0,
                    kind: STGScriptKind::Action,
                    script: 0,
                };
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::EditSTGStructure {
                            edit: STGStructuralEdit::InsertScript {
                                target: trigger_script,
                                type_id: 0,
                            },
                        },
                    )
                    .unwrap();
                for event in 1..=2 {
                    frame
                        .workspace
                        .apply(
                            document,
                            DocumentEdit::EditSTGStructure {
                                edit: STGStructuralEdit::InsertEvent {
                                    target: STGEventTarget { block: 0, event },
                                },
                            },
                        )
                        .unwrap();
                }
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetSTGText {
                            target: STGTextTarget::EventDescription { block: 0, event: 2 },
                            value: "Needle".to_owned(),
                        },
                    )
                    .unwrap();
                frame.activate_document(document, cx);
                frame.stg_presentations.select(
                    document,
                    STGSelection::Event(Some(STGEventTarget { block: 0, event: 0 })),
                    None,
                );
                frame
                    .stg_presentations
                    .set_expanded_script(document, Some(trigger_script), None);
                frame.stg_presentations.set_reference_picker(
                    document,
                    Some(STGReferencePickerState::new(
                        STGParameterTarget {
                            script: trigger_script,
                            parameter: 0,
                        },
                        STGReferenceKind::Trigger,
                        "needle".to_owned(),
                        Some(STGReferenceCursor::Event(STGEventTarget {
                            block: 0,
                            event: 2,
                        })),
                    )),
                    None,
                );

                let remove = STGStructuralEdit::RemoveEvent {
                    target: STGEventTarget { block: 0, event: 1 },
                };
                apply_and_reconcile_stg_structure(frame, document, remove, cx);
                assert_reference_cursor(
                    frame,
                    document,
                    STGReferenceCursor::Event(STGEventTarget { block: 0, event: 1 }),
                );

                frame.move_history(false, cx);
                assert_reference_cursor(
                    frame,
                    document,
                    STGReferenceCursor::Event(STGEventTarget { block: 0, event: 2 }),
                );

                frame.move_history(true, cx);
                assert_reference_cursor(
                    frame,
                    document,
                    STGReferenceCursor::Event(STGEventTarget { block: 0, event: 1 }),
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn stg_structural_edits_preserve_or_close_script_bindings(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let document = frame.workspace.open_loaded(
                    PathBuf::from("mission.stg"),
                    Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                );
                for _ in 0..2 {
                    frame
                        .workspace
                        .apply(
                            document,
                            DocumentEdit::EditSTGStructure {
                                edit: STGStructuralEdit::InsertScript {
                                    target: STGScriptTarget {
                                        block: 0,
                                        event: 0,
                                        kind: STGScriptKind::Condition,
                                        script: 0,
                                    },
                                    type_id: 7,
                                },
                            },
                        )
                        .unwrap();
                }
                frame.activate_document(document, cx);
                let expanded = STGScriptTarget {
                    block: 0,
                    event: 0,
                    kind: STGScriptKind::Condition,
                    script: 2,
                };
                frame
                    .stg_presentations
                    .set_expanded_script(document, Some(expanded), None);
                frame.stg_presentations.set_reference_picker(
                    document,
                    Some(STGReferencePickerState::new(
                        STGParameterTarget {
                            script: expanded,
                            parameter: 0,
                        },
                        STGReferenceKind::Area,
                        String::new(),
                        Some(STGReferenceCursor::Index(0)),
                    )),
                    None,
                );

                let remove_before = STGStructuralEdit::RemoveScript {
                    target: STGScriptTarget {
                        script: 0,
                        ..expanded
                    },
                };
                apply_and_reconcile_stg_structure(frame, document, remove_before, cx);
                let remapped = STGScriptTarget {
                    script: 1,
                    ..expanded
                };
                assert_script_binding(frame, document, remapped);

                frame.move_history(false, cx);
                assert_script_binding(frame, document, expanded);

                frame.move_history(true, cx);
                assert_script_binding(frame, document, remapped);

                let remove_selected = STGStructuralEdit::RemoveScript { target: remapped };
                apply_and_reconcile_stg_structure(frame, document, remove_selected, cx);
                let state = frame.stg_presentations.get(document).unwrap();
                assert_eq!(state.expanded_script(), None);
                assert_eq!(state.reference_picker(), None);
            })
            .unwrap();
    }

    #[gpui::test]
    fn stg_scalar_history_keeps_script_and_picker_identity(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let document = frame.workspace.open_loaded(
                    PathBuf::from("mission.stg"),
                    Document::STG(STGDocument::parse(stg_fixture()).unwrap()),
                );
                frame.activate_document(document, cx);
                let script = STGScriptTarget {
                    block: 0,
                    event: 0,
                    kind: STGScriptKind::Condition,
                    script: 0,
                };
                frame
                    .stg_presentations
                    .set_expanded_script(document, Some(script), None);
                frame.stg_presentations.set_reference_picker(
                    document,
                    Some(STGReferencePickerState::new(
                        STGParameterTarget {
                            script,
                            parameter: 0,
                        },
                        STGReferenceKind::Area,
                        String::new(),
                        Some(STGReferenceCursor::Index(0)),
                    )),
                    None,
                );
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetSTGNumber {
                            target: STGNumberTarget::EventID { block: 0, event: 0 },
                            value: 501,
                        },
                    )
                    .unwrap();
                frame.reconcile_stg_presentation(document, STGDocumentTransition::ScalarEdit, cx);

                frame.move_history(false, cx);

                assert_script_binding(frame, document, script);
            })
            .unwrap();
    }

    #[test]
    fn stg_fixture_is_a_regular_file_image() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mission.stg");
        fs::write(&path, stg_fixture()).unwrap();
        assert!(path.is_file());
    }
}
