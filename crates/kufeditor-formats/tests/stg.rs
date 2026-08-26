#![allow(
    clippy::indexing_slicing,
    reason = "synthetic STG fixtures expose checked offsets for direct wire corruption"
)]

use std::collections::HashSet;

use encoding_rs::EUC_KR;

#[path = "support/stg.rs"]
mod stg_support;

use kufeditor_formats::{
    Diagnostic, DiagnosticLocation, FormatError, STGAbilityOwner, STGAreaField, STGAreaFloatField,
    STGCleaveError, STGCleaveErrorKind, STGCollection, STGDocument, STGEditor, STGEncodeError,
    STGFieldAccess, STGFloatTarget, STGFloatValue, STGFooterField, STGHeaderTextField, STGMutation,
    STGNumberTarget, STGParameterTarget, STGParseError, STGPreflightError, STGRebaseError,
    STGRegion, STGScriptKind, STGScriptTarget, STGSkillField, STGSkillOwner, STGStructuralLocation,
    STGTailFailure, STGTailStatus, STGTarget, STGText, STGTextEncoding, STGTextError, STGTextImage,
    STGTextTarget, STGUnitField, STGUnitFloatField, STGUnitGroup, STGValueKind, STGValueTarget,
    Severity,
};
use stg_support::{complete_stg_fixture, empty_stg_fixture, stg_prefix_fixture};

#[test]
fn synthetic_stg_fixture_names_every_recursive_count_and_value_offset() {
    let fixture = complete_stg_fixture();
    let offsets = fixture.offsets;

    assert_eq!(offsets.tail_start, stg_prefix_fixture(1).len());
    assert!(offsets.area_count < offsets.variable_count);
    assert!(offsets.variable_count < offsets.event_block_count);
    assert!(offsets.event_block_count < offsets.event_count);
    assert!(offsets.event_count < offsets.condition_count);
    assert!(offsets.condition_count < offsets.action_count);
    assert!(offsets.action_count < offsets.footer_count);
    assert!(offsets.footer_count < offsets.suffix);
    assert_eq!(offsets.suffix + 4, fixture.bytes.len());
    assert_eq!(empty_stg_fixture().len(), stg_prefix_fixture(0).len() + 20);
}

#[test]
fn stg_parse_accepts_a_complete_two_phase_document() {
    let fixture = complete_stg_fixture();
    let document = STGDocument::parse(fixture.bytes.clone()).unwrap();

    assert_eq!(document.unit_count(), 1);
    assert_eq!(document.area_count(), Some(1));
    assert_eq!(document.variable_count(), Some(4));
    assert_eq!(document.event_block_count(), Some(2));
    assert_eq!(document.footer_count(), Some(2));
    assert_eq!(
        document.tail_status(),
        STGTailStatus::Parsed {
            suffix: &fixture.bytes[fixture.offsets.suffix..],
        }
    );
}

#[test]
fn stg_parse_rejects_invalid_or_incomplete_prefixes() {
    let bad_magic = 999_u32.to_le_bytes().to_vec();
    assert_eq!(
        stg_parse_error(bad_magic),
        STGParseError::InvalidMagic {
            offset: 0,
            actual: 999,
        }
    );

    let mut truncated_header = stg_prefix_fixture(0);
    truncated_header.truncate(4 + 619);
    assert_eq!(
        stg_parse_error(truncated_header),
        STGParseError::PrefixPreflight(STGPreflightError::Truncated {
            region: STGRegion::Header,
            offset: 4,
            needed: 620,
            remaining: 619,
        })
    );

    let mut missing_unit_count = stg_prefix_fixture(0);
    missing_unit_count.truncate(4 + 620);
    assert_eq!(
        stg_parse_error(missing_unit_count),
        STGParseError::PrefixPreflight(STGPreflightError::Truncated {
            region: STGRegion::Units,
            offset: 624,
            needed: 4,
            remaining: 0,
        })
    );

    let mut truncated_unit = stg_prefix_fixture(1);
    truncated_unit.truncate(truncated_unit.len() - 1);
    assert_eq!(
        stg_parse_error(truncated_unit),
        STGParseError::PrefixPreflight(STGPreflightError::ImpossibleCount {
            region: STGRegion::Units,
            offset: 624,
            count: 1,
            minimum_item_size: 544,
            remaining: 543,
        })
    );

    let mut impossible_units = stg_prefix_fixture(0);
    impossible_units[624..628].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        stg_parse_error(impossible_units),
        STGParseError::PrefixPreflight(STGPreflightError::ImpossibleCount {
            region: STGRegion::Units,
            offset: 624,
            count: u32::MAX,
            minimum_item_size: 544,
            remaining: 0,
        })
    );
}

#[test]
fn stg_parse_accepts_zero_units_and_keeps_fixed_text_as_raw_bytes() {
    let empty = STGDocument::parse(empty_stg_fixture()).unwrap();
    assert_eq!(empty.unit_count(), 0);
    assert!(matches!(
        empty.tail_status(),
        STGTailStatus::Parsed { suffix } if suffix == [0, 0, 0, 0]
    ));

    let mut fixture = complete_stg_fixture();
    fixture.bytes[4 + 68] = 0xff;
    fixture.bytes[628] = 0x81;
    let document = STGDocument::parse(fixture.bytes).unwrap();
    assert_eq!(document.unit_count(), 1);
    assert!(matches!(
        document.tail_status(),
        STGTailStatus::Parsed { .. }
    ));
}

#[test]
fn stg_parse_falls_back_to_the_exact_raw_tail_for_every_recursive_count() {
    let fixture = complete_stg_fixture();
    let cases = [
        (fixture.offsets.area_count, STGRegion::Areas),
        (fixture.offsets.variable_count, STGRegion::Variables),
        (fixture.offsets.event_block_count, STGRegion::EventBlocks),
        (fixture.offsets.event_count, STGRegion::Events),
        (fixture.offsets.condition_count, STGRegion::Conditions),
        (fixture.offsets.action_count, STGRegion::Actions),
        (
            fixture.offsets.condition_parameter_count,
            STGRegion::Parameters,
        ),
        (
            fixture.offsets.variable_string_length,
            STGRegion::Parameters,
        ),
        (fixture.offsets.footer_count, STGRegion::Footer),
    ];

    for (offset, region) in cases {
        let mut bytes = fixture.bytes.clone();
        bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let expected_tail = bytes[fixture.offsets.tail_start..].to_vec();
        let document = STGDocument::parse(bytes).unwrap();

        match document.tail_status() {
            STGTailStatus::Raw {
                bytes,
                failure:
                    STGTailFailure::Preflight(STGPreflightError::ImpossibleCount {
                        region: actual_region,
                        offset: actual_offset,
                        ..
                    }),
            } => {
                assert_eq!(*actual_region, region);
                assert_eq!(*actual_offset, offset);
                assert_eq!(bytes, expected_tail);
            }
            actual => panic!("unexpected tail status for {region} at {offset}: {actual:?}"),
        }
    }
}

#[test]
fn stg_parse_falls_back_for_unknown_variable_and_script_parameter_tags() {
    let fixture = complete_stg_fixture();

    for offset in [
        fixture.offsets.variable_integer_type,
        fixture.offsets.condition_integer_type,
    ] {
        let mut bytes = fixture.bytes.clone();
        bytes[offset..offset + 4].copy_from_slice(&99_u32.to_le_bytes());
        let expected_tail = bytes[fixture.offsets.tail_start..].to_vec();
        let document = STGDocument::parse(bytes).unwrap();

        assert_eq!(
            document.tail_status(),
            STGTailStatus::Raw {
                bytes: &expected_tail,
                failure: &STGTailFailure::Preflight(STGPreflightError::UnknownParameterType {
                    offset,
                    tag: 99
                }),
            }
        );
    }
}

