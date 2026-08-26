use gpui::{
    AnyElement, Context, Div, ElementId, Entity, ScrollStrategy, SharedString, div, prelude::*, px,
};
use kufeditor_workspace::{
    DocumentID, STGAbilityOwner, STGAreaField, STGAreaFloatField, STGEditor, STGEventTarget,
    STGFieldAccess, STGFloatTarget, STGFloatValue, STGFooterField, STGHeaderTextField,
    STGNumberTarget, STGReferenceKind, STGScriptKind, STGScriptTarget, STGSkillField,
    STGSkillOwner, STGTailStatus, STGText, STGTextTarget, STGUnitField, STGUnitFloatField,
    STGUnitGroup, STGValue, STGValueTarget, WorkspaceError,
};

use super::{
    ActiveFloatEdit, ActiveNumberEdit, AppFrame, NumberEditTarget, STGEditBinding, TextEditTarget,
};
use crate::{
    actions::{
        MoveSTGListDown, MoveSTGListEnd, MoveSTGListHome, MoveSTGListPageDown, MoveSTGListPageUp,
        MoveSTGListUp, SetSTGChoice,
    },
    components,
    crusaders_catalog_status::CrusadersCatalogStatus,
    notices::{Notice, NoticeSource},
    state::{
        STGDocumentTransition, STGDraftBinding, STGDraftStatus, STGDraftTarget, STGIndexVisibility,
        STGPresentationTransition, STGReferenceVisibility, STGSection, STGSelection,
        STGVisibleSelections,
    },
    text_input::{TextInput, TextInputEvent},
    views::{
        save,
        stg::{
            self, STGDocumentProjection, STGEventBlockProjection, STGEventDetailField,
            STGEventDetailRow, STGEventDetailRows, STGEventRows, STGFieldProjection, STGFieldState,
            STGIndexRows, STGProjectionField, STGProjectionID, STGReferenceRows, STGRowCursor,
            STGRowLocation, STGSearchQuery, STGSearchRecord, STGSectionProjection,
            STGTailProjection, STGVirtualRowKind, STGVirtualRows,
        },
    },
};

type STGProjectionResult<T> = Result<T, Box<WorkspaceError>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum STGListMovement {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum STGSearchKind {
    Units,
    Events,
}

impl STGSearchKind {
    const fn section(self) -> STGSection {
        match self {
            Self::Units => STGSection::Units,
            Self::Events => STGSection::Events,
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::Units => "Search units by name, ID, or source index",
            Self::Events => "Search events by description, ID, or source index",
        }
    }

    const fn selector(self) -> &'static str {
        match self {
            Self::Units => "stg-unit-search",
            Self::Events => "stg-event-search",
        }
    }

    const fn input_id(self) -> &'static str {
        match self {
            Self::Units => "stg-unit-search-input",
            Self::Events => "stg-event-search-input",
        }
    }
}

