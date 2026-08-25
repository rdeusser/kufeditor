mod support;

use std::{collections::HashSet, fmt::Debug, hash::Hash};

use kufeditor_formats::{
    DiagnosticField, DiagnosticLocation, FormatError, SaveDocument, SaveEditor, SaveEquipmentField,
    SaveEquipmentGroup, SaveEquipmentSlot, SaveMainField, SaveMutation, SaveNumberTarget,
    SaveParseError, SaveRegion, SaveRosterField, SaveTextField, SaveUnitField, SaveUnitGroup,
    Severity, TroopField,
};
use support::{
    SaveFixtureArrays, SaveFixtureOptions, complete_save_fixture, complete_save_offsets,
    fixture_with_count, fixture_with_unknown_choices, patch_i32, patch_u32, read_i32, read_u32,
    save_fixture, save_fixture_with_arrays, truncate_save,
};

#[test]
fn all_observed_envelopes_parse_and_no_op_encode_exactly() {
    for size_prefix in [false, true] {
        for context in [false, true] {
            let source = save_fixture(SaveFixtureOptions {
                size_prefix,
                context,
                ..SaveFixtureOptions::default()
            });

            let document = SaveDocument::parse(source.clone()).unwrap();

            assert_eq!(document.has_size_prefix(), size_prefix);
            assert_eq!(document.has_context(), context);
            assert_eq!(document.unit_count(), 0);
            assert_eq!(document.roster_count(), 0);
            assert_eq!(document.second_array_count(), 0);
            assert_eq!(document.encode().unwrap(), source);
        }
    }
}

#[test]
fn bad_save_magic_is_typed() {
    let mut source = save_fixture(SaveFixtureOptions {
        size_prefix: false,
        context: false,
        pad_to_32_kib: false,
        tail: Vec::new(),
    });
    source
        .get_mut(..4)
        .unwrap()
        .copy_from_slice(&0xdead_beefu32.to_le_bytes());

    assert!(matches!(
        SaveDocument::parse(source),
        Err(FormatError::SaveParse(SaveParseError::InvalidMagic {
            offset: 0,
            actual: 0xdead_beef,
        }))
    ));
}

#[test]
fn invalid_save_context_shape_is_typed() {
    let mut source = save_fixture(SaveFixtureOptions {
        pad_to_32_kib: false,
        ..SaveFixtureOptions::default()
    });
    source
        .get_mut(0x440..0x444)
        .unwrap()
        .copy_from_slice(&(-1_i32).to_le_bytes());

    assert!(matches!(
        SaveDocument::parse(source),
        Err(FormatError::SaveParse(SaveParseError::InvalidEnvelope))
    ));
}

#[test]
fn truncated_mandatory_save_regions_are_typed() {
    let mut units = save_fixture(SaveFixtureOptions {
        size_prefix: false,
        context: false,
        pad_to_32_kib: false,
        tail: Vec::new(),
    });
    units.truncate(350);

    let mut roster = save_fixture(SaveFixtureOptions {
        size_prefix: false,
        context: false,
        pad_to_32_kib: false,
        tail: Vec::new(),
    });
    roster.truncate(358);

    let mut second_array = save_fixture(SaveFixtureOptions {
        size_prefix: false,
        context: false,
        pad_to_32_kib: false,
        tail: Vec::new(),
    });
    second_array.truncate(362);

    let mut missions = save_fixture(SaveFixtureOptions {
        size_prefix: false,
        context: false,
        pad_to_32_kib: false,
        tail: Vec::new(),
    });
    missions.truncate(372);

    let cases = [
        (vec![0x6e, 0, 0], SaveRegion::Envelope, 0, 4, 3),
        (units, SaveRegion::Units, 348, 4, 2),
        (roster, SaveRegion::Roster, 356, 4, 2),
        (second_array, SaveRegion::SecondArray, 360, 4, 2),
        (missions, SaveRegion::Missions, 364, 84, 8),
    ];

    for (source, region, offset, needed, remaining) in cases {
        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(SaveParseError::Truncated {
                region: actual_region,
                offset: actual_offset,
                needed: actual_needed,
                remaining: actual_remaining,
            })) if actual_region == region
                && actual_offset == offset
                && actual_needed == needed
                && actual_remaining == remaining
        ));
    }
}

#[test]
fn preflight_truncation_offsets_use_source_coordinates_for_every_envelope() {
    for (size_prefix, context, unit_count_offset) in [
        (false, false, 348),
        (true, false, 352),
        (false, true, 1_428),
        (true, true, 1_432),
    ] {
        let mut source = save_fixture(SaveFixtureOptions {
            size_prefix,
            context,
            pad_to_32_kib: false,
            tail: Vec::new(),
        });
        truncate_save(&mut source, unit_count_offset + 2, size_prefix);

        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(SaveParseError::Truncated {
                region: SaveRegion::Units,
                offset,
                needed: 4,
                remaining: 2,
            })) if offset == unit_count_offset
        ));
    }
}