#[test]
fn stg_parse_distinguishes_missing_empty_and_garbage_tails() {
    let prefix = stg_prefix_fixture(0);
    let tail_start = prefix.len();
    let missing = STGDocument::parse(prefix.clone()).unwrap();
    assert_eq!(
        missing.tail_status(),
        STGTailStatus::Raw {
            bytes: &[],
            failure: &STGTailFailure::Preflight(STGPreflightError::Truncated {
                region: STGRegion::Areas,
                offset: tail_start,
                needed: 4,
                remaining: 0,
            }),
        }
    );

    let empty = STGDocument::parse(empty_stg_fixture()).unwrap();
    assert!(matches!(
        empty.tail_status(),
        STGTailStatus::Parsed { suffix } if suffix == [0, 0, 0, 0]
    ));

    let mut garbage = prefix;
    garbage.extend_from_slice(&[1, 2, 3]);
    let raw = STGDocument::parse(garbage).unwrap();
    assert!(matches!(
        raw.tail_status(),
        STGTailStatus::Raw {
            bytes: [1, 2, 3],
            failure: STGTailFailure::Preflight(STGPreflightError::Truncated {
                region: STGRegion::Areas,
                offset,
                needed: 4,
                remaining: 3,
            }),
        } if *offset == tail_start
    ));
}

fn stg_parse_error(bytes: Vec<u8>) -> STGParseError {
    match STGDocument::parse(bytes) {
        Err(FormatError::STGParse(error)) => error,
        Err(other) => panic!("unexpected STG error: {other}"),
        Ok(_) => panic!("expected STG parse failure"),
    }
}

#[test]
fn float_values_preserve_every_wire_bit() {
    let patterns = [
        0x0000_0000,
        0x8000_0000,
        0x3f80_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0001,
        0x7fc0_0002,
        u32::MAX,
    ];

    for bits in patterns {
        assert_eq!(STGFloatValue::from_bits(bits).to_bits(), bits);
    }

    assert_ne!(
        STGFloatValue::from_bits(0x0000_0000),
        STGFloatValue::from_bits(0x8000_0000)
    );
    assert_ne!(
        STGFloatValue::from_bits(0x7fc0_0001),
        STGFloatValue::from_bits(0x7fc0_0002)
    );
}

#[test]
fn finite_float_boundary_rejects_nonfinite_values_without_normalizing_bits() {
    for value in [0.0_f32, -0.0, 17.25, f32::MIN, f32::MAX] {
        let stored = STGFloatValue::from_finite(value).unwrap();
        assert_eq!(stored.to_bits(), value.to_bits());
        assert_eq!(stored.finite_value().unwrap().to_bits(), value.to_bits());
    }

    for bits in [0x7f80_0000, 0xff80_0000, 0x7fc0_0001, 0x7fc0_0002] {
        let value = f32::from_bits(bits);
        assert!(STGFloatValue::from_finite(value).is_none());
        assert!(STGFloatValue::from_bits(bits).finite_value().is_none());
        assert_eq!(STGFloatValue::from_bits(bits).to_bits(), bits);
    }
}

#[test]
fn value_targets_distinguish_variable_initial_values_from_script_parameters() {
    let script = STGScriptTarget {
        block: 2,
        event: 3,
        kind: STGScriptKind::Action,
        script: 4,
    };
    let parameter = STGParameterTarget {
        script,
        parameter: 1,
    };

    assert_eq!(
        STGValueTarget::VariableInitial { variable: 7 },
        STGValueTarget::VariableInitial { variable: 7 }
    );
    assert_eq!(
        STGValueTarget::ScriptParameter(parameter),
        STGValueTarget::ScriptParameter(parameter)
    );
    assert_ne!(
        STGValueTarget::VariableInitial { variable: 7 },
        STGValueTarget::ScriptParameter(parameter)
    );
}

#[test]
fn stable_targets_cover_each_editable_stg_path() {
    let script = STGScriptTarget {
        block: 1,
        event: 2,
        kind: STGScriptKind::Condition,
        script: 3,
    };
    let value = STGValueTarget::ScriptParameter(STGParameterTarget {
        script,
        parameter: 4,
    });

    let number_targets = [
        STGNumberTarget::Unit {
            unit: 0,
            field: STGUnitField::UniqueID,
        },
        STGNumberTarget::Skill {
            unit: 0,
            owner: STGSkillOwner::Officer1,
            slot: 1,
            field: STGSkillField::ID,
        },
        STGNumberTarget::Ability {
            unit: 0,
            owner: STGAbilityOwner::Officer2,
            slot: 2,
        },
        STGNumberTarget::Area {
            area: 0,
            field: STGAreaField::AreaID,
        },
        STGNumberTarget::VariableID { variable: 0 },
        STGNumberTarget::EventBlockHeader { block: 0 },
        STGNumberTarget::EventID { block: 0, event: 0 },
        STGNumberTarget::ParameterInteger { value },
        STGNumberTarget::Footer {
            entry: 0,
            field: STGFooterField::SlotData1,
        },
    ];
    let float_targets = [
        STGFloatTarget::Unit {
            unit: 0,
            field: STGUnitFloatField::LeaderHPOverride,
        },
        STGFloatTarget::StatOverride { unit: 0, slot: 0 },
        STGFloatTarget::Area {
            area: 0,
            field: STGAreaFloatField::BoundX1,
        },
        STGFloatTarget::Parameter { value },
    ];
    let text_targets = [
        STGTextTarget::Header(STGHeaderTextField::MapFilename),
        STGTextTarget::UnitName { unit: 0 },
        STGTextTarget::AreaDescription { area: 0 },
        STGTextTarget::VariableName { variable: 0 },
        STGTextTarget::EventDescription { block: 0, event: 0 },
        STGTextTarget::ParameterString { value },
    ];

    assert_eq!(number_targets.len(), 9);
    assert_eq!(float_targets.len(), 4);
    assert_eq!(text_targets.len(), 6);
    assert_eq!(number_targets[0].label(), "Unique ID");
    assert_eq!(float_targets[0].label(), "Leader HP Override");
    assert_eq!(text_targets[0].label(), "Map Filename");
}

#[test]
fn field_metadata_has_unique_members_and_stable_acronym_labels() {
    let unit_fields: HashSet<_> = STGUnitField::ALL.into_iter().collect();
    assert_eq!(unit_fields.len(), STGUnitField::ALL.len());
    assert_eq!(STGUnitField::UniqueID.label(), "Unique ID");
    assert_eq!(STGUnitField::UCD.label(), "UCD");
    assert_eq!(STGUnitField::LeaderModelID.label(), "Leader Model ID");
    assert_eq!(STGUnitField::LeaderWorldmapID.label(), "Leader Worldmap ID");
    assert_eq!(
        STGUnitFloatField::LeaderHPOverride.label(),
        "Leader HP Override"
    );
    assert_eq!(STGAreaField::AreaID.label(), "Area ID");
    assert_eq!(STGSkillField::ID.label(), "Skill ID");
    assert_eq!(STGUnitField::FormationType.group(), STGUnitGroup::Formation);
}