pub(super) struct ActiveSTGSearch {
    document: DocumentID,
    kind: STGSearchKind,
    original_query: String,
    input: Entity<TextInput>,
}

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
    fn active_stg_draft(&self) -> Option<(STGEditBinding, STGDraftTarget)> {
        if let Some(edit) = self.number_edit.as_ref()
            && let Some((binding, target)) = edit.target.stg_binding()
        {
            return Some((binding, STGDraftTarget::Number(target)));
        }
        if let Some(edit) = self.float_edit.as_ref() {
            return Some((edit.binding, STGDraftTarget::Float(edit.target)));
        }
        self.text_edit
            .as_ref()
            .and_then(|edit| edit.target.stg_binding())
            .map(|(binding, target)| (binding, STGDraftTarget::Text(target)))
    }

    fn active_stg_draft_status(&self, visible: bool) -> Option<STGDraftStatus> {
        self.active_stg_draft().map(|(binding, target)| {
            let binding = STGDraftBinding::new(binding.document, binding.section, target);
            if visible && self.stg_draft_target_is_visible(binding.document(), binding.target()) {
                STGDraftStatus::visible(binding)
            } else {
                STGDraftStatus::hidden(binding)
            }
        })
    }

    fn finish_stg_presentation_transition(
        &mut self,
        document: DocumentID,
        transition: STGPresentationTransition,
        cx: &mut Context<Self>,
    ) {
        if transition.cancels_draft() {
            self.cancel_property_edit();
        } else if let Some(generation) = transition.generation() {
            if let Some(edit) = self.number_edit.as_mut()
                && let NumberEditTarget::STG { binding, .. } = &mut edit.target
                && binding.document == document
            {
                binding.generation = generation;
            }
            if let Some(edit) = self.float_edit.as_mut()
                && edit.binding.document == document
            {
                edit.binding.generation = generation;
            }
            if let Some(edit) = self.text_edit.as_mut()
                && let TextEditTarget::STG { binding, .. } = &mut edit.target
                && binding.document == document
            {
                binding.generation = generation;
            }
        }

        if transition.changed() {
            self.stg_lists.invalidate_all();
            cx.notify();
        }
    }

    pub(super) fn stg_scalar_binding_is_current(
        &self,
        binding: STGEditBinding,
        target: STGDraftTarget,
    ) -> bool {
        self.active_document == Some(binding.document)
            && self
                .stg_presentations
                .get(binding.document)
                .is_some_and(|state| {
                    state.section() == binding.section
                        && state.binding_generation() == binding.generation
                })
            && self.stg_draft_target_is_visible(binding.document, target)
            && match target {
                STGDraftTarget::Number(target) => {
                    self.workspace.stg_number(binding.document, target).is_ok()
                }
                STGDraftTarget::Float(target) => {
                    self.workspace.stg_float(binding.document, target).is_ok()
                }
                STGDraftTarget::Text(target) => {
                    self.workspace.stg_text(binding.document, target).is_ok()
                }
            }
    }

    fn stg_draft_target_is_visible(&self, document: DocumentID, target: STGDraftTarget) -> bool {
        let Some(state) = self
            .stg_presentations
            .get(document)
            .filter(|_| self.active_document == Some(document))
        else {
            return false;
        };

        match target {
            STGDraftTarget::Number(target) => match target {
                STGNumberTarget::Unit { unit, .. }
                | STGNumberTarget::Skill { unit, .. }
                | STGNumberTarget::Ability { unit, .. } => {
                    state.section() == STGSection::Units && state.inspected_unit() == Some(unit)
                }
                STGNumberTarget::Area { area, .. } => {
                    state.section() == STGSection::Areas && state.inspected_area() == Some(area)
                }
                STGNumberTarget::VariableID { variable }
                | STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::VariableInitial { variable },
                } => {
                    state.section() == STGSection::Variables
                        && state.inspected_variable() == Some(variable)
                }
                STGNumberTarget::EventBlockHeader { block } => {
                    state.section() == STGSection::Events
                        && state
                            .inspected_event()
                            .is_some_and(|event| event.block == block)
                }
                STGNumberTarget::EventID { block, event }
                | STGNumberTarget::ParameterInteger {
                    value:
                        STGValueTarget::ScriptParameter(kufeditor_workspace::STGParameterTarget {
                            script: STGScriptTarget { block, event, .. },
                            ..
                        }),
                } => {
                    state.section() == STGSection::Events
                        && state.inspected_event() == Some(STGEventTarget { block, event })
                }
                STGNumberTarget::Footer { entry, .. } => {
                    state.section() == STGSection::Footer && state.inspected_footer() == Some(entry)
                }
            },
            STGDraftTarget::Float(target) => match target {
                STGFloatTarget::Unit { unit, .. } | STGFloatTarget::StatOverride { unit, .. } => {
                    state.section() == STGSection::Units && state.inspected_unit() == Some(unit)
                }
                STGFloatTarget::Area { area, .. } => {
                    state.section() == STGSection::Areas && state.inspected_area() == Some(area)
                }
                STGFloatTarget::Parameter {
                    value: STGValueTarget::VariableInitial { variable },
                } => {
                    state.section() == STGSection::Variables
                        && state.inspected_variable() == Some(variable)
                }
                STGFloatTarget::Parameter {
                    value:
                        STGValueTarget::ScriptParameter(kufeditor_workspace::STGParameterTarget {
                            script: STGScriptTarget { block, event, .. },
                            ..
                        }),
                } => {
                    state.section() == STGSection::Events
                        && state.inspected_event() == Some(STGEventTarget { block, event })
                }
            },
            STGDraftTarget::Text(target) => match target {
                STGTextTarget::Header(_) => state.section() == STGSection::Header,
                STGTextTarget::UnitName { unit } => {
                    state.section() == STGSection::Units && state.inspected_unit() == Some(unit)
                }
                STGTextTarget::AreaDescription { area } => {
                    state.section() == STGSection::Areas && state.inspected_area() == Some(area)
                }
                STGTextTarget::VariableName { variable }
                | STGTextTarget::ParameterString {
                    value: STGValueTarget::VariableInitial { variable },
                } => {
                    state.section() == STGSection::Variables
                        && state.inspected_variable() == Some(variable)
                }
                STGTextTarget::EventDescription { block, event }
                | STGTextTarget::ParameterString {
                    value:
                        STGValueTarget::ScriptParameter(kufeditor_workspace::STGParameterTarget {
                            script: STGScriptTarget { block, event, .. },
                            ..
                        }),
                } => {
                    state.section() == STGSection::Events
                        && state.inspected_event() == Some(STGEventTarget { block, event })
                }
            },
        }
    }

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
        let draft = self.active_stg_draft_status(false);
        let transition =
            self.stg_presentations
                .activate_document(document, &projection.visibility(), draft);
        self.finish_stg_presentation_transition(document, transition, cx);
    }

    pub(super) fn deactivate_stg_presentation(&mut self, cx: &mut Context<Self>) {
        self.stg_search = None;
        let draft = self.active_stg_draft_status(false);
        let document = self.active_stg_draft().map(|(binding, _)| binding.document);
        let transition = self.stg_presentations.deactivate_active_document(draft);
        if let Some(document) = document {
            self.finish_stg_presentation_transition(document, transition, cx);
        } else if transition.changed() {
            self.stg_lists.invalidate_all();
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
        let draft = self.active_stg_draft_status(matches!(cause, STGDocumentTransition::Catalog));
        let transition = self.stg_presentations.reconcile_document(
            document,
            &projection.visibility(),
            &visible_scripts,
            picker_visible,
            &reference_visibility,
            cause,
            draft,
        );
        self.finish_stg_presentation_transition(document, transition, cx);
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

    pub(super) fn stg_editor(&self, document: DocumentID, cx: &mut Context<Self>) -> Div {
        let projection = match self.stg_presentation_projection(document) {
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
        let state = self
            .stg_presentations
            .get(document)
            .cloned()
            .unwrap_or_default();
        let content = self.render_stg_section(document, &state, &projection, cx);
        stg::render_editor(
            &self.theme,
            self.stg_section_rail(document, state.section(), &projection.document, cx),
            self.stg_catalog_status_element(),
            content,
        )
        .tab_group()
        .key_context("STGEditor")
    }
}

impl AppFrame {
    fn render_stg_section(
        &self,
        document: DocumentID,
        state: &crate::state::STGPresentationState,
        projection: &STGPresentationProjection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match state.section() {
            STGSection::Header => self.stg_header_view(document, cx).into_any_element(),
            STGSection::Units => self
                .stg_units_view(document, state, projection.units.clone(), cx)
                .into_any_element(),
            STGSection::Areas => projection.document.areas().map_or_else(
                || self.stg_raw_tail_view(document, STGSection::Areas, &projection.document),
                |rows| {
                    self.stg_areas_view(document, state, rows.clone(), cx)
                        .into_any_element()
                },
            ),
            STGSection::Variables => projection.document.variables().map_or_else(
                || self.stg_raw_tail_view(document, STGSection::Variables, &projection.document),
                |rows| {
                    self.stg_variables_view(document, state, rows.clone(), cx)
                        .into_any_element()
                },
            ),
            STGSection::Events => projection.document.events().map_or_else(
                || self.stg_raw_tail_view(document, STGSection::Events, &projection.document),
                |_| {
                    self.stg_events_view(document, state, projection.events.clone(), cx)
                        .into_any_element()
                },
            ),
            STGSection::Footer => projection.document.footer().map_or_else(
                || self.stg_raw_tail_view(document, STGSection::Footer, &projection.document),
                |rows| {
                    self.stg_footer_view(document, state, rows.clone(), cx)
                        .into_any_element()
                },
            ),
        }
    }

    fn stg_section_rail(
        &self,
        document: DocumentID,
        selected: STGSection,
        projection: &STGDocumentProjection,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        STGSection::ALL
            .into_iter()
            .map(|section| {
                let selector = format!("stg-section-{}", section.label().to_ascii_lowercase());
                stg::section_rail_item(
                    &self.theme,
                    SharedString::from(selector.clone()),
                    stg_section_label(section, projection),
                    selected == section,
                )
                .debug_selector(move || selector.clone())
                .tab_index(0)
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_stg_section(document, section, cx);
                    window.focus(&frame.focus);
                }))
                .into_any_element()
            })
            .collect()
    }

    fn select_stg_section(
        &mut self,
        document: DocumentID,
        section: STGSection,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        if self
            .stg_presentations
            .get(document)
            .is_some_and(|state| state.section() != section)
        {
            self.stg_search = None;
        }
        let draft = self.active_stg_draft_status(true);
        let transition = self
            .stg_presentations
            .select_section(document, section, draft);
        self.finish_stg_presentation_transition(document, transition, cx);
    }

    fn start_stg_search(
        &mut self,
        document: DocumentID,
        kind: STGSearchKind,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.stg_presentations.get(document).filter(|state| {
            self.active_document == Some(document) && state.section() == kind.section()
        }) else {
            return;
        };
        if let Some(search) = self
            .stg_search
            .as_ref()
            .filter(|search| search.document == document && search.kind == kind)
        {
            window.focus(&search.input.read(cx).focus_handle());
            return;
        }

        let query = match kind {
            STGSearchKind::Units => state.unit_query(),
            STGSearchKind::Events => state.event_query(),
        }
        .to_owned();
        self.cancel_property_edit();
        let colors = self.text_input_colors();
        let input = cx.new(|cx| {
            TextInput::new(
                query.clone(),
                kind.placeholder(),
                kind.input_id(),
                colors,
                cx,
            )
        });
        cx.subscribe_in(&input, window, |frame, input, event, window, cx| {
            frame.handle_stg_search_event(input, event, window, cx);
        })
        .detach();
        window.focus(&input.read(cx).focus_handle());
        self.stg_search = Some(ActiveSTGSearch {
            document,
            kind,
            original_query: query,
            input,
        });
        cx.notify();
    }

    fn handle_stg_search_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &TextInputEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some((document, kind, original_query)) = self
            .stg_search
            .as_ref()
            .filter(|search| search.input == *input)
            .map(|search| (search.document, search.kind, search.original_query.clone()))
        else {
            return;
        };
        if self.active_document != Some(document) {
            self.stg_search = None;
            cx.notify();
            return;
        }

        match event {
            TextInputEvent::ContentChanged => {
                let query = input.read(cx).content().to_owned();
                self.apply_stg_search_query(document, kind, query, cx);
            }
            TextInputEvent::Cancel => {
                self.apply_stg_search_query(document, kind, original_query, cx);
                self.stg_search = None;
                window.focus(&self.focus);
                cx.notify();
            }
            TextInputEvent::Commit(query) => {
                self.apply_stg_search_query(document, kind, query.clone(), cx);
                self.stg_search = None;
                window.focus(&self.focus);
                cx.notify();
            }
        }
    }

    fn apply_stg_search_query(
        &mut self,
        document: DocumentID,
        kind: STGSearchKind,
        query: String,
        cx: &mut Context<Self>,
    ) {
        if self.active_document != Some(document) {
            return;
        }
        let projection = match self.stg_projection(document) {
            Ok(projection) => projection,
            Err(error) => {
                self.report_stg_search_error(&error, cx);
                return;
            }
        };
        let transition = match kind {
            STGSearchKind::Units => {
                let query_projection = STGSearchQuery::new(&query);
                let rows =
                    match self.stg_unit_rows(document, &query_projection, projection.units().len())
                    {
                        Ok(rows) => rows,
                        Err(error) => {
                            self.report_stg_search_error(&error, cx);
                            return;
                        }
                    };
                self.stg_presentations
                    .set_unit_query(document, query, &rows.visibility(), None)
            }
            STGSearchKind::Events => {
                let query_projection = STGSearchQuery::new(&query);
                let base = projection
                    .events()
                    .cloned()
                    .unwrap_or_else(|| STGEventRows::from_blocks(Vec::new()));
                let rows = match self.stg_event_rows(document, &query_projection, base) {
                    Ok(rows) => rows,
                    Err(error) => {
                        self.report_stg_search_error(&error, cx);
                        return;
                    }
                };
                self.stg_presentations
                    .set_event_query(document, query, &rows.visibility(), None)
            }
        };
        if transition.changed() {
            self.stg_lists.invalidate_all();
            self.notices.clear(NoticeSource::Editor);
            cx.notify();
        }
    }

    fn report_stg_search_error(&mut self, error: &WorkspaceError, cx: &mut Context<Self>) {
        self.notices.replace(
            NoticeSource::Editor,
            Notice::error("Could not search this STG", error),
        );
        cx.notify();
    }

    fn stg_catalog_status_element(&self) -> Option<AnyElement> {
        match self.crusaders_catalog.status() {
            CrusadersCatalogStatus::NotConfigured => Some(
                save::catalog_status(
                    &self.theme,
                    "stg-catalog-not-configured",
                    "Crusaders installation is not configured",
                    Some("Raw STG values remain available without game names.".to_owned()),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Dormant => Some(
                save::catalog_status(
                    &self.theme,
                    "stg-catalog-dormant",
                    "Crusaders names are unavailable",
                    Some("Raw STG values remain available.".to_owned()),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Loading { .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "stg-catalog-loading",
                    "Loading Crusaders names",
                    Some("The STG remains readable as source values.".to_owned()),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Failed { error, .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "stg-catalog-failed",
                    "Could not load Crusaders names",
                    Some(format!("{error}. Raw STG values remain available.")),
                )
                .into_any_element(),
            ),
            CrusadersCatalogStatus::Ready { issue_count: 0, .. } => None,
            CrusadersCatalogStatus::Ready { issue_count, .. } => Some(
                save::catalog_status(
                    &self.theme,
                    "stg-catalog-ready-issues",
                    format!("Loaded names with {issue_count} catalog issues"),
                    Some("Some STG rows can use raw fallback labels.".to_owned()),
                )
                .into_any_element(),
            ),
        }
    }

    fn stg_raw_tail_view(
        &self,
        document: DocumentID,
        section: STGSection,
        projection: &STGDocumentProjection,
    ) -> AnyElement {
        let STGTailProjection::Raw {
            bytes,
            region,
            offset,
        } = projection.tail()
        else {
            return stg::empty_state(
                &self.theme,
                "stg-tail-projection-error",
                "This parsed STG section could not be projected.",
            )
            .into_any_element();
        };
        let section =
            STGSectionProjection::from_raw_tail(document, section, *bytes, region.clone(), *offset);
        let Some(raw) = section.raw_tail() else {
            return stg::empty_state(
                &self.theme,
                "stg-tail-projection-error",
                "This raw STG section could not be projected.",
            )
            .into_any_element();
        };
        stg::scrolling_section(
            &self.theme,
            stg_section_id(section.section()),
            section.section().label(),
            "Opaque source data",
            vec![stg::raw_tail_panel(&self.theme, raw).into_any_element()],
        )
        .into_any_element()
    }

    fn stg_text_field(
        &self,
        document: DocumentID,
        section: STGSection,
        target: STGTextTarget,
    ) -> STGFieldProjection {
        match self.workspace.stg_text(document, target) {
            Ok(value) => STGFieldProjection::text(document, section, target, value),
            Err(error) => STGFieldProjection::error(
                document,
                section,
                STGProjectionField::Text(target),
                target.label(),
                error.to_string(),
            ),
        }
    }

    fn stg_number_field(
        &self,
        document: DocumentID,
        section: STGSection,
        target: STGNumberTarget,
    ) -> STGFieldProjection {
        match self.workspace.stg_number(document, target) {
            Ok(value) => {
                STGFieldProjection::number(document, section, target, value, target.editor())
            }
            Err(error) => STGFieldProjection::error(
                document,
                section,
                STGProjectionField::Number(target),
                target.label(),
                error.to_string(),
            ),
        }
    }

    fn stg_float_field(
        &self,
        document: DocumentID,
        section: STGSection,
        target: STGFloatTarget,
    ) -> STGFieldProjection {
        match self.workspace.stg_float(document, target) {
            Ok(value) => STGFieldProjection::float(document, section, target, value.to_bits()),
            Err(error) => STGFieldProjection::error(
                document,
                section,
                STGProjectionField::Float(target),
                target.label(),
                error.to_string(),
            ),
        }
    }

    fn stg_value_field(
        &self,
        document: DocumentID,
        section: STGSection,
        target: STGValueTarget,
        label: impl Into<String>,
    ) -> STGFieldProjection {
        let label = label.into();
        match self.workspace.stg_value(document, target) {
            Ok(STGValue::Integer(value)) => STGFieldProjection::value(
                document,
                section,
                target,
                label,
                format!("Integer · {value}"),
                STGFieldState::Value,
            ),
            Ok(STGValue::Enum(value)) => STGFieldProjection::value(
                document,
                section,
                target,
                label,
                format!("Enum · {value}"),
                STGFieldState::Value,
            ),
            Ok(STGValue::Float(value)) => {
                let bits = value.to_bits();
                let value = f32::from_bits(bits);
                let display = if value.is_finite() {
                    format!("Float · {value}")
                } else {
                    format!("Float · {value} · bits 0x{bits:08X}")
                };
                STGFieldProjection::value(
                    document,
                    section,
                    target,
                    label,
                    display,
                    STGFieldState::Value,
                )
            }
            Ok(STGValue::String(value)) => match value {
                kufeditor_workspace::STGText::Decoded(value) => STGFieldProjection::value(
                    document,
                    section,
                    target,
                    label,
                    format!("String · {}", empty_stg_text(value.as_ref())),
                    STGFieldState::Value,
                ),
                kufeditor_workspace::STGText::Raw(bytes) => STGFieldProjection::value(
                    document,
                    section,
                    target,
                    label,
                    format!("Invalid source string · {} bytes", bytes.len()),
                    STGFieldState::InvalidText,
                ),
            },
            Err(error) => STGFieldProjection::error(
                document,
                section,
                STGProjectionField::Value(target),
                label,
                error.to_string(),
            ),
        }
    }
}

impl AppFrame {
    fn stg_header_view(&self, document: DocumentID, cx: &mut Context<Self>) -> Div {
        let fields = STGHeaderTextField::ALL
            .into_iter()
            .map(|field| {
                self.stg_field_element(
                    &self.stg_text_field(
                        document,
                        STGSection::Header,
                        STGTextTarget::Header(field),
                    ),
                    format!("stg-header-{}", header_field_slug(field)),
                    cx,
                )
            })
            .collect();
        let advanced = vec![
            self.stg_field_element(
                &STGFieldProjection::read_only(
                    document,
                    STGSection::Header,
                    STGProjectionField::Magic,
                    "Magic",
                    "1001",
                ),
                "stg-header-magic".to_owned(),
                cx,
            ),
            self.stg_field_element(
                &STGFieldProjection::read_only(
                    document,
                    STGSection::Header,
                    STGProjectionField::Reserved("header-source"),
                    "Configuration and reserved data",
                    "620-byte header block preserved from source",
                ),
                "stg-header-reserved".to_owned(),
                cx,
            ),
        ];

        stg::scrolling_section(
            &self.theme,
            "stg-header",
            "Header",
            "Map resources, cameras, settings, and source-preserved metadata",
            vec![
                stg::group(&self.theme, "MISSION HEADER", fields).into_any_element(),
                stg::group(&self.theme, "ADVANCED · READ ONLY", advanced)
                    .id("stg-header-advanced")
                    .debug_selector(|| "stg-header-advanced".to_owned())
                    .into_any_element(),
            ],
        )
    }

    fn stg_searchable_master_list(
        &self,
        document: DocumentID,
        kind: STGSearchKind,
        query: &str,
        list: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.stg_search_control(document, kind, query, cx))
            .child(div().flex_1().min_h_0().overflow_hidden().child(list))
            .into_any_element()
    }

    fn stg_search_control(
        &self,
        document: DocumentID,
        kind: STGSearchKind,
        query: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = kind.selector();
        let control = self
            .stg_search
            .as_ref()
            .filter(|search| search.document == document && search.kind == kind)
            .map_or_else(
                || {
                    let (text, color) = if query.is_empty() {
                        (kind.placeholder().to_owned(), self.theme.text_dim)
                    } else {
                        (format!("⌕ {query}"), self.theme.text)
                    };
                    div()
                        .w_full()
                        .h(px(38.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(self.theme.border)
                        .bg(self.theme.raised)
                        .text_size(px(13.0))
                        .text_color(color)
                        .child(text)
                        .into_any_element()
                },
                |search| search.input.clone().into_any_element(),
            );

        div()
            .id(selector)
            .debug_selector(move || selector.to_owned())
            .flex_none()
            .p(px(10.0))
            .bg(self.theme.surface)
            .border_b_1()
            .border_color(self.theme.border)
            .cursor_pointer()
            .on_click(cx.listener(move |frame, _, window, cx| {
                frame.start_stg_search(document, kind, window, cx);
            }))
            .child(control)
            .into_any_element()
    }

    fn stg_units_view(
        &self,
        document: DocumentID,
        state: &crate::state::STGPresentationState,
        rows: STGIndexRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let count = rows.len();
        let rows = STGVirtualRows::units(document, rows);
        let list = if rows.is_empty() {
            stg::empty_state(
                &self.theme,
                "stg-unit-empty",
                "This STG has no unit records.",
            )
            .size_full()
            .into_any_element()
        } else {
            self.stg_master_list(
                document,
                rows,
                state.inspected_unit().map(STGRowCursor::Unit),
                cx,
            )
        };
        let list = self.stg_searchable_master_list(
            document,
            STGSearchKind::Units,
            state.unit_query(),
            list,
            cx,
        );
        let details = state.inspected_unit().map_or_else(
            || {
                stg::empty_state(
                    &self.theme,
                    "stg-unit-detail-empty",
                    "Select a unit to inspect its source fields.",
                )
                .size_full()
                .into_any_element()
            },
            |unit| self.stg_unit_details(document, unit, cx),
        );
        stg::split_section(
            &self.theme,
            "stg-units",
            "Units",
            format!("{count} visible unit records"),
            list,
            details,
        )
    }

    fn stg_unit_details(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut groups = vec![self.stg_unit_identity_group(document, unit, cx)];
        groups.push(self.stg_unit_float_group(document, unit, cx));
        groups.extend(self.stg_unit_number_groups(document, unit, cx));
        groups.push(self.stg_unit_stat_override_group(document, unit, cx));
        groups.extend(self.stg_unit_skill_groups(document, unit, cx));
        groups.extend(self.stg_unit_ability_groups(document, unit, cx));

        stg::scrolling_details(
            &self.theme,
            SharedString::from(format!("stg-unit-detail:{unit}")),
            groups,
        )
        .debug_selector(|| "stg-unit-detail".to_owned())
        .into_any_element()
    }

    fn stg_unit_identity_group(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        stg::group(
            &self.theme,
            "PRIMARY UNIT",
            vec![
                self.stg_field_element(
                    &self.stg_text_field(
                        document,
                        STGSection::Units,
                        STGTextTarget::UnitName { unit },
                    ),
                    format!("stg-unit-{unit}-name"),
                    cx,
                ),
                self.stg_field_element(
                    &self.stg_number_field(
                        document,
                        STGSection::Units,
                        STGNumberTarget::Unit {
                            unit,
                            field: STGUnitField::UniqueID,
                        },
                    ),
                    format!("stg-unit-{unit}-field-uniqueid"),
                    cx,
                ),
                self.stg_field_element(
                    &self.stg_number_field(
                        document,
                        STGSection::Units,
                        STGNumberTarget::Unit {
                            unit,
                            field: STGUnitField::UCD,
                        },
                    ),
                    format!("stg-unit-{unit}-field-ucd"),
                    cx,
                ),
                self.stg_field_element(
                    &self.stg_number_field(
                        document,
                        STGSection::Units,
                        STGNumberTarget::Unit {
                            unit,
                            field: STGUnitField::LeaderLevel,
                        },
                    ),
                    format!("stg-unit-{unit}-field-leaderlevel"),
                    cx,
                ),
                self.stg_field_element(
                    &self.stg_float_field(
                        document,
                        STGSection::Units,
                        STGFloatTarget::Unit {
                            unit,
                            field: STGUnitFloatField::LeaderHPOverride,
                        },
                    ),
                    format!("stg-unit-{unit}-float-LeaderHPOverride"),
                    cx,
                ),
                self.stg_field_element(
                    &self.stg_float_field(
                        document,
                        STGSection::Units,
                        STGFloatTarget::StatOverride { unit, slot: 0 },
                    ),
                    format!("stg-unit-{unit}-stat-0"),
                    cx,
                ),
            ],
        )
        .into_any_element()
    }

    fn stg_unit_number_groups(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut groups = Vec::new();
        for group in STGUnitGroup::ALL {
            let fields = STGUnitField::ALL
                .into_iter()
                .filter(|field| {
                    field.group() == group
                        && !matches!(
                            field,
                            STGUnitField::UniqueID | STGUnitField::UCD | STGUnitField::LeaderLevel
                        )
                })
                .map(|field| {
                    self.stg_field_element(
                        &self.stg_number_field(
                            document,
                            STGSection::Units,
                            STGNumberTarget::Unit { unit, field },
                        ),
                        format!("stg-unit-{unit}-field-{}", unit_field_slug(field)),
                        cx,
                    )
                })
                .collect();
            groups.push(stg::group(&self.theme, group.label(), fields).into_any_element());
        }
        groups
    }

    fn stg_unit_float_group(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        stg::group(
            &self.theme,
            "POSITION AND HP",
            STGUnitFloatField::ALL
                .into_iter()
                .filter(|field| *field != STGUnitFloatField::LeaderHPOverride)
                .map(|field| {
                    self.stg_field_element(
                        &self.stg_float_field(
                            document,
                            STGSection::Units,
                            STGFloatTarget::Unit { unit, field },
                        ),
                        format!("stg-unit-{unit}-float-{field:?}"),
                        cx,
                    )
                })
                .collect(),
        )
        .into_any_element()
    }

    fn stg_unit_skill_groups(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut groups = Vec::new();
        for owner in STGSkillOwner::ALL {
            let mut fields = Vec::new();
            for slot in 0..4 {
                for field in STGSkillField::ALL {
                    fields.push(self.stg_field_element(
                        &self.stg_number_field(
                            document,
                            STGSection::Units,
                            STGNumberTarget::Skill {
                                unit,
                                owner,
                                slot,
                                field,
                            },
                        ),
                        format!("stg-unit-{unit}-skill-{owner:?}-{slot}-{field:?}"),
                        cx,
                    ));
                }
            }
            groups.push(
                stg::group(
                    &self.theme,
                    format!("{} SKILLS · 4 SLOTS", owner.label()),
                    fields,
                )
                .into_any_element(),
            );
        }
        groups
    }

    fn stg_unit_ability_groups(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut groups = Vec::new();
        for owner in STGAbilityOwner::ALL {
            let count = match owner {
                STGAbilityOwner::Leader | STGAbilityOwner::Officer1 => 23,
                STGAbilityOwner::Officer2 => 19,
            };
            groups.push(
                stg::group(
                    &self.theme,
                    format!("{} ABILITIES · {count} SLOTS", owner.label()),
                    (0..count)
                        .map(|slot| {
                            self.stg_field_element(
                                &self.stg_number_field(
                                    document,
                                    STGSection::Units,
                                    STGNumberTarget::Ability { unit, owner, slot },
                                ),
                                format!("stg-unit-{unit}-ability-{owner:?}-{slot}"),
                                cx,
                            )
                        })
                        .collect(),
                )
                .into_any_element(),
            );
        }
        groups
    }

    fn stg_unit_stat_override_group(
        &self,
        document: DocumentID,
        unit: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        stg::group(
            &self.theme,
            "STAT OVERRIDES · 22 SLOTS",
            (1..22)
                .map(|slot| {
                    self.stg_field_element(
                        &self.stg_float_field(
                            document,
                            STGSection::Units,
                            STGFloatTarget::StatOverride { unit, slot },
                        ),
                        format!("stg-unit-{unit}-stat-{slot}"),
                        cx,
                    )
                })
                .collect(),
        )
        .into_any_element()
    }

    fn stg_areas_view(
        &self,
        document: DocumentID,
        state: &crate::state::STGPresentationState,
        rows: STGIndexRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let count = rows.len();
        let rows = STGVirtualRows::areas(document, rows);
        let list = if rows.is_empty() {
            stg::empty_state(&self.theme, "stg-area-empty", "This STG has no areas.")
                .size_full()
                .into_any_element()
        } else {
            self.stg_master_list(
                document,
                rows,
                state.inspected_area().map(STGRowCursor::Area),
                cx,
            )
        };
        let details = state.inspected_area().map_or_else(
            || {
                stg::empty_state(
                    &self.theme,
                    "stg-area-detail-empty",
                    "Select an area to inspect its source fields.",
                )
                .size_full()
                .into_any_element()
            },
            |area| self.stg_area_details(document, area, cx),
        );
        stg::split_section(
            &self.theme,
            "stg-areas",
            "Areas",
            format!("{count} area records"),
            list,
            details,
        )
    }

    fn stg_area_details(
        &self,
        document: DocumentID,
        area: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let identity = vec![
            self.stg_field_element(
                &self.stg_text_field(
                    document,
                    STGSection::Areas,
                    STGTextTarget::AreaDescription { area },
                ),
                format!("stg-area-{area}-description"),
                cx,
            ),
            self.stg_field_element(
                &self.stg_number_field(
                    document,
                    STGSection::Areas,
                    STGNumberTarget::Area {
                        area,
                        field: STGAreaField::AreaID,
                    },
                ),
                format!("stg-area-{area}-id"),
                cx,
            ),
        ];
        let bounds = STGAreaFloatField::ALL
            .into_iter()
            .map(|field| {
                self.stg_field_element(
                    &self.stg_float_field(
                        document,
                        STGSection::Areas,
                        STGFloatTarget::Area { area, field },
                    ),
                    format!("stg-area-{area}-bound-{field:?}"),
                    cx,
                )
            })
            .collect();
        let advanced = [STGAreaField::Unknown20, STGAreaField::Unknown24]
            .into_iter()
            .map(|field| {
                self.stg_field_element(
                    &self.stg_number_field(
                        document,
                        STGSection::Areas,
                        STGNumberTarget::Area { area, field },
                    ),
                    format!("stg-area-{area}-advanced-{field:?}"),
                    cx,
                )
            })
            .collect();
        stg::scrolling_details(
            &self.theme,
            SharedString::from(format!("stg-area-detail:{area}")),
            vec![
                stg::group(&self.theme, "AREA", identity).into_any_element(),
                stg::group(&self.theme, "BOUNDS", bounds).into_any_element(),
                stg::group(&self.theme, "ADVANCED · READ ONLY", advanced).into_any_element(),
            ],
        )
        .debug_selector(|| "stg-area-detail".to_owned())
        .into_any_element()
    }

    fn stg_variables_view(
        &self,
        document: DocumentID,
        state: &crate::state::STGPresentationState,
        rows: STGIndexRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let count = rows.len();
        let rows = STGVirtualRows::variables(document, rows);
        let list = if rows.is_empty() {
            stg::empty_state(
                &self.theme,
                "stg-variable-empty",
                "This STG has no variables.",
            )
            .size_full()
            .into_any_element()
        } else {
            self.stg_master_list(
                document,
                rows,
                state.inspected_variable().map(STGRowCursor::Variable),
                cx,
            )
        };
        let details = state.inspected_variable().map_or_else(
            || {
                stg::empty_state(
                    &self.theme,
                    "stg-variable-detail-empty",
                    "Select a variable to inspect its typed source value.",
                )
                .size_full()
                .into_any_element()
            },
            |variable| self.stg_variable_details(document, variable, cx),
        );
        stg::split_section(
            &self.theme,
            "stg-variables",
            "Variables",
            format!("{count} typed variables"),
            list,
            details,
        )
    }

    fn stg_variable_details(
        &self,
        document: DocumentID,
        variable: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = STGValueTarget::VariableInitial { variable };
        stg::scrolling_details(
            &self.theme,
            SharedString::from(format!("stg-variable-detail:{variable}")),
            vec![
                stg::group(
                    &self.theme,
                    "VARIABLE",
                    vec![
                        self.stg_field_element(
                            &self.stg_text_field(
                                document,
                                STGSection::Variables,
                                STGTextTarget::VariableName { variable },
                            ),
                            format!("stg-variable-{variable}-name"),
                            cx,
                        ),
                        self.stg_field_element(
                            &self.stg_number_field(
                                document,
                                STGSection::Variables,
                                STGNumberTarget::VariableID { variable },
                            ),
                            format!("stg-variable-{variable}-id"),
                            cx,
                        ),
                        self.stg_field_element(
                            &self.stg_value_field(
                                document,
                                STGSection::Variables,
                                value,
                                "Initial typed value",
                            ),
                            format!("stg-variable-{variable}-value"),
                            cx,
                        ),
                    ],
                )
                .into_any_element(),
            ],
        )
        .debug_selector(|| "stg-variable-detail".to_owned())
        .into_any_element()
    }

    fn stg_footer_view(
        &self,
        document: DocumentID,
        state: &crate::state::STGPresentationState,
        rows: STGIndexRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let count = rows.len();
        let rows = STGVirtualRows::footer(document, rows);
        let list = if rows.is_empty() {
            stg::empty_state(
                &self.theme,
                "stg-footer-empty",
                "This STG has no footer entries.",
            )
            .size_full()
            .into_any_element()
        } else {
            self.stg_master_list(
                document,
                rows,
                state.inspected_footer().map(STGRowCursor::Footer),
                cx,
            )
        };
        let details = state.inspected_footer().map_or_else(
            || {
                stg::empty_state(
                    &self.theme,
                    "stg-footer-detail-empty",
                    "Select a footer entry to inspect its slot data.",
                )
                .size_full()
                .into_any_element()
            },
            |entry| self.stg_footer_details(document, entry, cx),
        );
        stg::split_section(
            &self.theme,
            "stg-footer",
            "Footer",
            format!("{count} source-preserved entries"),
            list,
            details,
        )
    }

    fn stg_footer_details(
        &self,
        document: DocumentID,
        entry: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        stg::scrolling_details(
            &self.theme,
            SharedString::from(format!("stg-footer-detail:{entry}")),
            vec![
                stg::group(
                    &self.theme,
                    "SLOT DATA",
                    STGFooterField::ALL
                        .into_iter()
                        .map(|field| {
                            self.stg_field_element(
                                &self.stg_number_field(
                                    document,
                                    STGSection::Footer,
                                    STGNumberTarget::Footer { entry, field },
                                ),
                                format!("stg-footer-{entry}-{field:?}"),
                                cx,
                            )
                        })
                        .collect(),
                )
                .into_any_element(),
            ],
        )
        .debug_selector(|| "stg-footer-detail".to_owned())
        .into_any_element()
    }

    fn current_stg_edit_binding(
        &self,
        document: DocumentID,
        section: STGSection,
    ) -> Option<STGEditBinding> {
        let state = self.stg_presentations.get(document)?;
        (self.active_document == Some(document) && state.section() == section)
            .then(|| STGEditBinding::new(document, section, state.binding_generation()))
    }

    fn start_stg_number_edit(
        &mut self,
        projection: STGProjectionID,
        target: STGNumberTarget,
        source: i64,
        editor: STGEditor,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let document = projection.document();
        let section = projection.section();
        let Some(binding) = self.current_stg_edit_binding(document, section) else {
            return;
        };
        if !self.stg_scalar_binding_is_current(binding, STGDraftTarget::Number(target))
            || self.workspace.stg_number(document, target).ok() != Some(source)
        {
            return;
        }
        let Some(edit) = ActiveNumberEdit::stg(binding, target, source, editor) else {
            return;
        };
        self.begin_number_edit(edit);
        window.focus(&self.focus);
        cx.notify();
    }

    fn start_stg_float_edit(
        &mut self,
        projection: STGProjectionID,
        target: STGFloatTarget,
        source: STGFloatValue,
        replacement: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let document = projection.document();
        let section = projection.section();
        let Some(binding) = self.current_stg_edit_binding(document, section) else {
            return;
        };
        if target.access() == STGFieldAccess::ReadOnly
            || !self.stg_scalar_binding_is_current(binding, STGDraftTarget::Float(target))
            || self.workspace.stg_float(document, target).ok() != Some(source)
            || (source.finite_value().is_none() && !replacement)
        {
            return;
        }
        self.cancel_property_edit();
        self.float_edit = Some(ActiveFloatEdit {
            binding,
            target,
            editor: if replacement {
                crate::float_edit::FloatEdit::replacement(source)
            } else {
                crate::float_edit::FloatEdit::new(source)
            },
        });
        window.focus(&self.focus);
        cx.notify();
    }

    fn start_stg_text_edit(
        &mut self,
        document: DocumentID,
        section: STGSection,
        target: STGTextTarget,
        replacement: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(binding) = self.current_stg_edit_binding(document, section) else {
            return;
        };
        if !self.stg_scalar_binding_is_current(binding, STGDraftTarget::Text(target)) {
            return;
        }
        let Ok(source) = self.workspace.stg_text(document, target) else {
            return;
        };
        let value = match source {
            STGText::Decoded(value) if !replacement => value.as_ref().to_owned(),
            STGText::Raw(_) if replacement => String::new(),
            STGText::Decoded(_) | STGText::Raw(_) => return,
        };
        self.start_text_edit(TextEditTarget::stg(binding, target), value, window, cx);
    }

    pub(super) fn set_stg_choice(
        &mut self,
        action: &SetSTGChoice,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let binding = STGEditBinding::new(action.document, action.section, action.generation);
        let valid_choice = matches!(
            action.target.editor(),
            Some(STGEditor::Choice { choices })
                if choices.iter().any(|choice| choice.value == action.value)
        );
        if !valid_choice
            || !self.stg_scalar_binding_is_current(binding, STGDraftTarget::Number(action.target))
        {
            self.cancel_property_edit();
            self.notices.replace(
                NoticeSource::Workspace,
                Notice::info("The STG field changed; edit canceled"),
            );
            window.focus(&self.focus);
            cx.notify();
            return;
        }

        self.cancel_property_edit();
        match self.workspace.apply(
            action.document,
            kufeditor_workspace::DocumentEdit::SetSTGNumber {
                target: action.target,
                value: action.value,
            },
        ) {
            Ok(outcome) => {
                if outcome == kufeditor_workspace::ApplyOutcome::Changed {
                    self.document_did_mutate(action.document, cx);
                }
                self.notices.clear(NoticeSource::Editor);
            }
            Err(error) => self.notices.replace(
                NoticeSource::Editor,
                Notice::editor_error("Could not update Crusaders STG", &error),
            ),
        }
        window.focus(&self.focus);
        cx.notify();
    }

    fn stg_field_element(
        &self,
        field: &STGFieldProjection,
        selector: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if field.state() == STGFieldState::Error {
            return stg::field_row(&self.theme, field)
                .debug_selector(move || selector.clone())
                .into_any_element();
        }

        match field.id().field_kind() {
            STGProjectionField::Number(target) => {
                let Ok(source) = self.workspace.stg_number(field.id().document(), target) else {
                    return self.stg_read_only_field_element(field, selector);
                };
                let Some(editor) = target.editor() else {
                    return self.stg_read_only_field_element(field, selector);
                };
                self.stg_number_field_element(field, selector, target, source, editor, cx)
            }
            STGProjectionField::Float(target) => {
                let Ok(source) = self.workspace.stg_float(field.id().document(), target) else {
                    return self.stg_read_only_field_element(field, selector);
                };
                if target.access() == STGFieldAccess::ReadOnly {
                    self.stg_read_only_field_element(field, selector)
                } else {
                    self.stg_float_field_element(field, selector, target, source, cx)
                }
            }
            STGProjectionField::Text(target) => {
                let Ok(source) = self.workspace.stg_text(field.id().document(), target) else {
                    return self.stg_read_only_field_element(field, selector);
                };
                let source_is_decoded = matches!(source, STGText::Decoded(_));
                self.stg_text_field_element(field, selector, target, source_is_decoded, cx)
            }
            STGProjectionField::Value(target) => {
                self.stg_value_field_element(field, selector, target, cx)
            }
            STGProjectionField::Row
            | STGProjectionField::EventDetail(_)
            | STGProjectionField::Magic
            | STGProjectionField::Reserved(_) => self.stg_read_only_field_element(field, selector),
        }
    }

    fn stg_read_only_field_element(
        &self,
        field: &STGFieldProjection,
        selector: String,
    ) -> AnyElement {
        stg::field_row(&self.theme, field)
            .debug_selector(move || selector.clone())
            .into_any_element()
    }

    fn stg_number_field_element(
        &self,
        field: &STGFieldProjection,
        selector: String,
        target: STGNumberTarget,
        source: i64,
        editor: STGEditor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = field.id().document();
        let projection = field.id();
        match editor {
            STGEditor::Number { .. } => {
                let active = self.number_edit.as_ref().filter(|edit| {
                    matches!(
                        edit.target,
                        NumberEditTarget::STG {
                            binding,
                            target: active_target,
                        } if binding.document == document && active_target == target
                    )
                });
                let display = active.map_or_else(
                    || field.display_value().to_owned(),
                    |edit| edit.editor.draft().to_owned(),
                );
                stg::editable_value_row(
                    &self.theme,
                    SharedString::from(field.id().element_key("stg-number")),
                    field.label().to_owned(),
                    display,
                    active.is_some(),
                    active.is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
                )
                .debug_selector(move || selector.clone())
                .tab_index(0)
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.start_stg_number_edit(projection, target, source, editor, window, cx);
                }))
                .into_any_element()
            }
            STGEditor::Choice { choices } => {
                let generation = self
                    .stg_presentations
                    .get(document)
                    .map_or(0, crate::state::STGPresentationState::binding_generation);
                let known = choices.iter().any(|choice| choice.value == source);
                let buttons = choices
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, choice)| {
                        let action = SetSTGChoice {
                            document,
                            section: projection.section(),
                            generation,
                            target,
                            value: choice.value,
                        };
                        let choice_selector = format!("{selector}-choice-{}", choice.value);
                        components::choice_button(
                            &self.theme,
                            SharedString::from(
                                field.id().element_key(&format!("stg-choice-{index}")),
                            ),
                            choice.label,
                            choice.value == source,
                        )
                        .debug_selector(move || choice_selector.clone())
                        .tab_index(0)
                        .on_click(cx.listener(move |_, _, window, cx| {
                            window.dispatch_action(Box::new(action), cx);
                        }))
                        .into_any_element()
                    })
                    .collect();
                let unknown_selector = (!known).then(|| format!("{selector}-unknown"));
                stg::choice_value_row(
                    &self.theme,
                    SharedString::from(field.id().element_key("stg-choice")),
                    field.label().to_owned(),
                    field.display_value().to_owned(),
                    unknown_selector,
                    buttons,
                )
                .debug_selector(move || selector.clone())
                .into_any_element()
            }
        }
    }

    fn stg_float_field_element(
        &self,
        field: &STGFieldProjection,
        selector: String,
        target: STGFloatTarget,
        source: STGFloatValue,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = field.id().document();
        let projection = field.id();
        if source.finite_value().is_none() {
            let replace_selector = format!("{selector}-replace");
            let replace = components::choice_button(
                &self.theme,
                SharedString::from(field.id().element_key("stg-float-replace")),
                "Replace",
                false,
            )
            .debug_selector(move || replace_selector.clone())
            .tab_index(0)
            .on_click(cx.listener(move |frame, _, window, cx| {
                frame.start_stg_float_edit(projection, target, source, true, window, cx);
            }))
            .into_any_element();
            return stg::choice_value_row(
                &self.theme,
                SharedString::from(field.id().element_key("stg-float-nonfinite")),
                field.label().to_owned(),
                field.display_value().to_owned(),
                None,
                vec![replace],
            )
            .debug_selector(move || selector.clone())
            .into_any_element();
        }

        let active = self
            .float_edit
            .as_ref()
            .filter(|edit| edit.binding.document == document && edit.target == target);
        let display = active.map_or_else(
            || field.display_value().to_owned(),
            |edit| edit.editor.draft().to_owned(),
        );
        stg::editable_value_row(
            &self.theme,
            SharedString::from(field.id().element_key("stg-float")),
            field.label().to_owned(),
            display,
            active.is_some(),
            active.is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
        )
        .debug_selector(move || selector.clone())
        .tab_index(0)
        .on_click(cx.listener(move |frame, _, window, cx| {
            frame.start_stg_float_edit(projection, target, source, false, window, cx);
        }))
        .into_any_element()
    }

    fn stg_text_field_element(
        &self,
        field: &STGFieldProjection,
        selector: String,
        target: STGTextTarget,
        source_is_decoded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = field.id().document();
        let section = field.id().section();
        let active = self.text_edit.as_ref().filter(|edit| {
            matches!(
                edit.target,
                TextEditTarget::STG {
                    binding,
                    target: active_target,
                } if binding.document == document && active_target == target
            )
        });
        if let Some(active) = active {
            return stg::text_editor_row(
                &self.theme,
                SharedString::from(field.id().element_key("stg-text-editor")),
                field.label().to_owned(),
                active.input.clone().into_any_element(),
                active.validation_error.clone(),
            )
            .debug_selector(move || selector.clone())
            .into_any_element();
        }

        if !source_is_decoded {
            let replace_selector = format!("{selector}-replace");
            let replace = components::choice_button(
                &self.theme,
                SharedString::from(field.id().element_key("stg-text-replace")),
                "Replace",
                false,
            )
            .debug_selector(move || replace_selector.clone())
            .tab_index(0)
            .on_click(cx.listener(move |frame, _, window, cx| {
                frame.start_stg_text_edit(document, section, target, true, window, cx);
            }))
            .into_any_element();
            return stg::choice_value_row(
                &self.theme,
                SharedString::from(field.id().element_key("stg-text-invalid")),
                field.label().to_owned(),
                field.display_value().to_owned(),
                None,
                vec![replace],
            )
            .debug_selector(move || selector.clone())
            .into_any_element();
        }

        stg::editable_value_row(
            &self.theme,
            SharedString::from(field.id().element_key("stg-text")),
            field.label().to_owned(),
            field.display_value().to_owned(),
            false,
            false,
        )
        .debug_selector(move || selector.clone())
        .tab_index(0)
        .on_click(cx.listener(move |frame, _, window, cx| {
            frame.start_stg_text_edit(document, section, target, false, window, cx);
        }))
        .into_any_element()
    }

    fn stg_value_field_element(
        &self,
        field: &STGFieldProjection,
        selector: String,
        target: STGValueTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let document = field.id().document();
        match self.workspace.stg_value(document, target) {
            Ok(STGValue::Integer(value) | STGValue::Enum(value)) => {
                let target = STGNumberTarget::ParameterInteger { value: target };
                let Some(editor) = target.editor() else {
                    return self.stg_read_only_field_element(field, selector);
                };
                self.stg_number_field_element(field, selector, target, i64::from(value), editor, cx)
            }
            Ok(STGValue::Float(source)) => self.stg_float_field_element(
                field,
                selector,
                STGFloatTarget::Parameter { value: target },
                source,
                cx,
            ),
            Ok(STGValue::String(STGText::Decoded(_))) => self.stg_text_field_element(
                field,
                selector,
                STGTextTarget::ParameterString { value: target },
                true,
                cx,
            ),
            Ok(STGValue::String(STGText::Raw(_))) => self.stg_text_field_element(
                field,
                selector,
                STGTextTarget::ParameterString { value: target },
                false,
                cx,
            ),
            Err(_) => self.stg_read_only_field_element(field, selector),
        }
    }
}

