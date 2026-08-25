use std::{collections::HashSet, fmt::Debug, hash::Hash};

use kufeditor_formats::{
    DiagnosticField, DiagnosticLocation, SaveEditor, SaveEquipmentField, SaveEquipmentGroup,
    SaveEquipmentSlot, SaveMainField, SaveNumberTarget, SaveRosterField, SaveTextField,
    SaveUnitField, SaveUnitGroup, TroopField,
};

#[test]
fn save_fields_have_stable_uppercase_acronym_labels() {
    assert_eq!(SaveUnitField::ModelID.label(), "Model ID");
    assert_eq!(SaveUnitField::CharacterID.label(), "Character ID");
    assert_eq!(SaveUnitField::UCD.label(), "UCD");
    assert_eq!(SaveEquipmentField::AutoID.label(), "Auto ID");
    assert_eq!(SaveEquipmentField::ItemTypeID.label(), "Item Type ID");
}

#[test]
fn save_targets_keep_record_identity_in_the_target() {
    let target = SaveNumberTarget::Equipment {
        unit: 7,
        slot: SaveEquipmentSlot::TroopArmor,
        field: SaveEquipmentField::ResistType2,
    };

    assert_eq!(target.label(), "Resist Type 2");
    assert_eq!(DiagnosticLocation::Save(target).record(), Some(7));
}

#[test]
fn record_diagnostics_keep_the_existing_field_model() {
    let location = DiagnosticLocation::Record {
        record: 3,
        field: DiagnosticField::Troop(TroopField::MoveSpeed),
    };

    assert_eq!(location.record(), Some(3));
    assert_eq!(location.label(), "Move Speed");
}

#[test]
fn every_save_metadata_enum_has_complete_stable_labels() {
    assert_complete(
        &SaveMainField::ALL,
        &[
            "Field 0x00",
            "Field 0x04",
            "Field 0x08",
            "Field 0x0C",
            "Field 0x10",
            "Field 0x14",
            "Field 0x18",
        ],
        SaveMainField::label,
    );
    assert_complete(
        &SaveUnitField::ALL,
        &[
            "Leader Name Index",
            "Troop Info Index",
            "Job Type",
            "Model ID",
            "STG Field 0x34",
            "STG Field 0x38",
            "STG Field 0x3C",
            "STG Field 0x40",
            "Character ID",
            "Troop Info Index 2",
            "UCD",
            "Formation Type",
            "Grid Config",
            "Skill Level",
            "Byte 0x58",
            "Hero Flag",
            "Byte 0x5A",
            "Field 0x60",
            "Field 0x64",
            "Field 0x68",
            "Field 0x504",
        ],
        SaveUnitField::label,
    );
    assert_complete(
        &SaveEquipmentSlot::ALL,
        &[
            "Leader Weapon",
            "Leader Accessory",
            "Leader Armor",
            "Troop Weapon",
            "Troop Accessory",
            "Troop Armor",
        ],
        SaveEquipmentSlot::label,
    );
    assert_complete(
        &SaveEquipmentField::ALL,
        &[
            "Auto ID",
            "Item Type ID",
            "Level",
            "Enhancement Tier",
            "Variant Index",
            "Item Power",
            "Equipped Flag",
            "Reserved",
            "Attribute 1 Index",
            "Attribute 2 Index",
            "Skill Type 1",
            "Skill Bonus 1",
            "Skill Type 2",
            "Skill Bonus 2",
            "Resist Type 1",
            "Resist Bonus 1",
            "Resist Type 2",
            "Resist Bonus 2",
            "Slot Category",
        ],
        SaveEquipmentField::label,
    );
    assert_complete(
        &SaveRosterField::ALL,
        &["Byte 60", "Byte 61", "Byte 62", "Byte 63", "Value 64"],
        SaveRosterField::label,
    );
    assert_complete(
        &SaveTextField::ALL,
        &["Map Name", "Set File", "Sky Effects"],
        SaveTextField::label,
    );
    assert_complete(
        &SaveUnitGroup::ALL,
        &["Core", "Formation", "Advanced"],
        SaveUnitGroup::label,
    );
    assert_complete(
        &SaveEquipmentGroup::ALL,
        &["Core", "Skills", "Resistances", "Advanced"],
        SaveEquipmentGroup::label,
    );
}