#[test]
fn stg_fields_cover_every_numeric_target_without_cross_talk() {
    let targets = editable_number_targets();
    assert_eq!(targets.len(), 129);
    assert_eq!(targets.iter().copied().collect::<HashSet<_>>().len(), 129);

    for (changed_index, target) in targets.iter().copied().enumerate() {
        let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
        let before = number_snapshot(&document, &targets);
        let previous = before[changed_index];
        let replacement = replacement_number(target, previous);

        assert_eq!(
            document.set_number(target, replacement).unwrap(),
            STGMutation::Changed { previous },
            "failed to change {target:?}",
        );
        let after = number_snapshot(&document, &targets);
        for (index, (before, after)) in before.iter().zip(&after).enumerate() {
            if index == changed_index {
                assert_eq!(*after, replacement, "wrong value for {target:?}");
            } else {
                assert_eq!(before, after, "{target:?} changed target {index}");
            }
        }

        assert_eq!(
            document.set_number(target, previous).unwrap(),
            STGMutation::Changed {
                previous: replacement,
            },
            "failed to restore {target:?}",
        );
        assert_eq!(number_snapshot(&document, &targets), before);
        assert_eq!(
            document.set_number(target, previous).unwrap(),
            STGMutation::Unchanged,
        );
    }
}

#[test]
fn stg_fields_cover_every_float_target_without_cross_talk() {
    let targets = editable_float_targets();
    assert_eq!(targets.len(), 32);
    assert_eq!(targets.iter().copied().collect::<HashSet<_>>().len(), 32);

    for (changed_index, target) in targets.iter().copied().enumerate() {
        let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
        let before = float_snapshot(&document, &targets);
        let previous = before[changed_index];
        let replacement = STGFloatValue::from_bits(previous.to_bits() ^ 0x55aa_33cc);

        assert_eq!(
            document.set_float(target, replacement).unwrap(),
            STGMutation::Changed { previous },
            "failed to change {target:?}",
        );
        let after = float_snapshot(&document, &targets);
        for (index, (before, after)) in before.iter().zip(&after).enumerate() {
            if index == changed_index {
                assert_eq!(*after, replacement, "wrong value for {target:?}");
            } else {
                assert_eq!(before, after, "{target:?} changed target {index}");
            }
        }
    }
}

#[test]
fn stg_fields_keep_wire_bounds_separate_from_semantic_editors() {
    let ucd = STGNumberTarget::Unit {
        unit: 0,
        field: STGUnitField::UCD,
    };
    assert_eq!(ucd.storage_bounds(), (0, i64::from(u8::MAX)));
    let Some(STGEditor::Choice { choices }) = ucd.editor() else {
        panic!("UCD must use a choice editor");
    };
    assert_eq!(
        choices
            .iter()
            .map(|choice| choice.value)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3],
    );

    let skill_id = STGNumberTarget::Skill {
        unit: 0,
        owner: STGSkillOwner::Leader,
        slot: 0,
        field: STGSkillField::ID,
    };
    assert_eq!(skill_id.storage_bounds(), (0, i64::from(u8::MAX)));
    assert_eq!(
        skill_id.editor(),
        Some(STGEditor::Number {
            minimum: 0,
            maximum: 254,
        })
    );

    for (field, expected) in [
        (STGUnitField::LeaderLevel, (1, 99)),
        (STGUnitField::Officer1Level, (1, 99)),
        (STGUnitField::Officer2Level, (1, 99)),
        (STGUnitField::OfficerCount, (0, 2)),
        (STGUnitField::GridX, (1, i64::from(u32::MAX))),
        (STGUnitField::GridY, (1, i64::from(u32::MAX))),
    ] {
        let target = STGNumberTarget::Unit { unit: 0, field };
        assert_eq!(
            target.editor(),
            Some(STGEditor::Number {
                minimum: expected.0,
                maximum: expected.1,
            })
        );
        assert_ne!(target.storage_bounds(), expected, "{field:?}");
    }
}

#[test]
fn stg_fields_preserve_and_restore_unknown_choice_values() {
    let mut fixture = complete_stg_fixture();
    let unit = fixture.offsets.unit_name;
    for (relative, value) in [(36, 201), (37, 202), (38, 203), (76, 204), (86, 205)] {
        fixture.bytes[unit + relative] = value;
    }
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let targets = [
        (STGUnitField::UCD, 201),
        (STGUnitField::HeroFlag, 202),
        (STGUnitField::EnabledFlag, 203),
        (STGUnitField::FacingDirection, 204),
        (STGUnitField::LeaderWorldmapID, 205),
    ];

    document
        .set_number(
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::UniqueID,
            },
            77,
        )
        .unwrap();
    for (field, raw) in targets {
        let target = STGNumberTarget::Unit { unit: 0, field };
        assert_eq!(document.number(target).unwrap(), raw);
        assert_eq!(
            document.set_number(target, 0).unwrap(),
            STGMutation::Changed { previous: raw },
        );
        assert_eq!(
            document.set_number(target, raw).unwrap(),
            STGMutation::Changed { previous: 0 },
        );
        assert_eq!(document.number(target).unwrap(), raw);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one atomicity matrix keeps every STG collection, slot, and wire bound together"
)]
fn stg_fields_reject_invalid_targets_and_ranges_atomically() {
    let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let observed_targets = editable_number_targets();
    let before_numbers = number_snapshot(&document, &observed_targets);
    let observed_floats = editable_float_targets();
    let before_floats = float_snapshot(&document, &observed_floats);

    let invalid_targets = [
        (
            STGNumberTarget::Unit {
                unit: 1,
                field: STGUnitField::UniqueID,
            },
            STGCollection::Unit,
            1,
            1,
        ),
        (
            STGNumberTarget::Skill {
                unit: 0,
                owner: STGSkillOwner::Leader,
                slot: 4,
                field: STGSkillField::ID,
            },
            STGCollection::Skill,
            4,
            4,
        ),
        (
            STGNumberTarget::Skill {
                unit: 0,
                owner: STGSkillOwner::Officer1,
                slot: 4,
                field: STGSkillField::Level,
            },
            STGCollection::Skill,
            4,
            4,
        ),
        (
            STGNumberTarget::Skill {
                unit: 0,
                owner: STGSkillOwner::Officer2,
                slot: 4,
                field: STGSkillField::ID,
            },
            STGCollection::Skill,
            4,
            4,
        ),
        (
            STGNumberTarget::Ability {
                unit: 0,
                owner: STGAbilityOwner::Leader,
                slot: 23,
            },
            STGCollection::Ability,
            23,
            23,
        ),
        (
            STGNumberTarget::Ability {
                unit: 0,
                owner: STGAbilityOwner::Officer1,
                slot: 23,
            },
            STGCollection::Ability,
            23,
            23,
        ),
        (
            STGNumberTarget::Ability {
                unit: 0,
                owner: STGAbilityOwner::Officer2,
                slot: 19,
            },
            STGCollection::Ability,
            19,
            19,
        ),
        (
            STGNumberTarget::Area {
                area: 1,
                field: STGAreaField::AreaID,
            },
            STGCollection::Area,
            1,
            1,
        ),
        (
            STGNumberTarget::VariableID { variable: 4 },
            STGCollection::Variable,
            4,
            4,
        ),
        (
            STGNumberTarget::EventBlockHeader { block: 2 },
            STGCollection::EventBlock,
            2,
            2,
        ),
        (
            STGNumberTarget::EventID { block: 0, event: 2 },
            STGCollection::Event,
            2,
            2,
        ),
        (
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::VariableInitial { variable: 4 },
            },
            STGCollection::Variable,
            4,
            4,
        ),
        (
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Condition,
                        script: 1,
                    },
                    parameter: 0,
                }),
            },
            STGCollection::Condition,
            1,
            1,
        ),
        (
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Action,
                        script: 1,
                    },
                    parameter: 0,
                }),
            },
            STGCollection::Action,
            1,
            1,
        ),
        (
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Action,
                        script: 0,
                    },
                    parameter: 2,
                }),
            },
            STGCollection::Parameter,
            2,
            2,
        ),
        (
            STGNumberTarget::Footer {
                entry: 2,
                field: STGFooterField::SlotData1,
            },
            STGCollection::FooterEntry,
            2,
            2,
        ),
    ];
    for (target, collection, index, count) in invalid_targets {
        assert_target_out_of_range(
            document.set_number(target, 1),
            STGTarget::Number(target),
            collection,
            index,
            count,
        );
        assert_stg_observed_state(
            &document,
            &observed_targets,
            &before_numbers,
            &observed_floats,
            &before_floats,
        );
    }

    let invalid_float_targets = [
        (
            STGFloatTarget::Unit {
                unit: 1,
                field: STGUnitFloatField::PositionX,
            },
            STGCollection::Unit,
            1,
            1,
        ),
        (
            STGFloatTarget::StatOverride { unit: 0, slot: 22 },
            STGCollection::StatOverride,
            22,
            22,
        ),
        (
            STGFloatTarget::Area {
                area: 1,
                field: STGAreaFloatField::BoundX1,
            },
            STGCollection::Area,
            1,
            1,
        ),
        (
            STGFloatTarget::Parameter {
                value: STGValueTarget::VariableInitial { variable: 4 },
            },
            STGCollection::Variable,
            4,
            4,
        ),
    ];
    for (target, collection, index, count) in invalid_float_targets {
        assert_target_out_of_range(
            document.set_float(target, STGFloatValue::from_bits(1)),
            STGTarget::Float(target),
            collection,
            index,
            count,
        );
        assert_stg_observed_state(
            &document,
            &observed_targets,
            &before_numbers,
            &observed_floats,
            &before_floats,
        );
    }

    let wrong_kind_value = STGValueTarget::VariableInitial { variable: 1 };
    assert_value_kind_mismatch(
        document.set_number(
            STGNumberTarget::ParameterInteger {
                value: wrong_kind_value,
            },
            1,
        ),
        wrong_kind_value,
        STGValueKind::Integer,
        STGValueKind::Float,
    );
    assert_stg_observed_state(
        &document,
        &observed_targets,
        &before_numbers,
        &observed_floats,
        &before_floats,
    );

    let range_failures = [
        (
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::UCD,
            },
            -1,
        ),
        (
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::UCD,
            },
            256,
        ),
        (
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::UniqueID,
            },
            -1,
        ),
        (
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::UniqueID,
            },
            i64::from(u32::MAX) + 1,
        ),
        (
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::TroopInfoIndex,
            },
            i64::from(i32::MIN) - 1,
        ),
        (
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::TroopInfoIndex,
            },
            i64::from(i32::MAX) + 1,
        ),
    ];
    for (target, value) in range_failures {
        assert_number_out_of_range(document.set_number(target, value), target, value);
        assert_stg_observed_state(
            &document,
            &observed_targets,
            &before_numbers,
            &observed_floats,
            &before_floats,
        );
    }

    for target in [
        STGNumberTarget::Unit {
            unit: 0,
            field: STGUnitField::Reserved27,
        },
        STGNumberTarget::Area {
            area: 0,
            field: STGAreaField::Unknown20,
        },
    ] {
        assert_eq!(document.number(target).unwrap(), 0);
        assert!(matches!(
            document.set_number(target, 1),
            Err(FormatError::STGReadOnlyTarget { target: STGTarget::Number(actual) }) if actual == target
        ));
        assert_stg_observed_state(
            &document,
            &observed_targets,
            &before_numbers,
            &observed_floats,
            &before_floats,
        );
    }
}

