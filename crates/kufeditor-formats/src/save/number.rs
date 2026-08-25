use super::{
    SaveDocument, SaveEditor, SaveEquipmentField, SaveEquipmentSlot, SaveMainField, SaveMutation,
    SaveNumberTarget, SaveRosterField, SaveUnitField,
};
use crate::{
    diagnostic::{Diagnostic, DiagnosticLocation, Severity},
    error::FormatError,
    generated::kuf_save,
};

const I32_BOUNDS: (i64, i64) = (-2_147_483_648, 2_147_483_647);
const U32_BOUNDS: (i64, i64) = (0, 4_294_967_295);
const I16_BOUNDS: (i64, i64) = (-32_768, 32_767);
const U16_BOUNDS: (i64, i64) = (0, 65_535);
const U8_BOUNDS: (i64, i64) = (0, 255);

#[derive(Clone, Copy, Debug)]
enum WireValue {
    U8(u8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
}

impl SaveDocument {
    pub fn number(&self, target: SaveNumberTarget) -> Result<i64, FormatError> {
        match target {
            SaveNumberTarget::CampaignIndex => Ok(i64::from(self.file.campaign_index)),
            SaveNumberTarget::Main(field) => main_number(&self.file.main_save_block, field),
            SaveNumberTarget::SelectedUnit => Ok(signed_u32(self.file.selected_unit_ref)),
            SaveNumberTarget::Unit { unit, field } => {
                let unit = unit_at(&self.file, unit, target)?;
                Ok(unit_number(unit, field))
            }
            SaveNumberTarget::Equipment { unit, slot, field } => {
                let unit = unit_at(&self.file, unit, target)?;
                Ok(equipment_number(equipment_slot(unit, slot), field))
            }
            SaveNumberTarget::Roster { record, field } => {
                let record = roster_at(&self.file, record, target)?;
                Ok(roster_number(record, field))
            }
            SaveNumberTarget::MissionCompletion { slot } => self
                .file
                .mission_completion
                .get(slot)
                .copied()
                .map(signed_u32)
                .ok_or_else(|| target_out_of_range(target, self.file.mission_completion.len())),
            SaveNumberTarget::CurrentMissionIndex => Ok(signed_u32(self.file.current_mission_slot)),
            SaveNumberTarget::SecondArray { record } => self
                .file
                .second_array
                .get(record)
                .copied()
                .map(i64::from)
                .ok_or_else(|| target_out_of_range(target, self.file.second_array.len())),
        }
    }

    pub fn number_storage_bounds(
        &self,
        target: SaveNumberTarget,
    ) -> Result<(i64, i64), FormatError> {
        validate_target(&self.file, target)?;
        Ok(target_bounds(target))
    }

    pub fn number_editor(&self, target: SaveNumberTarget) -> Result<SaveEditor, FormatError> {
        let bounds = self.number_storage_bounds(target)?;
        Ok(match target {
            SaveNumberTarget::CampaignIndex => SaveEditor::CAMPAIGN,
            SaveNumberTarget::Unit {
                field: SaveUnitField::UCD,
                ..
            } => SaveEditor::UCD,
            SaveNumberTarget::Unit {
                field: SaveUnitField::HeroFlag,
                ..
            } => SaveEditor::HERO,
            SaveNumberTarget::Unit {
                field: SaveUnitField::SkillLevel,
                ..
            } => number_editor(U16_BOUNDS),
            SaveNumberTarget::Equipment {
                field: SaveEquipmentField::SkillType1 | SaveEquipmentField::SkillType2,
                ..
            } => SaveEditor::SKILL,
            SaveNumberTarget::Equipment {
                field: SaveEquipmentField::ResistType1 | SaveEquipmentField::ResistType2,
                ..
            } => SaveEditor::RESISTANCE,
            _ => number_editor(bounds),
        })
    }