#[test]
fn preflight_count_offsets_use_source_coordinates_for_every_envelope() {
    for (size_prefix, context, unit_count_offset) in [
        (false, false, 348),
        (true, false, 352),
        (false, true, 1_428),
        (true, true, 1_432),
    ] {
        let mut source = save_fixture(SaveFixtureOptions {
            size_prefix,
            context,
            pad_to_32_kib: false,
            tail: Vec::new(),
        });
        patch_u32(&mut source, unit_count_offset, u32::MAX);

        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(SaveParseError::ImpossibleCount {
                region: SaveRegion::Units,
                offset,
                count: u32::MAX,
                item_size: 483,
                remaining: 96,
            })) if offset == unit_count_offset
        ));
    }
}

#[test]
fn truncated_context_probe_is_typed() {
    let source = [0x6e_u32.to_le_bytes(), (-1_i32).to_le_bytes()].concat();

    assert!(matches!(
        SaveDocument::parse(source),
        Err(FormatError::SaveParse(SaveParseError::Truncated {
            region: SaveRegion::Envelope,
            offset: 1_084,
            needed: 4,
            remaining: 0,
        }))
    ));
}

#[test]
fn impossible_dynamic_counts_fail_before_generated_parsing() {
    for (region, offset, item_size) in [
        (SaveRegion::Units, 1_432, 483),
        (SaveRegion::Roster, 1_440, 8),
        (SaveRegion::SecondArray, 1_444, 4),
    ] {
        let source = fixture_with_count(region, u32::MAX);

        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(
                SaveParseError::ImpossibleCount {
                    region: actual_region,
                    offset: actual_offset,
                    count: u32::MAX,
                    item_size: actual_item_size,
                    ..
                }
            )) if actual_region == region
                && actual_offset == offset
                && actual_item_size == item_size
        ));
    }
}

#[test]
fn exact_fit_dynamic_counts_parse() {
    for (arrays, counts) in [
        (
            SaveFixtureArrays {
                unit_count: 1,
                unit_records: 1,
                ..SaveFixtureArrays::default()
            },
            (1, 0, 0),
        ),
        (
            SaveFixtureArrays {
                roster_count: 1,
                roster_records: 1,
                ..SaveFixtureArrays::default()
            },
            (0, 1, 0),
        ),
        (
            SaveFixtureArrays {
                second_array_count: 1,
                second_array_values: 1,
                ..SaveFixtureArrays::default()
            },
            (0, 0, 1),
        ),
    ] {
        let source = save_fixture_with_arrays(
            SaveFixtureOptions {
                pad_to_32_kib: false,
                ..SaveFixtureOptions::default()
            },
            &arrays,
        );

        let document = SaveDocument::parse(source).unwrap();

        assert_eq!(document.unit_count(), counts.0);
        assert_eq!(document.roster_count(), counts.1);
        assert_eq!(document.second_array_count(), counts.2);
    }
}

#[test]
fn one_over_dynamic_counts_are_impossible() {
    for (arrays, region, offset, count, item_size, remaining) in [
        (
            SaveFixtureArrays {
                unit_count: 2,
                unit_records: 1,
                ..SaveFixtureArrays::default()
            },
            SaveRegion::Units,
            1_432,
            2,
            483,
            579,
        ),
        (
            SaveFixtureArrays {
                roster_count: 2,
                roster_records: 1,
                ..SaveFixtureArrays::default()
            },
            SaveRegion::Roster,
            1_440,
            2,
            8,
            96,
        ),
        (
            SaveFixtureArrays {
                second_array_count: 2,
                second_array_values: 1,
                ..SaveFixtureArrays::default()
            },
            SaveRegion::SecondArray,
            1_444,
            2,
            4,
            88,
        ),
    ] {
        let source = save_fixture_with_arrays(
            SaveFixtureOptions {
                pad_to_32_kib: false,
                ..SaveFixtureOptions::default()
            },
            &arrays,
        );

        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(SaveParseError::ImpossibleCount {
                region: actual_region,
                offset: actual_offset,
                count: actual_count,
                item_size: actual_item_size,
                remaining: actual_remaining,
            })) if actual_region == region
                && actual_offset == offset
                && actual_count == count
                && actual_item_size == item_size
                && actual_remaining == remaining
        ));
    }
}