impl AppFrame {
    fn stg_events_view(
        &self,
        document: DocumentID,
        state: &crate::state::STGPresentationState,
        rows: STGEventRows,
        cx: &mut Context<Self>,
    ) -> Div {
        let count = rows.len();
        let rows = STGVirtualRows::events(document, rows);
        let list = if rows.is_empty() {
            stg::empty_state(
                &self.theme,
                "stg-event-empty",
                "This STG has no events. Parsed event blocks remain source-preserved.",
            )
            .size_full()
            .into_any_element()
        } else {
            self.stg_master_list(
                document,
                rows,
                state.inspected_event().map(STGRowCursor::Event),
                cx,
            )
        };
        let list = self.stg_searchable_master_list(
            document,
            STGSearchKind::Events,
            state.event_query(),
            list,
            cx,
        );
        let details = state.inspected_event().map_or_else(
            || {
                stg::empty_state(
                    &self.theme,
                    "stg-event-detail-empty",
                    "Select an event to inspect its conditions, actions, and parameters.",
                )
                .size_full()
                .into_any_element()
            },
            |event| self.stg_event_details(document, event, cx),
        );
        stg::split_section(
            &self.theme,
            "stg-events",
            "Events",
            format!("{count} events across source blocks"),
            list,
            details,
        )
    }