    pub fn set_number(
        &mut self,
        target: SaveNumberTarget,
        value: i64,
    ) -> Result<SaveMutation<i64>, FormatError> {
        let previous = self.number(target)?;
        let bounds = self.number_storage_bounds(target)?;
        let wire = convert_wire_value(target, value, bounds)?;

        if previous == value {
            return Ok(SaveMutation::Unchanged);
        }

        assign_number(&mut self.file, target, wire, value, bounds)?;
        Ok(SaveMutation::Changed { previous })
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (unit_index, unit) in self.file.units.iter().enumerate() {
            if unit.ucd > 3 {
                diagnostics.push(warning(
                    SaveNumberTarget::Unit {
                        unit: unit_index,
                        field: SaveUnitField::UCD,
                    },
                    "unknown UCD value",
                ));
            }

            for slot in SaveEquipmentSlot::ALL {
                let equipment = equipment_slot(unit, slot);
                for (field, value) in [
                    (SaveEquipmentField::SkillType1, equipment.skill_type_1),
                    (SaveEquipmentField::SkillType2, equipment.skill_type_2),
                ] {
                    if !(-1..=14).contains(&value) {
                        diagnostics.push(warning(
                            SaveNumberTarget::Equipment {
                                unit: unit_index,
                                slot,
                                field,
                            },
                            "unknown skill type",
                        ));
                    }
                }
                for (field, value) in [
                    (SaveEquipmentField::ResistType1, equipment.resist_type_1),
                    (SaveEquipmentField::ResistType2, equipment.resist_type_2),
                ] {
                    if !(-1..=9).contains(&value) {
                        diagnostics.push(warning(
                            SaveNumberTarget::Equipment {
                                unit: unit_index,
                                slot,
                                field,
                            },
                            "unknown resistance type",
                        ));
                    }
                }
            }
        }

        if signed_u32(self.file.current_mission_slot) < 0 {
            diagnostics.push(warning(
                SaveNumberTarget::CurrentMissionIndex,
                "current mission index is negative",
            ));
        }

        diagnostics
    }
}

const fn number_editor((minimum, maximum): (i64, i64)) -> SaveEditor {
    SaveEditor::Number { minimum, maximum }
}

fn warning(target: SaveNumberTarget, message: &'static str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        location: DiagnosticLocation::Save(target),
        message,
    }
}

fn validate_target(file: &kuf_save::File, target: SaveNumberTarget) -> Result<(), FormatError> {
    match target {
        SaveNumberTarget::Unit { unit, .. } | SaveNumberTarget::Equipment { unit, .. } => {
            unit_at(file, unit, target).map(|_| ())
        }
        SaveNumberTarget::Roster { record, .. } => roster_at(file, record, target).map(|_| ()),
        SaveNumberTarget::MissionCompletion { slot } => file
            .mission_completion
            .get(slot)
            .map(|_| ())
            .ok_or_else(|| target_out_of_range(target, file.mission_completion.len())),
        SaveNumberTarget::SecondArray { record } => file
            .second_array
            .get(record)
            .map(|_| ())
            .ok_or_else(|| target_out_of_range(target, file.second_array.len())),
        SaveNumberTarget::CampaignIndex
        | SaveNumberTarget::Main(_)
        | SaveNumberTarget::SelectedUnit
        | SaveNumberTarget::CurrentMissionIndex => Ok(()),
    }
}

fn unit_at(
    file: &kuf_save::File,
    index: usize,
    target: SaveNumberTarget,
) -> Result<&kuf_save::UnitSaveData, FormatError> {
    file.units
        .get(index)
        .ok_or_else(|| target_out_of_range(target, file.units.len()))
}

fn roster_at(
    file: &kuf_save::File,
    index: usize,
    target: SaveNumberTarget,
) -> Result<&kuf_save::WorldMapNodeState, FormatError> {
    file.roster_entries
        .get(index)
        .ok_or_else(|| target_out_of_range(target, file.roster_entries.len()))
}

const fn target_out_of_range(target: SaveNumberTarget, record_count: usize) -> FormatError {
    FormatError::SaveTargetOutOfRange {
        target,
        record_count,
    }
}

const fn value_out_of_range(
    target: SaveNumberTarget,
    value: i64,
    (minimum, maximum): (i64, i64),
) -> FormatError {
    FormatError::SaveValueOutOfRange {
        target,
        value,
        minimum,
        maximum,
    }
}