#[test]
fn stg_validation_matches_legacy_unit_rules_and_typed_locations() {
    let mut bytes = stg_prefix_fixture(2);
    let first = 4 + 620 + 4;
    let second = first + 544;
    bytes[first] = b'A';
    write_u32_at(&mut bytes, first + 32, 42);
    bytes[first + 36] = 4;
    bytes[first + 86] = 21;
    write_u32_at(&mut bytes, first + 188, 3);
    write_u32_at(&mut bytes, second + 32, 42);
    bytes[second + 87] = 100;

    let document = STGDocument::parse(bytes).unwrap();
    assert_eq!(
        document.diagnostics(),
        [
            stg_diagnostic(
                Severity::Error,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UCD
                }),
                "Invalid UCD value"
            ),
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::LeaderLevel
                }),
                "Level outside typical range (1-99)"
            ),
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::LeaderWorldmapID
                }),
                "Worldmap ID may cause post-mission issues"
            ),
            stg_diagnostic(
                Severity::Error,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UniqueID
                }),
                "Duplicate unique ID"
            ),
            stg_diagnostic(
                Severity::Error,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::OfficerCount
                }),
                "Officer count exceeds maximum of 2"
            ),
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGText(STGTextTarget::UnitName { unit: 1 }),
                "Unit has no name"
            ),
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 1,
                    field: STGUnitField::LeaderLevel
                }),
                "Level outside typical range (1-99)"
            ),
        ],
    );
}

#[test]
fn stg_validation_worldmap_boundary_exempts_20_and_0xff() {
    let mut bytes = stg_prefix_fixture(3);
    let first = 4 + 620 + 4;
    for (unit, worldmap_id) in [20_u8, 21, u8::MAX].into_iter().enumerate() {
        let start = first + unit * 544;
        let Ok(unit_id) = u32::try_from(unit) else {
            panic!("test STG unit index does not fit u32");
        };
        bytes[start] = b'A';
        write_u32_at(&mut bytes, start + 32, unit_id);
        bytes[start + 86] = worldmap_id;
        bytes[start + 87] = 1;
    }

    let document = STGDocument::parse(bytes).unwrap();
    assert_eq!(
        document.diagnostics(),
        [stg_diagnostic(
            Severity::Warning,
            DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                unit: 1,
                field: STGUnitField::LeaderWorldmapID,
            }),
            "Worldmap ID may cause post-mission issues",
        )],
    );
}

#[test]
fn undocumented_scalar_fields_are_projected_but_never_receive_editors() {
    for field in STGUnitField::ALL {
        let expected = if matches!(
            field,
            STGUnitField::Reserved27
                | STGUnitField::ExtraFlags1
                | STGUnitField::ExtraFlags2
                | STGUnitField::Category
                | STGUnitField::Reserved50
        ) {
            STGFieldAccess::ReadOnly
        } else {
            STGFieldAccess::Editable
        };
        assert_eq!(field.access(), expected, "unexpected access for {field:?}");
        assert_eq!(
            field.editor().is_some(),
            expected == STGFieldAccess::Editable,
            "unexpected editor for {field:?}"
        );
    }

    for field in STGUnitFloatField::ALL {
        let expected = if field == STGUnitFloatField::Unknown30 {
            STGFieldAccess::ReadOnly
        } else {
            STGFieldAccess::Editable
        };
        assert_eq!(field.access(), expected, "unexpected access for {field:?}");
    }

    for field in STGAreaField::ALL {
        let expected = if field == STGAreaField::AreaID {
            STGFieldAccess::Editable
        } else {
            STGFieldAccess::ReadOnly
        };
        assert_eq!(field.access(), expected, "unexpected access for {field:?}");
    }

    let target = STGNumberTarget::Unit {
        unit: 0,
        field: STGUnitField::Reserved27,
    };
    assert_eq!(target.access(), STGFieldAccess::ReadOnly);
    assert_eq!(target.editor(), None);
    assert!(matches!(
        FormatError::STGReadOnlyTarget {
            target: STGTarget::Number(target),
        },
        FormatError::STGReadOnlyTarget {
            target: STGTarget::Number(actual),
        } if actual == target
    ));
}