    fn stg_event_details(
        &self,
        document: DocumentID,
        event: STGEventTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = match self.stg_event_detail_rows(document, event) {
            Ok(Some(rows)) => STGVirtualRows::event_details(document, event, rows),
            Ok(None) => {
                return stg::empty_state(
                    &self.theme,
                    "stg-event-detail-overflow",
                    "This event has too many flattened detail rows to display safely.",
                )
                .size_full()
                .into_any_element();
            }
            Err(error) => {
                return stg::empty_state(
                    &self.theme,
                    "stg-event-detail-error",
                    format!("Could not read this event: {error}"),
                )
                .size_full()
                .into_any_element();
            }
        };
        let generation = self
            .stg_presentations
            .get(document)
            .map_or(0, crate::state::STGPresentationState::binding_generation);
        let render_rows = rows.clone();
        self.stg_virtual_list(
            SharedString::from(format!("stg-event-detail-list:{event:?}")),
            document,
            rows,
            None,
            cx.processor(move |frame, location, _, cx| {
                frame.stg_virtual_event_detail_row(document, generation, &render_rows, location, cx)
            }),
            cx,
        )
    }

    fn stg_event_detail_rows(
        &self,
        document: DocumentID,
        event: STGEventTarget,
    ) -> STGProjectionResult<Option<STGEventDetailRows>> {
        let projection = self.workspace.stg_event(document, event)?;
        let mut conditions = Vec::with_capacity(projection.condition_count);
        for script in 0..projection.condition_count {
            conditions.push(
                self.workspace
                    .stg_script(
                        document,
                        STGScriptTarget {
                            block: event.block,
                            event: event.event,
                            kind: STGScriptKind::Condition,
                            script,
                        },
                    )?
                    .parameter_count,
            );
        }
        let mut actions = Vec::with_capacity(projection.action_count);
        for script in 0..projection.action_count {
            actions.push(
                self.workspace
                    .stg_script(
                        document,
                        STGScriptTarget {
                            block: event.block,
                            event: event.event,
                            kind: STGScriptKind::Action,
                            script,
                        },
                    )?
                    .parameter_count,
            );
        }
        Ok(STGEventDetailRows::from_parameter_counts(
            event,
            &conditions,
            &actions,
        ))
    }