const fn target_bounds(target: SaveNumberTarget) -> (i64, i64) {
    match target {
        SaveNumberTarget::CampaignIndex => (0, 3),
        SaveNumberTarget::Main(SaveMainField::Field08)
        | SaveNumberTarget::SelectedUnit
        | SaveNumberTarget::Unit {
            field:
                SaveUnitField::LeaderNameIndex
                | SaveUnitField::TroopInfoIndex
                | SaveUnitField::CharacterID
                | SaveUnitField::TroopInfoIndex2,
            ..
        }
        | SaveNumberTarget::MissionCompletion { .. }
        | SaveNumberTarget::CurrentMissionIndex
        | SaveNumberTarget::Equipment {
            field:
                SaveEquipmentField::ItemTypeID
                | SaveEquipmentField::Attribute1Index
                | SaveEquipmentField::Attribute2Index
                | SaveEquipmentField::SkillType1
                | SaveEquipmentField::SkillBonus1
                | SaveEquipmentField::SkillType2
                | SaveEquipmentField::SkillBonus2
                | SaveEquipmentField::ResistType1
                | SaveEquipmentField::ResistBonus1
                | SaveEquipmentField::ResistType2
                | SaveEquipmentField::ResistBonus2
                | SaveEquipmentField::SlotCategory,
            ..
        } => I32_BOUNDS,
        SaveNumberTarget::Unit {
            field: SaveUnitField::Byte58 | SaveUnitField::HeroFlag | SaveUnitField::Byte5A,
            ..
        }
        | SaveNumberTarget::Roster {
            field:
                SaveRosterField::Byte60
                | SaveRosterField::Byte61
                | SaveRosterField::Byte62
                | SaveRosterField::Byte63,
            ..
        } => U8_BOUNDS,
        SaveNumberTarget::Equipment {
            field:
                SaveEquipmentField::Level
                | SaveEquipmentField::VariantIndex
                | SaveEquipmentField::EquippedFlag
                | SaveEquipmentField::Reserved,
            ..
        } => U16_BOUNDS,
        SaveNumberTarget::Equipment {
            field: SaveEquipmentField::EnhancementTier | SaveEquipmentField::ItemPower,
            ..
        } => I16_BOUNDS,
        SaveNumberTarget::Main(_)
        | SaveNumberTarget::Unit { .. }
        | SaveNumberTarget::Equipment {
            field: SaveEquipmentField::AutoID,
            ..
        }
        | SaveNumberTarget::Roster {
            field: SaveRosterField::Value64,
            ..
        }
        | SaveNumberTarget::SecondArray { .. } => U32_BOUNDS,
    }
}

fn main_number(block: &[u8; 340], field: SaveMainField) -> Result<i64, FormatError> {
    let target = SaveNumberTarget::Main(field);
    let bytes = main_field_bytes(block, field)
        .ok_or_else(|| target_out_of_range(target, SaveMainField::ALL.len()))?;
    if field == SaveMainField::Field08 {
        Ok(i64::from(i32::from_le_bytes(bytes)))
    } else {
        Ok(i64::from(u32::from_le_bytes(bytes)))
    }
}

fn main_field_bytes(block: &[u8; 340], field: SaveMainField) -> Option<[u8; 4]> {
    let offset = main_field_offset(field);
    let end = offset.checked_add(size_of::<u32>())?;
    block.get(offset..end)?.try_into().ok()
}

const fn main_field_offset(field: SaveMainField) -> usize {
    match field {
        SaveMainField::Field00 => 0x00,
        SaveMainField::Field04 => 0x04,
        SaveMainField::Field08 => 0x08,
        SaveMainField::Field0C => 0x0c,
        SaveMainField::Field10 => 0x10,
        SaveMainField::Field14 => 0x14,
        SaveMainField::Field18 => 0x18,
    }
}

