#![allow(
    dead_code,
    reason = "Task 11 connects this Task 10 projection surface to GPUI rendering"
)]

use std::ops::Range;

use gpui::{
    AnyElement, App, Div, ElementId, IntoElement, SharedString, Stateful, UniformList, Window, div,
    prelude::*, px, uniform_list,
};
use kufeditor_game::NameDictionary;
use kufeditor_workspace::{
    DocumentID, SaveEditor, SaveEquipmentField, SaveEquipmentSlot, SaveMainField, SaveNumberTarget,
    SaveRosterField, SaveTextField, SaveUnitField, Workspace, WorkspaceError,
};

use crate::{
    components,
    state::{SavePresentationState, SaveSection, SaveUnitVisibility},
    theme::Theme,
};

pub type SaveProjectionResult<T> = Result<T, Box<WorkspaceError>>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveProjectionField {
    Row,
    Campaign,
    Text(SaveTextField),
    Main(SaveMainField),
    SavedUnitReference,
    Unit(SaveUnitField),
    Equipment(SaveEquipmentField),
    EquipmentAttribute(SaveEquipmentField),
    Roster(SaveRosterField),
    MissionCompletion,
    CurrentMission,
    SecondArray,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SaveProjectionID {
    pub document: DocumentID,
    pub section: SaveSection,
    pub source_index: usize,
    pub slot: Option<SaveEquipmentSlot>,
    pub field: SaveProjectionField,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveNumberProjection {
    pub id: SaveProjectionID,
    pub target: SaveNumberTarget,
    pub label: String,
    pub raw_value: i64,
    pub display_value: String,
    pub storage_bounds: (i64, i64),
    pub editor: SaveEditor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveTextProjection {
    pub id: SaveProjectionID,
    pub field: SaveTextField,
    pub label: String,
    pub value: Result<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SaveUnitRoleCounts {
    pub leader: usize,
    pub officer_1: usize,
    pub officer_2: usize,
    pub troop: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveSummaryProjection {
    pub campaign: SaveNumberProjection,
    pub text_fields: Vec<SaveTextProjection>,
    pub main_fields: Vec<SaveNumberProjection>,
    pub saved_unit_reference: SaveNumberProjection,
    pub role_counts: SaveUnitRoleCounts,
    pub unit_count: usize,
    pub roster_count: usize,
    pub second_array_count: usize,
    pub has_size_prefix: bool,
    pub has_context: bool,
    pub context_text: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveUnitRowProjection {
    pub id: SaveProjectionID,
    pub source_index: usize,
    pub label: String,
    pub role_value: i64,
    pub role: String,
    pub skill_level: i64,
    pub character_id: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveUnitProjection {
    pub row: SaveUnitRowProjection,
    pub fields: Vec<SaveNumberProjection>,
    pub skill_data: [u8; 24],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveEquipmentAttributeProjection {
    pub id: SaveProjectionID,
    pub raw_index: i64,
    pub name: String,
    pub effect: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveEquipmentProjection {
    pub id: SaveProjectionID,
    pub unit: usize,
    pub slot: SaveEquipmentSlot,
    pub slot_label: String,
    pub item_type: i64,
    pub variant: i64,
    pub enhancement_tier: i64,
    pub item_name: String,
    pub attributes: Vec<SaveEquipmentAttributeProjection>,
    pub fields: Vec<SaveNumberProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveRosterRowProjection {
    pub id: SaveProjectionID,
    pub source_index: usize,
    pub label: String,
    pub fields: Vec<SaveNumberProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveMissionProjection {
    pub current_mission: SaveNumberProjection,
    pub completions: Vec<SaveNumberProjection>,
    pub second_array_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveSectionModel {
    Summary(SaveSummaryProjection),
    Units {
        rows: SaveRows,
        inspected: Option<SaveUnitProjection>,
    },
    Equipment {
        slots: [SaveEquipmentSlot; 6],
        inspected_unit: Option<SaveUnitRowProjection>,
        selected: Option<SaveEquipmentProjection>,
    },
    Roster {
        player_leaders: SaveRows,
        world_map_rows: SaveRows,
    },
    Missions {
        mission: SaveMissionProjection,
        second_array_rows: SaveRows,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveRowKind {
    Units,
    PlayerLeaders,
    Roster,
    SecondArray,
}

impl SaveRowKind {
    pub const fn section(self) -> SaveSection {
        match self {
            Self::Units => SaveSection::Units,
            Self::PlayerLeaders | Self::Roster => SaveSection::Roster,
            Self::SecondArray => SaveSection::Missions,
        }
    }

    const fn projection_field(self) -> SaveProjectionField {
        match self {
            Self::Units | Self::PlayerLeaders | Self::Roster => SaveProjectionField::Row,
            Self::SecondArray => SaveProjectionField::SecondArray,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SaveRowLocation {
    pub id: SaveProjectionID,
    pub kind: SaveRowKind,
    pub source_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveRowProjection {
    Unit(SaveUnitRowProjection),
    Roster(SaveRosterRowProjection),
    SecondArray(SaveNumberProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SaveRowIndices {
    Contiguous(usize),
    Filtered(Vec<usize>),
}

impl SaveRowIndices {
    fn len(&self) -> usize {
        match self {
            Self::Contiguous(count) => *count,
            Self::Filtered(indices) => indices.len(),
        }
    }

    fn source_index(&self, virtual_index: usize) -> Option<usize> {
        match self {
            Self::Contiguous(count) => (virtual_index < *count).then_some(virtual_index),
            Self::Filtered(indices) => indices.get(virtual_index).copied(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveRows {
    document: DocumentID,
    kind: SaveRowKind,
    indices: SaveRowIndices,
}

impl SaveRows {
    pub fn units(
        workspace: &Workspace,
        document: DocumentID,
        dictionary: Option<&NameDictionary>,
        filter: &str,
    ) -> SaveProjectionResult<Self> {
        let filter = filter.trim();
        let indices = if filter.is_empty() {
            SaveRowIndices::Contiguous(workspace.save_unit_count(document)?)
        } else {
            SaveRowIndices::Filtered(visible_unit_indices(
                workspace, document, dictionary, filter,
            )?)
        };
        Ok(Self {
            document,
            kind: SaveRowKind::Units,
            indices,
        })
    }

    pub fn roster(workspace: &Workspace, document: DocumentID) -> SaveProjectionResult<Self> {
        Ok(Self {
            document,
            kind: SaveRowKind::Roster,
            indices: SaveRowIndices::Contiguous(workspace.save_roster_count(document)?),
        })
    }

    pub fn player_leaders(
        workspace: &Workspace,
        document: DocumentID,
    ) -> SaveProjectionResult<Self> {
        let unit_count = workspace.save_unit_count(document)?;
        let mut indices = Vec::new();
        for unit in 0..unit_count {
            if workspace.save_number(
                document,
                SaveNumberTarget::Unit {
                    unit,
                    field: SaveUnitField::UCD,
                },
            )? == 0
            {
                indices.push(unit);
            }
        }
        Ok(Self {
            document,
            kind: SaveRowKind::PlayerLeaders,
            indices: SaveRowIndices::Filtered(indices),
        })
    }

    pub fn second_array(workspace: &Workspace, document: DocumentID) -> SaveProjectionResult<Self> {
        Ok(Self {
            document,
            kind: SaveRowKind::SecondArray,
            indices: SaveRowIndices::Contiguous(workspace.save_second_array_count(document)?),
        })
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn kind(&self) -> SaveRowKind {
        self.kind
    }

    pub fn unit_visibility(&self) -> Option<SaveUnitVisibility<'_>> {
        if self.kind != SaveRowKind::Units {
            return None;
        }
        Some(match &self.indices {
            SaveRowIndices::Contiguous(unit_count) => SaveUnitVisibility::All {
                unit_count: *unit_count,
            },
            SaveRowIndices::Filtered(indices) => SaveUnitVisibility::Filtered(indices),
        })
    }

    fn reconciled_unit(&self, requested_unit: usize) -> Option<usize> {
        if self.kind != SaveRowKind::Units {
            return None;
        }
        match &self.indices {
            SaveRowIndices::Contiguous(unit_count) => {
                (*unit_count > 0).then(|| requested_unit.min(unit_count - 1))
            }
            SaveRowIndices::Filtered(indices) => indices
                .contains(&requested_unit)
                .then_some(requested_unit)
                .or_else(|| indices.first().copied()),
        }
    }

    pub fn locations(&self, requested: Range<usize>) -> Vec<SaveRowLocation> {
        let requested = bounded_range(requested, self.len());
        requested
            .filter_map(|virtual_index| {
                let source_index = self.indices.source_index(virtual_index)?;
                Some(SaveRowLocation {
                    id: projection_id(
                        self.document,
                        self.kind.section(),
                        source_index,
                        None,
                        self.kind.projection_field(),
                    ),
                    kind: self.kind,
                    source_index,
                })
            })
            .collect()
    }

    pub fn project_range(
        &self,
        workspace: &Workspace,
        dictionary: Option<&NameDictionary>,
        requested: Range<usize>,
    ) -> SaveProjectionResult<Vec<SaveRowProjection>> {
        self.project_range_with_observer(workspace, dictionary, requested, |_, _| {})
    }

    pub fn project_range_with_observer(
        &self,
        workspace: &Workspace,
        dictionary: Option<&NameDictionary>,
        requested: Range<usize>,
        observe: impl FnOnce(SaveRowKind, Range<usize>),
    ) -> SaveProjectionResult<Vec<SaveRowProjection>> {
        let requested = bounded_range(requested, self.len());
        observe(self.kind, requested.clone());
        let mut projections = Vec::with_capacity(requested.len());
        for location in self.locations(requested) {
            projections.push(row_projection(
                workspace,
                self.document,
                dictionary,
                location,
            )?);
        }
        Ok(projections)
    }
}

pub fn row_projection(
    workspace: &Workspace,
    document: DocumentID,
    dictionary: Option<&NameDictionary>,
    location: SaveRowLocation,
) -> SaveProjectionResult<SaveRowProjection> {
    Ok(match location.kind {
        SaveRowKind::Units | SaveRowKind::PlayerLeaders => {
            let mut row =
                unit_row_projection(workspace, document, location.source_index, dictionary)?;
            row.id = location.id;
            SaveRowProjection::Unit(row)
        }
        SaveRowKind::Roster => SaveRowProjection::Roster(roster_row_projection(
            workspace,
            document,
            location.source_index,
        )?),
        SaveRowKind::SecondArray => SaveRowProjection::SecondArray(number_projection(
            workspace,
            document,
            SaveNumberTarget::SecondArray {
                record: location.source_index,
            },
        )?),
    })
}

pub fn uniform_save_rows<R>(
    id: impl Into<ElementId>,
    rows: SaveRows,
    render: impl 'static + Fn(SaveRowLocation, &mut Window, &mut App) -> R,
) -> UniformList
where
    R: IntoElement,
{
    uniform_list(id, rows.len(), move |requested, window, cx| {
        rows.locations(requested)
            .into_iter()
            .map(|location| render(location, window, cx))
            .collect()
    })
}

pub fn summary_projection(
    workspace: &Workspace,
    document: DocumentID,
) -> SaveProjectionResult<SaveSummaryProjection> {
    let unit_count = workspace.save_unit_count(document)?;
    let mut role_counts = SaveUnitRoleCounts::default();
    for unit in 0..unit_count {
        let role = workspace.save_number(
            document,
            SaveNumberTarget::Unit {
                unit,
                field: SaveUnitField::UCD,
            },
        )?;
        match role {
            0 => role_counts.leader += 1,
            1 => role_counts.officer_1 += 1,
            2 => role_counts.officer_2 += 1,
            3 => role_counts.troop += 1,
            _ => role_counts.unknown += 1,
        }
    }

    let text_fields = SaveTextField::ALL
        .into_iter()
        .map(|field| text_projection(workspace, document, field))
        .collect();
    let main_fields = SaveMainField::ALL
        .into_iter()
        .map(|field| number_projection(workspace, document, SaveNumberTarget::Main(field)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SaveSummaryProjection {
        campaign: number_projection(workspace, document, SaveNumberTarget::CampaignIndex)?,
        text_fields,
        main_fields,
        saved_unit_reference: number_projection(
            workspace,
            document,
            SaveNumberTarget::SelectedUnit,
        )?,
        role_counts,
        unit_count,
        roster_count: workspace.save_roster_count(document)?,
        second_array_count: workspace.save_second_array_count(document)?,
        has_size_prefix: workspace.save_has_size_prefix(document)?,
        has_context: workspace.save_has_context(document)?,
        context_text: workspace.save_context_text(document)?.to_vec(),
    })
}

pub fn unit_projection(
    workspace: &Workspace,
    document: DocumentID,
    unit: usize,
    dictionary: Option<&NameDictionary>,
) -> SaveProjectionResult<SaveUnitProjection> {
    let row = unit_row_projection(workspace, document, unit, dictionary)?;
    let fields = SaveUnitField::ALL
        .into_iter()
        .map(|field| number_projection(workspace, document, SaveNumberTarget::Unit { unit, field }))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SaveUnitProjection {
        row,
        fields,
        skill_data: workspace.save_unit_skill_data(document, unit)?,
    })
}

pub fn visible_unit_indices(
    workspace: &Workspace,
    document: DocumentID,
    dictionary: Option<&NameDictionary>,
    filter: &str,
) -> SaveProjectionResult<Vec<usize>> {
    let unit_count = workspace.save_unit_count(document)?;
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        return Ok((0..unit_count).collect());
    }

    let mut visible = Vec::new();
    for unit in 0..unit_count {
        let row = unit_row_projection(workspace, document, unit, dictionary)?;
        let index_label = format!("unit {}", unit + 1);
        if row.label.to_lowercase().contains(&filter)
            || row.role.to_lowercase().contains(&filter)
            || row.character_id.to_string().contains(&filter)
            || row.skill_level.to_string().contains(&filter)
            || index_label.contains(&filter)
        {
            visible.push(unit);
        }
    }
    Ok(visible)
}

pub fn equipment_projection(
    workspace: &Workspace,
    document: DocumentID,
    unit: usize,
    slot: SaveEquipmentSlot,
    dictionary: Option<&NameDictionary>,
) -> SaveProjectionResult<SaveEquipmentProjection> {
    let target = |field| SaveNumberTarget::Equipment { unit, slot, field };
    let item_type = workspace.save_number(document, target(SaveEquipmentField::ItemTypeID))?;
    let variant = workspace.save_number(document, target(SaveEquipmentField::VariantIndex))?;
    let enhancement_tier =
        workspace.save_number(document, target(SaveEquipmentField::EnhancementTier))?;
    let item_name = dictionary
        .and_then(|dictionary| {
            dictionary.weapon_name(
                i32::try_from(item_type).ok()?,
                u16::try_from(variant).ok()?,
                i16::try_from(enhancement_tier).ok()?,
            )
        })
        .unwrap_or_else(|| format!("Item Type {item_type} · Variant {variant}"));
    let attributes = [
        SaveEquipmentField::Attribute1Index,
        SaveEquipmentField::Attribute2Index,
    ]
    .into_iter()
    .map(|field| equipment_attribute_projection(workspace, document, unit, slot, field, dictionary))
    .collect::<Result<Vec<_>, _>>()?;
    let fields = SaveEquipmentField::ALL
        .into_iter()
        .map(|field| number_projection(workspace, document, target(field)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SaveEquipmentProjection {
        id: projection_id(
            document,
            SaveSection::Equipment,
            unit,
            Some(slot),
            SaveProjectionField::Row,
        ),
        unit,
        slot,
        slot_label: slot.label().to_owned(),
        item_type,
        variant,
        enhancement_tier,
        item_name,
        attributes,
        fields,
    })
}

pub fn mission_projection(
    workspace: &Workspace,
    document: DocumentID,
) -> SaveProjectionResult<SaveMissionProjection> {
    let completions = (0..20)
        .map(|slot| {
            number_projection(
                workspace,
                document,
                SaveNumberTarget::MissionCompletion { slot },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SaveMissionProjection {
        current_mission: number_projection(
            workspace,
            document,
            SaveNumberTarget::CurrentMissionIndex,
        )?,
        completions,
        second_array_count: workspace.save_second_array_count(document)?,
    })
}

pub fn save_section_model(
    workspace: &Workspace,
    document: DocumentID,
    state: &SavePresentationState,
    dictionary: Option<&NameDictionary>,
) -> SaveProjectionResult<SaveSectionModel> {
    match state.section() {
        SaveSection::Summary => Ok(SaveSectionModel::Summary(summary_projection(
            workspace, document,
        )?)),
        SaveSection::Units => {
            let rows = SaveRows::units(workspace, document, dictionary, state.unit_filter())?;
            let inspected = if rows.is_empty() {
                None
            } else {
                Some(unit_projection(
                    workspace,
                    document,
                    state.inspected_unit(),
                    dictionary,
                )?)
            };
            Ok(SaveSectionModel::Units { rows, inspected })
        }
        SaveSection::Equipment => {
            let rows = SaveRows::units(workspace, document, dictionary, state.unit_filter())?;
            let (inspected_unit, selected) =
                if let Some(unit) = rows.reconciled_unit(state.inspected_unit()) {
                    (
                        Some(unit_row_projection(workspace, document, unit, dictionary)?),
                        Some(equipment_projection(
                            workspace,
                            document,
                            unit,
                            state.equipment_slot(),
                            dictionary,
                        )?),
                    )
                } else {
                    (None, None)
                };
            Ok(SaveSectionModel::Equipment {
                slots: SaveEquipmentSlot::ALL,
                inspected_unit,
                selected,
            })
        }
        SaveSection::Roster => Ok(SaveSectionModel::Roster {
            player_leaders: SaveRows::player_leaders(workspace, document)?,
            world_map_rows: SaveRows::roster(workspace, document)?,
        }),
        SaveSection::Missions => Ok(SaveSectionModel::Missions {
            mission: mission_projection(workspace, document)?,
            second_array_rows: SaveRows::second_array(workspace, document)?,
        }),
    }
}

fn unit_row_projection(
    workspace: &Workspace,
    document: DocumentID,
    unit: usize,
    dictionary: Option<&NameDictionary>,
) -> SaveProjectionResult<SaveUnitRowProjection> {
    let value = |field| -> SaveProjectionResult<i64> {
        workspace
            .save_number(document, SaveNumberTarget::Unit { unit, field })
            .map_err(Box::new)
    };
    let leader_name_index = value(SaveUnitField::LeaderNameIndex)?;
    let troop_info_index = value(SaveUnitField::TroopInfoIndex)?;
    let job_type = value(SaveUnitField::JobType)?;
    let role_value = value(SaveUnitField::UCD)?;
    let label = dictionary
        .and_then(|dictionary| {
            dictionary.unit_name(
                i32::try_from(leader_name_index).ok()?,
                i32::try_from(troop_info_index).ok()?,
                u32::try_from(job_type).ok()?,
            )
        })
        .map_or_else(|| format!("Job {job_type}"), ToOwned::to_owned);
    Ok(SaveUnitRowProjection {
        id: projection_id(
            document,
            SaveSection::Units,
            unit,
            None,
            SaveProjectionField::Row,
        ),
        source_index: unit,
        label,
        role_value,
        role: display_editor_value(SaveEditor::UCD, role_value),
        skill_level: value(SaveUnitField::SkillLevel)?,
        character_id: value(SaveUnitField::CharacterID)?,
    })
}

fn roster_row_projection(
    workspace: &Workspace,
    document: DocumentID,
    record: usize,
) -> SaveProjectionResult<SaveRosterRowProjection> {
    let fields = SaveRosterField::ALL
        .into_iter()
        .map(|field| {
            number_projection(
                workspace,
                document,
                SaveNumberTarget::Roster { record, field },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SaveRosterRowProjection {
        id: projection_id(
            document,
            SaveSection::Roster,
            record,
            None,
            SaveProjectionField::Row,
        ),
        source_index: record,
        label: format!("World Map State {}", record + 1),
        fields,
    })
}

fn equipment_attribute_projection(
    workspace: &Workspace,
    document: DocumentID,
    unit: usize,
    slot: SaveEquipmentSlot,
    field: SaveEquipmentField,
    dictionary: Option<&NameDictionary>,
) -> SaveProjectionResult<SaveEquipmentAttributeProjection> {
    let target = SaveNumberTarget::Equipment { unit, slot, field };
    let raw_index = workspace.save_number(document, target)?;
    let dictionary_index = i32::try_from(raw_index).ok();
    let name = dictionary
        .and_then(|dictionary| {
            dictionary_index.and_then(|index| dictionary.item_attribute_name(index))
        })
        .map_or_else(|| format!("Attribute {raw_index}"), ToOwned::to_owned);
    let effect = dictionary
        .and_then(|dictionary| {
            dictionary_index.and_then(|index| dictionary.item_attribute_description(index))
        })
        .map(ToOwned::to_owned);
    Ok(SaveEquipmentAttributeProjection {
        id: projection_id(
            document,
            SaveSection::Equipment,
            unit,
            Some(slot),
            SaveProjectionField::EquipmentAttribute(field),
        ),
        raw_index,
        name,
        effect,
    })
}

fn number_projection(
    workspace: &Workspace,
    document: DocumentID,
    target: SaveNumberTarget,
) -> SaveProjectionResult<SaveNumberProjection> {
    let raw_value = workspace.save_number(document, target)?;
    let editor = workspace.save_number_editor(document, target)?;
    Ok(SaveNumberProjection {
        id: projection_id_for_target(document, target),
        target,
        label: target.label().to_owned(),
        raw_value,
        display_value: display_editor_value(editor, raw_value),
        storage_bounds: workspace.save_number_storage_bounds(document, target)?,
        editor,
    })
}

fn text_projection(
    workspace: &Workspace,
    document: DocumentID,
    field: SaveTextField,
) -> SaveTextProjection {
    SaveTextProjection {
        id: projection_id(
            document,
            SaveSection::Summary,
            0,
            None,
            SaveProjectionField::Text(field),
        ),
        field,
        label: field.label().to_owned(),
        value: workspace
            .save_text(document, field)
            .map_err(|error| error.to_string()),
    }
}

fn display_editor_value(editor: SaveEditor, raw_value: i64) -> String {
    match editor {
        SaveEditor::Number { .. } => raw_value.to_string(),
        SaveEditor::Choice { choices } => choices
            .iter()
            .find(|choice| choice.value == raw_value)
            .map_or_else(
                || format!("Unknown ({raw_value})"),
                |choice| format!("{} ({raw_value})", choice.label),
            ),
    }
}

const fn projection_id_for_target(
    document: DocumentID,
    target: SaveNumberTarget,
) -> SaveProjectionID {
    match target {
        SaveNumberTarget::CampaignIndex => projection_id(
            document,
            SaveSection::Summary,
            0,
            None,
            SaveProjectionField::Campaign,
        ),
        SaveNumberTarget::Main(field) => projection_id(
            document,
            SaveSection::Summary,
            0,
            None,
            SaveProjectionField::Main(field),
        ),
        SaveNumberTarget::SelectedUnit => projection_id(
            document,
            SaveSection::Summary,
            0,
            None,
            SaveProjectionField::SavedUnitReference,
        ),
        SaveNumberTarget::Unit { unit, field } => projection_id(
            document,
            SaveSection::Units,
            unit,
            None,
            SaveProjectionField::Unit(field),
        ),
        SaveNumberTarget::Equipment { unit, slot, field } => projection_id(
            document,
            SaveSection::Equipment,
            unit,
            Some(slot),
            SaveProjectionField::Equipment(field),
        ),
        SaveNumberTarget::Roster { record, field } => projection_id(
            document,
            SaveSection::Roster,
            record,
            None,
            SaveProjectionField::Roster(field),
        ),
        SaveNumberTarget::MissionCompletion { slot } => projection_id(
            document,
            SaveSection::Missions,
            slot,
            None,
            SaveProjectionField::MissionCompletion,
        ),
        SaveNumberTarget::CurrentMissionIndex => projection_id(
            document,
            SaveSection::Missions,
            0,
            None,
            SaveProjectionField::CurrentMission,
        ),
        SaveNumberTarget::SecondArray { record } => projection_id(
            document,
            SaveSection::Missions,
            record,
            None,
            SaveProjectionField::SecondArray,
        ),
    }
}

const fn projection_id(
    document: DocumentID,
    section: SaveSection,
    source_index: usize,
    slot: Option<SaveEquipmentSlot>,
    field: SaveProjectionField,
) -> SaveProjectionID {
    SaveProjectionID {
        document,
        section,
        source_index,
        slot,
        field,
    }
}

fn bounded_range(requested: Range<usize>, len: usize) -> Range<usize> {
    let start = requested.start.min(len);
    let end = requested.end.min(len).max(start);
    start..end
}

pub fn render_editor(
    theme: &Theme,
    rail: Vec<AnyElement>,
    catalog_status: Option<AnyElement>,
    content: AnyElement,
) -> Div {
    div().size_full().flex().min_h_0().child(
        div()
            .id("save-editor")
            .debug_selector(|| "save-editor".to_owned())
            .size_full()
            .flex()
            .min_h_0()
            .child(
                div()
                    .id("save-section-rail")
                    .debug_selector(|| "save-section-rail".to_owned())
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(184.0))
                    .min_h_0()
                    .p(px(10.0))
                    .gap(px(7.0))
                    .bg(theme.surface)
                    .border_r_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .px(px(8.0))
                            .pb(px(5.0))
                            .text_size(px(11.0))
                            .text_color(theme.text_dim)
                            .child("SAVE FILE"),
                    )
                    .children(rail)
                    .child(div().flex_1())
                    .child(read_only_badge(theme)),
            )
            .child(
                div()
                    .id("save-editor-content")
                    .debug_selector(|| "save-editor-content".to_owned())
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .children(catalog_status)
                    .child(div().flex_1().min_h_0().overflow_hidden().child(content)),
            ),
    )
}

pub fn section_rail_item(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<String>,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    let accent = theme.accent;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.0))
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
        .hover(move |style| style.bg(hover).text_color(accent))
        .active(move |style| style.border_color(accent))
        .child(
            div()
                .w(px(12.0))
                .text_color(if selected {
                    theme.accent
                } else {
                    theme.text_dim
                })
                .child(if selected { "◆" } else { "·" }),
        )
        .child(label.into())
        .children(selected.then(|| {
            div()
                .ml_auto()
                .text_size(px(9.0))
                .text_color(theme.accent)
                .child("ACTIVE")
        }))
}

pub fn catalog_status(
    theme: &Theme,
    id: &'static str,
    title: impl Into<String>,
    detail: impl Into<Option<String>>,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .px(px(18.0))
        .py(px(9.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .bg(theme.accent_dim)
        .border_b_1()
        .border_color(theme.accent)
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.accent)
                .child("NAMES"),
        )
        .child(div().text_color(theme.text).child(title.into()))
        .children(detail.into().map(|detail| {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(detail)
        }))
}

pub fn section_header(theme: &Theme, title: &'static str, subtitle: String) -> Div {
    div()
        .flex_none()
        .px(px(20.0))
        .py(px(13.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .bg(theme.surface)
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(px(18.0))
                .text_color(theme.text)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(subtitle),
        )
}

pub fn scrolling_section(
    theme: &Theme,
    id: &'static str,
    title: &'static str,
    subtitle: String,
    children: Vec<AnyElement>,
) -> Div {
    div().size_full().child(
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(section_header(theme, title, subtitle))
            .child(
                div()
                    .id(SharedString::from(format!("save-scroll:{id}")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(18.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .children(children),
            ),
    )
}

pub fn split_section(
    theme: &Theme,
    id: &'static str,
    title: &'static str,
    subtitle: String,
    list: AnyElement,
    details: Vec<AnyElement>,
) -> Div {
    let detail_selector = format!("save-detail-panel:{id}");
    div().size_full().child(
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(section_header(theme, title, subtitle))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(290.0))
                            .min_h_0()
                            .bg(theme.surface)
                            .border_r_1()
                            .border_color(theme.border)
                            .child(list),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("save-detail-scroll:{id}")))
                            .debug_selector(move || detail_selector.clone())
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p(px(18.0))
                            .flex()
                            .flex_col()
                            .gap(px(14.0))
                            .children(details),
                    ),
            ),
    )
}

pub fn group(theme: &Theme, label: &'static str, fields: Vec<AnyElement>) -> Div {
    components::surface(theme)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(38.0))
                .px(px(13.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child(label),
        )
        .child(
            div()
                .p(px(9.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(fields),
        )
}

pub fn mission_completion_group(theme: &Theme, fields: Vec<AnyElement>) -> Div {
    components::surface(theme)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(38.0))
                .px(px(13.0))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child("MISSION COMPLETION · 20 ROWS"),
        )
        .child(
            div()
                .p(px(9.0))
                .grid()
                .grid_cols(2)
                .gap(px(6.0))
                .children(fields),
        )
}

pub fn value_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_h(px(36.0))
        .px(px(10.0))
        .py(px(7.0))
        .flex()
        .items_center()
        .gap(px(14.0))
        .rounded_md()
        .bg(theme.background)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(theme.text_dim)
                .child(label.into()),
        )
        .child(div().flex_none().text_color(theme.text).child(value.into()))
}

pub fn text_value_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_h(px(48.0))
        .px(px(10.0))
        .py(px(7.0))
        .flex()
        .flex_col()
        .gap(px(4.0))
        .rounded_md()
        .bg(theme.background)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child(label.into()),
        )
        .child(div().text_color(theme.text).child(value.into()))
}

pub fn empty_state(theme: &Theme, message: impl Into<String>) -> Div {
    components::surface(theme)
        .p(px(18.0))
        .text_color(theme.text_dim)
        .child(message.into())
}

pub fn unit_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    row: &SaveUnitRowProjection,
    selected: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .h(px(54.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .gap(px(9.0))
        .border_b_1()
        .border_color(theme.border)
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.surface
        })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .w(px(30.0))
                .text_size(px(11.0))
                .text_color(if selected {
                    theme.accent
                } else {
                    theme.text_dim
                })
                .child(format!("{:03}", row.source_index + 1)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .truncate()
                        .text_color(theme.text)
                        .child(row.label.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.text_dim)
                        .child(unit_row_metadata(row)),
                ),
        )
        .children(selected.then(|| {
            div()
                .text_color(theme.accent)
                .text_size(px(11.0))
                .child("INSPECTING")
        }))
}

pub fn unit_row_metadata(row: &SaveUnitRowProjection) -> String {
    format!(
        "{} · Skill {} · Character {}",
        row.role, row.skill_level, row.character_id
    )
}

pub fn player_leader_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    row: &SaveUnitRowProjection,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(54.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .child(
            div()
                .w(px(42.0))
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child(format!("U{:03}", row.source_index + 1)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .truncate()
                        .text_color(theme.text)
                        .child(row.label.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.text_dim)
                        .child(format!("{} · Character {}", row.role, row.character_id)),
                ),
        )
}

pub fn roster_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    row: &SaveRosterRowProjection,
) -> Stateful<Div> {
    let fields = row
        .fields
        .iter()
        .map(|field| {
            let selector = match field.target {
                SaveNumberTarget::Roster { field, .. } => roster_field_selector(field),
                _ => "save-roster-field-unexpected",
            };
            div()
                .id(roster_field_element_id(field.id))
                .debug_selector(move || selector.to_owned())
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme.text_dim)
                        .child(field.label.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_color(theme.text)
                        .child(field.display_value.clone()),
                )
        })
        .collect::<Vec<_>>();
    div()
        .id(id)
        .h(px(64.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex_none()
                .w(px(150.0))
                .truncate()
                .text_color(theme.text)
                .child(row.label.clone()),
        )
        .child(
            div()
                .grid()
                .grid_cols(5)
                .flex_1()
                .min_w_0()
                .gap(px(10.0))
                .children(fields),
        )
}

pub fn roster_field_element_id(id: SaveProjectionID) -> SharedString {
    format!("save-roster-field:{id:?}").into()
}

const fn roster_field_selector(field: SaveRosterField) -> &'static str {
    match field {
        SaveRosterField::Byte60 => "save-roster-field-byte-60",
        SaveRosterField::Byte61 => "save-roster-field-byte-61",
        SaveRosterField::Byte62 => "save-roster-field-byte-62",
        SaveRosterField::Byte63 => "save-roster-field-byte-63",
        SaveRosterField::Value64 => "save-roster-field-value-64",
    }
}

pub fn second_array_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    index: usize,
    value: i64,
) -> Stateful<Div> {
    value_row(
        theme,
        id,
        format!("Second Array {}", index + 1),
        value.to_string(),
    )
    .h(px(42.0))
    .rounded_none()
    .border_b_1()
    .border_color(theme.border)
}

pub fn equipment_slot_button(
    theme: &Theme,
    id: impl Into<ElementId>,
    label: &'static str,
    selected: bool,
    enabled: bool,
) -> Stateful<Div> {
    let hover = theme.raised;
    div()
        .id(id)
        .h(px(36.0))
        .px(px(11.0))
        .flex()
        .items_center()
        .gap(px(7.0))
        .rounded_md()
        .border_1()
        .border_color(if selected { theme.accent } else { theme.border })
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.surface
        })
        .text_color(if selected { theme.text } else { theme.text_dim })
        .when(enabled, |button| {
            button.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .when(!enabled, |button| button.opacity(0.45))
        .child(if selected { "✓" } else { "" })
        .child(label)
}

pub fn skill_bytes(theme: &Theme, values: &[u8; 24]) -> Div {
    group(
        theme,
        "SKILL DATA · 24 BYTES",
        values
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| {
                value_row(
                    theme,
                    ("save-skill-byte", index),
                    format!("Byte {index:02}"),
                    format!("0x{value:02X} · {value}"),
                )
                .into_any_element()
            })
            .collect(),
    )
}

pub fn inline_name_unavailable(theme: &Theme, subject: &'static str) -> Stateful<Div> {
    div()
        .id("save-name-unavailable")
        .debug_selector(|| "save-name-unavailable".to_owned())
        .px(px(12.0))
        .py(px(8.0))
        .rounded_md()
        .bg(theme.accent_dim)
        .border_1()
        .border_color(theme.accent)
        .text_size(px(12.0))
        .text_color(theme.text_dim)
        .child(format!("{subject} name is unavailable; showing raw IDs."))
}

fn read_only_badge(theme: &Theme) -> Div {
    div()
        .px(px(8.0))
        .py(px(6.0))
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .text_size(px(10.0))
        .text_color(theme.text_dim)
        .child("READ ONLY")
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled save and catalog fixtures make failures fatal"
    )]

    use std::{
        cell::RefCell,
        collections::HashSet,
        fs,
        mem::size_of,
        ops::Range,
        path::{Path, PathBuf},
    };

    use gpui::{TestAppContext, div, point, prelude::*, px, size};
    use kufeditor_game::{CatalogRole, NameDictionary, load_name_dictionary};
    use kufeditor_workspace::{
        Document, DocumentID, SaveDocument, SaveEquipmentField, SaveEquipmentGroup,
        SaveEquipmentSlot, SaveNumberTarget, SaveUnitField, SaveUnitGroup, Workspace,
    };
    use tempfile::TempDir;

    use super::{
        SaveProjectionField, SaveRowKind, SaveRowProjection, SaveRows, SaveSectionModel,
        equipment_projection, mission_projection, render_editor, roster_field_element_id,
        roster_row, save_section_model, summary_projection, unit_projection, unit_row_metadata,
        visible_unit_indices,
    };
    use crate::{
        state::{SavePresentationState, SavePresentationStates, SaveSection, SaveUnitVisibility},
        theme::Theme,
    };

    const CONTEXT_SIZE: usize = 0x438;
    const MAIN_SIZE: usize = 0x154;
    const UNIT_SIZE: usize = 483;
    const EQUIPMENT_SIZE: usize = 64;

    struct CatalogTree {
        _temporary: TempDir,
        sox: PathBuf,
    }

    impl CatalogTree {
        fn names() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let sox = temporary.path().join("Data/SOX");
            fs::create_dir_all(&sox).unwrap();
            let tree = Self {
                _temporary: temporary,
                sox,
            };
            tree.write(CatalogRole::TroopNames, &indexed_table(&[(2, b"Footman")]));
            tree.write(
                CatalogRole::ItemAttributes,
                &indexed_fields_table(&[(91, &[b"Flame", b"Adds fire"])]),
            );
            tree.write(
                CatalogRole::WeaponNames,
                b"2\n\n2\t// swords\n9\nSword\nLong Sword\n27\nSabre\nLong Sabre\n  \n1 // axes\n3\nAxe\nWar Axe\n",
            );
            tree
        }

        fn write(&self, role: CatalogRole, bytes: &[u8]) {
            let path = role_path(&self.sox, role);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }

        fn load(&self) -> NameDictionary {
            load_name_dictionary(&self.sox).unwrap().dictionary
        }
    }

    fn role_path(sox: &Path, role: CatalogRole) -> PathBuf {
        if role == CatalogRole::WeaponNames {
            sox.parent().unwrap().join(role.relative_path())
        } else {
            sox.join(role.relative_path())
        }
    }

    fn indexed_table(records: &[(u32, &[u8])]) -> Vec<u8> {
        let mut bytes = table_header(records.len());
        for (id, value) in records {
            append_u32(&mut bytes, *id);
            append_field(&mut bytes, value);
        }
        bytes
    }

    fn indexed_fields_table(records: &[(u32, &[&[u8]])]) -> Vec<u8> {
        let mut bytes = table_header(records.len());
        for (id, fields) in records {
            append_u32(&mut bytes, *id);
            for field in *fields {
                append_field(&mut bytes, field);
            }
        }
        bytes
    }

    fn table_header(record_count: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 100);
        append_u32(&mut bytes, u32::try_from(record_count).unwrap());
        bytes
    }

    fn append_field(bytes: &mut Vec<u8>, value: &[u8]) {
        bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(value);
    }

    fn workspace_with_save(
        unit_count: usize,
        roster_count: usize,
        second_array_count: usize,
    ) -> (Workspace, DocumentID) {
        let document =
            SaveDocument::parse(save_fixture(unit_count, roster_count, second_array_count))
                .unwrap();
        let mut workspace = Workspace::new();
        let document =
            workspace.open_loaded(PathBuf::from("campaign.sav"), Document::Save(document));
        (workspace, document)
    }

    fn workspace_with_leader_save(
        roster_count: usize,
        second_array_count: usize,
    ) -> (Workspace, DocumentID) {
        workspace_with_role_save(0, roster_count, second_array_count)
    }

    fn workspace_with_role_save(
        role: u32,
        roster_count: usize,
        second_array_count: usize,
    ) -> (Workspace, DocumentID) {
        workspace_with_roles_save(&[role], roster_count, second_array_count)
    }

    fn workspace_with_roles_save(
        roles: &[u32],
        roster_count: usize,
        second_array_count: usize,
    ) -> (Workspace, DocumentID) {
        let mut bytes = save_fixture(roles.len(), roster_count, second_array_count);
        let unit_offset =
            2 * size_of::<u32>() + CONTEXT_SIZE + size_of::<u32>() + MAIN_SIZE + size_of::<u32>();
        for (unit, role) in roles.iter().copied().enumerate() {
            let ucd_offset = unit_offset + unit * UNIT_SIZE + 10 * size_of::<u32>();
            bytes
                .get_mut(ucd_offset..ucd_offset + size_of::<u32>())
                .unwrap()
                .copy_from_slice(&role.to_le_bytes());
        }
        let document = SaveDocument::parse(bytes).unwrap();
        let mut workspace = Workspace::new();
        let document =
            workspace.open_loaded(PathBuf::from("campaign.sav"), Document::Save(document));
        (workspace, document)
    }

    fn workspace_with_invalid_text_save() -> (Workspace, DocumentID) {
        let mut bytes = save_fixture(0, 0, 0);
        let main_offset = 2 * size_of::<u32>() + CONTEXT_SIZE + size_of::<u32>();
        *bytes.get_mut(main_offset + 0x20).unwrap() = 0x80;
        bytes
            .get_mut(main_offset + 0x60..main_offset + 0x68)
            .unwrap()
            .copy_from_slice(b"GOOD.SOX");
        let document = SaveDocument::parse(bytes).unwrap();
        let mut workspace = Workspace::new();
        let document =
            workspace.open_loaded(PathBuf::from("campaign.sav"), Document::Save(document));
        (workspace, document)
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

        if bytes.len() < 0x8000 {
            bytes.resize(0x8000, 0);
        }
        let length = u32::try_from(bytes.len()).unwrap();
        bytes
            .get_mut(..size_of::<u32>())
            .unwrap()
            .copy_from_slice(&length.to_le_bytes());
        bytes
    }

    fn append_complete_unit(bytes: &mut Vec<u8>) {
        let start = bytes.len();
        append_i32(bytes, -1);
        for value in [2_u32, 2, 4, 0x34, 0x38, 0x3c, 0x40] {
            append_u32(bytes, value);
        }
        append_i32(bytes, -1);
        for value in [5_u32, 99, 6, 7, 8] {
            append_u32(bytes, value);
        }
        bytes.extend_from_slice(&[1, 0, 1]);
        for value in [60_u32, 64, 68] {
            append_u32(bytes, value);
        }
        bytes.extend(0xa0_u8..=0xb7);
        append_named_equipment(bytes);
        append_zero_records(bytes, 5, EQUIPMENT_SIZE);
        append_u32(bytes, 504);
        assert_eq!(bytes.len() - start, UNIT_SIZE);
    }

    fn append_named_equipment(bytes: &mut Vec<u8>) {
        append_u32(bytes, 1_000);
        append_i32(bytes, 0);
        append_u16(bytes, 5);
        append_i16(bytes, -1);
        append_u16(bytes, 0);
        append_i16(bytes, 12);
        append_u16(bytes, 1);
        append_u16(bytes, 0);
        append_i32(bytes, 91);
        append_i32(bytes, -1);
        append_i32(bytes, -1);
        append_i32(bytes, 3);
        append_i32(bytes, 9);
        append_i32(bytes, 4);
        append_i32(bytes, -1);
        append_i32(bytes, 5);
        append_i32(bytes, 4);
        append_i32(bytes, 6);
        append_i32(bytes, 0);
    }

    fn append_zero_records(bytes: &mut Vec<u8>, count: usize, size: usize) {
        bytes.resize(bytes.len() + count * size, 0);
    }

    fn append_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn append_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn append_u16(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn append_i16(bytes: &mut Vec<u8>, value: i16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn presentation(document: DocumentID, section: SaveSection) -> SavePresentationState {
        let mut presentations = SavePresentationStates::default();
        presentations.select_section(document, section, false);
        presentations.get(document).unwrap().clone()
    }

    #[gpui::test]
    fn save_view_rail_and_content_share_the_editor_row(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(800.0), px(600.0)),
            |_, _| {
                render_editor(
                    &Theme::default(),
                    Vec::new(),
                    None,
                    div().size_full().into_any_element(),
                )
            },
        );

        let editor = cx.debug_bounds("save-editor").unwrap();
        let rail = cx.debug_bounds("save-section-rail").unwrap();
        let content = cx.debug_bounds("save-editor-content").unwrap();
        assert_eq!(rail.origin, editor.origin);
        assert_eq!(content.origin.y, editor.origin.y);
        assert_eq!(content.origin.x, rail.origin.x + rail.size.width);
        assert_eq!(rail.size.width + content.size.width, editor.size.width);
    }

    #[test]
    fn save_view_empty_summary_exposes_every_fixed_surface() {
        let (workspace, document) = workspace_with_save(0, 0, 0);
        let state = presentation(document, SaveSection::Summary);

        let SaveSectionModel::Summary(summary) =
            save_section_model(&workspace, document, &state, None).unwrap()
        else {
            panic!("summary state must produce the summary view");
        };

        assert_eq!(summary.campaign.label, "Campaign");
        assert_eq!(summary.main_fields.len(), 7);
        assert_eq!(summary.text_fields.len(), 3);
        assert_eq!(
            summary.saved_unit_reference.label,
            "Selected Unit Reference"
        );
        assert_eq!(
            (
                summary.unit_count,
                summary.roster_count,
                summary.second_array_count,
            ),
            (0, 0, 0),
        );
        assert!(summary.has_size_prefix);
        assert!(summary.has_context);
        assert!(summary.context_text.is_empty());
    }

    #[test]
    fn save_view_summary_isolates_one_invalid_fixed_text_field() {
        let (workspace, document) = workspace_with_invalid_text_save();

        let summary = summary_projection(&workspace, document).unwrap();

        assert_eq!(summary.campaign.raw_value, 0);
        assert_eq!(summary.main_fields.len(), 7);
        assert_eq!(summary.text_fields.len(), 3);
        assert_eq!(
            summary.text_fields.first().unwrap().value,
            Err(
                "save text field MapName contains non-ASCII stored byte 0x80 at index 0".to_owned()
            )
        );
        assert_eq!(
            summary.text_fields.get(1).unwrap().value,
            Ok("GOOD.SOX".to_owned())
        );
        assert_eq!(summary.text_fields.get(2).unwrap().value, Ok(String::new()));
        assert_eq!(summary.unit_count, 0);
        assert!(summary.has_size_prefix);
        assert!(summary.has_context);
    }

    #[test]
    fn save_view_units_keep_virtual_rows_groups_raw_identity_and_skill_bytes() {
        let (workspace, document) = workspace_with_leader_save(0, 0);
        let units = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Units),
            None,
        )
        .unwrap();
        let SaveSectionModel::Units { rows, inspected } = units else {
            panic!("units state must produce the units view");
        };
        let inspected = inspected.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(inspected.row.label, "Job 2");
        assert_eq!(inspected.fields.len(), 21);
        assert_eq!(
            inspected.skill_data,
            std::array::from_fn(|index| 0xa0 + u8::try_from(index).unwrap())
        );
        let unit_group_count = |group| {
            inspected
                .fields
                .iter()
                .filter(|field| {
                    matches!(
                        field.target,
                        SaveNumberTarget::Unit { field, .. } if field.group() == group
                    )
                })
                .count()
        };
        assert_eq!(unit_group_count(SaveUnitGroup::Core), 10);
        assert_eq!(unit_group_count(SaveUnitGroup::Formation), 2);
        assert_eq!(unit_group_count(SaveUnitGroup::Advanced), 9);
        assert_eq!(
            unit_row_metadata(&inspected.row),
            "Leader (0) · Skill 8 · Character -1",
        );
    }

    #[test]
    fn save_view_equipment_keeps_six_slots_and_all_field_groups() {
        let (workspace, document) = workspace_with_leader_save(0, 0);
        let equipment = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Equipment),
            None,
        )
        .unwrap();
        let SaveSectionModel::Equipment {
            slots,
            inspected_unit,
            selected,
        } = equipment
        else {
            panic!("equipment state must produce the equipment view");
        };
        assert_eq!(slots, SaveEquipmentSlot::ALL);
        assert_eq!(
            slots.map(SaveEquipmentSlot::label),
            [
                "Leader Weapon",
                "Leader Accessory",
                "Leader Armor",
                "Troop Weapon",
                "Troop Accessory",
                "Troop Armor",
            ],
        );
        assert_eq!(inspected_unit.unwrap().label, "Job 2");
        let selected = selected.unwrap();
        assert_eq!(selected.item_name, "Item Type 0 · Variant 0");
        assert_eq!(selected.fields.len(), 19);
        assert_eq!(selected.attributes.len(), 2);
        let equipment_group_count = |group| {
            selected
                .fields
                .iter()
                .filter(|field| {
                    matches!(
                        field.target,
                        SaveNumberTarget::Equipment { field, .. } if field.group() == group
                    )
                })
                .count()
        };
        assert_eq!(equipment_group_count(SaveEquipmentGroup::Core), 6);
        assert_eq!(equipment_group_count(SaveEquipmentGroup::Skills), 4);
        assert_eq!(equipment_group_count(SaveEquipmentGroup::Resistances), 4);
        assert_eq!(equipment_group_count(SaveEquipmentGroup::Advanced), 5);
    }

    #[test]
    fn save_view_equipment_does_not_resurrect_a_unit_hidden_by_the_filter() {
        let (workspace, document) = workspace_with_role_save(3, 0, 0);
        let filtered = SaveRows::units(&workspace, document, None, "leader").unwrap();
        assert!(filtered.is_empty());
        let mut presentations = SavePresentationStates::default();
        presentations.set_unit_filter(
            document,
            "leader".to_owned(),
            filtered.unit_visibility().unwrap(),
            false,
        );
        presentations.select_section(document, SaveSection::Equipment, false);

        let SaveSectionModel::Equipment {
            inspected_unit,
            selected,
            ..
        } = save_section_model(
            &workspace,
            document,
            presentations.get(document).unwrap(),
            None,
        )
        .unwrap()
        else {
            panic!("equipment state must produce the equipment view");
        };

        assert_eq!(inspected_unit, None);
        assert_eq!(selected, None);
    }

    #[test]
    fn save_view_equipment_uses_the_first_visible_unit_when_unit_zero_is_hidden() {
        let (workspace, document) = workspace_with_roles_save(&[3, 0], 0, 0);
        let filtered = SaveRows::units(&workspace, document, None, "leader").unwrap();
        assert_eq!(
            filtered
                .locations(0..filtered.len())
                .into_iter()
                .map(|location| location.source_index)
                .collect::<Vec<_>>(),
            [1],
        );
        let mut presentations = SavePresentationStates::default();
        presentations.set_unit_filter(
            document,
            "leader".to_owned(),
            filtered.unit_visibility().unwrap(),
            false,
        );
        presentations.select_section(document, SaveSection::Equipment, false);

        let SaveSectionModel::Equipment {
            inspected_unit,
            selected,
            ..
        } = save_section_model(
            &workspace,
            document,
            presentations.get(document).unwrap(),
            None,
        )
        .unwrap()
        else {
            panic!("equipment state must produce the equipment view");
        };

        assert_eq!(inspected_unit.unwrap().source_index, 1);
        assert_eq!(selected.unwrap().unit, 1);
    }

    #[test]
    fn save_view_roster_summarizes_player_leaders_and_keeps_world_rows_lazy() {
        let (workspace, document) = workspace_with_leader_save(2, 0);
        let roster = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Roster),
            None,
        )
        .unwrap();
        let SaveSectionModel::Roster {
            player_leaders,
            world_map_rows,
        } = roster
        else {
            panic!("roster state must produce the roster view");
        };
        assert_eq!(player_leaders.kind(), SaveRowKind::PlayerLeaders);
        assert_eq!(player_leaders.len(), 1);
        let leader = player_leaders
            .project_range(&workspace, None, 0..1)
            .unwrap();
        let Some(SaveRowProjection::Unit(leader)) = leader.first() else {
            panic!("player-leader rows must project units lazily");
        };
        assert_eq!(leader.role, "Leader (0)");
        assert_eq!(leader.id.section, SaveSection::Roster);
        assert_eq!(world_map_rows.len(), 2);
    }

    #[gpui::test]
    fn save_view_world_map_row_renders_five_labeled_stable_fields(cx: &mut TestAppContext) {
        let (workspace, document) = workspace_with_save(0, 1, 0);
        let rows = SaveRows::roster(&workspace, document).unwrap();
        let projected = rows.project_range(&workspace, None, 0..1).unwrap();
        let Some(SaveRowProjection::Roster(row)) = projected.first() else {
            panic!("world-map rows must project roster fields");
        };
        assert_eq!(
            row.fields
                .iter()
                .map(|field| field.label.as_str())
                .collect::<Vec<_>>(),
            ["Byte 60", "Byte 61", "Byte 62", "Byte 63", "Value 64"],
        );
        assert_eq!(
            row.fields
                .iter()
                .map(|field| field.id)
                .collect::<HashSet<_>>()
                .len(),
            5,
        );
        assert_eq!(
            row.fields
                .iter()
                .map(|field| roster_field_element_id(field.id))
                .collect::<HashSet<_>>()
                .len(),
            5,
        );

        let row = row.clone();
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(900.0), px(120.0)),
            |_, _| roster_row(&Theme::default(), "roster-row-test", &row),
        );
        for selector in [
            "save-roster-field-byte-60",
            "save-roster-field-byte-61",
            "save-roster-field-byte-62",
            "save-roster-field-byte-63",
            "save-roster-field-value-64",
        ] {
            assert!(cx.debug_bounds(selector).is_some(), "missing {selector}");
        }
    }

    #[test]
    fn save_view_missions_keep_twenty_fixed_rows_and_lazy_second_array() {
        let (workspace, document) = workspace_with_leader_save(0, 3);
        let missions = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Missions),
            None,
        )
        .unwrap();
        let SaveSectionModel::Missions {
            mission,
            second_array_rows,
        } = missions
        else {
            panic!("missions state must produce the missions view");
        };
        assert_eq!(mission.current_mission.raw_value, -2);
        assert_eq!(mission.completions.len(), 20);
        assert_eq!(second_array_rows.len(), 3);
    }

    #[test]
    fn save_view_unknown_choices_and_missing_names_keep_raw_fallbacks() {
        let (workspace, document) = workspace_with_save(1, 0, 0);

        let SaveSectionModel::Units { inspected, .. } = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Units),
            None,
        )
        .unwrap() else {
            panic!("units state must produce the units view");
        };
        let inspected = inspected.unwrap();
        assert_eq!(inspected.row.label, "Job 2");
        assert!(
            inspected
                .fields
                .iter()
                .any(|field| field.display_value == "Unknown (99)")
        );
    }

    #[test]
    fn save_view_ready_catalog_enriches_names_and_effects() {
        let (workspace, document) = workspace_with_save(1, 0, 0);
        let dictionary = CatalogTree::names().load();

        let SaveSectionModel::Units { inspected, .. } = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Units),
            Some(&dictionary),
        )
        .unwrap() else {
            panic!("units state must produce the units view");
        };
        assert_eq!(inspected.unwrap().row.label, "Footman");

        let SaveSectionModel::Equipment { selected, .. } = save_section_model(
            &workspace,
            document,
            &presentation(document, SaveSection::Equipment),
            Some(&dictionary),
        )
        .unwrap() else {
            panic!("equipment state must produce the equipment view");
        };
        let equipment = selected.unwrap();
        assert_eq!(equipment.item_name, "Long Sword");
        assert_eq!(equipment.attributes.first().unwrap().name, "Flame");
        assert_eq!(
            equipment.attributes.first().unwrap().effect.as_deref(),
            Some("Adds fire"),
        );
        let missing_attribute = equipment.attributes.get(1).unwrap();
        assert_eq!(missing_attribute.name, "Attribute -1");
        assert_eq!(missing_attribute.effect, None);
    }

    #[test]
    fn save_projection_formats_unknown_and_none_choices_without_losing_raw_values() {
        let (workspace, document) = workspace_with_save(1, 0, 0);

        let unit = unit_projection(&workspace, document, 0, None).unwrap();
        let ucd = unit
            .fields
            .iter()
            .find(|field| {
                field.target
                    == SaveNumberTarget::Unit {
                        unit: 0,
                        field: SaveUnitField::UCD,
                    }
            })
            .unwrap();
        assert_eq!(ucd.raw_value, 99);
        assert_eq!(ucd.display_value, "Unknown (99)");

        let equipment = equipment_projection(
            &workspace,
            document,
            0,
            SaveEquipmentSlot::LeaderWeapon,
            None,
        )
        .unwrap();
        for field in [
            SaveEquipmentField::SkillType1,
            SaveEquipmentField::ResistType1,
        ] {
            let projection = equipment
                .fields
                .iter()
                .find(|projection| {
                    projection.target
                        == SaveNumberTarget::Equipment {
                            unit: 0,
                            slot: SaveEquipmentSlot::LeaderWeapon,
                            field,
                        }
                })
                .unwrap();
            assert_eq!(projection.raw_value, -1);
            assert_eq!(projection.display_value, "None (-1)");
        }
    }

    #[test]
    fn save_projection_resolves_unit_names_only_from_the_supplied_dictionary() {
        let (workspace, document) = workspace_with_save(1, 0, 0);
        let dictionary = CatalogTree::names().load();

        let raw = unit_projection(&workspace, document, 0, None).unwrap();
        let resolved = unit_projection(&workspace, document, 0, Some(&dictionary)).unwrap();

        assert_eq!(raw.row.label, "Job 2");
        assert_eq!(resolved.row.label, "Footman");
        assert_eq!(
            visible_unit_indices(&workspace, document, None, "job 2").unwrap(),
            [0]
        );
        assert!(
            visible_unit_indices(&workspace, document, None, "missing")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn save_projection_exposes_compact_unit_visibility() {
        let (workspace, document) = workspace_with_save(1, 0, 0);

        let all_units = SaveRows::units(&workspace, document, None, "").unwrap();
        assert_eq!(
            all_units.unit_visibility(),
            Some(SaveUnitVisibility::All { unit_count: 1 }),
        );

        let filtered_units = SaveRows::units(&workspace, document, None, "job 2").unwrap();
        assert_eq!(
            filtered_units.unit_visibility(),
            Some(SaveUnitVisibility::Filtered(&[0])),
        );

        let roster = SaveRows::roster(&workspace, document).unwrap();
        assert_eq!(roster.unit_visibility(), None);
    }

    #[test]
    fn save_projection_resolves_equipment_names_and_attribute_effects() {
        let (workspace, document) = workspace_with_save(1, 0, 0);
        let dictionary = CatalogTree::names().load();

        let raw = equipment_projection(
            &workspace,
            document,
            0,
            SaveEquipmentSlot::LeaderWeapon,
            None,
        )
        .unwrap();
        let resolved = equipment_projection(
            &workspace,
            document,
            0,
            SaveEquipmentSlot::LeaderWeapon,
            Some(&dictionary),
        )
        .unwrap();

        assert_eq!(raw.item_name, "Item Type 0 · Variant 0");
        assert_eq!(resolved.item_name, "Long Sword");
        let attribute = resolved.attributes.first().unwrap();
        assert_eq!(attribute.raw_index, 91);
        assert_eq!(attribute.name, "Flame");
        assert_eq!(attribute.effect.as_deref(), Some("Adds fire"));
    }

    #[test]
    fn save_projection_equipment_component_ids_are_unique() {
        let (workspace, document) = workspace_with_save(1, 0, 0);
        let equipment = equipment_projection(
            &workspace,
            document,
            0,
            SaveEquipmentSlot::LeaderWeapon,
            None,
        )
        .unwrap();

        let ids = std::iter::once(equipment.id)
            .chain(equipment.attributes.iter().map(|attribute| attribute.id))
            .chain(equipment.fields.iter().map(|field| field.id))
            .collect::<HashSet<_>>();

        assert_eq!(
            ids.len(),
            1 + equipment.attributes.len() + equipment.fields.len(),
        );
    }

    #[test]
    fn save_projection_keeps_all_twenty_mission_rows_stable() {
        let (workspace, document) = workspace_with_save(1, 2, 3);

        let projection = mission_projection(&workspace, document).unwrap();
        assert_eq!(projection.completions.len(), 20);
        assert_eq!(projection.second_array_count, 3);

        let ids = projection
            .completions
            .iter()
            .map(|field| field.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 20);
        for (slot, field) in projection.completions.iter().enumerate() {
            assert_eq!(field.id.document, document);
            assert_eq!(field.id.section, SaveSection::Missions);
            assert_eq!(field.id.source_index, slot);
            assert_eq!(field.id.field, SaveProjectionField::MissionCompletion);
            assert_eq!(field.raw_value, i64::try_from(slot).unwrap() - 1);
        }
    }

    #[test]
    fn save_projection_summary_owns_document_values_and_counts() {
        let (workspace, document) = workspace_with_save(1, 2, 3);

        let projection = summary_projection(&workspace, document).unwrap();

        assert_eq!(projection.campaign.raw_value, 0);
        assert_eq!(projection.text_fields.len(), 3);
        assert_eq!(projection.main_fields.len(), 7);
        assert_eq!(projection.unit_count, 1);
        assert_eq!(projection.roster_count, 2);
        assert_eq!(projection.second_array_count, 3);
        assert!(projection.has_size_prefix);
        assert!(projection.has_context);
    }

    #[test]
    fn save_projection_large_lists_build_only_the_requested_ranges() {
        let (workspace, document) = workspace_with_save(2_048, 4_096, 8_192);
        let requests = RefCell::new(Vec::<(SaveRowKind, Range<usize>)>::new());

        let units = SaveRows::units(&workspace, document, None, "").unwrap();
        let roster = SaveRows::roster(&workspace, document).unwrap();
        let second_array = SaveRows::second_array(&workspace, document).unwrap();
        assert_eq!(
            (units.len(), roster.len(), second_array.len()),
            (2_048, 4_096, 8_192)
        );

        let unit_rows = units
            .project_range_with_observer(&workspace, None, 1_000..1_007, |kind, range| {
                requests.borrow_mut().push((kind, range));
            })
            .unwrap();
        let roster_rows = roster
            .project_range_with_observer(&workspace, None, 2_000..2_005, |kind, range| {
                requests.borrow_mut().push((kind, range));
            })
            .unwrap();
        let second_rows = second_array
            .project_range_with_observer(&workspace, None, 4_000..4_003, |kind, range| {
                requests.borrow_mut().push((kind, range));
            })
            .unwrap();

        assert_eq!(
            (unit_rows.len(), roster_rows.len(), second_rows.len()),
            (7, 5, 3)
        );
        assert_eq!(
            requests.into_inner(),
            [
                (SaveRowKind::Units, 1_000..1_007),
                (SaveRowKind::Roster, 2_000..2_005),
                (SaveRowKind::SecondArray, 4_000..4_003),
            ],
        );
    }
}