#[test]
fn nonzero_counts_with_short_suffixes_are_impossible() {
    for (arrays, region, offset, item_size, remaining) in [
        (
            SaveFixtureArrays {
                unit_count: 1,
                ..SaveFixtureArrays::default()
            },
            SaveRegion::Units,
            1_432,
            483,
            95,
        ),
        (
            SaveFixtureArrays {
                roster_count: 1,
                ..SaveFixtureArrays::default()
            },
            SaveRegion::Roster,
            1_440,
            8,
            87,
        ),
        (
            SaveFixtureArrays {
                second_array_count: 1,
                ..SaveFixtureArrays::default()
            },
            SaveRegion::SecondArray,
            1_444,
            4,
            83,
        ),
    ] {
        let mut source = save_fixture_with_arrays(
            SaveFixtureOptions {
                pad_to_32_kib: false,
                ..SaveFixtureOptions::default()
            },
            &arrays,
        );
        let truncated_length = source.len() - 1;
        truncate_save(&mut source, truncated_length, true);

        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(SaveParseError::ImpossibleCount {
                region: actual_region,
                offset: actual_offset,
                count: 1,
                item_size: actual_item_size,
                remaining: actual_remaining,
            })) if actual_region == region
                && actual_offset == offset
                && actual_item_size == item_size
                && actual_remaining == remaining
        ));
    }
}

#[test]
fn zero_counts_with_short_suffixes_are_truncated_at_the_missing_region() {
    for (length, region, offset, needed, remaining) in [
        (1_439, SaveRegion::Units, 1_436, 4, 3),
        (1_447, SaveRegion::SecondArray, 1_444, 4, 3),
        (1_451, SaveRegion::Missions, 1_448, 84, 3),
    ] {
        let mut source = save_fixture(SaveFixtureOptions {
            pad_to_32_kib: false,
            ..SaveFixtureOptions::default()
        });
        truncate_save(&mut source, length, true);

        assert!(matches!(
            SaveDocument::parse(source),
            Err(FormatError::SaveParse(SaveParseError::Truncated {
                region: actual_region,
                offset: actual_offset,
                needed: actual_needed,
                remaining: actual_remaining,
            })) if actual_region == region
                && actual_offset == offset
                && actual_needed == needed
                && actual_remaining == remaining
        ));
    }
}