#[test]
fn stable_editor_text_mutation_and_diagnostic_types_do_not_expose_generated_values() {
    let editor = STGEditor::Number {
        minimum: 0,
        maximum: u8::MAX.into(),
    };
    assert_eq!(editor.number_bounds(), Some((0, i64::from(u8::MAX))));

    assert_eq!(
        STGText::Decoded(std::borrow::Cow::Borrowed("hello")).decoded(),
        Some("hello")
    );
    assert_eq!(STGText::Raw(&[0x81]).decoded(), None);

    let changed = STGMutation::Changed { previous: 7_i64 };
    assert_eq!(changed.previous(), Some(&7));
    assert_eq!(STGMutation::<i64>::Unchanged.previous(), None);

    let location = DiagnosticLocation::STGNumber(STGNumberTarget::VariableID { variable: 9 });
    assert_eq!(location.record(), Some(9));
    assert_eq!(location.label(), "Variable ID");

    let tail = DiagnosticLocation::STGTail {
        region: STGRegion::Actions,
        offset: 1_024,
    };
    assert_eq!(tail.record(), None);
    assert_eq!(tail.label(), "Raw STG Tail");
    assert_eq!(tail.stg_tail(), Some((STGRegion::Actions, 1_024)));
}

#[test]
fn stg_failures_keep_regions_targets_and_value_kinds_typed() {
    let cleave = STGCleaveError::UnexpectedEOF {
        offset: 71,
        needed: 8,
        remaining: 3,
    };
    assert_eq!(cleave.kind(), STGCleaveErrorKind::UnexpectedEOF);
    assert_eq!(cleave.offset(), Some(71));

    let preflight = STGPreflightError::Truncated {
        region: STGRegion::Variables,
        offset: 700,
        needed: 8,
        remaining: 3,
    };
    let parse = FormatError::STGParse(STGParseError::PrefixPreflight(preflight.clone()));
    assert!(parse.to_string().contains("variables"));
    assert_eq!(
        STGTailFailure::Preflight(preflight).region(),
        STGRegion::Variables
    );

    let allocation = STGPreflightError::AllocationBudgetExceeded {
        region: STGRegion::Events,
        offset: 900,
        retained: 32,
        requested: 48,
        maximum: 64,
    };
    assert_eq!(allocation.offset(), 900);
    assert_eq!(STGTailFailure::Preflight(allocation).offset(), 900);

    let target = STGNumberTarget::Unit {
        unit: 4,
        field: STGUnitField::UniqueID,
    };
    let range = FormatError::STGTargetOutOfRange {
        target: STGTarget::Number(target),
        collection: STGCollection::Unit,
        index: 4,
        count: 2,
    };
    assert!(range.to_string().contains("unit 4"));

    let value = STGValueTarget::VariableInitial { variable: 2 };
    let mismatch = FormatError::STGValueKindMismatch {
        target: value,
        expected: STGValueKind::Float,
        actual: STGValueKind::Integer,
    };
    assert!(mismatch.to_string().contains("expected float"));

    let text = FormatError::STGText {
        target: STGTextTarget::VariableName { variable: 2 },
        source: STGTextError::Unencodable {
            encoding: STGTextEncoding::CP949,
        },
    };
    assert!(text.to_string().contains("CP949"));

    let structural = FormatError::STGStructuralLocationMismatch {
        expected: STGStructuralLocation::Event { block: 1, event: 2 },
        actual: STGStructuralLocation::EventBlock { block: 1 },
    };
    assert!(structural.to_string().contains("structural image"));

    let encode = FormatError::STGEncode(STGEncodeError::CursorMismatch {
        expected: 128,
        actual: 120,
    });
    assert!(encode.to_string().contains("encoded STG"));

    let rebase = FormatError::STGRebase(STGRebaseError::ForeignLineage);
    assert!(rebase.to_string().contains("lineage"));
}

#[test]
fn stg_text_decodes_utf8_and_cp949_without_replacement() {
    let mut fixture = complete_stg_fixture();
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.header_map,
        64,
        "map-é".as_bytes(),
    );
    let korean = cp949("기사");
    write_fixed_text(&mut fixture.bytes, fixture.offsets.unit_name, 32, &korean);
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.area_description,
        32,
        &korean,
    );
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.variable_name,
        64,
        &korean,
    );
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.event_description,
        64,
        &korean,
    );
    let document = STGDocument::parse(fixture.bytes).unwrap();

    let cases = [
        (
            STGTextTarget::Header(STGHeaderTextField::MapFilename),
            "map-é",
        ),
        (STGTextTarget::UnitName { unit: 0 }, "기사"),
        (STGTextTarget::AreaDescription { area: 0 }, "기사"),
        (STGTextTarget::VariableName { variable: 0 }, "기사"),
        (
            STGTextTarget::EventDescription { block: 0, event: 0 },
            "기사",
        ),
    ];
    for (target, expected) in cases {
        let text = document.text(target).unwrap();
        assert_eq!(text.decoded(), Some(expected), "wrong text for {target:?}");
        assert_eq!(text.raw(), None, "unexpected raw text for {target:?}");
    }
}

#[test]
fn stg_text_mutation_reaches_every_storage_shape_and_preserves_snapshots() {
    let mut fixture = complete_stg_fixture();
    fixture.bytes
        [fixture.offsets.condition_integer_type..fixture.offsets.condition_integer_type + 4]
        .copy_from_slice(&2_u32.to_le_bytes());
    fixture.bytes
        [fixture.offsets.condition_integer_type + 4..fixture.offsets.condition_integer_type + 8]
        .copy_from_slice(&0_u32.to_le_bytes());
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.header_map,
        64,
        b"source-map",
    );
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.unit_name,
        32,
        b"source-unit",
    );
    write_fixed_text(
        &mut fixture.bytes,
        fixture.offsets.area_description,
        32,
        b"source-area",
    );
    let original = STGDocument::parse(fixture.bytes).unwrap();
    let action_string = STGTextTarget::ParameterString {
        value: STGValueTarget::ScriptParameter(STGParameterTarget {
            script: STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Action,
                script: 0,
            },
            parameter: 0,
        }),
    };
    let condition_string = STGTextTarget::ParameterString {
        value: STGValueTarget::ScriptParameter(STGParameterTarget {
            script: STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Condition,
                script: 0,
            },
            parameter: 0,
        }),
    };
    let cases = [
        (
            STGTextTarget::Header(STGHeaderTextField::MapFilename),
            "source-map",
        ),
        (STGTextTarget::UnitName { unit: 0 }, "source-unit"),
        (STGTextTarget::AreaDescription { area: 0 }, "source-area"),
        (STGTextTarget::VariableName { variable: 0 }, "Variable 100"),
        (
            STGTextTarget::EventDescription { block: 0, event: 0 },
            "Primary Event",
        ),
        (condition_string, ""),
        (action_string, "action"),
    ];

    for (target, source) in cases {
        let mut edited = original.clone();
        let source_image =
            changed_text_image(edited.set_text(target, "edited".to_owned()).unwrap());
        assert_eq!(edited.text(target).unwrap().decoded(), Some("edited"));
        assert_eq!(original.text(target).unwrap().decoded(), Some(source));

        let edited_image = changed_text_image(edited.restore_text(target, source_image).unwrap());
        assert_eq!(edited.text(target).unwrap().decoded(), Some(source));
        assert!(matches!(
            edited.restore_text(target, edited_image).unwrap(),
            STGMutation::Changed { .. }
        ));
        assert_eq!(edited.text(target).unwrap().decoded(), Some("edited"));
        assert_eq!(original.text(target).unwrap().decoded(), Some(source));
    }
}

