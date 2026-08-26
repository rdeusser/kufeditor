use std::{collections::HashMap, ops::Range};

use kufeditor_game::Game;
use kufeditor_workspace::{
    DocumentID, STGEventTarget, STGFloatTarget, STGNumberTarget, STGParameterTarget,
    STGReferenceKind, STGScriptTarget, STGStructuralChange, STGTextTarget, SaveEquipmentSlot,
    SaveRosterField,
};

#[derive(Debug, Default)]
pub struct RecordSelections {
    records: HashMap<DocumentID, usize>,
}

impl RecordSelections {
    pub fn selected(&self, document: DocumentID) -> usize {
        self.records.get(&document).copied().unwrap_or(0)
    }

    pub fn select(&mut self, document: DocumentID, record: usize) {
        self.records.insert(document, record);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SaveSection {
    #[default]
    Summary,
    Units,
    Equipment,
    Roster,
    Missions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveUnitVisibility<'a> {
    All { unit_count: usize },
    Filtered(&'a [usize]),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveListKind {
    Units,
    Roster,
    SecondArray,
}

impl SaveListKind {
    pub const fn section(self) -> SaveSection {
        match self {
            Self::Units => SaveSection::Units,
            Self::Roster => SaveSection::Roster,
            Self::SecondArray => SaveSection::Missions,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveListCursor {
    Unit {
        source_index: usize,
    },
    Roster {
        record: usize,
        field: SaveRosterField,
    },
    SecondArray {
        record: usize,
    },
}

impl SaveListCursor {
    pub const fn kind(self) -> SaveListKind {
        match self {
            Self::Unit { .. } => SaveListKind::Units,
            Self::Roster { .. } => SaveListKind::Roster,
            Self::SecondArray { .. } => SaveListKind::SecondArray,
        }
    }

    pub const fn source_index(self) -> usize {
        match self {
            Self::Unit { source_index } => source_index,
            Self::Roster { record, .. } | Self::SecondArray { record } => record,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SaveUnitReconciliation {
    inspected_unit: usize,
    requested_visible: bool,
}

impl SaveUnitVisibility<'_> {
    fn reconcile(self, requested_unit: usize) -> SaveUnitReconciliation {
        match self {
            Self::All { unit_count } => SaveUnitReconciliation {
                inspected_unit: clamp_unit(requested_unit, unit_count),
                requested_visible: requested_unit < unit_count,
            },
            Self::Filtered(indices) => {
                let requested_visible = indices.contains(&requested_unit);
                SaveUnitReconciliation {
                    inspected_unit: if requested_visible {
                        requested_unit
                    } else {
                        indices.first().copied().unwrap_or(0)
                    },
                    requested_visible,
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavePresentationState {
    section: SaveSection,
    inspected_unit: usize,
    equipment_slot: SaveEquipmentSlot,
    player_only: bool,
    roster_record: usize,
    roster_field: SaveRosterField,
    second_array_record: usize,
}

impl SavePresentationState {
    pub const fn section(&self) -> SaveSection {
        self.section
    }

    pub const fn inspected_unit(&self) -> usize {
        self.inspected_unit
    }

    pub const fn equipment_slot(&self) -> SaveEquipmentSlot {
        self.equipment_slot
    }

    pub const fn player_only(&self) -> bool {
        self.player_only
    }

    pub const fn list_cursor(&self, kind: SaveListKind) -> SaveListCursor {
        match kind {
            SaveListKind::Units => SaveListCursor::Unit {
                source_index: self.inspected_unit,
            },
            SaveListKind::Roster => SaveListCursor::Roster {
                record: self.roster_record,
                field: self.roster_field,
            },
            SaveListKind::SecondArray => SaveListCursor::SecondArray {
                record: self.second_array_record,
            },
        }
    }
}

impl Default for SavePresentationState {
    fn default() -> Self {
        Self {
            section: SaveSection::Summary,
            inspected_unit: 0,
            equipment_slot: SaveEquipmentSlot::LeaderWeapon,
            player_only: false,
            roster_record: 0,
            roster_field: SaveRosterField::Byte60,
            second_array_record: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SavePresentationTransition {
    Unchanged,
    Changed,
    ChangedAndCancelDraft,
}

impl SavePresentationTransition {
    pub const fn cancels_draft(self) -> bool {
        matches!(self, Self::ChangedAndCancelDraft)
    }

    pub const fn changed(self) -> bool {
        !matches!(self, Self::Unchanged)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SavePresentationStates {
    documents: HashMap<DocumentID, SavePresentationState>,
    active_document: Option<DocumentID>,
}

impl SavePresentationStates {
    pub fn get(&self, document: DocumentID) -> Option<&SavePresentationState> {
        self.documents.get(&document)
    }

    pub fn activate_document(
        &mut self,
        document: DocumentID,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let reconciliation = self.reconcile_document(document, visibility, draft_active);
        if self.active_document == Some(document) {
            return reconciliation;
        }
        self.active_document = Some(document);
        changed_transition(draft_active)
    }

    pub fn reconcile_document(
        &mut self,
        document: DocumentID,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        let reconciliation = visibility.reconcile(state.inspected_unit);
        let transition = unit_reconciliation_transition(
            state.inspected_unit,
            reconciliation,
            false,
            draft_active,
        );
        state.inspected_unit = reconciliation.inspected_unit;
        transition
    }

    pub fn select_section(
        &mut self,
        document: DocumentID,
        section: SaveSection,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        if state.section == section {
            return SavePresentationTransition::Unchanged;
        }
        state.section = section;
        changed_transition(draft_active)
    }

    pub fn select_equipment_slot(
        &mut self,
        document: DocumentID,
        slot: SaveEquipmentSlot,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        if state.equipment_slot == slot {
            return SavePresentationTransition::Unchanged;
        }
        state.equipment_slot = slot;
        changed_transition(draft_active)
    }

    pub fn set_player_only(
        &mut self,
        document: DocumentID,
        player_only: bool,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        let reconciliation = visibility.reconcile(state.inspected_unit);
        let transition = unit_reconciliation_transition(
            state.inspected_unit,
            reconciliation,
            state.player_only != player_only,
            draft_active,
        );
        state.player_only = player_only;
        state.inspected_unit = reconciliation.inspected_unit;
        transition
    }

    pub fn set_list_cursor(
        &mut self,
        document: DocumentID,
        cursor: SaveListCursor,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        if state.list_cursor(cursor.kind()) == cursor {
            return SavePresentationTransition::Unchanged;
        }
        match cursor {
            SaveListCursor::Unit { source_index } => state.inspected_unit = source_index,
            SaveListCursor::Roster { record, field } => {
                state.roster_record = record;
                state.roster_field = field;
            }
            SaveListCursor::SecondArray { record } => state.second_array_record = record,
        }
        changed_transition(draft_active)
    }

    pub fn reconcile_list_cursor(
        &mut self,
        document: DocumentID,
        kind: SaveListKind,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        if kind == SaveListKind::Units {
            return self.reconcile_document(document, visibility, draft_active);
        }
        let state = self.documents.entry(document).or_default();
        let previous = state.list_cursor(kind).source_index();
        let reconciliation = visibility.reconcile(previous);
        let transition =
            unit_reconciliation_transition(previous, reconciliation, false, draft_active);
        match kind {
            SaveListKind::Units => state.inspected_unit = reconciliation.inspected_unit,
            SaveListKind::Roster => state.roster_record = reconciliation.inspected_unit,
            SaveListKind::SecondArray => {
                state.second_array_record = reconciliation.inspected_unit;
            }
        }
        transition
    }

    #[cfg(test)]
    pub fn remove_document(
        &mut self,
        document: DocumentID,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let was_active = self.active_document == Some(document);
        if was_active {
            self.active_document = None;
        }
        if self.documents.remove(&document).is_some() || was_active {
            changed_transition(draft_active)
        } else {
            SavePresentationTransition::Unchanged
        }
    }
}

const fn clamp_unit(unit: usize, unit_count: usize) -> usize {
    if unit_count == 0 {
        0
    } else if unit >= unit_count {
        unit_count - 1
    } else {
        unit
    }
}

const fn changed_transition(draft_active: bool) -> SavePresentationTransition {
    if draft_active {
        SavePresentationTransition::ChangedAndCancelDraft
    } else {
        SavePresentationTransition::Changed
    }
}

const fn unit_reconciliation_transition(
    previous_unit: usize,
    reconciliation: SaveUnitReconciliation,
    presentation_changed: bool,
    draft_active: bool,
) -> SavePresentationTransition {
    if previous_unit != reconciliation.inspected_unit {
        changed_transition(draft_active)
    } else if draft_active && !reconciliation.requested_visible {
        SavePresentationTransition::ChangedAndCancelDraft
    } else if presentation_changed {
        SavePresentationTransition::Changed
    } else {
        SavePresentationTransition::Unchanged
    }
}

#[allow(
    dead_code,
    reason = "all STG sections become active when the structured renderer is connected"
)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum STGSection {
    #[default]
    Header,
    Units,
    Areas,
    Variables,
    Events,
    Footer,
}

#[allow(
    dead_code,
    reason = "the complete section rail consumes this metadata in the structured renderer"
)]
impl STGSection {
    pub const ALL: [Self; 6] = [
        Self::Header,
        Self::Units,
        Self::Areas,
        Self::Variables,
        Self::Events,
        Self::Footer,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Header => "Header",
            Self::Units => "Units",
            Self::Areas => "Areas",
            Self::Variables => "Variables",
            Self::Events => "Events",
            Self::Footer => "Footer",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGIndexVisibility<'a> {
    Range(Range<usize>),
    Sparse(&'a [usize]),
}

impl STGIndexVisibility<'_> {
    fn first(&self) -> Option<usize> {
        match self {
            Self::Range(range) => (range.start < range.end).then_some(range.start),
            Self::Sparse(indices) => indices.first().copied(),
        }
    }

    fn contains(&self, index: usize) -> bool {
        match self {
            Self::Range(range) => range.contains(&index),
            Self::Sparse(indices) => indices.contains(&index),
        }
    }

    fn reconcile(&self, selected: Option<usize>) -> Option<usize> {
        selected
            .filter(|index| self.contains(*index))
            .or_else(|| self.first())
    }

    fn reconcile_after_structure(&self, selected: Option<usize>) -> Option<usize> {
        if selected.is_some_and(|index| self.contains(index)) {
            return selected;
        }
        match self {
            Self::Range(range) => {
                let last = range.end.checked_sub(1)?;
                Some(selected.unwrap_or(range.start).clamp(range.start, last))
            }
            Self::Sparse(indices) => selected.map_or_else(
                || indices.first().copied(),
                |selected| {
                    indices
                        .iter()
                        .find(|index| **index > selected)
                        .or_else(|| indices.last())
                        .copied()
                },
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGEventBlockRange {
    block: usize,
    event_count: usize,
}

impl STGEventBlockRange {
    pub const fn new(block: usize, event_count: usize) -> Self {
        Self { block, event_count }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGEventVisibility<'a> {
    Blocks(&'a [STGEventBlockRange]),
    Filtered(&'a [STGEventTarget]),
}

impl STGEventVisibility<'_> {
    fn first(&self) -> Option<STGEventTarget> {
        match self {
            Self::Blocks(blocks) => {
                blocks
                    .iter()
                    .find(|block| block.event_count > 0)
                    .map(|block| STGEventTarget {
                        block: block.block,
                        event: 0,
                    })
            }
            Self::Filtered(targets) => targets.first().copied(),
        }
    }

    fn contains(&self, target: STGEventTarget) -> bool {
        match self {
            Self::Blocks(blocks) => blocks
                .iter()
                .any(|block| block.block == target.block && target.event < block.event_count),
            Self::Filtered(targets) => targets.contains(&target),
        }
    }

    fn reconcile(&self, selected: Option<STGEventTarget>) -> Option<STGEventTarget> {
        selected
            .filter(|target| self.contains(*target))
            .or_else(|| self.first())
    }

    fn reconcile_after_structure(
        &self,
        selected: Option<STGEventTarget>,
    ) -> Option<STGEventTarget> {
        if selected.is_some_and(|target| self.contains(target)) {
            return selected;
        }
        let Some(selected) = selected else {
            return self.first();
        };
        let blocks = match self {
            Self::Blocks(blocks) => blocks,
            Self::Filtered(targets) => {
                return targets
                    .iter()
                    .find(|target| event_follows(**target, selected))
                    .or_else(|| targets.last())
                    .copied();
            }
        };
        if let Some(block) = blocks
            .iter()
            .find(|block| block.block == selected.block && block.event_count > 0)
        {
            return Some(STGEventTarget {
                block: block.block,
                event: selected.event.min(block.event_count - 1),
            });
        }
        if let Some(block) = blocks
            .iter()
            .find(|block| block.block > selected.block && block.event_count > 0)
        {
            return Some(STGEventTarget {
                block: block.block,
                event: 0,
            });
        }
        blocks
            .iter()
            .rev()
            .find(|block| block.block < selected.block && block.event_count > 0)
            .map(|block| STGEventTarget {
                block: block.block,
                event: block.event_count - 1,
            })
    }
}

const fn event_follows(candidate: STGEventTarget, selected: STGEventTarget) -> bool {
    candidate.block > selected.block
        || (candidate.block == selected.block && candidate.event > selected.event)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGReferenceCursor {
    Index(usize),
    Event(STGEventTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGReferenceVisibility<'a> {
    Indices(STGIndexVisibility<'a>),
    Events(STGEventVisibility<'a>),
}

impl STGReferenceVisibility<'_> {
    pub fn empty() -> STGReferenceVisibility<'static> {
        STGReferenceVisibility::Indices(STGIndexVisibility::Range(0..0))
    }

    fn reconcile(&self, cursor: Option<STGReferenceCursor>) -> Option<STGReferenceCursor> {
        match self {
            Self::Indices(visible) => visible
                .reconcile(match cursor {
                    Some(STGReferenceCursor::Index(index)) => Some(index),
                    Some(STGReferenceCursor::Event(_)) | None => None,
                })
                .map(STGReferenceCursor::Index),
            Self::Events(visible) => visible
                .reconcile(match cursor {
                    Some(STGReferenceCursor::Event(target)) => Some(target),
                    Some(STGReferenceCursor::Index(_)) | None => None,
                })
                .map(STGReferenceCursor::Event),
        }
    }

    fn reconcile_after_structure(
        &self,
        cursor: Option<STGReferenceCursor>,
    ) -> Option<STGReferenceCursor> {
        match self {
            Self::Indices(visible) => visible
                .reconcile_after_structure(match cursor {
                    Some(STGReferenceCursor::Index(index)) => Some(index),
                    Some(STGReferenceCursor::Event(_)) | None => None,
                })
                .map(STGReferenceCursor::Index),
            Self::Events(visible) => visible
                .reconcile_after_structure(match cursor {
                    Some(STGReferenceCursor::Event(target)) => Some(target),
                    Some(STGReferenceCursor::Index(_)) | None => None,
                })
                .map(STGReferenceCursor::Event),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGVisibleSelections<'a> {
    units: STGIndexVisibility<'a>,
    areas: STGIndexVisibility<'a>,
    variables: STGIndexVisibility<'a>,
    events: STGEventVisibility<'a>,
    footer: STGIndexVisibility<'a>,
}

impl<'a> STGVisibleSelections<'a> {
    pub fn new(
        units: STGIndexVisibility<'a>,
        areas: STGIndexVisibility<'a>,
        variables: STGIndexVisibility<'a>,
        events: STGEventVisibility<'a>,
        footer: STGIndexVisibility<'a>,
    ) -> Self {
        Self {
            units,
            areas,
            variables,
            events,
            footer,
        }
    }

    #[cfg(test)]
    pub fn empty() -> STGVisibleSelections<'static> {
        STGVisibleSelections {
            units: STGIndexVisibility::Range(0..0),
            areas: STGIndexVisibility::Range(0..0),
            variables: STGIndexVisibility::Range(0..0),
            events: STGEventVisibility::Blocks(&[]),
            footer: STGIndexVisibility::Range(0..0),
        }
    }
}

#[allow(
    dead_code,
    reason = "typed STG selections are consumed by the structured renderer"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGSelection {
    Unit(Option<usize>),
    Area(Option<usize>),
    Variable(Option<usize>),
    Event(Option<STGEventTarget>),
    Footer(Option<usize>),
}

#[allow(
    dead_code,
    reason = "every typed binding path is consumed by STG virtual controls"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGBindingPath {
    Header,
    Unit { unit: usize },
    Area { area: usize },
    Variable { variable: usize },
    Event(STGEventTarget),
    Script(STGScriptTarget),
    Parameter(STGParameterTarget),
    Footer { entry: usize },
    Reference { kind: STGReferenceKind },
}

#[allow(
    dead_code,
    reason = "binding cursors are consumed by STG virtual keyboard controls"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGBindingCursor {
    document: DocumentID,
    section: STGSection,
    generation: u64,
    path: STGBindingPath,
    source_index: usize,
}

#[allow(
    dead_code,
    reason = "binding cursor accessors are consumed by STG virtual keyboard controls"
)]
impl STGBindingCursor {
    pub const fn document(self) -> DocumentID {
        self.document
    }

    pub const fn section(self) -> STGSection {
        self.section
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn path(self) -> STGBindingPath {
        self.path
    }

    pub const fn source_index(self) -> usize {
        self.source_index
    }
}

#[allow(
    dead_code,
    reason = "number and float draft targets are consumed by STG scalar controls"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGDraftTarget {
    Number(STGNumberTarget),
    Float(STGFloatTarget),
    Text(STGTextTarget),
}

#[allow(
    dead_code,
    reason = "draft bindings are consumed by STG scalar controls"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGDraftBinding {
    document: DocumentID,
    section: STGSection,
    target: STGDraftTarget,
}

#[allow(
    dead_code,
    reason = "draft binding accessors are consumed by STG scalar controls"
)]
impl STGDraftBinding {
    pub const fn new(document: DocumentID, section: STGSection, target: STGDraftTarget) -> Self {
        Self {
            document,
            section,
            target,
        }
    }

    pub const fn document(self) -> DocumentID {
        self.document
    }

    pub const fn section(self) -> STGSection {
        self.section
    }

    pub const fn target(self) -> STGDraftTarget {
        self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGDraftStatus {
    binding: STGDraftBinding,
    visible: bool,
}

#[allow(
    dead_code,
    reason = "draft visibility is consumed by STG scalar controls"
)]
impl STGDraftStatus {
    pub const fn visible(binding: STGDraftBinding) -> Self {
        Self {
            binding,
            visible: true,
        }
    }

    pub const fn hidden(binding: STGDraftBinding) -> Self {
        Self {
            binding,
            visible: false,
        }
    }

    fn remains_visible(self, document: DocumentID, section: STGSection) -> bool {
        self.visible && self.binding.document == document && self.binding.section == section
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGReferencePickerState {
    target: STGParameterTarget,
    kind: STGReferenceKind,
    query: String,
    cursor: Option<STGReferenceCursor>,
}

#[allow(
    dead_code,
    reason = "reference query controls consume the complete picker state"
)]
impl STGReferencePickerState {
    pub fn new(
        target: STGParameterTarget,
        kind: STGReferenceKind,
        query: String,
        cursor: Option<STGReferenceCursor>,
    ) -> Self {
        Self {
            target,
            kind,
            query,
            cursor,
        }
    }

    pub const fn target(&self) -> STGParameterTarget {
        self.target
    }

    pub const fn kind(&self) -> STGReferenceKind {
        self.kind
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub const fn cursor(&self) -> Option<STGReferenceCursor> {
        self.cursor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGPresentationState {
    section: STGSection,
    inspected_unit: Option<usize>,
    inspected_area: Option<usize>,
    inspected_variable: Option<usize>,
    inspected_event: Option<STGEventTarget>,
    inspected_footer: Option<usize>,
    expanded_script: Option<STGScriptTarget>,
    unit_query: String,
    event_query: String,
    binding_generation: u64,
    reference_picker: Option<STGReferencePickerState>,
}

#[allow(
    dead_code,
    reason = "the structured renderer consumes every document-scoped selection"
)]
impl STGPresentationState {
    pub const fn section(&self) -> STGSection {
        self.section
    }

    pub const fn inspected_unit(&self) -> Option<usize> {
        self.inspected_unit
    }

    pub const fn inspected_area(&self) -> Option<usize> {
        self.inspected_area
    }

    pub const fn inspected_variable(&self) -> Option<usize> {
        self.inspected_variable
    }

    pub const fn inspected_event(&self) -> Option<STGEventTarget> {
        self.inspected_event
    }

    pub const fn inspected_footer(&self) -> Option<usize> {
        self.inspected_footer
    }

    pub const fn expanded_script(&self) -> Option<STGScriptTarget> {
        self.expanded_script
    }

    pub fn unit_query(&self) -> &str {
        &self.unit_query
    }

    pub fn event_query(&self) -> &str {
        &self.event_query
    }

    pub const fn binding_generation(&self) -> u64 {
        self.binding_generation
    }

    pub const fn reference_picker(&self) -> Option<&STGReferencePickerState> {
        self.reference_picker.as_ref()
    }

    fn reconcile_selections(
        &mut self,
        visible: &STGVisibleSelections<'_>,
        after_structure: bool,
    ) -> bool {
        let reconcile_index = |visibility: &STGIndexVisibility<'_>, selected| {
            if after_structure {
                visibility.reconcile_after_structure(selected)
            } else {
                visibility.reconcile(selected)
            }
        };
        let unit = reconcile_index(&visible.units, self.inspected_unit);
        let area = reconcile_index(&visible.areas, self.inspected_area);
        let variable = reconcile_index(&visible.variables, self.inspected_variable);
        let event = if after_structure {
            visible
                .events
                .reconcile_after_structure(self.inspected_event)
        } else {
            visible.events.reconcile(self.inspected_event)
        };
        let footer = reconcile_index(&visible.footer, self.inspected_footer);
        let changed = self.inspected_unit != unit
            || self.inspected_area != area
            || self.inspected_variable != variable
            || self.inspected_event != event
            || self.inspected_footer != footer;
        self.inspected_unit = unit;
        self.inspected_area = area;
        self.inspected_variable = variable;
        self.inspected_event = event;
        self.inspected_footer = footer;
        changed
    }

    fn select(&mut self, selection: STGSelection) -> bool {
        match selection {
            STGSelection::Unit(selected) => replace_if_changed(&mut self.inspected_unit, selected),
            STGSelection::Area(selected) => replace_if_changed(&mut self.inspected_area, selected),
            STGSelection::Variable(selected) => {
                replace_if_changed(&mut self.inspected_variable, selected)
            }
            STGSelection::Event(selected) => {
                let changed = replace_if_changed(&mut self.inspected_event, selected);
                if changed {
                    self.expanded_script = None;
                    self.reference_picker = None;
                }
                changed
            }
            STGSelection::Footer(selected) => {
                replace_if_changed(&mut self.inspected_footer, selected)
            }
        }
    }

    fn remap_after_structure(&mut self, change: STGStructuralChange) -> bool {
        let mut changed = false;
        if let Some(selected) = self.inspected_event {
            changed |= replace_if_changed(
                &mut self.inspected_event,
                Some(remap_selected_event(selected, change)),
            );
        }
        let expanded = self
            .expanded_script
            .and_then(|target| remap_script(target, change));
        changed |= replace_if_changed(&mut self.expanded_script, expanded);
        let picker = self.reference_picker.take().and_then(|mut picker| {
            let script = remap_script(picker.target.script, change)?;
            if self.expanded_script != Some(script) {
                return None;
            }
            picker.target.script = script;
            if let Some(STGReferenceCursor::Event(target)) = picker.cursor {
                picker.cursor = Some(STGReferenceCursor::Event(remap_selected_event(
                    target, change,
                )));
            }
            Some(picker)
        });
        changed |= replace_if_changed(&mut self.reference_picker, picker);
        changed
    }
}

fn remap_selected_event(
    mut selected: STGEventTarget,
    change: STGStructuralChange,
) -> STGEventTarget {
    match change {
        STGStructuralChange::InsertEvent { target }
            if selected.block == target.block && selected.event >= target.event =>
        {
            selected.event = selected.event.saturating_add(1);
        }
        STGStructuralChange::RemoveEvent { target }
            if selected.block == target.block && selected.event > target.event =>
        {
            selected.event = selected.event.saturating_sub(1);
        }
        STGStructuralChange::InsertEvent { .. }
        | STGStructuralChange::RemoveEvent { .. }
        | STGStructuralChange::InsertScript { .. }
        | STGStructuralChange::RemoveScript { .. }
        | STGStructuralChange::ReplaceScript { .. }
        | STGStructuralChange::ReplaceValue { .. } => {}
    }
    selected
}

fn remap_script(
    mut selected: STGScriptTarget,
    change: STGStructuralChange,
) -> Option<STGScriptTarget> {
    match change {
        STGStructuralChange::InsertEvent { target }
            if selected.block == target.block && selected.event >= target.event =>
        {
            selected.event = selected.event.saturating_add(1);
        }
        STGStructuralChange::RemoveEvent { target }
            if selected.block == target.block && selected.event == target.event =>
        {
            return None;
        }
        STGStructuralChange::RemoveEvent { target }
            if selected.block == target.block && selected.event > target.event =>
        {
            selected.event = selected.event.saturating_sub(1);
        }
        STGStructuralChange::InsertScript { target }
            if same_script_collection(selected, target) && selected.script >= target.script =>
        {
            selected.script = selected.script.saturating_add(1);
        }
        STGStructuralChange::RemoveScript { target }
            if same_script_collection(selected, target) && selected.script == target.script =>
        {
            return None;
        }
        STGStructuralChange::RemoveScript { target }
            if same_script_collection(selected, target) && selected.script > target.script =>
        {
            selected.script = selected.script.saturating_sub(1);
        }
        STGStructuralChange::InsertEvent { .. }
        | STGStructuralChange::RemoveEvent { .. }
        | STGStructuralChange::InsertScript { .. }
        | STGStructuralChange::RemoveScript { .. }
        | STGStructuralChange::ReplaceScript { .. }
        | STGStructuralChange::ReplaceValue { .. } => {}
    }
    Some(selected)
}

fn same_script_collection(first: STGScriptTarget, second: STGScriptTarget) -> bool {
    first.block == second.block && first.event == second.event && first.kind == second.kind
}

impl Default for STGPresentationState {
    fn default() -> Self {
        Self {
            section: STGSection::Header,
            inspected_unit: None,
            inspected_area: None,
            inspected_variable: None,
            inspected_event: None,
            inspected_footer: None,
            expanded_script: None,
            unit_query: String::new(),
            event_query: String::new(),
            binding_generation: 0,
            reference_picker: None,
        }
    }
}

fn replace_if_changed<T: Eq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        false
    } else {
        *slot = value;
        true
    }
}

#[allow(
    dead_code,
    reason = "later STG edit paths identify their exact reconciliation cause"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGDocumentTransition {
    Unchanged,
    ScalarEdit,
    StructuralEdit(Option<STGStructuralChange>),
    Undo(Option<STGStructuralChange>),
    Redo(Option<STGStructuralChange>),
    Catalog,
}

impl STGDocumentTransition {
    const fn changed(self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    const fn structural_change(self) -> Option<STGStructuralChange> {
        match self {
            Self::StructuralEdit(change) | Self::Undo(change) | Self::Redo(change) => change,
            Self::Unchanged | Self::ScalarEdit | Self::Catalog => None,
        }
    }

    pub(crate) fn remap_script_target(self, target: STGScriptTarget) -> Option<STGScriptTarget> {
        match self {
            Self::StructuralEdit(Some(change))
            | Self::Undo(Some(change))
            | Self::Redo(Some(change)) => remap_script(target, change),
            Self::StructuralEdit(None) => None,
            Self::Undo(None)
            | Self::Redo(None)
            | Self::Unchanged
            | Self::ScalarEdit
            | Self::Catalog => Some(target),
        }
    }

    pub(crate) const fn inserted_event_target(self) -> Option<STGEventTarget> {
        match self {
            Self::StructuralEdit(Some(STGStructuralChange::InsertEvent { target })) => Some(target),
            Self::StructuralEdit(_)
            | Self::Undo(_)
            | Self::Redo(_)
            | Self::Unchanged
            | Self::ScalarEdit
            | Self::Catalog => None,
        }
    }

    pub(crate) const fn inserted_script_target(self) -> Option<STGScriptTarget> {
        match self {
            Self::StructuralEdit(Some(STGStructuralChange::InsertScript { target })) => {
                Some(target)
            }
            Self::StructuralEdit(_)
            | Self::Undo(_)
            | Self::Redo(_)
            | Self::Unchanged
            | Self::ScalarEdit
            | Self::Catalog => None,
        }
    }

    const fn may_change_structure(self) -> bool {
        matches!(
            self,
            Self::StructuralEdit(_) | Self::Undo(Some(_)) | Self::Redo(Some(_))
        )
    }

    const fn invalidates_script_identity(self) -> bool {
        matches!(self, Self::StructuralEdit(None))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGPresentationTransition {
    Unchanged,
    Changed { generation: u64, cancel_draft: bool },
}

#[allow(
    dead_code,
    reason = "STG draft owners consume cancellation and generation details"
)]
impl STGPresentationTransition {
    pub const fn changed(self) -> bool {
        matches!(self, Self::Changed { .. })
    }

    pub const fn cancels_draft(self) -> bool {
        matches!(
            self,
            Self::Changed {
                cancel_draft: true,
                ..
            }
        )
    }

    pub const fn generation(self) -> Option<u64> {
        match self {
            Self::Unchanged => None,
            Self::Changed { generation, .. } => Some(generation),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct STGPresentationStates {
    documents: HashMap<DocumentID, STGPresentationState>,
    active_document: Option<DocumentID>,
    next_binding_generation: u64,
}

#[allow(
    dead_code,
    reason = "STG controls consume selection and stale-binding transitions incrementally"
)]
impl STGPresentationStates {
    pub fn get(&self, document: DocumentID) -> Option<&STGPresentationState> {
        self.documents.get(&document)
    }

    pub fn activate_document(
        &mut self,
        document: DocumentID,
        visible: &STGVisibleSelections<'_>,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        let lifetime_changed = self.active_document != Some(document);
        self.active_document = Some(document);
        self.update_document(document, draft, lifetime_changed, |state| {
            state.reconcile_selections(visible, false)
        })
    }

    pub fn deactivate_active_document(
        &mut self,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        let Some(document) = self.active_document.take() else {
            return STGPresentationTransition::Unchanged;
        };
        let generation = self.advance_generation();
        self.documents
            .entry(document)
            .or_default()
            .binding_generation = generation;
        STGPresentationTransition::Changed {
            generation,
            cancel_draft: draft.is_some_and(|draft| draft.binding.document == document),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the reconciliation boundary validates every independent STG binding family"
    )]
    pub fn reconcile_document(
        &mut self,
        document: DocumentID,
        visible: &STGVisibleSelections<'_>,
        visible_scripts: &[STGScriptTarget],
        reference_picker_visible: bool,
        visible_references: &STGReferenceVisibility<'_>,
        transition: STGDocumentTransition,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        let after_structure = transition.may_change_structure();
        let structural_change = transition.structural_change();
        let inserted_event = transition.inserted_event_target();
        let inserted_script = transition.inserted_script_target();
        let invalidate_script_identity = transition.invalidates_script_identity();
        self.update_document(document, draft, transition.changed(), |state| {
            let mut changed =
                structural_change.is_some_and(|change| state.remap_after_structure(change));
            if invalidate_script_identity {
                changed |= replace_if_changed(&mut state.expanded_script, None);
                changed |= replace_if_changed(&mut state.reference_picker, None);
            }
            changed |= state.reconcile_selections(visible, after_structure);
            if state.expanded_script.is_some_and(|target| {
                !visible_scripts.contains(&target)
                    || state.inspected_event.is_none_or(|event| {
                        event.block != target.block || event.event != target.event
                    })
            }) {
                state.expanded_script = None;
                changed = true;
            }
            let picker_rebound = state
                .reference_picker
                .as_ref()
                .is_some_and(|picker| state.expanded_script != Some(picker.target.script));
            match state.reference_picker.as_mut() {
                Some(_) if !reference_picker_visible || picker_rebound => {
                    state.reference_picker = None;
                    changed = true;
                }
                Some(picker) => {
                    let cursor = if after_structure {
                        visible_references.reconcile_after_structure(picker.cursor)
                    } else {
                        visible_references.reconcile(picker.cursor)
                    };
                    changed |= replace_if_changed(&mut picker.cursor, cursor);
                }
                None => {}
            }
            if let Some(target) = inserted_event {
                changed |= state.select(STGSelection::Event(Some(target)));
            }
            if let Some(target) = inserted_script {
                changed |= replace_if_changed(&mut state.expanded_script, Some(target));
                if state
                    .reference_picker
                    .as_ref()
                    .is_some_and(|picker| picker.target.script != target)
                {
                    state.reference_picker = None;
                    changed = true;
                }
            }
            changed
        })
    }

    pub fn select_section(
        &mut self,
        document: DocumentID,
        section: STGSection,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        self.update_document(document, draft, false, |state| {
            replace_if_changed(&mut state.section, section)
        })
    }

    pub fn select(
        &mut self,
        document: DocumentID,
        selection: STGSelection,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        self.update_document(document, draft, false, |state| state.select(selection))
    }

    pub fn set_unit_query(
        &mut self,
        document: DocumentID,
        query: String,
        visible: &STGIndexVisibility<'_>,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        self.update_document(document, draft, false, |state| {
            let mut changed = replace_if_changed(&mut state.unit_query, query);
            let selected = visible.reconcile(state.inspected_unit);
            changed |= replace_if_changed(&mut state.inspected_unit, selected);
            changed
        })
    }

    pub fn set_event_query(
        &mut self,
        document: DocumentID,
        query: String,
        visible: &STGEventVisibility<'_>,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        self.update_document(document, draft, false, |state| {
            let mut changed = replace_if_changed(&mut state.event_query, query);
            let selected = visible.reconcile(state.inspected_event);
            if replace_if_changed(&mut state.inspected_event, selected) {
                state.expanded_script = None;
                state.reference_picker = None;
                changed = true;
            }
            changed
        })
    }

    pub fn set_expanded_script(
        &mut self,
        document: DocumentID,
        target: Option<STGScriptTarget>,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        self.update_document(document, draft, false, |state| {
            let changed = replace_if_changed(&mut state.expanded_script, target);
            if changed
                && state
                    .reference_picker
                    .as_ref()
                    .is_some_and(|picker| Some(picker.target.script) != target)
            {
                state.reference_picker = None;
            }
            changed
        })
    }

    pub fn set_reference_picker(
        &mut self,
        document: DocumentID,
        picker: Option<STGReferencePickerState>,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        self.update_document(document, draft, false, |state| {
            replace_if_changed(&mut state.reference_picker, picker)
        })
    }

    pub fn set_reference_cursor(
        &mut self,
        document: DocumentID,
        target: STGParameterTarget,
        kind: STGReferenceKind,
        cursor: Option<STGReferenceCursor>,
    ) -> bool {
        let Some(state) = self.documents.get_mut(&document) else {
            return false;
        };
        let Some(picker) = state
            .reference_picker
            .as_mut()
            .filter(|picker| picker.target == target && picker.kind == kind)
        else {
            return false;
        };
        replace_if_changed(&mut picker.cursor, cursor)
    }

    pub fn binding_cursor(
        &self,
        document: DocumentID,
        path: STGBindingPath,
        source_index: usize,
    ) -> Option<STGBindingCursor> {
        if self.active_document != Some(document) {
            return None;
        }
        let state = self.documents.get(&document)?;
        Some(STGBindingCursor {
            document,
            section: state.section,
            generation: state.binding_generation,
            path,
            source_index,
        })
    }

    pub fn accepts_binding_cursor(&self, cursor: STGBindingCursor) -> bool {
        self.active_document == Some(cursor.document)
            && self.documents.get(&cursor.document).is_some_and(|state| {
                state.section == cursor.section && state.binding_generation == cursor.generation
            })
    }

    pub fn remove_document(
        &mut self,
        document: DocumentID,
        draft: Option<STGDraftStatus>,
    ) -> STGPresentationTransition {
        let was_active = self.active_document == Some(document);
        if was_active {
            self.active_document = None;
        }
        if self.documents.remove(&document).is_none() && !was_active {
            return STGPresentationTransition::Unchanged;
        }
        let generation = self.advance_generation();
        STGPresentationTransition::Changed {
            generation,
            cancel_draft: draft.is_some_and(|draft| draft.binding.document == document),
        }
    }

    fn update_document(
        &mut self,
        document: DocumentID,
        draft: Option<STGDraftStatus>,
        force_changed: bool,
        update: impl FnOnce(&mut STGPresentationState) -> bool,
    ) -> STGPresentationTransition {
        let (changed, section) = {
            let state = self.documents.entry(document).or_default();
            (update(state) || force_changed, state.section)
        };
        if !changed {
            return STGPresentationTransition::Unchanged;
        }

        let generation = self.advance_generation();
        self.documents
            .entry(document)
            .or_default()
            .binding_generation = generation;
        STGPresentationTransition::Changed {
            generation,
            cancel_draft: draft.is_some_and(|draft| !draft.remains_visible(document, section)),
        }
    }

    fn advance_generation(&mut self) -> u64 {
        self.next_binding_generation = self.next_binding_generation.wrapping_add(1);
        self.next_binding_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestID(u64);

impl RequestID {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrusadersCatalogRequestID(u64);

impl CrusadersCatalogRequestID {
    #[cfg(test)]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosePolicy {
    Allow,
    PromptForUnsaved { count: usize },
}

impl ClosePolicy {
    pub const fn from_dirty_count(count: usize) -> Self {
        if count == 0 {
            Self::Allow
        } else {
            Self::PromptForUnsaved { count }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Area {
    #[default]
    Home,
    Files,
    Mods,
    Patches,
    Settings,
}

impl Area {
    pub const PRIMARY: [Self; 4] = [Self::Home, Self::Files, Self::Mods, Self::Patches];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Files => "Files",
            Self::Mods => "Mods",
            Self::Patches => "Patches",
            Self::Settings => "Settings",
        }
    }

    pub const fn element_id(self) -> &'static str {
        match self {
            Self::Home => "rail-home",
            Self::Files => "rail-files",
            Self::Mods => "rail-mods",
            Self::Patches => "rail-patches",
            Self::Settings => "rail-settings",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NavigationProjection {
    pub(crate) primary: &'static [Area],
    pub(crate) bottom: Area,
}

pub(crate) const fn navigation_projection() -> NavigationProjection {
    NavigationProjection {
        primary: &Area::PRIMARY,
        bottom: Area::Settings,
    }
}

#[derive(Debug, Default)]
pub struct ShellState {
    area: Area,
    game: Game,
    next_request_id: u64,
    next_crusaders_catalog_request_id: u64,
    active_open_request: Option<RequestID>,
    active_catalog_request: Option<RequestID>,
    active_crusaders_catalog_request: Option<CrusadersCatalogRequestID>,
    active_crusaders_browse_request: Option<RequestID>,
    active_heroes_browse_request: Option<RequestID>,
    active_discovery_request: Option<RequestID>,
}

impl ShellState {
    pub fn with_game(game: Game) -> Self {
        Self {
            game,
            ..Self::default()
        }
    }

    pub const fn area(&self) -> Area {
        self.area
    }

    pub const fn game(&self) -> Game {
        self.game
    }

    pub fn select_area(&mut self, area: Area) {
        self.area = area;
    }

    pub fn select_game(&mut self, game: Game) {
        self.game = game;
    }

    pub fn begin_open(&mut self) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        self.active_open_request = Some(request);
        request
    }

    pub fn accepts_open(&self, request: RequestID) -> bool {
        self.active_open_request == Some(request)
    }

    pub fn begin_catalog(&mut self) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        self.active_catalog_request = Some(request);
        request
    }

    pub fn accepts_catalog(&self, request: RequestID) -> bool {
        self.active_catalog_request == Some(request)
    }

    pub fn invalidate_catalog(&mut self) {
        self.active_catalog_request = None;
    }

    pub fn begin_crusaders_catalog(&mut self) -> CrusadersCatalogRequestID {
        self.next_crusaders_catalog_request_id += 1;
        let request = CrusadersCatalogRequestID(self.next_crusaders_catalog_request_id);
        self.active_crusaders_catalog_request = Some(request);
        request
    }

    pub fn accepts_crusaders_catalog(&self, request: CrusadersCatalogRequestID) -> bool {
        self.active_crusaders_catalog_request == Some(request)
    }

    pub fn invalidate_crusaders_catalog(&mut self) {
        self.active_crusaders_catalog_request = None;
    }

    pub fn begin_browse(&mut self, game: Game) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        match game {
            Game::Crusaders => self.active_crusaders_browse_request = Some(request),
            Game::Heroes => self.active_heroes_browse_request = Some(request),
        }
        request
    }

    pub fn accepts_browse(&self, game: Game, request: RequestID) -> bool {
        (match game {
            Game::Crusaders => self.active_crusaders_browse_request,
            Game::Heroes => self.active_heroes_browse_request,
        }) == Some(request)
    }

    pub fn invalidate_browse(&mut self, game: Game) {
        match game {
            Game::Crusaders => self.active_crusaders_browse_request = None,
            Game::Heroes => self.active_heroes_browse_request = None,
        }
    }

    pub fn begin_discovery(&mut self) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        self.active_discovery_request = Some(request);
        request
    }

    pub fn accepts_discovery(&self, request: RequestID) -> bool {
        self.active_discovery_request == Some(request)
    }

    pub fn invalidate_discovery(&mut self) {
        self.active_discovery_request = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, ShellState, navigation_projection};
    use kufeditor_game::Game;

    #[test]
    fn shell_starts_at_home_for_crusaders() {
        let state = ShellState::default();
        assert_eq!(state.area(), Area::Home);
        assert_eq!(state.game(), Game::Crusaders);
    }

    #[test]
    fn navigation_and_game_selection_are_independent() {
        let mut state = ShellState::default();
        state.select_area(Area::Files);
        state.select_game(Game::Heroes);
        assert_eq!(state.area(), Area::Files);
        assert_eq!(state.game(), Game::Heroes);
    }

    #[test]
    fn shell_can_start_with_the_persisted_game() {
        let state = ShellState::with_game(Game::Heroes);

        assert_eq!(state.area(), Area::Home);
        assert_eq!(state.game(), Game::Heroes);
    }

    #[test]
    fn request_ids_make_old_open_results_stale() {
        let mut state = ShellState::default();
        let first = state.begin_open();
        let second = state.begin_open();
        assert!(!state.accepts_open(first));
        assert!(state.accepts_open(second));
    }

    #[test]
    fn catalog_requests_can_be_superseded_or_invalidated() {
        let mut state = ShellState::default();
        let first = state.begin_catalog();
        let second = state.begin_catalog();

        assert!(!state.accepts_catalog(first));
        assert!(state.accepts_catalog(second));

        state.invalidate_catalog();
        assert!(!state.accepts_catalog(second));
    }

    #[test]
    fn global_and_crusaders_catalog_requests_use_independent_generations() {
        let mut state = ShellState::default();

        let first_global = state.begin_catalog();
        let first_crusaders = state.begin_crusaders_catalog();
        let second_global = state.begin_catalog();
        let second_crusaders = state.begin_crusaders_catalog();

        assert_eq!(first_global.get(), 1);
        assert_eq!(second_global.get(), 2);
        assert_eq!(first_crusaders.get(), 1);
        assert_eq!(second_crusaders.get(), 2);
        assert!(!state.accepts_catalog(first_global));
        assert!(state.accepts_catalog(second_global));
        assert!(!state.accepts_crusaders_catalog(first_crusaders));
        assert!(state.accepts_crusaders_catalog(second_crusaders));
    }

    #[test]
    fn global_and_crusaders_catalog_invalidations_are_independent() {
        let mut state = ShellState::default();
        let global = state.begin_catalog();
        let crusaders = state.begin_crusaders_catalog();

        state.invalidate_crusaders_catalog();

        assert!(state.accepts_catalog(global));
        assert!(!state.accepts_crusaders_catalog(crusaders));

        let next_crusaders = state.begin_crusaders_catalog();
        state.invalidate_catalog();

        assert!(!state.accepts_catalog(global));
        assert!(state.accepts_crusaders_catalog(next_crusaders));
    }

    #[test]
    fn navigation_keeps_settings_below_the_primary_destinations() {
        assert_eq!(
            Area::PRIMARY,
            [Area::Home, Area::Files, Area::Mods, Area::Patches],
        );
        assert_eq!(Area::Settings.label(), "Settings");
        assert_eq!(Area::Settings.element_id(), "rail-settings");

        let projection = navigation_projection();
        assert_eq!(projection.primary, Area::PRIMARY);
        assert_eq!(projection.bottom, Area::Settings);
    }

    #[test]
    fn browse_discovery_catalog_and_open_request_gates_are_independent() {
        let mut state = ShellState::default();
        let crusaders_browse = state.begin_browse(Game::Crusaders);
        let heroes_browse = state.begin_browse(Game::Heroes);
        let discovery = state.begin_discovery();
        let catalog = state.begin_catalog();
        let open = state.begin_open();

        assert!(state.accepts_browse(Game::Crusaders, crusaders_browse));
        assert!(state.accepts_browse(Game::Heroes, heroes_browse));
        assert!(state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        state.invalidate_browse(Game::Crusaders);
        assert!(!state.accepts_browse(Game::Crusaders, crusaders_browse));
        assert!(state.accepts_browse(Game::Heroes, heroes_browse));
        assert!(state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        let next_heroes_browse = state.begin_browse(Game::Heroes);
        assert!(state.accepts_browse(Game::Heroes, next_heroes_browse));
        assert!(state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        state.invalidate_discovery();
        assert!(state.accepts_browse(Game::Heroes, next_heroes_browse));
        assert!(!state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        state.invalidate_catalog();
        assert!(state.accepts_browse(Game::Heroes, next_heroes_browse));
        assert!(!state.accepts_discovery(discovery));
        assert!(!state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));
    }
}

#[cfg(test)]
mod close_tests {
    use super::ClosePolicy;

    #[test]
    fn only_dirty_documents_require_a_prompt() {
        assert_eq!(ClosePolicy::from_dirty_count(0), ClosePolicy::Allow);
        assert_eq!(
            ClosePolicy::from_dirty_count(2),
            ClosePolicy::PromptForUnsaved { count: 2 },
        );
    }
}

#[cfg(test)]
mod save_presentation_tests {
    use std::path::PathBuf;

    use kufeditor_workspace::{
        Document, DocumentID, SaveEquipmentSlot, SaveRosterField, TroopDocument, Workspace,
    };

    use super::{
        SaveListCursor, SaveListKind, SavePresentationState, SavePresentationStates,
        SavePresentationTransition, SaveSection, SaveUnitVisibility,
    };

    const fn all_units(unit_count: usize) -> SaveUnitVisibility<'static> {
        SaveUnitVisibility::All { unit_count }
    }

    const fn filtered_units(indices: &[usize]) -> SaveUnitVisibility<'_> {
        SaveUnitVisibility::Filtered(indices)
    }

    fn document_ids() -> (DocumentID, DocumentID) {
        let mut workspace = Workspace::new();
        let first = workspace.open_loaded(
            PathBuf::from("first.sox"),
            Document::Troop(troop_document()),
        );
        let second = workspace.open_loaded(
            PathBuf::from("second.sox"),
            Document::Troop(troop_document()),
        );
        (first, second)
    }

    fn troop_document() -> TroopDocument {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(..8)
            .unwrap()
            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
        TroopDocument::parse(bytes).unwrap()
    }

    #[test]
    fn save_presentation_starts_at_summary_and_retains_each_document_section() {
        let (first, second) = document_ids();
        let mut states = SavePresentationStates::default();

        assert_eq!(
            SavePresentationState::default().section(),
            SaveSection::Summary
        );
        assert_eq!(
            states.select_section(first, SaveSection::Units, false),
            SavePresentationTransition::Changed,
        );
        assert_eq!(
            states.select_section(second, SaveSection::Equipment, false),
            SavePresentationTransition::Changed,
        );

        assert_eq!(states.get(first).unwrap().section(), SaveSection::Units);
        assert_eq!(
            states.get(second).unwrap().section(),
            SaveSection::Equipment,
        );
    }

    #[test]
    fn save_presentation_clamps_inspected_unit_after_document_replacement() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, all_units(10), false);
        states.set_list_cursor(document, SaveListCursor::Unit { source_index: 8 }, false);

        assert_eq!(
            states.reconcile_document(document, all_units(3), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 2);
    }

    #[test]
    fn save_presentation_activation_preserves_a_reconciliation_cancel_result() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, all_units(10), false);
        states.set_list_cursor(document, SaveListCursor::Unit { source_index: 8 }, false);

        assert_eq!(
            states.activate_document(document, all_units(3), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 2);
    }

    #[test]
    fn save_presentation_keeps_a_valid_equipment_slot_after_reconciliation() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.select_equipment_slot(document, SaveEquipmentSlot::TroopArmor, false);

        states.reconcile_document(document, all_units(1), false);

        assert_eq!(
            states.get(document).unwrap().equipment_slot(),
            SaveEquipmentSlot::TroopArmor,
        );
    }

    #[test]
    fn save_presentation_player_only_moves_inspection_to_the_first_visible_unit() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_list_cursor(document, SaveListCursor::Unit { source_index: 7 }, false);

        assert_eq!(
            states.set_player_only(document, true, filtered_units(&[2, 4]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        let state = states.get(document).unwrap();
        assert!(state.player_only());
        assert_eq!(state.inspected_unit(), 2);
    }

    #[test]
    fn save_presentation_changes_explicitly_cancel_active_drafts() {
        let (first, second) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(first, all_units(5), false);

        assert_eq!(
            states.select_section(first, SaveSection::Units, true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.set_list_cursor(first, SaveListCursor::Unit { source_index: 1 }, true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.set_player_only(first, true, filtered_units(&[1, 3]), true),
            SavePresentationTransition::Changed,
        );
        assert_eq!(
            states.select_equipment_slot(first, SaveEquipmentSlot::LeaderArmor, true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.activate_document(second, all_units(2), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
    }

    #[test]
    fn save_presentation_same_count_replacement_reconciles_changed_filter_membership() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_player_only(document, true, filtered_units(&[1, 3]), false);
        states.set_list_cursor(document, SaveListCursor::Unit { source_index: 3 }, false);

        assert_eq!(
            states.reconcile_document(document, filtered_units(&[1, 2]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        let state = states.get(document).unwrap();
        assert!(state.player_only());
        assert_eq!(state.inspected_unit(), 1);
    }

    #[test]
    fn save_presentation_empty_filtered_results_reset_inspection() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_list_cursor(document, SaveListCursor::Unit { source_index: 3 }, false);

        assert_eq!(
            states.set_player_only(document, true, filtered_units(&[]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_unchanged_filter_cancels_unit_zero_draft_when_visibility_empties() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_player_only(document, true, filtered_units(&[0]), false);

        assert_eq!(
            states.set_player_only(document, true, filtered_units(&[]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_zero_count_reconciliation_cancels_unit_zero_draft() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.reconcile_document(document, all_units(1), false);

        assert_eq!(
            states.reconcile_document(document, all_units(0), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_active_activation_cancels_unit_zero_draft_when_visibility_empties() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, filtered_units(&[0]), false);

        assert_eq!(
            states.activate_document(document, filtered_units(&[]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_active_document_activation_reconciles_changed_membership() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, filtered_units(&[1, 3]), false);
        states.set_list_cursor(document, SaveListCursor::Unit { source_index: 3 }, false);

        assert_eq!(
            states.activate_document(document, filtered_units(&[1, 2]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 1);
    }

    #[test]
    fn save_presentation_reconciles_roster_and_second_array_cursors_after_list_shrink() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_list_cursor(
            document,
            SaveListCursor::Roster {
                record: 9,
                field: SaveRosterField::Value64,
            },
            false,
        );
        states.set_list_cursor(document, SaveListCursor::SecondArray { record: 7 }, false);

        assert_eq!(
            states.reconcile_list_cursor(document, SaveListKind::Roster, all_units(3), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .list_cursor(SaveListKind::Roster),
            SaveListCursor::Roster {
                record: 2,
                field: SaveRosterField::Value64,
            },
        );
        assert_eq!(
            states.reconcile_list_cursor(document, SaveListKind::SecondArray, all_units(0), true,),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .list_cursor(SaveListKind::SecondArray),
            SaveListCursor::SecondArray { record: 0 },
        );
    }

    #[test]
    fn save_presentation_keeps_virtual_list_cursors_per_document() {
        let (first, second) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_list_cursor(
            first,
            SaveListCursor::Roster {
                record: 7,
                field: SaveRosterField::Value64,
            },
            false,
        );
        states.set_list_cursor(
            second,
            SaveListCursor::Roster {
                record: 2,
                field: SaveRosterField::Byte62,
            },
            false,
        );

        assert_eq!(
            states.get(first).unwrap().list_cursor(SaveListKind::Roster),
            SaveListCursor::Roster {
                record: 7,
                field: SaveRosterField::Value64,
            },
        );
        assert_eq!(
            states
                .get(second)
                .unwrap()
                .list_cursor(SaveListKind::Roster),
            SaveListCursor::Roster {
                record: 2,
                field: SaveRosterField::Byte62,
            },
        );
    }

    #[test]
    fn save_presentation_removes_document_state_on_close() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, all_units(1), false);

        assert_eq!(
            states.remove_document(document, true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert!(states.get(document).is_none());
        assert_eq!(states.active_document, None);
    }
}

#[cfg(test)]
mod stg_presentation_tests {
    use std::path::PathBuf;

    use kufeditor_workspace::{
        Document, DocumentID, STGEventTarget, STGHeaderTextField, STGParameterTarget,
        STGReferenceKind, STGScriptKind, STGScriptTarget, STGTextTarget, TroopDocument, Workspace,
    };

    use super::{
        STGBindingPath, STGDocumentTransition, STGDraftBinding, STGDraftStatus, STGDraftTarget,
        STGEventBlockRange, STGEventVisibility, STGIndexVisibility, STGPresentationState,
        STGPresentationStates, STGPresentationTransition, STGReferenceCursor,
        STGReferencePickerState, STGReferenceVisibility, STGSection, STGSelection,
        STGStructuralChange, STGVisibleSelections,
    };

    fn document_ids() -> (DocumentID, DocumentID) {
        let mut workspace = Workspace::new();
        let first = workspace.open_loaded(
            PathBuf::from("first.sox"),
            Document::Troop(troop_document()),
        );
        let second = workspace.open_loaded(
            PathBuf::from("second.sox"),
            Document::Troop(troop_document()),
        );
        (first, second)
    }

    fn troop_document() -> TroopDocument {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(..8)
            .unwrap()
            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
        TroopDocument::parse(bytes).unwrap()
    }

    fn event(block: usize, event: usize) -> STGEventTarget {
        STGEventTarget { block, event }
    }

    fn script(block: usize, event: usize, kind: STGScriptKind, script: usize) -> STGScriptTarget {
        STGScriptTarget {
            block,
            event,
            kind,
            script,
        }
    }

    fn visibility(blocks: &[STGEventBlockRange]) -> STGVisibleSelections<'_> {
        STGVisibleSelections::new(
            STGIndexVisibility::Range(0..4),
            STGIndexVisibility::Range(0..3),
            STGIndexVisibility::Range(0..2),
            STGEventVisibility::Blocks(blocks),
            STGIndexVisibility::Range(0..5),
        )
    }

    fn reference_indices(visible: STGIndexVisibility<'_>) -> STGReferenceVisibility<'_> {
        STGReferenceVisibility::Indices(visible)
    }

    #[test]
    fn stg_presentation_defaults_to_header_without_phantom_selections() {
        let state = STGPresentationState::default();

        assert_eq!(state.section(), STGSection::Header);
        assert_eq!(state.inspected_unit(), None);
        assert_eq!(state.inspected_area(), None);
        assert_eq!(state.inspected_variable(), None);
        assert_eq!(state.inspected_event(), None);
        assert_eq!(state.inspected_footer(), None);
        assert_eq!(state.expanded_script(), None);
        assert_eq!(state.unit_query(), "");
        assert_eq!(state.event_query(), "");
        assert_eq!(state.binding_generation(), 0);
        assert!(state.reference_picker().is_none());
        assert_eq!(
            STGSection::ALL,
            [
                STGSection::Header,
                STGSection::Units,
                STGSection::Areas,
                STGSection::Variables,
                STGSection::Events,
                STGSection::Footer,
            ]
        );
    }

    #[test]
    fn stg_presentation_keeps_every_field_independent_per_document() {
        let (first, second) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 2), STGEventBlockRange::new(1, 3)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();

        states.activate_document(first, &visible, None);
        states.select_section(first, STGSection::Units, None);
        states.select(first, STGSelection::Unit(Some(3)), None);
        states.select(first, STGSelection::Area(Some(2)), None);
        states.select(first, STGSelection::Variable(Some(1)), None);
        states.select(first, STGSelection::Event(Some(event(1, 2))), None);
        states.select(first, STGSelection::Footer(Some(4)), None);
        states.set_unit_query(
            first,
            "paladin".to_owned(),
            &STGIndexVisibility::Sparse(&[3]),
            None,
        );
        states.set_event_query(
            first,
            "reinforcements".to_owned(),
            &STGEventVisibility::Filtered(&[event(1, 2)]),
            None,
        );
        let expanded = script(1, 2, STGScriptKind::Condition, 1);
        states.set_expanded_script(first, Some(expanded), None);
        let parameter = STGParameterTarget {
            script: expanded,
            parameter: 3,
        };
        states.set_reference_picker(
            first,
            Some(STGReferencePickerState::new(
                parameter,
                STGReferenceKind::Troop,
                "archer".to_owned(),
                Some(STGReferenceCursor::Index(17)),
            )),
            None,
        );

        states.activate_document(second, &visible, None);
        states.select_section(second, STGSection::Footer, None);
        states.select(second, STGSelection::Unit(Some(1)), None);
        states.set_unit_query(
            second,
            "orc".to_owned(),
            &STGIndexVisibility::Sparse(&[1]),
            None,
        );

        let first_state = states.get(first).unwrap();
        assert_eq!(first_state.section(), STGSection::Units);
        assert_eq!(first_state.inspected_unit(), Some(3));
        assert_eq!(first_state.inspected_area(), Some(2));
        assert_eq!(first_state.inspected_variable(), Some(1));
        assert_eq!(first_state.inspected_event(), Some(event(1, 2)));
        assert_eq!(first_state.inspected_footer(), Some(4));
        assert_eq!(first_state.expanded_script(), Some(expanded));
        assert_eq!(first_state.unit_query(), "paladin");
        assert_eq!(first_state.event_query(), "reinforcements");
        let picker = first_state.reference_picker().unwrap();
        assert_eq!(picker.target(), parameter);
        assert_eq!(picker.kind(), STGReferenceKind::Troop);
        assert_eq!(picker.query(), "archer");
        assert_eq!(picker.cursor(), Some(STGReferenceCursor::Index(17)));

        let second_state = states.get(second).unwrap();
        assert_eq!(second_state.section(), STGSection::Footer);
        assert_eq!(second_state.inspected_unit(), Some(1));
        assert_eq!(second_state.inspected_area(), Some(0));
        assert_eq!(second_state.inspected_event(), Some(event(0, 0)));
        assert_eq!(second_state.unit_query(), "orc");
        assert_eq!(second_state.event_query(), "");
        assert_eq!(second_state.expanded_script(), None);
        assert!(second_state.reference_picker().is_none());
    }

    #[test]
    fn stg_presentation_reconciliation_keeps_source_targets_or_uses_first_visible() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 3)];
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visibility(&blocks), None);
        states.select(document, STGSelection::Unit(Some(3)), None);
        states.select(document, STGSelection::Event(Some(event(0, 2))), None);

        let retained_events = [event(0, 0), event(0, 2)];
        let retained = STGVisibleSelections::new(
            STGIndexVisibility::Sparse(&[1, 3]),
            STGIndexVisibility::Range(0..3),
            STGIndexVisibility::Range(0..2),
            STGEventVisibility::Filtered(&retained_events),
            STGIndexVisibility::Range(0..5),
        );
        states.reconcile_document(
            document,
            &retained,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::ScalarEdit,
            None,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), Some(3));
        assert_eq!(
            states.get(document).unwrap().inspected_event(),
            Some(event(0, 2))
        );

        let rebound_events = [event(0, 0), event(0, 1)];
        let rebound = STGVisibleSelections::new(
            STGIndexVisibility::Sparse(&[1, 2]),
            STGIndexVisibility::Range(0..3),
            STGIndexVisibility::Range(0..2),
            STGEventVisibility::Filtered(&rebound_events),
            STGIndexVisibility::Range(0..5),
        );
        states.reconcile_document(
            document,
            &rebound,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::StructuralEdit(None),
            None,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), Some(2));
        assert_eq!(
            states.get(document).unwrap().inspected_event(),
            Some(event(0, 1))
        );

        let empty = STGVisibleSelections::empty();
        states.reconcile_document(
            document,
            &empty,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::Undo(None),
            None,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.inspected_unit(), None);
        assert_eq!(state.inspected_area(), None);
        assert_eq!(state.inspected_variable(), None);
        assert_eq!(state.inspected_event(), None);
        assert_eq!(state.inspected_footer(), None);
    }

    #[test]
    fn stg_presentation_structural_shrink_selects_the_previous_last_record() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 10)];
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visibility(&blocks), None);
        states.select(document, STGSelection::Unit(Some(9)), None);
        states.select(document, STGSelection::Event(Some(event(0, 9))), None);

        let shrunk_blocks = [STGEventBlockRange::new(0, 9)];
        let shrunk = STGVisibleSelections::new(
            STGIndexVisibility::Range(0..9),
            STGIndexVisibility::Range(0..3),
            STGIndexVisibility::Range(0..2),
            STGEventVisibility::Blocks(&shrunk_blocks),
            STGIndexVisibility::Range(0..5),
        );
        states.reconcile_document(
            document,
            &shrunk,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::StructuralEdit(None),
            None,
        );

        let state = states.get(document).unwrap();
        assert_eq!(state.inspected_unit(), Some(8));
        assert_eq!(state.inspected_event(), Some(event(0, 8)));
    }

    #[test]
    fn stg_presentation_remaps_event_identity_across_remove_insert_and_redo() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 7)];
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visibility(&blocks), None);
        states.select(document, STGSelection::Event(Some(event(0, 5))), None);

        let removed_blocks = [STGEventBlockRange::new(0, 6)];
        states.reconcile_document(
            document,
            &visibility(&removed_blocks),
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::StructuralEdit(Some(STGStructuralChange::RemoveEvent {
                target: event(0, 2),
            })),
            None,
        );
        assert_eq!(
            states.get(document).unwrap().inspected_event(),
            Some(event(0, 4))
        );

        states.reconcile_document(
            document,
            &visibility(&blocks),
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::Undo(Some(STGStructuralChange::InsertEvent {
                target: event(0, 2),
            })),
            None,
        );
        assert_eq!(
            states.get(document).unwrap().inspected_event(),
            Some(event(0, 5))
        );

        states.reconcile_document(
            document,
            &visibility(&removed_blocks),
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::Redo(Some(STGStructuralChange::RemoveEvent {
                target: event(0, 5),
            })),
            None,
        );
        assert_eq!(
            states.get(document).unwrap().inspected_event(),
            Some(event(0, 5))
        );
    }

    #[test]
    fn stg_presentation_remaps_or_closes_expanded_script_identity() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visible, None);
        let expanded = script(0, 0, STGScriptKind::Condition, 2);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
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

        let remapped = script(0, 0, STGScriptKind::Condition, 1);
        states.reconcile_document(
            document,
            &visible,
            &[remapped],
            true,
            &reference_indices(STGIndexVisibility::Range(0..1)),
            STGDocumentTransition::StructuralEdit(Some(STGStructuralChange::RemoveScript {
                target: script(0, 0, STGScriptKind::Condition, 0),
            })),
            None,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.expanded_script(), Some(remapped));
        assert_eq!(state.reference_picker().unwrap().target().script, remapped);

        states.reconcile_document(
            document,
            &visible,
            &[script(0, 0, STGScriptKind::Condition, 0)],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::StructuralEdit(Some(STGStructuralChange::RemoveScript {
                target: remapped,
            })),
            None,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.expanded_script(), None);
        assert!(state.reference_picker().is_none());
    }

    #[test]
    fn stg_presentation_remaps_unfiltered_event_reference_cursor_through_history() {
        let (document, _) = document_ids();
        let before_blocks = [STGEventBlockRange::new(0, 1), STGEventBlockRange::new(2, 3)];
        let after_blocks = [STGEventBlockRange::new(0, 1), STGEventBlockRange::new(2, 2)];
        let before = visibility(&before_blocks);
        let after = visibility(&after_blocks);
        let expanded = script(0, 0, STGScriptKind::Condition, 0);
        let removed = event(2, 0);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &before, None);
        states.select(document, STGSelection::Event(Some(event(0, 0))), None);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
            document,
            Some(STGReferencePickerState::new(
                STGParameterTarget {
                    script: expanded,
                    parameter: 0,
                },
                STGReferenceKind::Trigger,
                String::new(),
                Some(STGReferenceCursor::Event(event(2, 2))),
            )),
            None,
        );

        states.reconcile_document(
            document,
            &after,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Blocks(&after_blocks)),
            STGDocumentTransition::StructuralEdit(Some(STGStructuralChange::RemoveEvent {
                target: removed,
            })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 1)))
        );

        states.reconcile_document(
            document,
            &before,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Blocks(&before_blocks)),
            STGDocumentTransition::Undo(Some(STGStructuralChange::InsertEvent { target: removed })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 2)))
        );

        states.reconcile_document(
            document,
            &after,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Blocks(&after_blocks)),
            STGDocumentTransition::Redo(Some(STGStructuralChange::RemoveEvent { target: removed })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 1)))
        );
    }

    #[test]
    fn stg_presentation_reconciles_filtered_event_reference_history() {
        let (document, _) = document_ids();
        let before_blocks = [STGEventBlockRange::new(0, 1), STGEventBlockRange::new(2, 3)];
        let after_blocks = [STGEventBlockRange::new(0, 1), STGEventBlockRange::new(2, 2)];
        let before = visibility(&before_blocks);
        let after = visibility(&after_blocks);
        let before_references = [event(2, 0), event(2, 2)];
        let after_references = [event(2, 1)];
        let expanded = script(0, 0, STGScriptKind::Condition, 0);
        let removed = event(2, 0);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &before, None);
        states.select(document, STGSelection::Event(Some(event(0, 0))), None);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
            document,
            Some(STGReferencePickerState::new(
                STGParameterTarget {
                    script: expanded,
                    parameter: 0,
                },
                STGReferenceKind::Event,
                "needle".to_owned(),
                Some(STGReferenceCursor::Event(event(2, 2))),
            )),
            None,
        );

        states.reconcile_document(
            document,
            &after,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Filtered(&after_references)),
            STGDocumentTransition::StructuralEdit(Some(STGStructuralChange::RemoveEvent {
                target: removed,
            })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 1)))
        );

        states.reconcile_document(
            document,
            &before,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Filtered(&before_references)),
            STGDocumentTransition::Undo(Some(STGStructuralChange::InsertEvent { target: removed })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 2)))
        );

        states.reconcile_document(
            document,
            &after,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Filtered(&after_references)),
            STGDocumentTransition::Redo(Some(STGStructuralChange::RemoveEvent { target: removed })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 1)))
        );
    }

    #[test]
    fn stg_presentation_event_reference_deletion_selects_the_previous_last_candidate() {
        let (document, _) = document_ids();
        let before_blocks = [STGEventBlockRange::new(0, 1), STGEventBlockRange::new(2, 2)];
        let after_blocks = [STGEventBlockRange::new(0, 1), STGEventBlockRange::new(2, 1)];
        let before = visibility(&before_blocks);
        let after = visibility(&after_blocks);
        let references = [event(0, 0), event(2, 0)];
        let expanded = script(0, 0, STGScriptKind::Condition, 0);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &before, None);
        states.select(document, STGSelection::Event(Some(event(0, 0))), None);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
            document,
            Some(STGReferencePickerState::new(
                STGParameterTarget {
                    script: expanded,
                    parameter: 0,
                },
                STGReferenceKind::Event,
                "needle".to_owned(),
                Some(STGReferenceCursor::Event(event(2, 1))),
            )),
            None,
        );

        states.reconcile_document(
            document,
            &after,
            &[expanded],
            true,
            &STGReferenceVisibility::Events(STGEventVisibility::Filtered(&references)),
            STGDocumentTransition::StructuralEdit(Some(STGStructuralChange::RemoveEvent {
                target: event(2, 1),
            })),
            None,
        );
        assert_eq!(
            states
                .get(document)
                .unwrap()
                .reference_picker()
                .unwrap()
                .cursor(),
            Some(STGReferenceCursor::Event(event(2, 0)))
        );
    }

    #[test]
    fn stg_presentation_unknown_structure_closes_same_index_script_identity() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let expanded = script(0, 0, STGScriptKind::Condition, 0);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visible, None);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
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

        states.reconcile_document(
            document,
            &visible,
            &[expanded],
            true,
            &reference_indices(STGIndexVisibility::Range(0..1)),
            STGDocumentTransition::StructuralEdit(None),
            None,
        );

        let state = states.get(document).unwrap();
        assert_eq!(state.expanded_script(), None);
        assert_eq!(state.reference_picker(), None);
    }

    #[test]
    fn stg_presentation_changed_transitions_advance_generation_exactly_once() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();
        let activated = states.activate_document(document, &visible, None);
        let mut generation = activated.generation().unwrap();

        for cause in [
            STGDocumentTransition::ScalarEdit,
            STGDocumentTransition::StructuralEdit(None),
            STGDocumentTransition::Undo(None),
            STGDocumentTransition::Redo(None),
            STGDocumentTransition::Catalog,
        ] {
            let transition = states.reconcile_document(
                document,
                &visible,
                &[],
                false,
                &reference_indices(STGIndexVisibility::Range(0..0)),
                cause,
                None,
            );
            assert!(transition.changed());
            assert_eq!(transition.generation(), Some(generation + 1));
            generation += 1;
            assert_eq!(
                states.get(document).unwrap().binding_generation(),
                generation
            );
        }

        let transition = states.set_unit_query(
            document,
            "knight".to_owned(),
            &STGIndexVisibility::Sparse(&[0]),
            None,
        );
        assert_eq!(transition.generation(), Some(generation + 1));
        generation += 1;
        let transition = states.select_section(document, STGSection::Units, None);
        assert_eq!(transition.generation(), Some(generation + 1));
        generation += 1;
        let transition = states.select(document, STGSelection::Unit(Some(2)), None);
        assert_eq!(transition.generation(), Some(generation + 1));
        generation += 1;
        let expanded = script(0, 0, STGScriptKind::Action, 0);
        let transition = states.set_expanded_script(document, Some(expanded), None);
        assert_eq!(transition.generation(), Some(generation + 1));
        generation += 1;
        let transition = states.set_expanded_script(document, None, None);
        assert_eq!(transition.generation(), Some(generation + 1));
    }

    #[test]
    fn stg_presentation_preserves_only_a_visible_exact_draft() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visible, None);
        let binding = STGDraftBinding::new(
            document,
            STGSection::Header,
            STGDraftTarget::Text(STGTextTarget::Header(STGHeaderTextField::MapFilename)),
        );

        let kept = states.reconcile_document(
            document,
            &visible,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::ScalarEdit,
            Some(STGDraftStatus::visible(binding)),
        );
        assert!(kept.changed());
        assert!(!kept.cancels_draft());

        let hidden = states.reconcile_document(
            document,
            &visible,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::Undo(None),
            Some(STGDraftStatus::hidden(binding)),
        );
        assert!(hidden.cancels_draft());

        let section_change = states.select_section(
            document,
            STGSection::Units,
            Some(STGDraftStatus::visible(binding)),
        );
        assert!(section_change.cancels_draft());

        let generation = states.get(document).unwrap().binding_generation();
        let unchanged = states.reconcile_document(
            document,
            &visible,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::Unchanged,
            Some(STGDraftStatus::hidden(binding)),
        );
        assert_eq!(unchanged, STGPresentationTransition::Unchanged);
        assert_eq!(
            states.get(document).unwrap().binding_generation(),
            generation
        );
    }

    #[test]
    fn stg_presentation_reconciles_expanded_scripts_and_reference_cursor() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visible, None);
        states.select_section(document, STGSection::Events, None);
        let expanded = script(0, 0, STGScriptKind::Condition, 0);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
            document,
            Some(STGReferencePickerState::new(
                STGParameterTarget {
                    script: expanded,
                    parameter: 0,
                },
                STGReferenceKind::Area,
                String::new(),
                Some(STGReferenceCursor::Index(5)),
            )),
            None,
        );

        let rebound = script(0, 0, STGScriptKind::Action, 0);
        states.set_expanded_script(document, Some(rebound), None);
        assert!(states.get(document).unwrap().reference_picker().is_none());
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
            document,
            Some(STGReferencePickerState::new(
                STGParameterTarget {
                    script: expanded,
                    parameter: 0,
                },
                STGReferenceKind::Area,
                String::new(),
                Some(STGReferenceCursor::Index(5)),
            )),
            None,
        );

        states.reconcile_document(
            document,
            &visible,
            &[expanded],
            true,
            &reference_indices(STGIndexVisibility::Sparse(&[2, 5])),
            STGDocumentTransition::Redo(None),
            None,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.expanded_script(), Some(expanded));
        assert_eq!(
            state.reference_picker().unwrap().cursor(),
            Some(STGReferenceCursor::Index(5))
        );

        let transition = states.reconcile_document(
            document,
            &visible,
            &[expanded],
            true,
            &reference_indices(STGIndexVisibility::Sparse(&[2, 4])),
            STGDocumentTransition::Catalog,
            None,
        );
        assert!(transition.changed());
        let state = states.get(document).unwrap();
        assert_eq!(state.expanded_script(), Some(expanded));
        assert_eq!(
            state.reference_picker().unwrap().cursor(),
            Some(STGReferenceCursor::Index(2))
        );

        states.reconcile_document(
            document,
            &visible,
            &[],
            false,
            &reference_indices(STGIndexVisibility::Range(0..0)),
            STGDocumentTransition::Catalog,
            None,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.expanded_script(), None);
        assert!(state.reference_picker().is_none());
    }

    #[test]
    fn stg_presentation_closes_script_and_picker_when_the_event_rebinds() {
        let (document, _) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();
        states.activate_document(document, &visible, None);
        states.select_section(document, STGSection::Events, None);
        let expanded = script(0, 0, STGScriptKind::Condition, 0);
        states.set_expanded_script(document, Some(expanded), None);
        states.set_reference_picker(
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
        let other_event = [event(0, 1)];
        let rebound = STGVisibleSelections::new(
            STGIndexVisibility::Range(0..4),
            STGIndexVisibility::Range(0..3),
            STGIndexVisibility::Range(0..2),
            STGEventVisibility::Filtered(&other_event),
            STGIndexVisibility::Range(0..5),
        );
        states.reconcile_document(
            document,
            &rebound,
            &[expanded],
            true,
            &reference_indices(STGIndexVisibility::Range(0..1)),
            STGDocumentTransition::StructuralEdit(None),
            None,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.inspected_event(), Some(event(0, 1)));
        assert_eq!(state.expanded_script(), None);
        assert!(state.reference_picker().is_none());
    }

    #[test]
    fn stg_binding_cursor_rejects_generation_tab_and_close_staleness() {
        let (first, second) = document_ids();
        let blocks = [STGEventBlockRange::new(0, 1)];
        let visible = visibility(&blocks);
        let mut states = STGPresentationStates::default();
        states.activate_document(first, &visible, None);
        states.select_section(first, STGSection::Units, None);
        let cursor = states
            .binding_cursor(first, STGBindingPath::Unit { unit: 2 }, 2)
            .unwrap();

        assert_eq!(cursor.document(), first);
        assert_eq!(cursor.section(), STGSection::Units);
        assert_eq!(cursor.path(), STGBindingPath::Unit { unit: 2 });
        assert_eq!(cursor.source_index(), 2);
        assert!(states.accepts_binding_cursor(cursor));

        states.set_unit_query(
            first,
            "two".to_owned(),
            &STGIndexVisibility::Sparse(&[2]),
            None,
        );
        assert!(!states.accepts_binding_cursor(cursor));

        let current = states
            .binding_cursor(first, STGBindingPath::Unit { unit: 2 }, 2)
            .unwrap();
        let deactivated = states.deactivate_active_document(None);
        assert!(deactivated.changed());
        assert!(!states.accepts_binding_cursor(current));
        let reactivated = states.activate_document(first, &visible, None);
        assert!(reactivated.changed());
        assert!(reactivated.generation().unwrap() > current.generation());

        let current = states
            .binding_cursor(first, STGBindingPath::Unit { unit: 2 }, 2)
            .unwrap();
        states.activate_document(second, &visible, None);
        assert!(!states.accepts_binding_cursor(current));

        states.activate_document(first, &visible, None);
        let closing = states
            .binding_cursor(first, STGBindingPath::Unit { unit: 2 }, 2)
            .unwrap();
        assert!(states.remove_document(first, None).changed());
        assert!(!states.accepts_binding_cursor(closing));
        assert!(states.get(first).is_none());
    }
}