fn unit_number(unit: &kuf_save::UnitSaveData, field: SaveUnitField) -> i64 {
    match field {
        SaveUnitField::LeaderNameIndex => i64::from(unit.leader_name_index),
        SaveUnitField::TroopInfoIndex => signed_u32(unit.troop_info_index),
        SaveUnitField::JobType => i64::from(unit.job_type),
        SaveUnitField::ModelID => i64::from(unit.model_id),
        SaveUnitField::STGField34 => i64::from(unit.stg_field_190),
        SaveUnitField::STGField38 => i64::from(unit.stg_field_192),
        SaveUnitField::STGField3C => i64::from(unit.stg_field_194),
        SaveUnitField::STGField40 => i64::from(unit.stg_field_198),
        SaveUnitField::CharacterID => signed_u32(unit.char_id),
        SaveUnitField::TroopInfoIndex2 => signed_u32(unit.troop_info_index_2),
        SaveUnitField::UCD => i64::from(unit.ucd),
        SaveUnitField::FormationType => i64::from(unit.formation_type),
        SaveUnitField::GridConfig => i64::from(unit.grid_config),
        SaveUnitField::SkillLevel => i64::from(unit.skill_level),
        SaveUnitField::Byte58 => i64::from(unit.byte_58),
        SaveUnitField::HeroFlag => i64::from(unit.hero_flag),
        SaveUnitField::Byte5A => i64::from(unit.byte_5a),
        SaveUnitField::Field60 => i64::from(unit.field_60),
        SaveUnitField::Field64 => i64::from(unit.field_64),
        SaveUnitField::Field68 => i64::from(unit.field_68),
        SaveUnitField::Field504 => i64::from(unit.field_504),
    }
}

fn equipment_number(equipment: &kuf_save::EquipmentSlot, field: SaveEquipmentField) -> i64 {
    match field {
        SaveEquipmentField::AutoID => i64::from(equipment.auto_id),
        SaveEquipmentField::ItemTypeID => i64::from(equipment.item_type_id),
        SaveEquipmentField::Level => i64::from(equipment.level),
        SaveEquipmentField::EnhancementTier => i64::from(equipment.enhancement_tier),
        SaveEquipmentField::VariantIndex => i64::from(equipment.variant_index),
        SaveEquipmentField::ItemPower => i64::from(equipment.item_power),
        SaveEquipmentField::EquippedFlag => i64::from(equipment.equipped_flag),
        SaveEquipmentField::Reserved => i64::from(equipment.reserved),
        SaveEquipmentField::Attribute1Index => i64::from(equipment.attribute1_index),
        SaveEquipmentField::Attribute2Index => i64::from(equipment.attribute2_index),
        SaveEquipmentField::SkillType1 => i64::from(equipment.skill_type_1),
        SaveEquipmentField::SkillBonus1 => i64::from(equipment.skill_bonus_1),
        SaveEquipmentField::SkillType2 => i64::from(equipment.skill_type_2),
        SaveEquipmentField::SkillBonus2 => i64::from(equipment.skill_bonus_2),
        SaveEquipmentField::ResistType1 => i64::from(equipment.resist_type_1),
        SaveEquipmentField::ResistBonus1 => i64::from(equipment.resist_bonus_1),
        SaveEquipmentField::ResistType2 => i64::from(equipment.resist_type_2),
        SaveEquipmentField::ResistBonus2 => i64::from(equipment.resist_bonus_2),
        SaveEquipmentField::SlotCategory => i64::from(equipment.slot_category),
    }
}

fn roster_number(record: &kuf_save::WorldMapNodeState, field: SaveRosterField) -> i64 {
    match field {
        SaveRosterField::Byte60 => i64::from(record.byte_60),
        SaveRosterField::Byte61 => i64::from(record.byte_61),
        SaveRosterField::Byte62 => i64::from(record.byte_62),
        SaveRosterField::Byte63 => i64::from(record.byte_63),
        SaveRosterField::Value64 => i64::from(record.uint_64),
    }
}

fn signed_u32(value: u32) -> i64 {
    i64::from(i32::from_le_bytes(value.to_le_bytes()))
}

const fn signed_u32_wire(value: i32) -> u32 {
    u32::from_le_bytes(value.to_le_bytes())
}

const fn equipment_slot(
    unit: &kuf_save::UnitSaveData,
    slot: SaveEquipmentSlot,
) -> &kuf_save::EquipmentSlot {
    match slot {
        SaveEquipmentSlot::LeaderWeapon => &unit.leader_weapon,
        SaveEquipmentSlot::LeaderAccessory => &unit.leader_accessory,
        SaveEquipmentSlot::LeaderArmor => &unit.leader_armor,
        SaveEquipmentSlot::TroopWeapon => &unit.troop_weapon,
        SaveEquipmentSlot::TroopAccessory => &unit.troop_accessory,
        SaveEquipmentSlot::TroopArmor => &unit.troop_armor,
    }
}