#[test]
fn nonzero_save_tail_survives_no_op_encode_exactly() {
    let source = save_fixture(SaveFixtureOptions {
        pad_to_32_kib: false,
        tail: vec![0xde, 0xad, 0, 0xbe, 0xef],
        ..SaveFixtureOptions::default()
    });

    let document = SaveDocument::parse(source.clone()).unwrap();

    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn context_text_strips_color_codes_filters_and_deduplicates() {
    let mut source = save_fixture(SaveFixtureOptions {
        pad_to_32_kib: false,
        ..SaveFixtureOptions::default()
    });
    let context = b"@(color=gold) Alpha \n(color=blue) Beta\r\nabc\nAlpha\n  Delta  \0xyz\0Gamma\0";
    source
        .get_mut(12..12 + context.len())
        .unwrap()
        .copy_from_slice(context);

    let document = SaveDocument::parse(source).unwrap();

    assert_eq!(document.context_text(), ["Alpha", "Beta", "Delta", "Gamma"]);
}

#[test]
fn context_text_discards_unterminated_color_fragments() {
    let mut source = save_fixture(SaveFixtureOptions {
        pad_to_32_kib: false,
        ..SaveFixtureOptions::default()
    });
    let context = b"Alpha@(color=broken\0Beta(color=broken\0Omega\0";
    source
        .get_mut(12..12 + context.len())
        .unwrap()
        .copy_from_slice(context);

    let document = SaveDocument::parse(source).unwrap();

    assert_eq!(document.context_text(), ["Alpha", "Beta", "Omega"]);
}

#[test]
fn context_text_removes_orphan_hex_fragment_before_deduplication() {
    let mut source = save_fixture(SaveFixtureOptions {
        pad_to_32_kib: false,
        ..SaveFixtureOptions::default()
    });
    let context = b"FF00aa)Alpha\0Alpha\0";
    source
        .get_mut(12..12 + context.len())
        .unwrap()
        .copy_from_slice(context);

    let document = SaveDocument::parse(source).unwrap();

    assert_eq!(document.context_text(), ["Alpha"]);
}

#[test]
fn absent_context_has_no_text_projection() {
    let source = save_fixture(SaveFixtureOptions {
        context: false,
        ..SaveFixtureOptions::default()
    });

    let document = SaveDocument::parse(source).unwrap();

    assert!(document.context_text().is_empty());
}

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

#[test]
fn complete_numeric_fixture_has_the_expected_wire_values() {
    let source = complete_save_fixture(SaveFixtureOptions {
        pad_to_32_kib: false,
        ..SaveFixtureOptions::default()
    });
    let offsets = complete_save_offsets(true, true);

    assert_eq!(read_u32(&source, offsets.magic), 0x6e);
    assert_eq!(offsets.context, Some(8));
    assert_eq!(read_u32(&source, offsets.campaign), 0);
    assert_eq!(read_i32(&source, offsets.main + 8), -8);
    assert_eq!(read_i32(&source, offsets.unit), -1);
    assert_eq!(read_i32(&source, offsets.selected_unit), -1);
    assert_eq!(
        source.get(offsets.roster..offsets.roster + 4),
        Some(&[61, 60, 62, 63][..])
    );
    assert_eq!(read_u32(&source, offsets.second_array), 0x0203_0405);
    assert_eq!(read_i32(&source, offsets.mission_completion), -1);
    assert_eq!(read_i32(&source, offsets.current_mission), -2);
    assert_eq!(offsets.tail, source.len());
}

#[test]
fn number_projects_every_numeric_save_leaf_and_equipment_slot() {
    let document =
        SaveDocument::parse(complete_save_fixture(SaveFixtureOptions::default())).unwrap();

    assert_eq!(document.number(SaveNumberTarget::CampaignIndex).unwrap(), 0);
    for (field, expected) in SaveMainField::ALL
        .into_iter()
        .zip([256, 260, -8, 268, 272, 276, 280])
    {
        assert_eq!(
            document.number(SaveNumberTarget::Main(field)).unwrap(),
            expected,
            "{field:?}"
        );
    }
    assert_eq!(document.number(SaveNumberTarget::SelectedUnit).unwrap(), -1);

    let unit_values = [
        -1, 2, 3, 4, 0x34, 0x38, 0x3c, 0x40, -1, 5, 0, 6, 7, 8, 1, 0, 1, 60, 64, 68, 504,
    ];
    for (field, expected) in SaveUnitField::ALL.into_iter().zip(unit_values) {
        let target = SaveNumberTarget::Unit { unit: 0, field };
        assert_eq!(document.number(target).unwrap(), expected, "{target:?}");
    }

    let equipment_values = [
        [
            1_000, -100, 200, -200, 300, -300, 400, 500, -400, 600, 0, -500, 9, 700, 0, -600, 4,
            800, -700,
        ],
        [
            1_001, -101, 201, -201, 301, -301, 401, 501, -401, 601, 1, -501, 10, 701, 1, -601, 5,
            801, -701,
        ],
        [
            1_002, -102, 202, -202, 302, -302, 402, 502, -402, 602, 2, -502, 11, 702, 2, -602, 6,
            802, -702,
        ],
        [
            1_003, -103, 203, -203, 303, -303, 403, 503, -403, 603, 3, -503, 12, 703, 3, -603, 7,
            803, -703,
        ],
        [
            1_004, -104, 204, -204, 304, -304, 404, 504, -404, 604, 4, -504, 13, 704, 4, -604, 8,
            804, -704,
        ],
        [
            1_005, -105, 205, -205, 305, -305, 405, 505, -405, 605, 5, -505, 14, 705, 5, -605, 9,
            805, -705,
        ],
    ];
    for (slot_index, slot) in SaveEquipmentSlot::ALL.into_iter().enumerate() {
        for (field_index, field) in SaveEquipmentField::ALL.into_iter().enumerate() {
            let target = SaveNumberTarget::Equipment {
                unit: 0,
                slot,
                field,
            };
            let expected = equipment_values
                .get(slot_index)
                .and_then(|values| values.get(field_index))
                .copied()
                .unwrap();
            assert_eq!(document.number(target).unwrap(), expected, "{target:?}");
        }
    }

    for (field, expected) in SaveRosterField::ALL
        .into_iter()
        .zip([60, 61, 62, 63, 6_400])
    {
        let target = SaveNumberTarget::Roster { record: 0, field };
        assert_eq!(document.number(target).unwrap(), expected, "{target:?}");
    }
    for slot in 0..20 {
        let target = SaveNumberTarget::MissionCompletion { slot };
        assert_eq!(
            document.number(target).unwrap(),
            i64::try_from(slot).unwrap() - 1
        );
    }
    assert_eq!(
        document
            .number(SaveNumberTarget::CurrentMissionIndex)
            .unwrap(),
        -2
    );
    assert_eq!(
        document
            .number(SaveNumberTarget::SecondArray { record: 0 })
            .unwrap(),
        0x0203_0405,
    );
}

#[test]
fn number_storage_bounds_and_editors_cover_every_target_family() {
    let document =
        SaveDocument::parse(complete_save_fixture(SaveFixtureOptions::default())).unwrap();

    assert_number_metadata(
        &document,
        SaveNumberTarget::CampaignIndex,
        (0, 3),
        SaveEditor::CAMPAIGN,
    );
    for field in SaveMainField::ALL {
        let bounds = if field == SaveMainField::Field08 {
            I32_BOUNDS
        } else {
            U32_BOUNDS
        };
        assert_number_metadata(
            &document,
            SaveNumberTarget::Main(field),
            bounds,
            number(bounds),
        );
    }
    assert_number_metadata(
        &document,
        SaveNumberTarget::SelectedUnit,
        I32_BOUNDS,
        number(I32_BOUNDS),
    );
    for field in SaveUnitField::ALL {
        let (bounds, editor) = unit_metadata(field);
        assert_number_metadata(
            &document,
            SaveNumberTarget::Unit { unit: 0, field },
            bounds,
            editor,
        );
    }
    for slot in SaveEquipmentSlot::ALL {
        for field in SaveEquipmentField::ALL {
            let (bounds, editor) = equipment_metadata(field);
            assert_number_metadata(
                &document,
                SaveNumberTarget::Equipment {
                    unit: 0,
                    slot,
                    field,
                },
                bounds,
                editor,
            );
        }
    }
    for field in SaveRosterField::ALL {
        let bounds = if field == SaveRosterField::Value64 {
            U32_BOUNDS
        } else {
            U8_BOUNDS
        };
        assert_number_metadata(
            &document,
            SaveNumberTarget::Roster { record: 0, field },
            bounds,
            number(bounds),
        );
    }
    for slot in 0..20 {
        assert_number_metadata(
            &document,
            SaveNumberTarget::MissionCompletion { slot },
            I32_BOUNDS,
            number(I32_BOUNDS),
        );
    }
    assert_number_metadata(
        &document,
        SaveNumberTarget::CurrentMissionIndex,
        I32_BOUNDS,
        number(I32_BOUNDS),
    );
    assert_number_metadata(
        &document,
        SaveNumberTarget::SecondArray { record: 0 },
        U32_BOUNDS,
        number(U32_BOUNDS),
    );
}

#[test]
fn signed_wire_bit_patterns_project_without_unsigned_widening() {
    let mut source = complete_save_fixture(SaveFixtureOptions::default());
    let offsets = complete_save_offsets(true, true);
    patch_i32(&mut source, offsets.main + 8, -8);
    patch_i32(&mut source, offsets.unit + 4, -2);
    patch_i32(&mut source, offsets.unit + 32, -3);
    patch_i32(&mut source, offsets.unit + 36, -4);
    patch_i32(&mut source, offsets.selected_unit, -5);
    patch_i32(&mut source, offsets.mission_completion, -6);
    patch_i32(&mut source, offsets.current_mission, -7);
    let document = SaveDocument::parse(source).unwrap();

    let cases = [
        (SaveNumberTarget::Main(SaveMainField::Field08), -8),
        (
            SaveNumberTarget::Unit {
                unit: 0,
                field: SaveUnitField::TroopInfoIndex,
            },
            -2,
        ),
        (
            SaveNumberTarget::Unit {
                unit: 0,
                field: SaveUnitField::CharacterID,
            },
            -3,
        ),
        (
            SaveNumberTarget::Unit {
                unit: 0,
                field: SaveUnitField::TroopInfoIndex2,
            },
            -4,
        ),
        (SaveNumberTarget::SelectedUnit, -5),
        (SaveNumberTarget::MissionCompletion { slot: 0 }, -6),
        (SaveNumberTarget::CurrentMissionIndex, -7),
    ];
    for (target, expected) in cases {
        assert_eq!(document.number(target).unwrap(), expected, "{target:?}");
    }
}

#[test]
fn equal_numeric_edit_is_unchanged_and_keeps_exact_source_bytes() {
    let source = complete_save_fixture(SaveFixtureOptions::default());
    let mut document = SaveDocument::parse(source.clone()).unwrap();

    assert_eq!(
        document
            .set_number(SaveNumberTarget::CampaignIndex, 0)
            .unwrap(),
        SaveMutation::Unchanged,
    );
    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn unknown_choice_values_survive_an_unrelated_edit_and_reparse() {
    let source = fixture_with_unknown_choices(99, 1_234, -99);
    let mut document = SaveDocument::parse(source).unwrap();
    assert_eq!(
        document
            .number(SaveNumberTarget::Unit {
                unit: 0,
                field: SaveUnitField::UCD
            })
            .unwrap(),
        99,
    );

    assert_eq!(
        document
            .set_number(
                SaveNumberTarget::Unit {
                    unit: 0,
                    field: SaveUnitField::SkillLevel
                },
                77,
            )
            .unwrap(),
        SaveMutation::Changed { previous: 8 },
    );

    let reparsed = SaveDocument::parse(document.encode().unwrap()).unwrap();
    assert_eq!(
        reparsed
            .number(SaveNumberTarget::Unit {
                unit: 0,
                field: SaveUnitField::UCD
            })
            .unwrap(),
        99,
    );
    assert_eq!(
        reparsed
            .number(SaveNumberTarget::Equipment {
                unit: 0,
                slot: SaveEquipmentSlot::LeaderWeapon,
                field: SaveEquipmentField::SkillType1,
            })
            .unwrap(),
        1_234,
    );
    assert_eq!(
        reparsed
            .number(SaveNumberTarget::Equipment {
                unit: 0,
                slot: SaveEquipmentSlot::LeaderWeapon,
                field: SaveEquipmentField::ResistType1,
            })
            .unwrap(),
        -99,
    );
}

#[test]
fn changed_numeric_mutation_round_trips_every_leaf_and_equipment_slot() {
    let mut document =
        SaveDocument::parse(complete_save_fixture(SaveFixtureOptions::default())).unwrap();
    let cases = changed_numeric_cases();

    for &(target, value) in &cases {
        let previous = document.number(target).unwrap();
        assert_ne!(
            previous, value,
            "fixture did not force a change for {target:?}"
        );
        assert_eq!(
            document.set_number(target, value).unwrap(),
            SaveMutation::Changed { previous },
            "{target:?}",
        );
        assert_eq!(document.number(target).unwrap(), value, "{target:?}");
    }

    let reparsed = SaveDocument::parse(document.encode().unwrap()).unwrap();
    for (target, expected) in cases {
        assert_eq!(reparsed.number(target).unwrap(), expected, "{target:?}");
    }
}

#[test]
fn invalid_numeric_target_and_value_leave_document_bytes_unchanged() {
    let source = complete_save_fixture(SaveFixtureOptions::default());
    let mut document = SaveDocument::parse(source.clone()).unwrap();
    let target = SaveNumberTarget::Equipment {
        unit: 9,
        slot: SaveEquipmentSlot::TroopArmor,
        field: SaveEquipmentField::EnhancementTier,
    };

    assert!(matches!(
        document.set_number(target, 1),
        Err(FormatError::SaveTargetOutOfRange {
            target: actual,
            record_count: 1,
        }) if actual == target
    ));
    assert_eq!(document.encode().unwrap(), source);

    let target = SaveNumberTarget::Equipment {
        unit: 0,
        slot: SaveEquipmentSlot::TroopArmor,
        field: SaveEquipmentField::EnhancementTier,
    };
    assert!(matches!(
        document.set_number(target, 32_768),
        Err(FormatError::SaveValueOutOfRange {
            target: actual,
            value: 32_768,
            minimum: -32_768,
            maximum: 32_767,
        }) if actual == target
    ));
    assert_eq!(document.encode().unwrap(), source);

    assert!(matches!(
        document.set_number(SaveNumberTarget::CampaignIndex, 4),
        Err(FormatError::SaveValueOutOfRange {
            target: SaveNumberTarget::CampaignIndex,
            value: 4,
            minimum: 0,
            maximum: 3,
        })
    ));
    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn edited_envelope_restores_flags_context_tail_padding_and_prefix_length() {
    let tail = [0xde, 0xad, 0, 0xbe, 0xef];
    for size_prefix in [false, true] {
        for context in [false, true] {
            let source = complete_save_fixture(SaveFixtureOptions {
                size_prefix,
                context,
                pad_to_32_kib: false,
                tail: tail.to_vec(),
            });
            let offsets = complete_save_offsets(size_prefix, context);
            let mut document = SaveDocument::parse(source.clone()).unwrap();
            assert!(matches!(
                document.set_number(
                    SaveNumberTarget::Unit {
                        unit: 0,
                        field: SaveUnitField::SkillLevel
                    },
                    77,
                ),
                Ok(SaveMutation::Changed { previous: 8 })
            ));

            let encoded = document.encode().unwrap();

            assert_eq!(encoded.len(), 0x8000 + tail.len());
            assert_eq!(read_u32(&encoded, offsets.magic), 0x6e);
            if size_prefix {
                assert_eq!(read_u32(&encoded, 0), u32::try_from(encoded.len()).unwrap());
            }
            if let Some(context_start) = offsets.context {
                let context_end = context_start + 0x438;
                assert_eq!(
                    encoded.get(context_start..context_end),
                    source.get(context_start..context_end)
                );
            }

            let edited = offsets.unit + 52;
            assert_eq!(read_u32(&encoded, edited), 77);
            assert_eq!(
                encoded.get(offsets.magic..edited),
                source.get(offsets.magic..edited)
            );
            assert_eq!(
                encoded.get(edited + 4..offsets.tail),
                source.get(edited + 4..offsets.tail)
            );
            assert!(
                encoded
                    .get(offsets.tail..0x8000)
                    .is_some_and(|padding| padding.iter().all(|byte| *byte == 0))
            );
            assert_eq!(encoded.get(0x8000..0x8000 + tail.len()), Some(&tail[..]));

            let reparsed = SaveDocument::parse(encoded).unwrap();
            assert_eq!(reparsed.has_size_prefix(), size_prefix);
            assert_eq!(reparsed.has_context(), context);
            assert_eq!(
                reparsed
                    .number(SaveNumberTarget::Unit {
                        unit: 0,
                        field: SaveUnitField::SkillLevel,
                    })
                    .unwrap(),
                77,
            );
        }
    }
}

#[test]
fn save_diagnostics_use_exact_numeric_targets_for_unknown_values() {
    let document = SaveDocument::parse(fixture_with_unknown_choices(99, 1_234, -99)).unwrap();
    let diagnostics = document.diagnostics();

    let expected = [
        SaveNumberTarget::Unit {
            unit: 0,
            field: SaveUnitField::UCD,
        },
        SaveNumberTarget::Equipment {
            unit: 0,
            slot: SaveEquipmentSlot::LeaderWeapon,
            field: SaveEquipmentField::SkillType1,
        },
        SaveNumberTarget::Equipment {
            unit: 0,
            slot: SaveEquipmentSlot::LeaderWeapon,
            field: SaveEquipmentField::ResistType1,
        },
        SaveNumberTarget::CurrentMissionIndex,
    ];
    assert_eq!(diagnostics.len(), expected.len());
    for target in expected {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == Severity::Warning
                    && diagnostic.location == DiagnosticLocation::Save(target)
            }),
            "missing diagnostic for {target:?}"
        );
    }
}