#[test]
fn stg_text_keeps_invalid_fixed_images_and_restores_them_exactly() {
    let mut fixture = complete_stg_fixture();
    write_fixed_bytes(&mut fixture.bytes, fixture.offsets.unit_name, 32, &[0x81]);
    fixture.bytes[fixture.offsets.unit_name + 2] = 0x7a;
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let target = STGTextTarget::UnitName { unit: 0 };
    assert_eq!(document.text(target).unwrap().raw(), Some(&[0x81][..]));

    let original = changed_text_image(document.set_text(target, "Knight".to_owned()).unwrap());
    assert_eq!(document.text(target).unwrap().decoded(), Some("Knight"));

    let edited = changed_text_image(document.restore_text(target, original.clone()).unwrap());
    assert_eq!(document.text(target).unwrap().raw(), Some(&[0x81][..]));
    assert_eq!(
        changed_text_image(document.restore_text(target, edited.clone()).unwrap()),
        original
    );
    assert_eq!(document.text(target).unwrap().decoded(), Some("Knight"));

    let header = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    assert_stg_text_error(
        document.restore_text(header, edited),
        header,
        &STGTextError::ImageKindMismatch,
    );
    assert_eq!(document.text(target).unwrap().decoded(), Some("Knight"));
}

#[test]
fn stg_text_keeps_invalid_utf8_header_images_and_restores_them_exactly() {
    let mut fixture = complete_stg_fixture();
    write_fixed_bytes(&mut fixture.bytes, fixture.offsets.header_map, 64, &[0xff]);
    fixture.bytes[fixture.offsets.header_map + 2] = 0xaa;
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let target = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    assert_eq!(document.text(target).unwrap().raw(), Some(&[0xff][..]));

    let original = changed_text_image(document.set_text(target, "map".to_owned()).unwrap());
    let edited = changed_text_image(document.restore_text(target, original.clone()).unwrap());
    assert_eq!(document.text(target).unwrap().raw(), Some(&[0xff][..]));
    assert_eq!(
        changed_text_image(document.restore_text(target, edited).unwrap()),
        original
    );
    assert_eq!(document.text(target).unwrap().decoded(), Some("map"));
}

#[test]
fn stg_text_equal_visible_values_are_neutral_and_replacements_are_checked() {
    let mut fixture = complete_stg_fixture();
    write_fixed_bytes(&mut fixture.bytes, fixture.offsets.header_map, 64, b"same");
    fixture.bytes[fixture.offsets.header_map + 8] = 0xaa;
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let header = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    assert_eq!(
        document.set_text(header, "same".to_owned()).unwrap(),
        STGMutation::Unchanged
    );

    let unit = STGTextTarget::UnitName { unit: 0 };
    for (value, source) in [
        (
            "x".repeat(32),
            STGTextError::TooLong {
                length: 32,
                maximum: 31,
            },
        ),
        ("a\0b".to_owned(), STGTextError::ContainsZero { index: 1 }),
        (
            "🙂".to_owned(),
            STGTextError::Unencodable {
                encoding: STGTextEncoding::CP949,
            },
        ),
    ] {
        assert_stg_text_error(document.set_text(unit, value), unit, &source);
        assert_eq!(document.text(unit).unwrap().decoded(), Some(""));
    }

    let maximum_unit = "x".repeat(31);
    assert!(matches!(
        document.set_text(unit, maximum_unit.clone()).unwrap(),
        STGMutation::Changed { .. }
    ));
    assert_eq!(
        document.text(unit).unwrap().decoded(),
        Some(maximum_unit.as_str())
    );

    let maximum_header = "y".repeat(63);
    assert!(matches!(
        document.set_text(header, maximum_header.clone()).unwrap(),
        STGMutation::Changed { .. }
    ));
    assert_eq!(
        document.text(header).unwrap().decoded(),
        Some(maximum_header.as_str())
    );
    assert_stg_text_error(
        document.set_text(header, "y".repeat(64)),
        header,
        &STGTextError::TooLong {
            length: 64,
            maximum: 63,
        },
    );
}

#[test]
fn stg_text_full_width_source_is_readable_and_equal_value_is_neutral() {
    let mut fixture = complete_stg_fixture();
    let full_width = "z".repeat(64);
    write_fixed_bytes(
        &mut fixture.bytes,
        fixture.offsets.header_map,
        64,
        full_width.as_bytes(),
    );
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let target = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    assert_eq!(
        document.text(target).unwrap().decoded(),
        Some(full_width.as_str())
    );
    assert_eq!(
        document.set_text(target, full_width).unwrap(),
        STGMutation::Unchanged
    );
}

#[test]
fn stg_text_keeps_invalid_dynamic_bytes_and_restores_exact_capacity_images() {
    let mut fixture = complete_stg_fixture();
    let payload = fixture.offsets.variable_string_length + 4;
    fixture.bytes[payload..payload + 8]
        .copy_from_slice(&[0x81, 0xff, 0x81, 0xff, 0x81, 0xff, 0x81, 0xff]);
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let target = STGTextTarget::ParameterString {
        value: STGValueTarget::VariableInitial { variable: 2 },
    };
    assert_eq!(
        document.text(target).unwrap().raw(),
        Some(&[0x81, 0xff, 0x81, 0xff, 0x81, 0xff, 0x81, 0xff][..])
    );

    let original = changed_text_image(document.set_text(target, "동적".to_owned()).unwrap());
    assert_eq!(document.text(target).unwrap().decoded(), Some("동적"));
    for (value, source) in [
        ("a\0b".to_owned(), STGTextError::ContainsZero { index: 1 }),
        (
            "🙂".to_owned(),
            STGTextError::Unencodable {
                encoding: STGTextEncoding::CP949,
            },
        ),
    ] {
        assert_stg_text_error(document.set_text(target, value), target, &source);
        assert_eq!(document.text(target).unwrap().decoded(), Some("동적"));
    }

    let edited = changed_text_image(document.restore_text(target, original).unwrap());
    assert_eq!(
        document.text(target).unwrap().raw(),
        Some(&[0x81, 0xff, 0x81, 0xff, 0x81, 0xff, 0x81, 0xff][..])
    );
    assert!(matches!(
        document.restore_text(target, edited).unwrap(),
        STGMutation::Changed { .. }
    ));
    assert_eq!(document.text(target).unwrap().decoded(), Some("동적"));
}