#[test]
fn every_unit_and_equipment_field_has_one_stable_group() {
    let core_unit_fields = [
        SaveUnitField::TroopInfoIndex,
        SaveUnitField::JobType,
        SaveUnitField::ModelID,
        SaveUnitField::CharacterID,
        SaveUnitField::TroopInfoIndex2,
        SaveUnitField::UCD,
        SaveUnitField::SkillLevel,
        SaveUnitField::Byte58,
        SaveUnitField::HeroFlag,
        SaveUnitField::Byte5A,
    ];
    let formation_unit_fields = [SaveUnitField::FormationType, SaveUnitField::GridConfig];
    let advanced_unit_fields = [
        SaveUnitField::LeaderNameIndex,
        SaveUnitField::STGField34,
        SaveUnitField::STGField38,
        SaveUnitField::STGField3C,
        SaveUnitField::STGField40,
        SaveUnitField::Field60,
        SaveUnitField::Field64,
        SaveUnitField::Field68,
        SaveUnitField::Field504,
    ];
    assert_group(&core_unit_fields, SaveUnitGroup::Core, SaveUnitField::group);
    assert_group(
        &formation_unit_fields,
        SaveUnitGroup::Formation,
        SaveUnitField::group,
    );
    assert_group(
        &advanced_unit_fields,
        SaveUnitGroup::Advanced,
        SaveUnitField::group,
    );
    assert_partition(
        &SaveUnitField::ALL,
        &[
            core_unit_fields.as_slice(),
            formation_unit_fields.as_slice(),
            advanced_unit_fields.as_slice(),
        ],
    );

    let core_equipment_fields = [
        SaveEquipmentField::ItemTypeID,
        SaveEquipmentField::Level,
        SaveEquipmentField::EnhancementTier,
        SaveEquipmentField::VariantIndex,
        SaveEquipmentField::Attribute1Index,
        SaveEquipmentField::Attribute2Index,
    ];
    let skill_equipment_fields = [
        SaveEquipmentField::SkillType1,
        SaveEquipmentField::SkillBonus1,
        SaveEquipmentField::SkillType2,
        SaveEquipmentField::SkillBonus2,
    ];
    let resistance_equipment_fields = [
        SaveEquipmentField::ResistType1,
        SaveEquipmentField::ResistBonus1,
        SaveEquipmentField::ResistType2,
        SaveEquipmentField::ResistBonus2,
    ];
    let advanced_equipment_fields = [
        SaveEquipmentField::AutoID,
        SaveEquipmentField::ItemPower,
        SaveEquipmentField::EquippedFlag,
        SaveEquipmentField::Reserved,
        SaveEquipmentField::SlotCategory,
    ];
    assert_group(
        &core_equipment_fields,
        SaveEquipmentGroup::Core,
        SaveEquipmentField::group,
    );
    assert_group(
        &skill_equipment_fields,
        SaveEquipmentGroup::Skills,
        SaveEquipmentField::group,
    );
    assert_group(
        &resistance_equipment_fields,
        SaveEquipmentGroup::Resistances,
        SaveEquipmentField::group,
    );
    assert_group(
        &advanced_equipment_fields,
        SaveEquipmentGroup::Advanced,
        SaveEquipmentField::group,
    );
    assert_partition(
        &SaveEquipmentField::ALL,
        &[
            core_equipment_fields.as_slice(),
            skill_equipment_fields.as_slice(),
            resistance_equipment_fields.as_slice(),
            advanced_equipment_fields.as_slice(),
        ],
    );
}