const I32_BOUNDS: (i64, i64) = (-2_147_483_648, 2_147_483_647);
const U32_BOUNDS: (i64, i64) = (0, 4_294_967_295);
const I16_BOUNDS: (i64, i64) = (-32_768, 32_767);
const U16_BOUNDS: (i64, i64) = (0, 65_535);
const U8_BOUNDS: (i64, i64) = (0, 255);

fn changed_numeric_cases() -> Vec<(SaveNumberTarget, i64)> {
    let mut cases = vec![(SaveNumberTarget::CampaignIndex, 2)];
    for (field, value) in SaveMainField::ALL
        .into_iter()
        .zip([10_000, 10_001, -10_002, 10_003, 10_004, 10_005, 10_006])
    {
        cases.push((SaveNumberTarget::Main(field), value));
    }
    cases.push((SaveNumberTarget::SelectedUnit, -10_010));

    for (index, field) in SaveUnitField::ALL.into_iter().enumerate() {
        let Ok(index) = i64::try_from(index) else {
            panic!("unit field index does not fit i64");
        };
        let value = match field {
            SaveUnitField::LeaderNameIndex
            | SaveUnitField::TroopInfoIndex
            | SaveUnitField::CharacterID
            | SaveUnitField::TroopInfoIndex2 => -20_000 - index,
            SaveUnitField::Byte58 | SaveUnitField::HeroFlag | SaveUnitField::Byte5A => 20 + index,
            SaveUnitField::JobType
            | SaveUnitField::ModelID
            | SaveUnitField::STGField34
            | SaveUnitField::STGField38
            | SaveUnitField::STGField3C
            | SaveUnitField::STGField40
            | SaveUnitField::UCD
            | SaveUnitField::FormationType
            | SaveUnitField::GridConfig
            | SaveUnitField::SkillLevel
            | SaveUnitField::Field60
            | SaveUnitField::Field64
            | SaveUnitField::Field68
            | SaveUnitField::Field504 => 20_000 + index,
        };
        cases.push((SaveNumberTarget::Unit { unit: 0, field }, value));
    }

    for (slot_index, slot) in SaveEquipmentSlot::ALL.into_iter().enumerate() {
        let Ok(slot_index) = i64::try_from(slot_index) else {
            panic!("equipment slot index does not fit i64");
        };
        for (field_index, field) in SaveEquipmentField::ALL.into_iter().enumerate() {
            let Ok(field_index) = i64::try_from(field_index) else {
                panic!("equipment field index does not fit i64");
            };
            let magnitude = 30_000 + slot_index * 100 + field_index;
            let value = match field {
                SaveEquipmentField::AutoID
                | SaveEquipmentField::Level
                | SaveEquipmentField::VariantIndex
                | SaveEquipmentField::EquippedFlag
                | SaveEquipmentField::Reserved => magnitude,
                SaveEquipmentField::EnhancementTier
                | SaveEquipmentField::ItemPower
                | SaveEquipmentField::ItemTypeID
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
                | SaveEquipmentField::SlotCategory => -magnitude,
            };
            cases.push((
                SaveNumberTarget::Equipment {
                    unit: 0,
                    slot,
                    field,
                },
                value,
            ));
        }
    }

    for (index, field) in SaveRosterField::ALL.into_iter().enumerate() {
        let Ok(index) = i64::try_from(index) else {
            panic!("roster field index does not fit i64");
        };
        let value = if field == SaveRosterField::Value64 {
            40_000
        } else {
            100 + index
        };
        cases.push((SaveNumberTarget::Roster { record: 0, field }, value));
    }
    for slot in 0..20 {
        let Ok(slot_value) = i64::try_from(slot) else {
            panic!("mission slot does not fit i64");
        };
        cases.push((
            SaveNumberTarget::MissionCompletion { slot },
            -100 - slot_value,
        ));
    }
    cases.push((SaveNumberTarget::CurrentMissionIndex, -200));
    cases.push((SaveNumberTarget::SecondArray { record: 0 }, 50_000));
    cases
}