    fn stg_master_list(
        &self,
        document: DocumentID,
        rows: STGVirtualRows,
        selected: Option<STGRowCursor>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let generation = self
            .stg_presentations
            .get(document)
            .map_or(0, crate::state::STGPresentationState::binding_generation);
        let kind = rows.kind();
        self.stg_virtual_list(
            SharedString::from(format!("stg-master-list:{document:?}:{kind:?}")),
            document,
            rows,
            selected,
            cx.processor(move |frame, location, _, cx| {
                frame.stg_virtual_master_row(document, generation, location, cx)
            }),
            cx,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "a virtual root binds its typed rows, cursor, renderer, and GPUI context"
    )]
    fn stg_virtual_list<R>(
        &self,
        id: impl Into<ElementId>,
        document: DocumentID,
        rows: STGVirtualRows,
        preferred: Option<STGRowCursor>,
        render: impl 'static + Fn(STGRowLocation, &mut gpui::Window, &mut gpui::App) -> R,
        cx: &mut Context<Self>,
    ) -> AnyElement
    where
        R: IntoElement,
    {
        let kind = rows.kind();
        let generation = self
            .stg_presentations
            .get(document)
            .map_or(0, crate::state::STGPresentationState::binding_generation);
        let control = self.stg_lists.get(kind);
        let cursor = preferred
            .filter(|cursor| rows.position_of(*cursor).is_some())
            .or_else(|| {
                control.binding.get().and_then(|binding| {
                    (binding.document == document && binding.generation == generation)
                        .then_some(binding.cursor)
                        .filter(|cursor| rows.position_of(*cursor).is_some())
                })
            })
            .or_else(|| rows.cursor(0));
        let Some(cursor) = cursor else {
            control.binding.set(None);
            return stg::empty_state(
                &self.theme,
                "stg-virtual-list-empty",
                "This STG section has no rows.",
            )
            .size_full()
            .into_any_element();
        };
        let position = rows.position_of(cursor).unwrap_or(0);
        let binding = super::STGListBinding {
            document,
            cursor,
            position,
            row_count: rows.len(),
            generation,
        };
        if control.binding.get() != Some(binding) {
            control
                .scroll
                .scroll_to_item(position, ScrollStrategy::Center);
            control.binding.set(Some(binding));
        }
        let root_selector = stg_list_root_selector(kind);
        let accent = self.theme.accent;
        let surface = self.theme.surface;
        let list = stg::uniform_stg_rows(id, rows, render)
            .track_scroll(control.scroll.clone())
            .size_full();

        div()
            .id(SharedString::from(format!(
                "stg-list-root:{document:?}:{kind:?}"
            )))
            .debug_selector(move || root_selector.to_owned())
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .border_1()
            .border_color(surface)
            .track_focus(&control.focus)
            .tab_index(0)
            .tab_stop(true)
            .key_context("STGVirtualList")
            .focus(move |style| style.border_2().border_color(accent))
            .on_action(cx.listener(move |frame, _: &MoveSTGListUp, window, cx| {
                frame.move_stg_list_cursor(document, kind, STGListMovement::Up, window, cx);
            }))
            .on_action(cx.listener(move |frame, _: &MoveSTGListDown, window, cx| {
                frame.move_stg_list_cursor(document, kind, STGListMovement::Down, window, cx);
            }))
            .on_action(cx.listener(move |frame, _: &MoveSTGListHome, window, cx| {
                frame.move_stg_list_cursor(document, kind, STGListMovement::Home, window, cx);
            }))
            .on_action(cx.listener(move |frame, _: &MoveSTGListEnd, window, cx| {
                frame.move_stg_list_cursor(document, kind, STGListMovement::End, window, cx);
            }))
            .on_action(
                cx.listener(move |frame, _: &MoveSTGListPageUp, window, cx| {
                    frame.move_stg_list_cursor(document, kind, STGListMovement::PageUp, window, cx);
                }),
            )
            .on_action(
                cx.listener(move |frame, _: &MoveSTGListPageDown, window, cx| {
                    frame.move_stg_list_cursor(
                        document,
                        kind,
                        STGListMovement::PageDown,
                        window,
                        cx,
                    );
                }),
            )
            .child(list)
            .into_any_element()
    }