fn equipment_slot_mut(
    unit: &mut kuf_save::UnitSaveData,
    slot: SaveEquipmentSlot,
) -> &mut kuf_save::EquipmentSlot {
    match slot {
        SaveEquipmentSlot::LeaderWeapon => &mut unit.leader_weapon,
        SaveEquipmentSlot::LeaderAccessory => &mut unit.leader_accessory,
        SaveEquipmentSlot::LeaderArmor => &mut unit.leader_armor,
        SaveEquipmentSlot::TroopWeapon => &mut unit.troop_weapon,
        SaveEquipmentSlot::TroopAccessory => &mut unit.troop_accessory,
        SaveEquipmentSlot::TroopArmor => &mut unit.troop_armor,
    }
}

fn convert_wire_value(
    target: SaveNumberTarget,
    value: i64,
    bounds: (i64, i64),
) -> Result<WireValue, FormatError> {
    if value < bounds.0 || value > bounds.1 {
        return Err(value_out_of_range(target, value, bounds));
    }

    match target {
        SaveNumberTarget::Main(SaveMainField::Field08)
        | SaveNumberTarget::Unit {
            field: SaveUnitField::LeaderNameIndex,
            ..
        }
        | SaveNumberTarget::Equipment {
            field:
                SaveEquipmentField::ItemTypeID
                | SaveEquipmentField::Attribute1Index
                | SaveEquipmentField::Attribute2Index
                | SaveEquipmentField::SkillType1
                | SaveEquipmentField::SkillBonus1
                | SaveEquipmentField::SkillType2
                | SaveEquipmentField::SkillBonus2
                | SaveEquipmentField::ResistType1
                | SaveEquipmentField::ResistBonus1
                | SaveEquipmentField::ResistType2
                | SaveEquipmentField::ResistBonus2
                | SaveEquipmentField::SlotCategory,
            ..
        } => checked_i32(target, value, bounds).map(WireValue::I32),
        SaveNumberTarget::SelectedUnit
        | SaveNumberTarget::Unit {
            field:
                SaveUnitField::TroopInfoIndex
                | SaveUnitField::CharacterID
                | SaveUnitField::TroopInfoIndex2,
            ..
        }
        | SaveNumberTarget::MissionCompletion { .. }
        | SaveNumberTarget::CurrentMissionIndex => checked_i32(target, value, bounds)
            .map(signed_u32_wire)
            .map(WireValue::U32),
        SaveNumberTarget::Unit {
            field: SaveUnitField::Byte58 | SaveUnitField::HeroFlag | SaveUnitField::Byte5A,
            ..
        }
        | SaveNumberTarget::Roster {
            field:
                SaveRosterField::Byte60
                | SaveRosterField::Byte61
                | SaveRosterField::Byte62
                | SaveRosterField::Byte63,
            ..
        } => checked_u8(target, value, bounds).map(WireValue::U8),
        SaveNumberTarget::Equipment {
            field:
                SaveEquipmentField::Level
                | SaveEquipmentField::VariantIndex
                | SaveEquipmentField::EquippedFlag
                | SaveEquipmentField::Reserved,
            ..
        } => checked_u16(target, value, bounds).map(WireValue::U16),
        SaveNumberTarget::Equipment {
            field: SaveEquipmentField::EnhancementTier | SaveEquipmentField::ItemPower,
            ..
        } => checked_i16(target, value, bounds).map(WireValue::I16),
        SaveNumberTarget::CampaignIndex
        | SaveNumberTarget::Main(_)
        | SaveNumberTarget::Unit { .. }
        | SaveNumberTarget::Equipment {
            field: SaveEquipmentField::AutoID,
            ..
        }
        | SaveNumberTarget::Roster {
            field: SaveRosterField::Value64,
            ..
        }
        | SaveNumberTarget::SecondArray { .. } => {
            checked_u32(target, value, bounds).map(WireValue::U32)
        }
    }
}

fn checked_u8(target: SaveNumberTarget, value: i64, bounds: (i64, i64)) -> Result<u8, FormatError> {
    u8::try_from(value).map_err(|_| value_out_of_range(target, value, bounds))
}

fn checked_u16(
    target: SaveNumberTarget,
    value: i64,
    bounds: (i64, i64),
) -> Result<u16, FormatError> {
    u16::try_from(value).map_err(|_| value_out_of_range(target, value, bounds))
}