#[test]
fn stg_float_access_and_mutation_preserve_every_wire_bit() {
    let targets = [
        STGFloatTarget::Unit {
            unit: 0,
            field: STGUnitFloatField::LeaderHPOverride,
        },
        STGFloatTarget::StatOverride { unit: 0, slot: 0 },
        STGFloatTarget::Area {
            area: 0,
            field: STGAreaFloatField::BoundX1,
        },
        STGFloatTarget::Parameter {
            value: STGValueTarget::VariableInitial { variable: 1 },
        },
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
    ];
    for bits in [
        0x0000_0000_u32,
        0x8000_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0001,
        0x7fc0_0002,
    ] {
        let mut fixture = complete_stg_fixture();
        for offset in [
            fixture.offsets.unit_leader_hp,
            fixture.offsets.unit_stat_override,
            fixture.offsets.area_bound_x1,
            fixture.offsets.variable_float_type + 4,
            fixture.offsets.condition_float_type + 4,
        ] {
            fixture.bytes[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
        }
        let mut document = STGDocument::parse(fixture.bytes).unwrap();
        for target in targets {
            assert_eq!(document.float(target).unwrap().to_bits(), bits);
        }
        document
            .set_text(
                STGTextTarget::Header(STGHeaderTextField::MapFilename),
                "unrelated".to_owned(),
            )
            .unwrap();
        for target in targets {
            assert_eq!(document.float(target).unwrap().to_bits(), bits);
        }
        assert_float_mutation_round_trip(&mut document, &targets, bits);
    }

    let mut fixture = complete_stg_fixture();
    fixture.bytes[fixture.offsets.variable_float_type + 4..fixture.offsets.variable_float_type + 8]
        .copy_from_slice(&0x7fc0_0001_u32.to_le_bytes());
    let mut document = STGDocument::parse(fixture.bytes).unwrap();
    let original = document.clone();
    let target = targets[3];
    assert_eq!(
        document
            .set_float(target, STGFloatValue::from_bits(0x7fc0_0002))
            .unwrap(),
        STGMutation::Changed {
            previous: STGFloatValue::from_bits(0x7fc0_0001),
        }
    );
    assert_eq!(
        document
            .set_float(target, STGFloatValue::from_bits(0x7fc0_0002))
            .unwrap(),
        STGMutation::Unchanged
    );
    assert_eq!(
        document
            .set_float(target, STGFloatValue::from_bits(0x7fc0_0001))
            .unwrap(),
        STGMutation::Changed {
            previous: STGFloatValue::from_bits(0x7fc0_0002),
        }
    );
    assert_eq!(original.float(target).unwrap().to_bits(), 0x7fc0_0001);
}

#[test]
fn stg_text_and_float_mutation_failures_are_typed_and_atomic() {
    let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let map = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    let bitmap = STGTextTarget::Header(STGHeaderTextField::BitmapFilename);
    let map_image = changed_text_image(document.set_text(map, "map".to_owned()).unwrap());

    assert_stg_text_error(
        document.restore_text(bitmap, map_image),
        bitmap,
        &STGTextError::ImageKindMismatch,
    );
    assert_eq!(document.text(map).unwrap().decoded(), Some("map"));
    assert_eq!(document.text(bitmap).unwrap().decoded(), Some(""));

    let missing_unit = STGTextTarget::UnitName { unit: 1 };
    assert_target_out_of_range(
        document.set_text(missing_unit, "missing".to_owned()),
        STGTarget::Text(missing_unit),
        STGCollection::Unit,
        1,
        1,
    );
    assert_eq!(document.text(map).unwrap().decoded(), Some("map"));

    let integer_value = STGValueTarget::VariableInitial { variable: 0 };
    let text_value = STGTextTarget::ParameterString {
        value: integer_value,
    };
    assert_value_kind_mismatch(
        document.set_text(text_value, "wrong kind".to_owned()),
        integer_value,
        STGValueKind::String,
        STGValueKind::Integer,
    );

    let float_value = STGFloatTarget::Parameter {
        value: integer_value,
    };
    assert_value_kind_mismatch(
        document.set_float(float_value, STGFloatValue::from_bits(1)),
        integer_value,
        STGValueKind::Float,
        STGValueKind::Integer,
    );

    let read_only = STGFloatTarget::Unit {
        unit: 0,
        field: STGUnitFloatField::Unknown30,
    };
    let original = document.float(read_only).unwrap();
    match document.set_float(read_only, STGFloatValue::from_bits(0x7fc0_0001)) {
        Err(FormatError::STGReadOnlyTarget { target }) => {
            assert_eq!(target, STGTarget::Float(read_only));
        }
        Err(other) => panic!("unexpected read-only STG error: {other}"),
        Ok(_) => panic!("expected read-only STG failure"),
    }
    assert_eq!(document.float(read_only).unwrap(), original);
}

#[test]
fn stg_prefix_mutations_preserve_raw_tails_and_reject_tail_targets() {
    let mut bytes = stg_prefix_fixture(1);
    let tail_start = bytes.len();
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let expected_tail = bytes[tail_start..].to_vec();
    let mut document = STGDocument::parse(bytes).unwrap();
    assert_raw_tail(&document, &expected_tail);

    let header = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    assert!(matches!(
        document.set_text(header, "map".to_owned()).unwrap(),
        STGMutation::Changed { .. }
    ));
    let unit_float = STGFloatTarget::Unit {
        unit: 0,
        field: STGUnitFloatField::PositionX,
    };
    assert!(matches!(
        document
            .set_float(unit_float, STGFloatValue::from_bits(0x3f80_0000))
            .unwrap(),
        STGMutation::Changed { .. }
    ));
    let unit_number = STGNumberTarget::Unit {
        unit: 0,
        field: STGUnitField::UniqueID,
    };
    assert!(matches!(
        document.set_number(unit_number, 42).unwrap(),
        STGMutation::Changed { .. }
    ));
    assert_raw_tail(&document, &expected_tail);

    let area_text = STGTextTarget::AreaDescription { area: 0 };
    assert_target_out_of_range(
        document.set_text(area_text, "area".to_owned()),
        STGTarget::Text(area_text),
        STGCollection::Area,
        0,
        0,
    );
    let area_float = STGFloatTarget::Area {
        area: 0,
        field: STGAreaFloatField::BoundX1,
    };
    assert_target_out_of_range(
        document.set_float(area_float, STGFloatValue::from_bits(0)),
        STGTarget::Float(area_float),
        STGCollection::Area,
        0,
        0,
    );
    let area_number = STGNumberTarget::Area {
        area: 0,
        field: STGAreaField::AreaID,
    };
    assert_target_out_of_range(
        document.set_number(area_number, 1),
        STGTarget::Number(area_number),
        STGCollection::Area,
        0,
        0,
    );
    assert_eq!(document.text(header).unwrap().decoded(), Some("map"));
    assert_eq!(document.float(unit_float).unwrap().to_bits(), 0x3f80_0000);
    assert_eq!(document.number(unit_number).unwrap(), 42);
    assert_raw_tail(&document, &expected_tail);
}

fn changed_text_image(mutation: STGMutation<STGTextImage>) -> STGTextImage {
    match mutation {
        STGMutation::Changed { previous } => previous,
        STGMutation::Unchanged => panic!("expected an STG text change"),
    }
}

fn assert_float_mutation_round_trip(
    document: &mut STGDocument,
    targets: &[STGFloatTarget],
    bits: u32,
) {
    let replacement = STGFloatValue::from_bits(bits ^ 1);
    for target in targets {
        let changed = document
            .set_float(*target, replacement)
            .unwrap_or_else(|error| panic!("STG float replacement failed: {error}"));
        assert_eq!(
            changed,
            STGMutation::Changed {
                previous: STGFloatValue::from_bits(bits),
            }
        );
        let current = document
            .float(*target)
            .unwrap_or_else(|error| panic!("STG float projection failed: {error}"));
        assert_eq!(current, replacement);
        let restored = document
            .set_float(*target, STGFloatValue::from_bits(bits))
            .unwrap_or_else(|error| panic!("STG float restore failed: {error}"));
        assert_eq!(
            restored,
            STGMutation::Changed {
                previous: replacement,
            }
        );
    }
}

fn assert_stg_text_error(
    result: Result<STGMutation<STGTextImage>, FormatError>,
    target: STGTextTarget,
    expected: &STGTextError,
) {
    match result {
        Err(FormatError::STGText {
            target: actual_target,
            source,
        }) => {
            assert_eq!(actual_target, target);
            assert_eq!(&source, expected);
        }
        Err(other) => panic!("unexpected STG text error: {other}"),
        Ok(_) => panic!("expected STG text failure"),
    }
}

fn assert_target_out_of_range<T>(
    result: Result<T, FormatError>,
    expected_target: STGTarget,
    expected_collection: STGCollection,
    expected_index: usize,
    expected_count: usize,
) {
    match result {
        Err(FormatError::STGTargetOutOfRange {
            target,
            collection,
            index,
            count,
        }) => {
            assert_eq!(target, expected_target);
            assert_eq!(collection, expected_collection);
            assert_eq!(index, expected_index);
            assert_eq!(count, expected_count);
        }
        Err(other) => panic!("unexpected STG target error: {other}"),
        Ok(_) => panic!("expected STG target failure"),
    }
}

fn assert_value_kind_mismatch<T>(
    result: Result<T, FormatError>,
    expected_target: STGValueTarget,
    expected_kind: STGValueKind,
    expected_actual: STGValueKind,
) {
    match result {
        Err(FormatError::STGValueKindMismatch {
            target,
            expected,
            actual,
        }) => {
            assert_eq!(target, expected_target);
            assert_eq!(expected, expected_kind);
            assert_eq!(actual, expected_actual);
        }
        Err(other) => panic!("unexpected STG value-kind error: {other}"),
        Ok(_) => panic!("expected STG value-kind failure"),
    }
}

fn editable_number_targets() -> Vec<STGNumberTarget> {
    let mut targets = Vec::new();
    for field in STGUnitField::ALL {
        let target = STGNumberTarget::Unit { unit: 0, field };
        if target.access() == STGFieldAccess::Editable {
            targets.push(target);
        }
    }
    for owner in STGSkillOwner::ALL {
        for slot in 0..4 {
            for field in STGSkillField::ALL {
                targets.push(STGNumberTarget::Skill {
                    unit: 0,
                    owner,
                    slot,
                    field,
                });
            }
        }
    }
    for owner in STGAbilityOwner::ALL {
        let count = match owner {
            STGAbilityOwner::Leader | STGAbilityOwner::Officer1 => 23,
            STGAbilityOwner::Officer2 => 19,
        };
        for slot in 0..count {
            targets.push(STGNumberTarget::Ability {
                unit: 0,
                owner,
                slot,
            });
        }
    }
    targets.push(STGNumberTarget::Area {
        area: 0,
        field: STGAreaField::AreaID,
    });
    for variable in 0..4 {
        targets.push(STGNumberTarget::VariableID { variable });
    }
    for variable in [0, 3] {
        targets.push(STGNumberTarget::ParameterInteger {
            value: STGValueTarget::VariableInitial { variable },
        });
    }
    for block in 0..2 {
        targets.push(STGNumberTarget::EventBlockHeader { block });
    }
    for event in 0..2 {
        targets.push(STGNumberTarget::EventID { block: 0, event });
    }
    targets.push(STGNumberTarget::ParameterInteger {
        value: STGValueTarget::ScriptParameter(STGParameterTarget {
            script: STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Condition,
                script: 0,
            },
            parameter: 0,
        }),
    });
    targets.push(STGNumberTarget::ParameterInteger {
        value: STGValueTarget::ScriptParameter(STGParameterTarget {
            script: STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Action,
                script: 0,
            },
            parameter: 1,
        }),
    });
    for entry in 0..2 {
        for field in STGFooterField::ALL {
            targets.push(STGNumberTarget::Footer { entry, field });
        }
    }
    targets
}