    fn stg_virtual_master_row(
        &mut self,
        document: DocumentID,
        generation: u64,
        location: STGRowLocation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let projection = self.stg_master_row_projection(document, location.cursor());
        match projection {
            Ok((title, metadata)) => {
                let cursor = location.cursor();
                let selected = self.stg_cursor_is_selected(document, cursor);
                let selector = stg_master_row_selector(cursor);
                stg::master_row(
                    &self.theme,
                    SharedString::from(location.id().element_key("stg-master-row")),
                    title,
                    metadata,
                    selected,
                )
                .debug_selector(move || selector.clone())
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_stg_row(document, generation, location, window, cx);
                }))
                .into_any_element()
            }
            Err(error) => stg::empty_state(
                &self.theme,
                "stg-master-row-error",
                format!("Could not read this STG row: {error}"),
            )
            .into_any_element(),
        }
    }

    fn stg_master_row_projection(
        &self,
        document: DocumentID,
        cursor: STGRowCursor,
    ) -> STGProjectionResult<(String, String)> {
        match cursor {
            STGRowCursor::Unit(unit) => self.stg_unit_master_projection(document, unit),
            STGRowCursor::Area(area) => self.stg_area_master_projection(document, area),
            STGRowCursor::Variable(variable) => {
                self.stg_variable_master_projection(document, variable)
            }
            STGRowCursor::EventBlock(block) => {
                self.stg_event_block_master_projection(document, block)
            }
            STGRowCursor::Event(target) => self.stg_event_master_projection(document, target),
            STGRowCursor::Footer(entry) => self.stg_footer_master_projection(document, entry),
            STGRowCursor::EventDetail { .. } => {
                unreachable!("event-detail cursors use the flattened detail renderer")
            }
        }
    }

    fn stg_unit_master_projection(
        &self,
        document: DocumentID,
        unit: usize,
    ) -> STGProjectionResult<(String, String)> {
        let name = self
            .workspace
            .stg_text(document, STGTextTarget::UnitName { unit })?;
        let internal = name.decoded().unwrap_or("Invalid source name");
        let read_unit_field = |field| -> STGProjectionResult<i64> {
            self.workspace
                .stg_number(document, STGNumberTarget::Unit { unit, field })
                .map_err(Box::new)
        };
        let unique_id = read_unit_field(STGUnitField::UniqueID)?;
        let ucd = read_unit_field(STGUnitField::UCD)?;
        let level = read_unit_field(STGUnitField::LeaderLevel)?;
        let job = read_unit_field(STGUnitField::LeaderJobType)
            .ok()
            .and_then(|value| u8::try_from(value).ok());
        let model = read_unit_field(STGUnitField::LeaderModelID)
            .ok()
            .and_then(|value| u8::try_from(value).ok());
        let title = self
            .visible_crusaders_dictionary()
            .zip(job.zip(model))
            .map_or_else(
                || empty_stg_text(internal).to_owned(),
                |(dictionary, (job, model))| {
                    dictionary.stg_unit_name(internal, job, model).into_owned()
                },
            );
        Ok((
            format!("{:03} · {title}", unit + 1),
            format!("{internal} · ID {unique_id} · UCD {ucd} · level {level}"),
        ))
    }

    fn stg_area_master_projection(
        &self,
        document: DocumentID,
        area: usize,
    ) -> STGProjectionResult<(String, String)> {
        let description = self
            .workspace
            .stg_text(document, STGTextTarget::AreaDescription { area })?;
        let source = description
            .decoded()
            .unwrap_or("Invalid source description");
        let id = self.workspace.stg_number(
            document,
            STGNumberTarget::Area {
                area,
                field: STGAreaField::AreaID,
            },
        )?;
        let title = self
            .visible_crusaders_dictionary()
            .and_then(|dictionary| dictionary.translate(source))
            .unwrap_or_else(|| empty_stg_text(source).to_owned());
        Ok((
            format!("{:03} · {title}", area + 1),
            format!("{source} · area ID {id}"),
        ))
    }

    fn stg_variable_master_projection(
        &self,
        document: DocumentID,
        variable: usize,
    ) -> STGProjectionResult<(String, String)> {
        let name = self
            .workspace
            .stg_text(document, STGTextTarget::VariableName { variable })?;
        let source = name.decoded().unwrap_or("Invalid source name");
        let id = self
            .workspace
            .stg_number(document, STGNumberTarget::VariableID { variable })?;
        let value = self
            .workspace
            .stg_value(document, STGValueTarget::VariableInitial { variable })?;
        let title = self
            .visible_crusaders_dictionary()
            .and_then(|dictionary| dictionary.translate(source))
            .unwrap_or_else(|| empty_stg_text(source).to_owned());
        Ok((
            format!("{:03} · {title}", variable + 1),
            format!(
                "{source} · variable ID {id} · {}",
                stg_value_summary(&value)
            ),
        ))
    }

    fn stg_event_block_master_projection(
        &self,
        document: DocumentID,
        block: usize,
    ) -> STGProjectionResult<(String, String)> {
        let projection = self.workspace.stg_event_block(document, block)?;
        Ok((
            format!("Event block {block} · empty"),
            format!(
                "block header {} · no events · source block remains visible",
                projection.header
            ),
        ))
    }

    fn stg_event_master_projection(
        &self,
        document: DocumentID,
        target: STGEventTarget,
    ) -> STGProjectionResult<(String, String)> {
        let event = self.workspace.stg_event(document, target)?;
        let description = event.description.decoded().unwrap_or("Invalid event label");
        let block = self.workspace.stg_event_block(document, target.block)?;
        Ok((
            format!(
                "B{} · E{} · {}",
                target.block,
                target.event,
                empty_stg_text(description)
            ),
            format!(
                "block header {} · event ID {} · {} conditions · {} actions",
                block.header, event.id, event.condition_count, event.action_count
            ),
        ))
    }

    fn stg_footer_master_projection(
        &self,
        document: DocumentID,
        entry: usize,
    ) -> STGProjectionResult<(String, String)> {
        let read_footer_field = |field| -> STGProjectionResult<i64> {
            self.workspace
                .stg_number(document, STGNumberTarget::Footer { entry, field })
                .map_err(Box::new)
        };
        let first = read_footer_field(STGFooterField::SlotData1)?;
        let second = read_footer_field(STGFooterField::SlotData2)?;
        Ok((
            format!("Footer entry {:03}", entry + 1),
            format!("slot data 1: {first} · slot data 2: {second}"),
        ))
    }

    fn stg_virtual_event_detail_row(
        &mut self,
        document: DocumentID,
        generation: u64,
        rows: &STGVirtualRows,
        location: STGRowLocation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = rows.event_detail_row(location.position()) else {
            return stg::empty_state(
                &self.theme,
                "stg-event-detail-row-error",
                "This event detail row is no longer available.",
            )
            .into_any_element();
        };
        let STGRowCursor::EventDetail { event, .. } = location.cursor() else {
            return stg::empty_state(
                &self.theme,
                "stg-event-detail-row-error",
                "This flattened row lost its event identity.",
            )
            .into_any_element();
        };
        if let Some(field) = self.stg_event_detail_field_element(document, event, row, cx) {
            return div()
                .h(px(64.0))
                .px(px(7.0))
                .py(px(5.0))
                .flex()
                .items_center()
                .child(field)
                .into_any_element();
        }
        let projection = self.stg_event_detail_row_projection(document, event, row);
        match projection {
            Ok((title, metadata)) => {
                let selected = self
                    .stg_lists
                    .get(STGVirtualRowKind::EventDetail)
                    .binding
                    .get()
                    .is_some_and(|binding| binding.cursor == location.cursor());
                let selector = format!("stg-event-detail-row-{}", location.position());
                stg::master_row(
                    &self.theme,
                    SharedString::from(location.id().element_key("stg-event-detail-row")),
                    title,
                    metadata,
                    selected,
                )
                .h(px(64.0))
                .debug_selector(move || selector.clone())
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_stg_row(document, generation, location, window, cx);
                }))
                .into_any_element()
            }
            Err(error) => stg::empty_state(
                &self.theme,
                "stg-event-detail-row-error",
                format!("Could not read this event detail: {error}"),
            )
            .into_any_element(),
        }
    }

    fn stg_event_detail_field_element(
        &self,
        document: DocumentID,
        event: STGEventTarget,
        row: STGEventDetailRow,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (field, selector) = match row {
            STGEventDetailRow::EventField(STGEventDetailField::BlockHeader) => (
                self.stg_number_field(
                    document,
                    STGSection::Events,
                    STGNumberTarget::EventBlockHeader { block: event.block },
                ),
                format!("stg-event-block-{}-header", event.block),
            ),
            STGEventDetailRow::EventField(STGEventDetailField::Description) => (
                self.stg_text_field(
                    document,
                    STGSection::Events,
                    STGTextTarget::EventDescription {
                        block: event.block,
                        event: event.event,
                    },
                ),
                format!("stg-event-{}-{}-description", event.block, event.event),
            ),
            STGEventDetailRow::EventField(STGEventDetailField::ID) => (
                self.stg_number_field(
                    document,
                    STGSection::Events,
                    STGNumberTarget::EventID {
                        block: event.block,
                        event: event.event,
                    },
                ),
                format!("stg-event-{}-{}-id", event.block, event.event),
            ),
            STGEventDetailRow::Parameter(target) => {
                let parameter = self.workspace.stg_parameter(document, target).ok()?;
                let hint = parameter.hint.unwrap_or("Unlabeled parameter");
                (
                    self.stg_value_field(
                        document,
                        STGSection::Events,
                        STGValueTarget::ScriptParameter(target),
                        format!("Parameter {} · {hint}", target.parameter + 1),
                    ),
                    format!(
                        "stg-parameter-{}-{}-{}-{}-{}",
                        target.script.block,
                        target.script.event,
                        match target.script.kind {
                            STGScriptKind::Condition => "condition",
                            STGScriptKind::Action => "action",
                        },
                        target.script.script,
                        target.parameter,
                    ),
                )
            }
            STGEventDetailRow::ScriptHeader(_) | STGEventDetailRow::AddScript(_) => return None,
        };
        Some(self.stg_field_element(&field, selector, cx))
    }

    fn stg_event_detail_row_projection(
        &self,
        document: DocumentID,
        event: STGEventTarget,
        row: STGEventDetailRow,
    ) -> STGProjectionResult<(String, String)> {
        match row {
            STGEventDetailRow::EventField(STGEventDetailField::BlockHeader) => {
                let projection = self.workspace.stg_event_block(document, event.block)?;
                Ok((
                    "Event block header".to_owned(),
                    projection.header.to_string(),
                ))
            }
            STGEventDetailRow::EventField(STGEventDetailField::Description) => {
                let projection = self.workspace.stg_event(document, event)?;
                Ok((
                    "Event description".to_owned(),
                    projection
                        .description
                        .decoded()
                        .map_or_else(|| "Invalid source text".to_owned(), empty_stg_text_owned),
                ))
            }
            STGEventDetailRow::EventField(STGEventDetailField::ID) => {
                let projection = self.workspace.stg_event(document, event)?;
                Ok(("Event ID".to_owned(), projection.id.to_string()))
            }
            STGEventDetailRow::ScriptHeader(target) => {
                let script = self.workspace.stg_script(document, target)?;
                Ok((
                    format!(
                        "{} {} · {}",
                        target.kind.label(),
                        target.script + 1,
                        script.label()
                    ),
                    format!(
                        "raw type {} · {} parameters",
                        script.id, script.parameter_count
                    ),
                ))
            }
            STGEventDetailRow::Parameter(target) => {
                let parameter = self.workspace.stg_parameter(document, target)?;
                let hint = parameter.hint.unwrap_or("Unlabeled parameter");
                let reference = parameter.reference.map_or_else(
                    || "literal".to_owned(),
                    |kind| format!("{kind:?} reference"),
                );
                Ok((
                    format!("Parameter {} · {hint}", target.parameter + 1),
                    format!("{} · {reference}", stg_value_summary(&parameter.value)),
                ))
            }
            STGEventDetailRow::AddScript(kind) => Ok((
                format!("Add {}", kind.label()),
                "Read-only in this structured-view stage".to_owned(),
            )),
        }
    }

    fn stg_cursor_is_selected(&self, document: DocumentID, cursor: STGRowCursor) -> bool {
        self.stg_presentations
            .get(document)
            .is_some_and(|state| match cursor {
                STGRowCursor::Unit(unit) => state.inspected_unit() == Some(unit),
                STGRowCursor::Area(area) => state.inspected_area() == Some(area),
                STGRowCursor::Variable(variable) => state.inspected_variable() == Some(variable),
                STGRowCursor::Event(event) => state.inspected_event() == Some(event),
                STGRowCursor::Footer(entry) => state.inspected_footer() == Some(entry),
                STGRowCursor::EventBlock(_) | STGRowCursor::EventDetail { .. } => self
                    .stg_lists
                    .get(stg_cursor_kind(cursor))
                    .binding
                    .get()
                    .is_some_and(|binding| binding.cursor == cursor),
            })
    }

    fn select_stg_row(
        &mut self,
        document: DocumentID,
        generation: u64,
        location: STGRowLocation,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = location.cursor();
        let kind = stg_cursor_kind(cursor);
        let valid_state = self.stg_presentations.get(document).is_some_and(|state| {
            state.binding_generation() == generation
                && state.section() == cursor.section()
                && self.active_document == Some(document)
        });
        if !valid_state {
            return;
        }
        let Ok(Some(rows)) = self.stg_rows_for_kind(document, kind) else {
            return;
        };
        if rows.cursor(location.position()) != Some(cursor) {
            return;
        }

        let control = self.stg_lists.get(kind);
        window.focus(&control.focus);
        if kind == STGVirtualRowKind::EventDetail || matches!(cursor, STGRowCursor::EventBlock(_)) {
            control.binding.set(Some(super::STGListBinding {
                document,
                cursor,
                position: location.position(),
                row_count: rows.len(),
                generation,
            }));
            control
                .scroll
                .scroll_to_item(location.position(), ScrollStrategy::Center);
            cx.notify();
            return;
        }

        let selection = match cursor {
            STGRowCursor::Unit(unit) => STGSelection::Unit(Some(unit)),
            STGRowCursor::Area(area) => STGSelection::Area(Some(area)),
            STGRowCursor::Variable(variable) => STGSelection::Variable(Some(variable)),
            STGRowCursor::EventBlock(_) | STGRowCursor::EventDetail { .. } => unreachable!(),
            STGRowCursor::Event(event) => STGSelection::Event(Some(event)),
            STGRowCursor::Footer(entry) => STGSelection::Footer(Some(entry)),
        };
        let draft = self.active_stg_draft_status(false);
        let transition = self.stg_presentations.select(document, selection, draft);
        self.finish_stg_presentation_transition(document, transition, cx);
    }

    fn move_stg_list_cursor(
        &mut self,
        document: DocumentID,
        kind: STGVirtualRowKind,
        movement: STGListMovement,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let control = self.stg_lists.get(kind);
        let Some(binding) = control.binding.get() else {
            return;
        };
        if binding.document != document || self.active_document != Some(document) {
            return;
        }
        let Some(state) = self.stg_presentations.get(document) else {
            return;
        };
        if binding.generation != state.binding_generation()
            || binding.cursor.section() != state.section()
        {
            return;
        }
        let Ok(Some(rows)) = self.stg_rows_for_kind(document, kind) else {
            return;
        };
        if rows.len() != binding.row_count || rows.cursor(binding.position) != Some(binding.cursor)
        {
            return;
        }
        let page = 8_usize;
        let position = match movement {
            STGListMovement::Up => binding.position.saturating_sub(1),
            STGListMovement::Down => binding
                .position
                .saturating_add(1)
                .min(rows.len().saturating_sub(1)),
            STGListMovement::Home => 0,
            STGListMovement::End => rows.len().saturating_sub(1),
            STGListMovement::PageUp => binding.position.saturating_sub(page),
            STGListMovement::PageDown => binding
                .position
                .saturating_add(page)
                .min(rows.len().saturating_sub(1)),
        };
        let Some(cursor) = rows.cursor(position) else {
            return;
        };
        let Some(location) = rows
            .locations(position..position.saturating_add(1))
            .into_iter()
            .next()
        else {
            return;
        };
        self.select_stg_row(document, binding.generation, location, window, cx);
        let strategy = match movement {
            STGListMovement::Up | STGListMovement::Home | STGListMovement::PageUp => {
                ScrollStrategy::Top
            }
            STGListMovement::Down | STGListMovement::End | STGListMovement::PageDown => {
                ScrollStrategy::Bottom
            }
        };
        self.stg_lists
            .get(kind)
            .scroll
            .scroll_to_item(position, strategy);
        debug_assert_eq!(cursor, location.cursor());
    }

    fn stg_rows_for_kind(
        &self,
        document: DocumentID,
        kind: STGVirtualRowKind,
    ) -> STGProjectionResult<Option<STGVirtualRows>> {
        let projection = self.stg_presentation_projection(document)?;
        Ok(match kind {
            STGVirtualRowKind::Unit => Some(STGVirtualRows::units(document, projection.units)),
            STGVirtualRowKind::Area => projection
                .document
                .areas()
                .cloned()
                .map(|rows| STGVirtualRows::areas(document, rows)),
            STGVirtualRowKind::Variable => projection
                .document
                .variables()
                .cloned()
                .map(|rows| STGVirtualRows::variables(document, rows)),
            STGVirtualRowKind::Event => projection
                .document
                .events()
                .map(|_| STGVirtualRows::events(document, projection.events)),
            STGVirtualRowKind::Footer => projection
                .document
                .footer()
                .cloned()
                .map(|rows| STGVirtualRows::footer(document, rows)),
            STGVirtualRowKind::EventDetail => {
                let Some(event) = self
                    .stg_presentations
                    .get(document)
                    .and_then(crate::state::STGPresentationState::inspected_event)
                else {
                    return Ok(None);
                };
                self.stg_event_detail_rows(document, event)?
                    .map(|rows| STGVirtualRows::event_details(document, event, rows))
            }
        })
    }
}

fn stg_section_label(section: STGSection, projection: &STGDocumentProjection) -> String {
    let count = match section {
        STGSection::Header => return "Header".to_owned(),
        STGSection::Units => Some(projection.units().len()),
        STGSection::Areas => projection.areas().map(STGIndexRows::len),
        STGSection::Variables => projection.variables().map(STGIndexRows::len),
        STGSection::Events => projection.events().map(STGEventRows::len),
        STGSection::Footer => projection.footer().map(STGIndexRows::len),
    };
    count.map_or_else(
        || format!("{} · unparsed", section.label()),
        |count| format!("{} · {count}", section.label()),
    )
}

const fn stg_section_id(section: STGSection) -> &'static str {
    match section {
        STGSection::Header => "stg-header",
        STGSection::Units => "stg-units",
        STGSection::Areas => "stg-areas",
        STGSection::Variables => "stg-variables",
        STGSection::Events => "stg-events",
        STGSection::Footer => "stg-footer",
    }
}

fn empty_stg_text(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}

fn empty_stg_text_owned(value: &str) -> String {
    empty_stg_text(value).to_owned()
}

const fn header_field_slug(field: STGHeaderTextField) -> &'static str {
    match field {
        STGHeaderTextField::MapFilename => "map-filename",
        STGHeaderTextField::BitmapFilename => "bitmap-filename",
        STGHeaderTextField::DefaultCamera => "default-camera",
        STGHeaderTextField::UserCamera => "user-camera",
        STGHeaderTextField::SettingsFile => "settings-file",
        STGHeaderTextField::SkyEffects => "sky-effects",
        STGHeaderTextField::AIScript => "ai-script",
        STGHeaderTextField::CubemapTexture => "cubemap-texture",
    }
}

fn unit_field_slug(field: STGUnitField) -> String {
    format!("{field:?}").to_ascii_lowercase()
}

const fn stg_list_root_selector(kind: STGVirtualRowKind) -> &'static str {
    match kind {
        STGVirtualRowKind::Unit => "stg-unit-list-root",
        STGVirtualRowKind::Area => "stg-area-list-root",
        STGVirtualRowKind::Variable => "stg-variable-list-root",
        STGVirtualRowKind::Event => "stg-event-list-root",
        STGVirtualRowKind::Footer => "stg-footer-list-root",
        STGVirtualRowKind::EventDetail => "stg-event-detail-list-root",
    }
}

fn stg_master_row_selector(cursor: STGRowCursor) -> String {
    match cursor {
        STGRowCursor::Unit(unit) => format!("stg-unit-master-row-{unit}"),
        STGRowCursor::Area(area) => format!("stg-area-master-row-{area}"),
        STGRowCursor::Variable(variable) => format!("stg-variable-master-row-{variable}"),
        STGRowCursor::EventBlock(block) => format!("stg-event-block-row-{block}"),
        STGRowCursor::Event(target) => {
            format!("stg-event-master-row-{}-{}", target.block, target.event)
        }
        STGRowCursor::Footer(entry) => format!("stg-footer-master-row-{entry}"),
        STGRowCursor::EventDetail { row, .. } => format!("stg-event-detail-row-{row}"),
    }
}

const fn stg_cursor_kind(cursor: STGRowCursor) -> STGVirtualRowKind {
    match cursor {
        STGRowCursor::Unit(_) => STGVirtualRowKind::Unit,
        STGRowCursor::Area(_) => STGVirtualRowKind::Area,
        STGRowCursor::Variable(_) => STGVirtualRowKind::Variable,
        STGRowCursor::EventBlock(_) | STGRowCursor::Event(_) => STGVirtualRowKind::Event,
        STGRowCursor::Footer(_) => STGVirtualRowKind::Footer,
        STGRowCursor::EventDetail { .. } => STGVirtualRowKind::EventDetail,
    }
}