const fn number((minimum, maximum): (i64, i64)) -> SaveEditor {
    SaveEditor::Number { minimum, maximum }
}

fn assert_number_metadata(
    document: &SaveDocument,
    target: SaveNumberTarget,
    bounds: (i64, i64),
    editor: SaveEditor,
) {
    let actual_bounds = match document.number_storage_bounds(target) {
        Ok(actual) => actual,
        Err(error) => panic!("failed to read bounds for {target:?}: {error}"),
    };
    assert_eq!(actual_bounds, bounds, "{target:?}");
    let actual_editor = match document.number_editor(target) {
        Ok(actual) => actual,
        Err(error) => panic!("failed to read editor for {target:?}: {error}"),
    };
    assert_eq!(actual_editor, editor, "{target:?}");
}

fn unit_metadata(field: SaveUnitField) -> ((i64, i64), SaveEditor) {
    let bounds = match field {
        SaveUnitField::LeaderNameIndex
        | SaveUnitField::TroopInfoIndex
        | SaveUnitField::CharacterID
        | SaveUnitField::TroopInfoIndex2 => I32_BOUNDS,
        SaveUnitField::Byte58 | SaveUnitField::HeroFlag | SaveUnitField::Byte5A => U8_BOUNDS,
        SaveUnitField::JobType
        | SaveUnitField::ModelID
        | SaveUnitField::STGField34
        | SaveUnitField::STGField38
        | SaveUnitField::STGField3C
        | SaveUnitField::STGField40
        | SaveUnitField::UCD
        | SaveUnitField::FormationType
        | SaveUnitField::GridConfig
        | SaveUnitField::SkillLevel
        | SaveUnitField::Field60
        | SaveUnitField::Field64
        | SaveUnitField::Field68
        | SaveUnitField::Field504 => U32_BOUNDS,
    };
    let editor = match field {
        SaveUnitField::UCD => SaveEditor::UCD,
        SaveUnitField::HeroFlag => SaveEditor::HERO,
        SaveUnitField::SkillLevel => number(U16_BOUNDS),
        _ => number(bounds),
    };
    (bounds, editor)
}

fn equipment_metadata(field: SaveEquipmentField) -> ((i64, i64), SaveEditor) {
    let bounds = match field {
        SaveEquipmentField::AutoID => U32_BOUNDS,
        SaveEquipmentField::Level
        | SaveEquipmentField::VariantIndex
        | SaveEquipmentField::EquippedFlag
        | SaveEquipmentField::Reserved => U16_BOUNDS,
        SaveEquipmentField::EnhancementTier | SaveEquipmentField::ItemPower => I16_BOUNDS,
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
        | SaveEquipmentField::SlotCategory => I32_BOUNDS,
    };
    let editor = match field {
        SaveEquipmentField::SkillType1 | SaveEquipmentField::SkillType2 => SaveEditor::SKILL,
        SaveEquipmentField::ResistType1 | SaveEquipmentField::ResistType2 => SaveEditor::RESISTANCE,
        _ => number(bounds),
    };
    (bounds, editor)
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