fn editable_float_targets() -> Vec<STGFloatTarget> {
    let mut targets = Vec::new();
    for field in STGUnitFloatField::ALL {
        let target = STGFloatTarget::Unit { unit: 0, field };
        if target.access() == STGFieldAccess::Editable {
            targets.push(target);
        }
    }
    for slot in 0..22 {
        targets.push(STGFloatTarget::StatOverride { unit: 0, slot });
    }
    for field in STGAreaFloatField::ALL {
        targets.push(STGFloatTarget::Area { area: 0, field });
    }
    targets.push(STGFloatTarget::Parameter {
        value: STGValueTarget::VariableInitial { variable: 1 },
    });
    targets.push(STGFloatTarget::Parameter {
        value: STGValueTarget::ScriptParameter(STGParameterTarget {
            script: STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Condition,
                script: 0,
            },
            parameter: 1,
        }),
    });
    targets
}

fn number_snapshot(document: &STGDocument, targets: &[STGNumberTarget]) -> Vec<i64> {
    targets
        .iter()
        .map(|target| {
            document
                .number(*target)
                .unwrap_or_else(|error| panic!("failed to read {target:?}: {error}"))
        })
        .collect()
}

fn float_snapshot(document: &STGDocument, targets: &[STGFloatTarget]) -> Vec<STGFloatValue> {
    targets
        .iter()
        .map(|target| {
            document
                .float(*target)
                .unwrap_or_else(|error| panic!("failed to read {target:?}: {error}"))
        })
        .collect()
}

fn replacement_number(target: STGNumberTarget, previous: i64) -> i64 {
    let (minimum, maximum) = target.storage_bounds();
    if previous == maximum {
        minimum
    } else {
        maximum
    }
}

fn assert_stg_observed_state(
    document: &STGDocument,
    number_targets: &[STGNumberTarget],
    expected_numbers: &[i64],
    float_targets: &[STGFloatTarget],
    expected_floats: &[STGFloatValue],
) {
    assert_eq!(number_snapshot(document, number_targets), expected_numbers);
    assert_eq!(float_snapshot(document, float_targets), expected_floats);
}

fn assert_number_out_of_range(
    result: Result<STGMutation<i64>, FormatError>,
    expected_target: STGNumberTarget,
    expected_value: i64,
) {
    let (expected_minimum, expected_maximum) = expected_target.storage_bounds();
    match result {
        Err(FormatError::STGNumberOutOfRange {
            target,
            value,
            minimum,
            maximum,
        }) => {
            assert_eq!(target, expected_target);
            assert_eq!(value, expected_value);
            assert_eq!(minimum, expected_minimum);
            assert_eq!(maximum, expected_maximum);
        }
        Err(other) => panic!("unexpected STG number error: {other}"),
        Ok(_) => panic!("expected STG number range failure"),
    }
}

const fn stg_diagnostic(
    severity: Severity,
    location: DiagnosticLocation,
    message: &'static str,
) -> Diagnostic {
    Diagnostic {
        severity,
        location,
        message,
    }
}

fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn assert_raw_tail(document: &STGDocument, expected: &[u8]) {
    match document.tail_status() {
        STGTailStatus::Raw { bytes, .. } => assert_eq!(bytes, expected),
        STGTailStatus::Parsed { .. } => panic!("expected an opaque STG tail"),
    }
}

fn cp949(value: &str) -> Vec<u8> {
    let (bytes, _, had_errors) = EUC_KR.encode(value);
    assert!(!had_errors, "test text must be representable in CP949");
    bytes.into_owned()
}

fn write_fixed_text(bytes: &mut [u8], offset: usize, width: usize, value: &[u8]) {
    assert!(value.len() < width);
    write_fixed_bytes(bytes, offset, width, value);
}

fn write_fixed_bytes(bytes: &mut [u8], offset: usize, width: usize, value: &[u8]) {
    bytes[offset..offset + width].fill(0);
    bytes[offset..offset + value.len()].copy_from_slice(value);
}