fn stg_value_summary(value: &STGValue<'_>) -> String {
    match value {
        STGValue::Integer(value) => format!("integer {value}"),
        STGValue::Enum(value) => format!("enum {value}"),
        STGValue::Float(value) => {
            let bits = value.to_bits();
            let number = f32::from_bits(bits);
            if number.is_finite() {
                format!("float {number}")
            } else {
                format!("float {number} · bits 0x{bits:08X}")
            }
        }
        STGValue::String(kufeditor_workspace::STGText::Decoded(value)) => {
            format!("string {}", empty_stg_text(value.as_ref()))
        }
        STGValue::String(kufeditor_workspace::STGText::Raw(bytes)) => {
            format!("invalid string · {} bytes", bytes.len())
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled GPUI and STG fixtures make failures fatal"
    )]

    use std::{fs, mem::size_of, path::PathBuf};

    use gpui::{
        AppContext, Context, Entity, EntityInputHandler, Modifiers, TestAppContext,
        VisualTestContext, WindowOptions, point, px, size,
    };
    use kufeditor_game::Game;
    use kufeditor_workspace::{
        Document, DocumentEdit, DocumentID, STGAreaField, STGAreaFloatField, STGDocument,
        STGEventTarget, STGFloatTarget, STGFooterField, STGHeaderTextField, STGNumberTarget,
        STGParameterTarget, STGReferenceKind, STGScriptKind, STGScriptTarget, STGStructuralEdit,
        STGText, STGTextTarget, STGUnitField, STGUnitFloatField, STGValueKind, STGValueTarget,
        TroopDocument,
    };

    use super::{
        super::{AppFrame, EditorRoute, editor_route},
        STGListMovement, STGSearchKind,
    };
    use crate::{
        crusaders_catalog_status::CrusadersCatalogStatus,
        settings::SettingsStartup,
        state::{
            Area, STGDocumentTransition, STGReferenceCursor, STGReferencePickerState, STGSection,
            STGSelection,
        },
        text_input::TextInputEvent,
        views::stg::STGVirtualRowKind,
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

    fn raw_tail_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 1_001);
        bytes.resize(bytes.len() + 620, 0);
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, u32::MAX);
        bytes
    }

    fn unit_list_fixture(unit_count: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 1_001);
        bytes.resize(bytes.len() + 620, 0);
        append_u32(&mut bytes, u32::try_from(unit_count).unwrap());
        bytes.resize(bytes.len() + unit_count * 544, 0);
        for _ in 0..4 {
            append_u32(&mut bytes, 0);
        }
        bytes
    }

    struct ScalarSTGFixture {
        bytes: Vec<u8>,
        area_description: usize,
    }

    #[derive(Clone, Copy)]
    enum ScalarParameter<'a> {
        Integer(i32),
        Float(f32),
        String(&'a [u8]),
        Enum(i32),
    }

    fn scalar_stg_fixture() -> ScalarSTGFixture {
        const HEADER_SIZE: usize = 620;
        const UNIT_SIZE: usize = 544;
        const AREA_SIZE: usize = 84;

        let mut bytes = Vec::new();
        append_u32(&mut bytes, 1_001);
        bytes.resize(bytes.len() + HEADER_SIZE, 0);
        let header_map = 4 + 68;
        write_fixture_text(&mut bytes, header_map, b"Map");

        append_u32(&mut bytes, 1);
        let unit_name = bytes.len();
        bytes.resize(bytes.len() + UNIT_SIZE, 0);
        write_fixture_text(&mut bytes, unit_name, b"Unit");
        write_fixture_u32(&mut bytes, unit_name + 32, 7);
        *bytes.get_mut(unit_name + 36).unwrap() = 99;
        write_fixture_u32(&mut bytes, unit_name + 40, 1.25_f32.to_bits());
        write_fixture_u32(
            &mut bytes,
            unit_name + UNIT_SIZE - 22 * size_of::<f32>(),
            f32::NAN.to_bits(),
        );

        append_u32(&mut bytes, 1);
        let area_description = bytes.len();
        bytes.resize(bytes.len() + AREA_SIZE, 0);
        write_fixture_text(&mut bytes, area_description, b"Area");
        write_fixture_u32(&mut bytes, area_description + 64, 22);
        write_fixture_u32(&mut bytes, area_description + 68, 2.5_f32.to_bits());

        append_u32(&mut bytes, 4);
        append_scalar_variable(&mut bytes, 100, ScalarParameter::Integer(-12));
        append_scalar_variable(&mut bytes, 101, ScalarParameter::Float(17.25));
        append_scalar_variable(&mut bytes, 102, ScalarParameter::String(b"variable"));
        append_scalar_variable(&mut bytes, 103, ScalarParameter::Enum(7));

        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 0x0102_0304);
        append_u32(&mut bytes, 1);
        append_fixed_fixture_text::<64>(&mut bytes, b"Primary Event");
        append_u32(&mut bytes, 500);
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 19);
        append_u32(&mut bytes, 2);
        append_scalar_parameter(&mut bytes, ScalarParameter::Integer(23));
        append_scalar_parameter(&mut bytes, ScalarParameter::Float(-0.0));
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 55);
        append_u32(&mut bytes, 2);
        append_scalar_parameter(&mut bytes, ScalarParameter::String(b"action"));
        append_scalar_parameter(&mut bytes, ScalarParameter::Enum(-3));

        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 700);
        append_u32(&mut bytes, 701);

        ScalarSTGFixture {
            bytes,
            area_description,
        }
    }

    fn append_scalar_variable(bytes: &mut Vec<u8>, id: u32, value: ScalarParameter<'_>) {
        append_fixed_fixture_text::<64>(bytes, format!("Variable {id}").as_bytes());
        append_u32(bytes, id);
        append_scalar_parameter(bytes, value);
    }

    fn append_scalar_parameter(bytes: &mut Vec<u8>, value: ScalarParameter<'_>) {
        match value {
            ScalarParameter::Integer(value) => {
                append_u32(bytes, 0);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            ScalarParameter::Float(value) => {
                append_u32(bytes, 1);
                append_u32(bytes, value.to_bits());
            }
            ScalarParameter::String(value) => {
                append_u32(bytes, 2);
                append_u32(bytes, u32::try_from(value.len()).unwrap());
                bytes.extend_from_slice(value);
            }
            ScalarParameter::Enum(value) => {
                append_u32(bytes, 3);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }

    fn append_fixed_fixture_text<const N: usize>(bytes: &mut Vec<u8>, value: &[u8]) {
        assert!(value.len() < N);
        bytes.extend_from_slice(value);
        bytes.resize(bytes.len() + N - value.len(), 0);
    }

    fn write_fixture_text(bytes: &mut [u8], offset: usize, value: &[u8]) {
        bytes
            .get_mut(offset..offset + value.len())
            .unwrap()
            .copy_from_slice(value);
    }

    fn write_fixture_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes
            .get_mut(offset..offset + size_of::<u32>())
            .unwrap()
            .copy_from_slice(&value.to_le_bytes());
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
        activate_stg(&frame, cx, stg_fixture());
        draw_stg_frame(cx, &frame);

        assert!(
            cx.debug_bounds("stg-editor").is_some(),
            "error={}",
            cx.debug_bounds("stg-editor-error").is_some(),
        );
    }

    #[gpui::test]
    fn stg_view_draws_all_six_sections_inside_the_product_navigation(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_stg(&frame, cx, stg_fixture());

        draw_stg_frame(cx, &frame);
        assert!(cx.debug_bounds("product-navigation").is_some());
        assert!(cx.debug_bounds("stg-section-rail").is_some());
        assert!(cx.debug_bounds("stg-header").is_some());
        assert!(cx.debug_bounds("stg-header-map-filename").is_some());
        assert!(cx.debug_bounds("stg-header-advanced").is_some());

        select_stg_section(&frame, cx, document, STGSection::Units);
        assert!(cx.debug_bounds("stg-units").is_some());
        assert!(cx.debug_bounds("stg-unit-list-root").is_some());
        assert!(cx.debug_bounds("stg-unit-master-row-0").is_some());
        assert!(cx.debug_bounds("stg-unit-detail").is_some());

        select_stg_section(&frame, cx, document, STGSection::Areas);
        assert!(cx.debug_bounds("stg-areas").is_some());
        assert!(cx.debug_bounds("stg-area-list-root").is_some());
        assert!(cx.debug_bounds("stg-area-master-row-0").is_some());
        assert!(cx.debug_bounds("stg-area-detail").is_some());

        select_stg_section(&frame, cx, document, STGSection::Variables);
        assert!(cx.debug_bounds("stg-variables").is_some());
        assert!(cx.debug_bounds("stg-variable-empty").is_some());
        assert!(cx.debug_bounds("stg-variable-list-root").is_none());

        select_stg_section(&frame, cx, document, STGSection::Events);
        assert!(cx.debug_bounds("stg-events").is_some());
        assert!(cx.debug_bounds("stg-event-list-root").is_some());
        assert!(cx.debug_bounds("stg-event-master-row-0-0").is_some());
        assert!(cx.debug_bounds("stg-event-detail-list-root").is_some());

        select_stg_section(&frame, cx, document, STGSection::Footer);
        assert!(cx.debug_bounds("stg-footer").is_some());
        assert!(cx.debug_bounds("stg-footer-list-root").is_some());
        assert!(cx.debug_bounds("stg-footer-master-row-0").is_some());
        assert!(cx.debug_bounds("stg-footer-detail").is_some());
    }

    #[gpui::test]
    fn stg_view_raw_tail_is_honest_for_every_unparsed_section(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_stg(&frame, cx, raw_tail_fixture());

        for (section, panel, list_root) in [
            (
                STGSection::Areas,
                "stg-raw-tail-areas",
                "stg-area-list-root",
            ),
            (
                STGSection::Variables,
                "stg-raw-tail-variables",
                "stg-variable-list-root",
            ),
            (
                STGSection::Events,
                "stg-raw-tail-events",
                "stg-event-list-root",
            ),
            (
                STGSection::Footer,
                "stg-raw-tail-footer",
                "stg-footer-list-root",
            ),
        ] {
            select_stg_section(&frame, cx, document, section);
            assert!(cx.debug_bounds(panel).is_some(), "missing {panel}");
            assert!(
                cx.debug_bounds(list_root).is_none(),
                "raw tail created {list_root}"
            );
        }
    }

    #[gpui::test]
    fn stg_view_search_filters_units_and_events(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_stg(&frame, cx, stg_fixture());

        select_stg_section(&frame, cx, document, STGSection::Units);
        assert!(cx.debug_bounds("stg-unit-search").is_some());
        frame.update_in(cx, |frame, window, cx| {
            frame.start_stg_search(document, STGSearchKind::Units, window, cx);
        });
        let unit_search = frame.update(cx, |frame, _| {
            frame.stg_search.as_ref().unwrap().input.clone()
        });
        unit_search.update_in(cx, |input, window, cx| {
            input.replace_text_in_range(None, "missing-unit", window, cx);
        });
        draw_stg_frame(cx, &frame);
        assert!(cx.debug_bounds("stg-unit-search-input").is_some());
        assert!(cx.debug_bounds("stg-unit-empty").is_some());
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.stg_presentations.get(document).unwrap().unit_query(),
                "missing-unit"
            );
        });
        unit_search.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("missing-unit".to_owned()));
        });

        select_stg_section(&frame, cx, document, STGSection::Events);
        assert!(cx.debug_bounds("stg-event-search").is_some());
        frame.update_in(cx, |frame, window, cx| {
            frame.start_stg_search(document, STGSearchKind::Events, window, cx);
        });
        let event_search = frame.update(cx, |frame, _| {
            frame.stg_search.as_ref().unwrap().input.clone()
        });
        event_search.update_in(cx, |input, window, cx| {
            input.replace_text_in_range(None, "500", window, cx);
        });
        draw_stg_frame(cx, &frame);
        assert!(cx.debug_bounds("stg-event-search-input").is_some());
        assert!(cx.debug_bounds("stg-event-master-row-0-0").is_some());
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.stg_presentations.get(document).unwrap().event_query(),
                "500"
            );
        });
        event_search.update_in(cx, |input, window, cx| {
            input.replace_text_in_range(Some(0..3), "missing-event", window, cx);
        });
        draw_stg_frame(cx, &frame);
        assert!(cx.debug_bounds("stg-event-empty").is_some());
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .stg_presentations
                    .get(document)
                    .unwrap()
                    .inspected_event(),
                None
            );
        });
    }

    #[gpui::test]
    fn stg_number_edit_handles_unit_bounds_choices_and_exact_history(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_stg(&frame, cx, scalar_stg_fixture().bytes);

        select_stg_section(&frame, cx, document, STGSection::Units);
        assert!(cx.debug_bounds("stg-unit-0-field-ucd-unknown").is_some());
        click(cx, "stg-unit-0-field-uniqueid");
        cx.simulate_keystrokes("4 2 enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::Unit {
                            unit: 0,
                            field: STGUnitField::UniqueID,
                        },
                    )
                    .unwrap(),
                42,
            );
        });

        click(cx, "stg-unit-0-field-uniqueid");
        cx.simulate_keystrokes("enter");
        frame.update(cx, |frame, cx| frame.move_history(false, cx));
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::Unit {
                            unit: 0,
                            field: STGUnitField::UniqueID,
                        },
                    )
                    .unwrap(),
                7,
            );
        });
        frame.update(cx, |frame, cx| frame.move_history(true, cx));

        click(cx, "stg-unit-0-field-leaderlevel");
        cx.simulate_keystrokes("0 enter");
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_some()));
        cx.simulate_keystrokes("escape");

        click(cx, "stg-unit-0-field-ucd-choice-1");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::Unit {
                            unit: 0,
                            field: STGUnitField::UCD,
                        },
                    )
                    .unwrap(),
                1,
            );
        });
    }

    #[gpui::test]
    fn stg_number_edit_dispatches_area_and_variable_targets(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_stg(&frame, cx, scalar_stg_fixture().bytes);

        select_stg_section(&frame, cx, document, STGSection::Areas);
        click(cx, "stg-area-0-id");
        cx.simulate_keystrokes("2 3 enter");
        select_stg_section(&frame, cx, document, STGSection::Variables);
        click(cx, "stg-variable-0-id");
        cx.simulate_keystrokes("2 0 0 enter");
        click(cx, "stg-variable-0-value");
        cx.simulate_keystrokes("- 2 4 enter");

        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::Area {
                            area: 0,
                            field: STGAreaField::AreaID,
                        },
                    )
                    .unwrap(),
                23,
            );
            assert_eq!(
                frame
                    .workspace
                    .stg_number(document, STGNumberTarget::VariableID { variable: 0 })
                    .unwrap(),
                200,
            );
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::ParameterInteger {
                            value: STGValueTarget::VariableInitial { variable: 0 },
                        },
                    )
                    .unwrap(),
                -24,
            );
        });
    }

    #[gpui::test]
    fn stg_number_edit_dispatches_event_and_footer_targets(cx: &mut TestAppContext) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_stg(&frame, cx, scalar_stg_fixture().bytes);

        select_stg_section(&frame, cx, document, STGSection::Events);
        click(cx, "stg-event-block-0-header");
        cx.simulate_keystrokes("8 enter");
        click(cx, "stg-event-0-0-id");
        cx.simulate_keystrokes("5 0 1 enter");
        click(cx, "stg-parameter-0-0-condition-0-0");
        cx.simulate_keystrokes("2 4 enter");
        select_stg_section(&frame, cx, document, STGSection::Footer);
        click(cx, "stg-footer-0-SlotData1");
        cx.simulate_keystrokes("8 0 0 enter");

        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_number(document, STGNumberTarget::EventBlockHeader { block: 0 })
                    .unwrap(),
                8,
            );
            assert_eq!(
                frame
                    .workspace
                    .stg_number(document, STGNumberTarget::EventID { block: 0, event: 0 },)
                    .unwrap(),
                501,
            );
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::ParameterInteger {
                            value: STGValueTarget::ScriptParameter(STGParameterTarget {
                                script: STGScriptTarget {
                                    block: 0,
                                    event: 0,
                                    kind: STGScriptKind::Condition,
                                    script: 0,
                                },
                                parameter: 0,
                            }),
                        },
                    )
                    .unwrap(),
                24,
            );
            assert_eq!(
                frame
                    .workspace
                    .stg_number(
                        document,
                        STGNumberTarget::Footer {
                            entry: 0,
                            field: STGFooterField::SlotData1,
                        },
                    )
                    .unwrap(),
                800,
            );
        });
    }

    #[gpui::test]
    fn stg_float_edit_preserves_bits_and_requires_explicit_nonfinite_replacement(
        cx: &mut TestAppContext,
    ) {
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_stg(&frame, cx, scalar_stg_fixture().bytes);
        select_stg_section(&frame, cx, document, STGSection::Units);

        click(cx, "stg-unit-0-float-LeaderHPOverride");
        frame.update(cx, |frame, _| {
            let edit = frame.float_edit.as_ref().unwrap();
            assert_eq!(
                edit.target,
                STGFloatTarget::Unit {
                    unit: 0,
                    field: STGUnitFloatField::LeaderHPOverride,
                }
            );
        });
        cx.simulate_keystrokes("- 0 . 0 enter");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_float(
                        document,
                        STGFloatTarget::Unit {
                            unit: 0,
                            field: STGUnitFloatField::LeaderHPOverride,
                        },
                    )
                    .unwrap()
                    .to_bits(),
                (-0.0_f32).to_bits(),
            );
        });

        assert!(cx.debug_bounds("stg-unit-0-stat-0-replace").is_some());
        click(cx, "stg-unit-0-stat-0-replace");
        frame.update(cx, |frame, _| {
            let edit = frame.float_edit.as_ref().unwrap();
            assert_eq!(edit.editor.draft(), "");
            assert_eq!(edit.editor.source().to_bits(), f32::NAN.to_bits());
        });
        cx.simulate_keystrokes("escape");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame
                    .workspace
                    .stg_float(document, STGFloatTarget::StatOverride { unit: 0, slot: 0 })
                    .unwrap()
                    .to_bits(),
                f32::NAN.to_bits(),
            );
        });
        click(cx, "stg-unit-0-stat-0-replace");
        cx.simulate_keystrokes("2 . 5 enter");

        select_stg_section(&frame, cx, document, STGSection::Areas);
        click(cx, "stg-area-0-bound-BoundX1");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.float_edit.as_ref().unwrap().target,
                STGFloatTarget::Area {
                    area: 0,
                    field: STGAreaFloatField::BoundX1,
                },
            );
        });
        cx.simulate_keystrokes("escape");

        select_stg_section(&frame, cx, document, STGSection::Variables);
        click(cx, "stg-variable-master-row-1");
        click(cx, "stg-variable-1-value");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.float_edit.as_ref().unwrap().target,
                STGFloatTarget::Parameter {
                    value: STGValueTarget::VariableInitial { variable: 1 },
                },
            );
        });
        cx.simulate_keystrokes("escape");

        select_stg_section(&frame, cx, document, STGSection::Events);
        click(cx, "stg-parameter-0-0-condition-0-1");
        frame.update(cx, |frame, _| {
            assert_eq!(
                frame.float_edit.as_ref().unwrap().target,
                STGFloatTarget::Parameter {
                    value: STGValueTarget::ScriptParameter(STGParameterTarget {
                        script: STGScriptTarget {
                            block: 0,
                            event: 0,
                            kind: STGScriptKind::Condition,
                            script: 0,
                        },
                        parameter: 1,
                    }),
                },
            );
        });
    }

    #[gpui::test]
    fn stg_text_edit_uses_source_text_and_keeps_raw_replacement_explicit(cx: &mut TestAppContext) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_stg(&frame, cx, scalar_stg_fixture().bytes);

        click(cx, "stg-header-map-filename");
        let header_input = frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input.read(cx).content(), "Map");
            edit.input.clone()
        });
        header_input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("Café".to_owned()));
        });
        frame.update(cx, |frame, _| {
            assert!(matches!(
                frame
                    .workspace
                    .stg_text(
                        document,
                        STGTextTarget::Header(STGHeaderTextField::MapFilename),
                    )
                    .unwrap(),
                STGText::Decoded(value) if value.as_ref() == "Café"
            ));
        });

        select_stg_section(&frame, cx, document, STGSection::Units);
        click(cx, "stg-unit-0-name");
        let unit_input = frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input.read(cx).content(), "Unit");
            edit.input.clone()
        });
        unit_input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("수호자".to_owned()));
        });

        select_stg_section(&frame, cx, document, STGSection::Events);
        click(cx, "stg-parameter-0-0-action-0-0");
        frame.update(cx, |frame, cx| {
            assert_eq!(
                frame.text_edit.as_ref().unwrap().input.read(cx).content(),
                "action",
            );
        });
        cx.simulate_keystrokes("escape");

        let mut raw_fixture = scalar_stg_fixture();
        *raw_fixture
            .bytes
            .get_mut(raw_fixture.area_description)
            .unwrap() = 0x80;
        let raw_document = activate_stg(&frame, cx, raw_fixture.bytes);
        select_stg_section(&frame, cx, raw_document, STGSection::Areas);
        assert!(cx.debug_bounds("stg-area-0-description-replace").is_some());
        click(cx, "stg-area-0-description-replace");
        let replacement = frame.update(cx, |frame, cx| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input.read(cx).content(), "");
            edit.input.clone()
        });
        replacement.update(cx, |_, cx| cx.emit(TextInputEvent::Cancel));
        frame.update(cx, |frame, _| {
            assert!(matches!(
                frame
                    .workspace
                    .stg_text(raw_document, STGTextTarget::AreaDescription { area: 0 },)
                    .unwrap(),
                STGText::Raw(_)
            ));
        });
        click(cx, "stg-area-0-description-replace");
        let replacement = frame.update(cx, |frame, _| {
            frame.text_edit.as_ref().unwrap().input.clone()
        });
        replacement.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("Area repaired".to_owned()));
        });
        frame.update(cx, |frame, _| {
            assert!(matches!(
                frame
                    .workspace
                    .stg_text(
                        raw_document,
                        STGTextTarget::AreaDescription { area: 0 },
                    )
                    .unwrap(),
                STGText::Decoded(value) if value.as_ref() == "Area repaired"
            ));
        });
    }

    #[gpui::test]
    fn stg_draft_ownership_cancels_hidden_work_and_rebinds_catalog_refresh(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::text_input::bind);
        let (frame, cx) = cx.add_window_view(|_, cx| AppFrame::new(test_startup(), cx));
        let document = activate_stg(&frame, cx, scalar_stg_fixture().bytes);

        select_stg_section(&frame, cx, document, STGSection::Units);
        click(cx, "stg-unit-0-field-uniqueid");
        select_stg_section(&frame, cx, document, STGSection::Areas);
        frame.update(cx, |frame, _| assert!(frame.number_edit.is_none()));

        select_stg_section(&frame, cx, document, STGSection::Header);
        click(cx, "stg-header-map-filename");
        let (input, generation) = frame.update(cx, |frame, _| {
            let edit = frame.text_edit.as_ref().unwrap();
            (edit.input.clone(), edit.target.stg_generation().unwrap())
        });
        frame.update(cx, |frame, cx| {
            frame.reconcile_stg_presentation(document, STGDocumentTransition::Catalog, cx);
        });
        frame.update(cx, |frame, _| {
            let edit = frame.text_edit.as_ref().unwrap();
            assert_eq!(edit.input, input);
            assert!(edit.target.stg_generation().unwrap() > generation);
        });
        input.update(cx, |_, cx| cx.emit(TextInputEvent::Cancel));

        select_stg_section(&frame, cx, document, STGSection::Units);
        click(cx, "stg-unit-0-name");
        frame.update_in(cx, |frame, window, cx| {
            frame.start_stg_search(document, STGSearchKind::Units, window, cx);
            assert!(frame.text_edit.is_none());
        });
        let search = frame.update(cx, |frame, _| {
            frame.stg_search.as_ref().unwrap().input.clone()
        });
        search.update(cx, |_, cx| cx.emit(TextInputEvent::Commit(String::new())));

        click(cx, "stg-unit-0-field-uniqueid");
        frame.update(cx, |frame, cx| {
            frame
                .workspace
                .apply(
                    document,
                    DocumentEdit::SetSTGNumber {
                        target: STGNumberTarget::Unit {
                            unit: 0,
                            field: STGUnitField::EnabledFlag,
                        },
                        value: 1,
                    },
                )
                .unwrap();
            frame.reconcile_stg_presentation(document, STGDocumentTransition::ScalarEdit, cx);
            assert!(frame.number_edit.is_none());
        });

        click(cx, "stg-unit-0-field-uniqueid");
        frame.update(cx, |frame, cx| {
            let edit = STGStructuralEdit::InsertEvent {
                target: STGEventTarget { block: 0, event: 1 },
            };
            frame
                .workspace
                .apply(document, DocumentEdit::EditSTGStructure { edit })
                .unwrap();
            frame.reconcile_stg_presentation(
                document,
                STGDocumentTransition::StructuralEdit(Some(edit.change())),
                cx,
            );
            assert!(frame.number_edit.is_none());
        });

        let other = frame.update(cx, |frame, cx| {
            let other = frame.workspace.open_loaded(
                PathBuf::from("other.stg"),
                Document::STG(STGDocument::parse(scalar_stg_fixture().bytes).unwrap()),
            );
            frame.activate_document(other, cx);
            other
        });
        frame.update(cx, |frame, _| {
            assert_eq!(frame.active_document, Some(other));
            assert!(frame.number_edit.is_none());
            assert!(frame.text_edit.is_none());
            assert!(frame.float_edit.is_none());
        });
    }

    #[gpui::test]
    fn stg_virtual_keyboard_moves_a_typed_cursor_and_keeps_list_focus(cx: &mut TestAppContext) {
        let frame = cx.new(|cx| AppFrame::new(test_startup(), cx));
        let cx = cx.add_empty_window();
        let document = activate_stg(&frame, cx, unit_list_fixture(3));
        select_stg_section(&frame, cx, document, STGSection::Units);

        frame.update_in(cx, |frame, window, cx| {
            window.focus(&frame.stg_lists.units.focus);
            frame.move_stg_list_cursor(
                document,
                STGVirtualRowKind::Unit,
                STGListMovement::Down,
                window,
                cx,
            );

            assert_eq!(
                frame
                    .stg_presentations
                    .get(document)
                    .unwrap()
                    .inspected_unit(),
                Some(1)
            );
            assert!(frame.stg_lists.units.focus.is_focused(window));
        });

        draw_stg_frame(cx, &frame);
        assert!(cx.debug_bounds("stg-unit-master-row-1").is_some());
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

    fn activate_stg(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        bytes: Vec<u8>,
    ) -> DocumentID {
        frame.update(cx, |frame, cx| {
            let document = frame.workspace.open_loaded(
                PathBuf::from("mission.stg"),
                Document::STG(STGDocument::parse(bytes).unwrap()),
            );
            frame.activate_document(document, cx);
            frame.shell.select_area(Area::Files);
            cx.notify();
            document
        })
    }

    fn select_stg_section(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        document: DocumentID,
        section: STGSection,
    ) {
        frame.update(cx, |frame, cx| {
            frame.select_stg_section(document, section, cx);
        });
        draw_stg_frame(cx, frame);
    }

    fn draw_stg_frame(cx: &mut VisualTestContext, frame: &Entity<AppFrame>) {
        let frame = frame.clone();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(1_280.0), px(820.0)),
            move |_, _| frame,
        );
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing click target {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
    }

    #[test]
    fn stg_fixture_is_a_regular_file_image() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mission.stg");
        fs::write(&path, stg_fixture()).unwrap();
        assert!(path.is_file());
    }
}
