#![allow(
    dead_code,
    reason = "Task 11 connects this Task 10 projection surface to GPUI rendering"
)]

use std::ops::Range;

use gpui::{App, ElementId, IntoElement, UniformList, Window, uniform_list};
use kufeditor_game::NameDictionary;
use kufeditor_workspace::{
    DocumentID, SaveEditor, SaveEquipmentField, SaveEquipmentSlot, SaveMainField, SaveNumberTarget,
    SaveRosterField, SaveTextField, SaveUnitField, Workspace, WorkspaceError,
};

use crate::state::SaveSection;

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
    pub value: String,
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveRowKind {
    Units,
    Roster,
    SecondArray,
}

impl SaveRowKind {
    pub const fn section(self) -> SaveSection {
        match self {
            Self::Units => SaveSection::Units,
            Self::Roster => SaveSection::Roster,
            Self::SecondArray => SaveSection::Missions,
        }
    }

    const fn projection_field(self) -> SaveProjectionField {
        match self {
            Self::Units | Self::Roster => SaveProjectionField::Row,
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
            let projection = match location.kind {
                SaveRowKind::Units => SaveRowProjection::Unit(unit_row_projection(
                    workspace,
                    self.document,
                    location.source_index,
                    dictionary,
                )?),
                SaveRowKind::Roster => SaveRowProjection::Roster(roster_row_projection(
                    workspace,
                    self.document,
                    location.source_index,
                )?),
                SaveRowKind::SecondArray => SaveRowProjection::SecondArray(number_projection(
                    workspace,
                    self.document,
                    SaveNumberTarget::SecondArray {
                        record: location.source_index,
                    },
                )?),
            };
            projections.push(projection);
        }
        Ok(projections)
    }
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
        .collect::<Result<Vec<_>, _>>()?;
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
            SaveProjectionField::Equipment(field),
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
) -> SaveProjectionResult<SaveTextProjection> {
    Ok(SaveTextProjection {
        id: projection_id(
            document,
            SaveSection::Summary,
            0,
            None,
            SaveProjectionField::Text(field),
        ),
        field,
        label: field.label().to_owned(),
        value: workspace.save_text(document, field)?,
    })
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

    use kufeditor_game::{CatalogRole, NameDictionary, load_name_dictionary};
    use kufeditor_workspace::{
        Document, DocumentID, SaveDocument, SaveEquipmentField, SaveEquipmentSlot,
        SaveNumberTarget, SaveUnitField, Workspace,
    };
    use tempfile::TempDir;

    use super::{
        SaveProjectionField, SaveRowKind, SaveRows, equipment_projection, mission_projection,
        summary_projection, unit_projection, visible_unit_indices,
    };
    use crate::state::SaveSection;

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