#[test]
fn save_target_labels_and_optional_record_identity_are_stable() {
    let cases = [
        (SaveNumberTarget::CampaignIndex, "Campaign", None),
        (
            SaveNumberTarget::Main(SaveMainField::Field0C),
            "Field 0x0C",
            None,
        ),
        (
            SaveNumberTarget::SelectedUnit,
            "Selected Unit Reference",
            None,
        ),
        (
            SaveNumberTarget::Unit {
                unit: 2,
                field: SaveUnitField::CharacterID,
            },
            "Character ID",
            Some(2),
        ),
        (
            SaveNumberTarget::Equipment {
                unit: 3,
                slot: SaveEquipmentSlot::LeaderWeapon,
                field: SaveEquipmentField::Level,
            },
            "Level",
            Some(3),
        ),
        (
            SaveNumberTarget::Roster {
                record: 4,
                field: SaveRosterField::Byte62,
            },
            "Byte 62",
            Some(4),
        ),
        (
            SaveNumberTarget::MissionCompletion { slot: 5 },
            "Mission Completion",
            Some(5),
        ),
        (
            SaveNumberTarget::CurrentMissionIndex,
            "Current Mission Index",
            None,
        ),
        (
            SaveNumberTarget::SecondArray { record: 6 },
            "Second Array Value",
            Some(6),
        ),
    ];

    for (target, label, record) in cases {
        let location = DiagnosticLocation::Save(target);
        assert_eq!(target.label(), label);
        assert_eq!(location.label(), label);
        assert_eq!(location.record(), record);
    }
}

#[test]
fn save_choice_editors_keep_the_exact_legacy_values_and_labels() {
    assert_choices(
        SaveEditor::CAMPAIGN,
        &[
            (0, "Hironeiden (Gerald)"),
            (1, "Vellond (Lucretia)"),
            (2, "Ecclesia (Kendal)"),
            (3, "Dark Legion (Regnier)"),
        ],
    );
    assert_choices(
        SaveEditor::UCD,
        &[
            (0, "Leader"),
            (1, "Officer 1"),
            (2, "Officer 2"),
            (3, "Troop"),
        ],
    );
    assert_choices(SaveEditor::HERO, &[(0, "Hero"), (1, "Troop")]);
    assert_choices(
        SaveEditor::SKILL,
        &[
            (-1, "None"),
            (0, "Melee"),
            (1, "Range"),
            (2, "Frontal"),
            (3, "Riding"),
            (4, "Teamwork"),
            (5, "Scout"),
            (6, "Gunpowder"),
            (7, "Taming"),
            (8, "Fire"),
            (9, "Lightning"),
            (10, "Ice"),
            (11, "Holy"),
            (12, "Earth"),
            (13, "Curse"),
            (14, "Elemental"),
        ],
    );
    assert_choices(
        SaveEditor::RESISTANCE,
        &[
            (-1, "None"),
            (0, "Melee"),
            (1, "Ranged"),
            (2, "Explosion"),
            (3, "Frontal"),
            (4, "Fire"),
            (5, "Lightning"),
            (6, "Ice"),
            (7, "Holy"),
            (8, "Poison"),
            (9, "Curse"),
        ],
    );
}

fn assert_complete<T>(all: &[T], labels: &[&str], label: impl Fn(T) -> &'static str)
where
    T: Copy + Debug + Eq + Hash,
{
    assert_eq!(all.len(), labels.len());
    assert_eq!(all.iter().copied().collect::<HashSet<_>>().len(), all.len());
    assert_eq!(all.iter().copied().map(label).collect::<Vec<_>>(), labels);
}

fn assert_group<T, G>(fields: &[T], group: G, field_group: impl Fn(T) -> G)
where
    T: Copy + Debug,
    G: Copy + Debug + Eq,
{
    assert!(
        fields
            .iter()
            .copied()
            .all(|field| field_group(field) == group)
    );
}

fn assert_partition<T>(all: &[T], groups: &[&[T]])
where
    T: Copy + Debug + Eq + Hash,
{
    let flattened = groups
        .iter()
        .flat_map(|group| group.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(flattened.len(), all.len());
    assert_eq!(
        flattened.iter().copied().collect::<HashSet<_>>(),
        all.iter().copied().collect()
    );
}

fn assert_choices(editor: SaveEditor, expected: &[(i64, &str)]) {
    let SaveEditor::Choice { choices } = editor else {
        panic!("expected a choice editor");
    };
    let observed = choices
        .iter()
        .map(|choice| (choice.value, choice.label))
        .collect::<Vec<_>>();
    assert_eq!(observed, expected);
}