fn checked_i16(
    target: SaveNumberTarget,
    value: i64,
    bounds: (i64, i64),
) -> Result<i16, FormatError> {
    i16::try_from(value).map_err(|_| value_out_of_range(target, value, bounds))
}

fn checked_u32(
    target: SaveNumberTarget,
    value: i64,
    bounds: (i64, i64),
) -> Result<u32, FormatError> {
    u32::try_from(value).map_err(|_| value_out_of_range(target, value, bounds))
}

fn checked_i32(
    target: SaveNumberTarget,
    value: i64,
    bounds: (i64, i64),
) -> Result<i32, FormatError> {
    i32::try_from(value).map_err(|_| value_out_of_range(target, value, bounds))
}

fn assign_number(
    file: &mut kuf_save::File,
    target: SaveNumberTarget,
    wire: WireValue,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<(), FormatError> {
    match target {
        SaveNumberTarget::CampaignIndex => {
            file.campaign_index = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveNumberTarget::Main(field) => {
            let bytes = match wire {
                WireValue::U32(value) => value.to_le_bytes(),
                WireValue::I32(value) => value.to_le_bytes(),
                _ => return Err(value_out_of_range(target, source_value, bounds)),
            };
            let offset = main_field_offset(field);
            let end = offset
                .checked_add(bytes.len())
                .ok_or_else(|| target_out_of_range(target, SaveMainField::ALL.len()))?;
            let destination = file
                .main_save_block
                .get_mut(offset..end)
                .ok_or_else(|| target_out_of_range(target, SaveMainField::ALL.len()))?;
            destination.copy_from_slice(&bytes);
        }
        SaveNumberTarget::SelectedUnit => {
            file.selected_unit_ref = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveNumberTarget::Unit { unit, field } => {
            let record_count = file.units.len();
            let record = file
                .units
                .get_mut(unit)
                .ok_or_else(|| target_out_of_range(target, record_count))?;
            assign_unit(record, field, wire, target, source_value, bounds)?;
        }
        SaveNumberTarget::Equipment { unit, slot, field } => {
            let record_count = file.units.len();
            let record = file
                .units
                .get_mut(unit)
                .ok_or_else(|| target_out_of_range(target, record_count))?;
            assign_equipment(
                equipment_slot_mut(record, slot),
                field,
                wire,
                target,
                source_value,
                bounds,
            )?;
        }
        SaveNumberTarget::Roster { record, field } => {
            let record_count = file.roster_entries.len();
            let record = file
                .roster_entries
                .get_mut(record)
                .ok_or_else(|| target_out_of_range(target, record_count))?;
            assign_roster(record, field, wire, target, source_value, bounds)?;
        }
        SaveNumberTarget::MissionCompletion { slot } => {
            let record_count = file.mission_completion.len();
            let destination = file
                .mission_completion
                .get_mut(slot)
                .ok_or_else(|| target_out_of_range(target, record_count))?;
            *destination = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveNumberTarget::CurrentMissionIndex => {
            file.current_mission_slot = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveNumberTarget::SecondArray { record } => {
            let record_count = file.second_array.len();
            let destination = file
                .second_array
                .get_mut(record)
                .ok_or_else(|| target_out_of_range(target, record_count))?;
            *destination = wire_u32(wire, target, source_value, bounds)?;
        }
    }
    Ok(())
}

fn assign_unit(
    unit: &mut kuf_save::UnitSaveData,
    field: SaveUnitField,
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<(), FormatError> {
    match field {
        SaveUnitField::LeaderNameIndex => {
            unit.leader_name_index = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::TroopInfoIndex => {
            unit.troop_info_index = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::JobType => unit.job_type = wire_u32(wire, target, source_value, bounds)?,
        SaveUnitField::ModelID => unit.model_id = wire_u32(wire, target, source_value, bounds)?,
        SaveUnitField::STGField34 => {
            unit.stg_field_190 = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::STGField38 => {
            unit.stg_field_192 = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::STGField3C => {
            unit.stg_field_194 = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::STGField40 => {
            unit.stg_field_198 = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::CharacterID => {
            unit.char_id = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::TroopInfoIndex2 => {
            unit.troop_info_index_2 = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::UCD => unit.ucd = wire_u32(wire, target, source_value, bounds)?,
        SaveUnitField::FormationType => {
            unit.formation_type = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::GridConfig => {
            unit.grid_config = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::SkillLevel => {
            unit.skill_level = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveUnitField::Byte58 => unit.byte_58 = wire_u8(wire, target, source_value, bounds)?,
        SaveUnitField::HeroFlag => unit.hero_flag = wire_u8(wire, target, source_value, bounds)?,
        SaveUnitField::Byte5A => unit.byte_5a = wire_u8(wire, target, source_value, bounds)?,
        SaveUnitField::Field60 => unit.field_60 = wire_u32(wire, target, source_value, bounds)?,
        SaveUnitField::Field64 => unit.field_64 = wire_u32(wire, target, source_value, bounds)?,
        SaveUnitField::Field68 => unit.field_68 = wire_u32(wire, target, source_value, bounds)?,
        SaveUnitField::Field504 => unit.field_504 = wire_u32(wire, target, source_value, bounds)?,
    }
    Ok(())
}

fn assign_equipment(
    equipment: &mut kuf_save::EquipmentSlot,
    field: SaveEquipmentField,
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<(), FormatError> {
    match field {
        SaveEquipmentField::AutoID => {
            equipment.auto_id = wire_u32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::ItemTypeID => {
            equipment.item_type_id = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::Level => {
            equipment.level = wire_u16(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::EnhancementTier => {
            equipment.enhancement_tier = wire_i16(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::VariantIndex => {
            equipment.variant_index = wire_u16(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::ItemPower => {
            equipment.item_power = wire_i16(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::EquippedFlag => {
            equipment.equipped_flag = wire_u16(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::Reserved => {
            equipment.reserved = wire_u16(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::Attribute1Index => {
            equipment.attribute1_index = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::Attribute2Index => {
            equipment.attribute2_index = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::SkillType1 => {
            equipment.skill_type_1 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::SkillBonus1 => {
            equipment.skill_bonus_1 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::SkillType2 => {
            equipment.skill_type_2 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::SkillBonus2 => {
            equipment.skill_bonus_2 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::ResistType1 => {
            equipment.resist_type_1 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::ResistBonus1 => {
            equipment.resist_bonus_1 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::ResistType2 => {
            equipment.resist_type_2 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::ResistBonus2 => {
            equipment.resist_bonus_2 = wire_i32(wire, target, source_value, bounds)?;
        }
        SaveEquipmentField::SlotCategory => {
            equipment.slot_category = wire_i32(wire, target, source_value, bounds)?;
        }
    }
    Ok(())
}

fn assign_roster(
    record: &mut kuf_save::WorldMapNodeState,
    field: SaveRosterField,
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<(), FormatError> {
    match field {
        SaveRosterField::Byte60 => {
            record.byte_60 = wire_u8(wire, target, source_value, bounds)?;
        }
        SaveRosterField::Byte61 => {
            record.byte_61 = wire_u8(wire, target, source_value, bounds)?;
        }
        SaveRosterField::Byte62 => {
            record.byte_62 = wire_u8(wire, target, source_value, bounds)?;
        }
        SaveRosterField::Byte63 => {
            record.byte_63 = wire_u8(wire, target, source_value, bounds)?;
        }
        SaveRosterField::Value64 => {
            record.uint_64 = wire_u32(wire, target, source_value, bounds)?;
        }
    }
    Ok(())
}

fn wire_u8(
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<u8, FormatError> {
    let WireValue::U8(value) = wire else {
        return Err(value_out_of_range(target, source_value, bounds));
    };
    Ok(value)
}

fn wire_u16(
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<u16, FormatError> {
    let WireValue::U16(value) = wire else {
        return Err(value_out_of_range(target, source_value, bounds));
    };
    Ok(value)
}

fn wire_i16(
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<i16, FormatError> {
    let WireValue::I16(value) = wire else {
        return Err(value_out_of_range(target, source_value, bounds));
    };
    Ok(value)
}

fn wire_u32(
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<u32, FormatError> {
    let WireValue::U32(value) = wire else {
        return Err(value_out_of_range(target, source_value, bounds));
    };
    Ok(value)
}

fn wire_i32(
    wire: WireValue,
    target: SaveNumberTarget,
    source_value: i64,
    bounds: (i64, i64),
) -> Result<i32, FormatError> {
    let WireValue::I32(value) = wire else {
        return Err(value_out_of_range(target, source_value, bounds));
    };
    Ok(value)
}
