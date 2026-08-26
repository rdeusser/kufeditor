use gpui::{
    AnyElement, Context, Div, ElementId, Entity, ScrollStrategy, SharedString, div, prelude::*, px,
};
use kufeditor_workspace::{
    DocumentID, STGAbilityOwner, STGAreaField, STGAreaFloatField, STGEventTarget, STGFloatTarget,
    STGFooterField, STGHeaderTextField, STGNumberTarget, STGReferenceKind, STGScriptKind,
    STGScriptTarget, STGSkillField, STGSkillOwner, STGTailStatus, STGTextTarget, STGUnitField,
    STGUnitFloatField, STGUnitGroup, STGValue, STGValueTarget, WorkspaceError,
};

use super::AppFrame;
use crate::{
    actions::{
        MoveSTGListDown, MoveSTGListEnd, MoveSTGListHome, MoveSTGListPageDown, MoveSTGListPageUp,
        MoveSTGListUp,
    },
    crusaders_catalog_status::CrusadersCatalogStatus,
    notices::{Notice, NoticeSource},
    state::{
        STGDocumentTransition, STGIndexVisibility, STGReferenceVisibility, STGSection,
        STGSelection, STGVisibleSelections,
    },
    text_input::{TextInput, TextInputEvent},
    views::{
        save,
        stg::{
            self, STGDocumentProjection, STGEventBlockProjection, STGEventDetailField,
            STGEventDetailRow, STGEventDetailRows, STGEventRows, STGFieldProjection, STGFieldState,
            STGIndexRows, STGProjectionField, STGReferenceRows, STGRowCursor, STGRowLocation,
            STGSearchQuery, STGSearchRecord, STGSectionProjection, STGTailProjection,
            STGVirtualRowKind, STGVirtualRows,
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
        self.stg_search = None;
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
            STGSection::Header => self.stg_header_view(document).into_any_element(),
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
        let transition = self
            .stg_presentations
            .select_section(document, section, None);
        if transition.changed() {
            self.stg_lists.invalidate_all();
            cx.notify();
        }
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
    fn stg_header_view(&self, document: DocumentID) -> Div {
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
            |unit| self.stg_unit_details(document, unit),
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

    fn stg_unit_details(&self, document: DocumentID, unit: usize) -> AnyElement {
        let mut groups = vec![self.stg_unit_identity_group(document, unit)];
        groups.extend(self.stg_unit_number_groups(document, unit));
        groups.push(self.stg_unit_float_group(document, unit));
        groups.extend(self.stg_unit_skill_groups(document, unit));
        groups.extend(self.stg_unit_ability_groups(document, unit));
        groups.push(self.stg_unit_stat_override_group(document, unit));

        stg::scrolling_details(
            &self.theme,
            SharedString::from(format!("stg-unit-detail:{unit}")),
            groups,
        )
        .debug_selector(|| "stg-unit-detail".to_owned())
        .into_any_element()
    }

    fn stg_unit_identity_group(&self, document: DocumentID, unit: usize) -> AnyElement {
        stg::group(
            &self.theme,
            "SOURCE IDENTITY",
            vec![self.stg_field_element(
                &self.stg_text_field(
                    document,
                    STGSection::Units,
                    STGTextTarget::UnitName { unit },
                ),
                format!("stg-unit-{unit}-name"),
            )],
        )
        .into_any_element()
    }

    fn stg_unit_number_groups(&self, document: DocumentID, unit: usize) -> Vec<AnyElement> {
        let mut groups = Vec::new();
        for group in STGUnitGroup::ALL {
            let fields = STGUnitField::ALL
                .into_iter()
                .filter(|field| field.group() == group)
                .map(|field| {
                    self.stg_field_element(
                        &self.stg_number_field(
                            document,
                            STGSection::Units,
                            STGNumberTarget::Unit { unit, field },
                        ),
                        format!("stg-unit-{unit}-field-{}", unit_field_slug(field)),
                    )
                })
                .collect();
            groups.push(stg::group(&self.theme, group.label(), fields).into_any_element());
        }
        groups
    }

    fn stg_unit_float_group(&self, document: DocumentID, unit: usize) -> AnyElement {
        stg::group(
            &self.theme,
            "POSITION AND HP",
            STGUnitFloatField::ALL
                .into_iter()
                .map(|field| {
                    self.stg_field_element(
                        &self.stg_float_field(
                            document,
                            STGSection::Units,
                            STGFloatTarget::Unit { unit, field },
                        ),
                        format!("stg-unit-{unit}-float-{field:?}"),
                    )
                })
                .collect(),
        )
        .into_any_element()
    }

    fn stg_unit_skill_groups(&self, document: DocumentID, unit: usize) -> Vec<AnyElement> {
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

    fn stg_unit_ability_groups(&self, document: DocumentID, unit: usize) -> Vec<AnyElement> {
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
                            )
                        })
                        .collect(),
                )
                .into_any_element(),
            );
        }
        groups
    }

    fn stg_unit_stat_override_group(&self, document: DocumentID, unit: usize) -> AnyElement {
        stg::group(
            &self.theme,
            "STAT OVERRIDES · 22 SLOTS",
            (0..22)
                .map(|slot| {
                    self.stg_field_element(
                        &self.stg_float_field(
                            document,
                            STGSection::Units,
                            STGFloatTarget::StatOverride { unit, slot },
                        ),
                        format!("stg-unit-{unit}-stat-{slot}"),
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
            |area| self.stg_area_details(document, area),
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

    fn stg_area_details(&self, document: DocumentID, area: usize) -> AnyElement {
        let identity = vec![
            self.stg_field_element(
                &self.stg_text_field(
                    document,
                    STGSection::Areas,
                    STGTextTarget::AreaDescription { area },
                ),
                format!("stg-area-{area}-description"),
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
            |variable| self.stg_variable_details(document, variable),
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

    fn stg_variable_details(&self, document: DocumentID, variable: usize) -> AnyElement {
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
                        ),
                        self.stg_field_element(
                            &self.stg_number_field(
                                document,
                                STGSection::Variables,
                                STGNumberTarget::VariableID { variable },
                            ),
                            format!("stg-variable-{variable}-id"),
                        ),
                        self.stg_field_element(
                            &self.stg_value_field(
                                document,
                                STGSection::Variables,
                                value,
                                "Initial typed value",
                            ),
                            format!("stg-variable-{variable}-value"),
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
            |entry| self.stg_footer_details(document, entry),
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

    fn stg_footer_details(&self, document: DocumentID, entry: usize) -> AnyElement {
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

    fn stg_field_element(&self, field: &STGFieldProjection, selector: String) -> AnyElement {
        stg::field_row(&self.theme, field)
            .debug_selector(move || selector.clone())
            .into_any_element()
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

    fn stg_event_detail_row_projection(
        &self,
        document: DocumentID,
        event: STGEventTarget,
        row: STGEventDetailRow,
    ) -> STGProjectionResult<(String, String)> {
        match row {
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
        if self
            .stg_presentations
            .select(document, selection, None)
            .changed()
        {
            self.stg_lists.invalidate_all();
            cx.notify();
        }
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

    use std::{fs, path::PathBuf};

    use gpui::{
        AppContext, Context, Entity, EntityInputHandler, TestAppContext, VisualTestContext,
        WindowOptions, point, px, size,
    };
    use kufeditor_game::Game;
    use kufeditor_workspace::{
        Document, DocumentEdit, DocumentID, STGDocument, STGEventTarget, STGNumberTarget,
        STGParameterTarget, STGReferenceKind, STGScriptKind, STGScriptTarget, STGStructuralEdit,
        STGTextTarget, STGValueKind, STGValueTarget, TroopDocument,
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
            frame
                .stg_presentations
                .select_section(document, section, None);
            cx.notify();
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

    #[test]
    fn stg_fixture_is_a_regular_file_image() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mission.stg");
        fs::write(&path, stg_fixture()).unwrap();
        assert!(path.is_file());
    }
}
