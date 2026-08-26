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
    STGEventTarget, STGFieldAccess, STGFloatTarget, STGFloatValue, STGFooterField,
    STGHeaderTextField, STGMutation, STGNumberTarget, STGParameterTarget, STGParseError,
    STGPreflightError, STGRebaseError, STGReferenceKind, STGRegion, STGScriptKind, STGScriptTarget,
    STGSkillField, STGSkillOwner, STGStructuralEdit, STGStructuralLocation, STGTailFailure,
    STGTailStatus, STGTarget, STGText, STGTextEncoding, STGTextError, STGTextImage, STGTextTarget,
    STGUnitField, STGUnitFloatField, STGUnitGroup, STGValue, STGValueKind, STGValueTarget,
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

    let tail_start = bytes.len();
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
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGTail {
                    region: STGRegion::Areas,
                    offset: tail_start,
                },
                "STG tail is preserved as raw bytes"
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

    let tail_start = bytes.len();
    let document = STGDocument::parse(bytes).unwrap();
    assert_eq!(
        document.diagnostics(),
        [
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGNumber(STGNumberTarget::Unit {
                    unit: 1,
                    field: STGUnitField::LeaderWorldmapID,
                }),
                "Worldmap ID may cause post-mission issues",
            ),
            stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGTail {
                    region: STGRegion::Areas,
                    offset: tail_start,
                },
                "STG tail is preserved as raw bytes",
            ),
        ],
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

#[test]
fn stg_events_project_blocks_scripts_parameters_and_catalog_metadata() {
    let fixture = complete_stg_fixture();
    let document = STGDocument::parse(fixture.bytes).unwrap();

    let first_block = document.event_block(0).unwrap();
    assert_eq!(first_block.header, 0x0102_0304);
    assert_eq!(first_block.event_count, 2);
    let empty_block = document.event_block(1).unwrap();
    assert_eq!(empty_block.header, 0x0506_0708);
    assert_eq!(empty_block.event_count, 0);

    let event_target = STGEventTarget { block: 0, event: 0 };
    let event = document.event(event_target).unwrap();
    assert_eq!(event.target, event_target);
    assert_eq!(event.id, 500);
    assert_eq!(event.description.decoded(), Some("Primary Event"));
    assert_eq!(event.condition_count, 1);
    assert_eq!(event.action_count, 1);

    let condition_target = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let condition = document.script(condition_target).unwrap();
    assert_eq!(condition.target, condition_target);
    assert_eq!(condition.id, 19);
    assert_eq!(condition.name, Some("CON_VAR"));
    assert_eq!(condition.parameter_count, 2);
    assert_eq!(condition.expected_parameter_count, Some(3));
    assert_eq!(condition.label().to_string(), "CON_VAR");

    let integer_target = STGParameterTarget {
        script: condition_target,
        parameter: 0,
    };
    assert_eq!(
        document.parameter(integer_target).unwrap(),
        kufeditor_formats::STGParameter {
            target: integer_target,
            hint: Some("VariableID"),
            reference: Some(STGReferenceKind::Variable),
            value: STGValue::Integer(23),
        }
    );
    let float_target = STGParameterTarget {
        script: condition_target,
        parameter: 1,
    };
    assert_eq!(
        document.parameter(float_target).unwrap().value,
        STGValue::Float(STGFloatValue::from_bits((-0.0_f32).to_bits()))
    );

    let action_target = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Action,
        script: 0,
    };
    let string_target = STGParameterTarget {
        script: action_target,
        parameter: 0,
    };
    assert_eq!(
        document.parameter(string_target).unwrap().value,
        STGValue::String(STGText::Decoded("action".into()))
    );
    let enum_target = STGParameterTarget {
        script: action_target,
        parameter: 1,
    };
    assert_eq!(
        document.parameter(enum_target).unwrap().value,
        STGValue::Enum(-3)
    );

    let mut unknown_fixture = complete_stg_fixture();
    let condition_type = unknown_fixture.offsets.condition_parameter_count - 4;
    unknown_fixture.bytes[condition_type..condition_type + 4]
        .copy_from_slice(&9_999_u32.to_le_bytes());
    let unknown = STGDocument::parse(unknown_fixture.bytes).unwrap();
    let script = unknown.script(condition_target).unwrap();
    assert_eq!(script.id, 9_999);
    assert_eq!(script.name, None);
    assert_eq!(script.expected_parameter_count, None);
    assert_eq!(script.label().to_string(), "Unknown condition 9999");
    assert_eq!(
        unknown.parameter(integer_target).unwrap().hint,
        None,
        "unknown scripts must not borrow hints from another catalog entry"
    );
}

#[test]
fn stg_structure_inserts_removes_and_restores_exact_subtrees() {
    let mut empty = STGDocument::parse(empty_stg_fixture()).unwrap();
    let undo_insert = changed_structure(empty.insert_event(0, 0).unwrap());
    let expected_undo_insert = undo_insert.clone();
    assert_eq!(empty.event_block_count(), Some(1));
    assert_eq!(empty.event_block(0).unwrap().header, 0);
    let inserted = empty.event(STGEventTarget { block: 0, event: 0 }).unwrap();
    assert_eq!(inserted.id, 0);
    assert_eq!(inserted.description.decoded(), Some("New Event"));
    assert_eq!((inserted.condition_count, inserted.action_count), (0, 0));

    let redo_insert = changed_structure(empty.restore_structure(undo_insert).unwrap());
    assert_eq!(empty.event_block_count(), Some(0));
    let undo_again = changed_structure(empty.restore_structure(redo_insert).unwrap());
    assert_eq!(undo_again, expected_undo_insert);
    assert_eq!(empty.event_block_count(), Some(1));
    changed_structure(empty.restore_structure(undo_again).unwrap());
    assert_eq!(empty.event_block_count(), Some(0));

    let mut direct_remove = STGDocument::parse(empty_stg_fixture()).unwrap();
    changed_structure(direct_remove.insert_event(0, 0).unwrap());
    let restore_removed = changed_structure(direct_remove.remove_event(0, 0).unwrap());
    assert_eq!(direct_remove.event_block_count(), Some(1));
    assert_eq!(direct_remove.event_block(0).unwrap().event_count, 0);
    changed_structure(direct_remove.restore_structure(restore_removed).unwrap());
    assert_eq!(direct_remove.event_block(0).unwrap().event_count, 1);

    let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let block_one_before = document.event_block(1).unwrap();
    let undo_event = changed_structure(document.insert_event(0, 1).unwrap());
    assert_eq!(document.event_block(0).unwrap().event_count, 3);
    assert_eq!(
        document
            .event(STGEventTarget { block: 0, event: 1 })
            .unwrap()
            .id,
        0
    );
    assert_eq!(document.event_block(1).unwrap(), block_one_before);
    let redo_event = changed_structure(document.restore_structure(undo_event).unwrap());
    assert_eq!(document.event_block(0).unwrap().event_count, 2);
    assert_eq!(document.event_block(1).unwrap(), block_one_before);
    changed_structure(document.restore_structure(redo_event).unwrap());

    let condition_target = STGScriptTarget {
        block: 0,
        event: 1,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let undo_condition = changed_structure(document.insert_script(condition_target, 27).unwrap());
    let condition = document.script(condition_target).unwrap();
    assert_eq!(condition.name, Some("CON_ALWAYS_TRUE"));
    assert_eq!(condition.parameter_count, 0);
    let redo_condition = changed_structure(document.restore_structure(undo_condition).unwrap());
    assert_eq!(
        document
            .event(STGEventTarget { block: 0, event: 1 })
            .unwrap()
            .condition_count,
        0
    );
    changed_structure(document.restore_structure(redo_condition).unwrap());

    let action_target = STGScriptTarget {
        kind: STGScriptKind::Action,
        ..condition_target
    };
    let undo_action = changed_structure(document.insert_script(action_target, 7).unwrap());
    assert_eq!(document.script(action_target).unwrap().parameter_count, 3);
    for parameter in 0..3 {
        assert_eq!(
            document
                .parameter(STGParameterTarget {
                    script: action_target,
                    parameter,
                })
                .unwrap()
                .value,
            STGValue::Integer(0)
        );
    }
    let redo_action = changed_structure(document.restore_structure(undo_action).unwrap());
    assert_eq!(document.event_block(1).unwrap(), block_one_before);
    changed_structure(document.restore_structure(redo_action).unwrap());

    let removed_action = changed_structure(document.remove_script(action_target).unwrap());
    assert_eq!(
        document
            .event(STGEventTarget { block: 0, event: 1 })
            .unwrap()
            .action_count,
        0
    );
    changed_structure(document.restore_structure(removed_action).unwrap());
    assert_eq!(document.script(action_target).unwrap().parameter_count, 3);
    assert_eq!(document.event_block(1).unwrap(), block_one_before);
}

#[test]
fn stg_structure_changes_script_and_value_types_with_exact_undo() {
    let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let condition = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };

    let repair = changed_structure(document.change_script_type(condition, 19).unwrap());
    assert_eq!(document.script(condition).unwrap().parameter_count, 3);
    assert_eq!(
        document
            .parameter(STGParameterTarget {
                script: condition,
                parameter: 0,
            })
            .unwrap()
            .value,
        STGValue::Integer(23)
    );
    assert_eq!(
        document
            .parameter(STGParameterTarget {
                script: condition,
                parameter: 1,
            })
            .unwrap()
            .value,
        STGValue::Float(STGFloatValue::from_bits((-0.0_f32).to_bits()))
    );
    assert_eq!(
        document
            .parameter(STGParameterTarget {
                script: condition,
                parameter: 2,
            })
            .unwrap()
            .value,
        STGValue::Integer(0)
    );
    assert_eq!(
        document.change_script_type(condition, 19).unwrap(),
        STGMutation::Unchanged
    );

    let redo_repair = changed_structure(document.restore_structure(repair).unwrap());
    assert_eq!(document.script(condition).unwrap().parameter_count, 2);
    changed_structure(document.restore_structure(redo_repair).unwrap());
    assert_eq!(document.script(condition).unwrap().parameter_count, 3);

    for (type_id, expected_count) in [(27, 0), (8, 1), (0, 2), (1, 3)] {
        let mut candidate = document.clone();
        changed_structure(candidate.change_script_type(condition, type_id).unwrap());
        assert_eq!(
            candidate.script(condition).unwrap().parameter_count,
            expected_count
        );
    }

    let float_value = STGValueTarget::ScriptParameter(STGParameterTarget {
        script: condition,
        parameter: 1,
    });
    let float_bits = (-0.0_f32).to_bits();
    let undo_value = changed_structure(
        document
            .change_value_type(float_value, STGValueKind::String)
            .unwrap(),
    );
    assert_eq!(
        document.value(float_value).unwrap(),
        STGValue::String(STGText::Decoded("".into()))
    );
    let redo_value = changed_structure(document.restore_structure(undo_value).unwrap());
    assert_eq!(
        document.value(float_value).unwrap(),
        STGValue::Float(STGFloatValue::from_bits(float_bits))
    );
    changed_structure(document.restore_structure(redo_value).unwrap());
    assert_eq!(
        document.value(float_value).unwrap(),
        STGValue::String(STGText::Decoded("".into()))
    );

    match document.change_script_type(condition, 9_999) {
        Err(FormatError::STGUnknownScriptType {
            kind: STGScriptKind::Condition,
            id: 9_999,
        }) => {}
        Err(other) => panic!("unexpected unknown-script error: {other}"),
        Ok(_) => panic!("expected an unknown-script rejection"),
    }
}

#[test]
fn stg_structure_removes_first_middle_and_last_events_with_exact_restore() {
    for removed_index in 0..3 {
        let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
        changed_structure(document.insert_event(0, 2).unwrap());
        let before = event_ids(&document, 0);
        assert_eq!(before, vec![500, 501, 0]);

        let undo = changed_structure(document.remove_event(0, removed_index).unwrap());
        let mut expected = before.clone();
        expected.remove(removed_index);
        assert_eq!(event_ids(&document, 0), expected);

        let redo = changed_structure(document.restore_structure(undo).unwrap());
        assert_eq!(event_ids(&document, 0), before);
        changed_structure(document.restore_structure(redo).unwrap());
        assert_eq!(event_ids(&document, 0), expected);
    }
}

#[test]
fn stg_structure_repairs_both_script_kinds_and_unknown_types() {
    let condition = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let action = STGScriptTarget {
        kind: STGScriptKind::Action,
        ..condition
    };

    for (target, shapes) in [
        (condition, [(27, 0), (8, 1), (0, 2), (1, 3)]),
        (action, [(22, 0), (8, 1), (10, 2), (7, 3)]),
    ] {
        for (type_id, expected_count) in shapes {
            let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
            let undo = changed_structure(document.change_script_type(target, type_id).unwrap());
            assert_eq!(
                document.script(target).unwrap().parameter_count,
                expected_count
            );
            changed_structure(document.restore_structure(undo).unwrap());
        }
    }

    let fixture = complete_stg_fixture();
    let mut longer = fixture.bytes;
    longer
        [fixture.offsets.condition_parameter_count..fixture.offsets.condition_parameter_count + 4]
        .copy_from_slice(&4_u32.to_le_bytes());
    let mut extra = Vec::new();
    extra.extend_from_slice(&0_u32.to_le_bytes());
    extra.extend_from_slice(&77_i32.to_le_bytes());
    extra.extend_from_slice(&0_u32.to_le_bytes());
    extra.extend_from_slice(&88_i32.to_le_bytes());
    longer.splice(
        fixture.offsets.action_count..fixture.offsets.action_count,
        extra,
    );
    let mut document = STGDocument::parse(longer).unwrap();
    let undo = changed_structure(document.change_script_type(condition, 19).unwrap());
    assert_eq!(document.script(condition).unwrap().parameter_count, 3);
    assert_eq!(
        document
            .parameter(STGParameterTarget {
                script: condition,
                parameter: 2,
            })
            .unwrap()
            .value,
        STGValue::Integer(77)
    );
    changed_structure(document.restore_structure(undo).unwrap());
    assert_eq!(document.script(condition).unwrap().parameter_count, 4);

    let mut unknown_fixture = complete_stg_fixture();
    let type_offset = unknown_fixture.offsets.condition_parameter_count - 4;
    unknown_fixture.bytes[type_offset..type_offset + 4].copy_from_slice(&9_999_u32.to_le_bytes());
    let mut unknown = STGDocument::parse(unknown_fixture.bytes).unwrap();
    let undo = changed_structure(unknown.change_script_type(condition, 2).unwrap());
    assert_eq!(
        unknown.script(condition).unwrap().name,
        Some("CON_TROOP_IN_AREA")
    );
    assert_eq!(unknown.script(condition).unwrap().parameter_count, 2);
    changed_structure(unknown.restore_structure(undo).unwrap());
    assert_eq!(unknown.script(condition).unwrap().id, 9_999);
}

#[test]
fn stg_structure_changes_every_value_kind_for_variables_and_scripts() {
    let condition = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let action = STGScriptTarget {
        kind: STGScriptKind::Action,
        ..condition
    };
    let sources = [
        (
            STGValueTarget::VariableInitial { variable: 0 },
            STGValueKind::Integer,
            ExpectedSTGValue::Integer(-12),
        ),
        (
            STGValueTarget::VariableInitial { variable: 1 },
            STGValueKind::Float,
            ExpectedSTGValue::Float(17.25_f32.to_bits()),
        ),
        (
            STGValueTarget::VariableInitial { variable: 2 },
            STGValueKind::String,
            ExpectedSTGValue::String("variable"),
        ),
        (
            STGValueTarget::VariableInitial { variable: 3 },
            STGValueKind::Enum,
            ExpectedSTGValue::Enum(7),
        ),
        (
            STGValueTarget::ScriptParameter(STGParameterTarget {
                script: condition,
                parameter: 0,
            }),
            STGValueKind::Integer,
            ExpectedSTGValue::Integer(23),
        ),
        (
            STGValueTarget::ScriptParameter(STGParameterTarget {
                script: condition,
                parameter: 1,
            }),
            STGValueKind::Float,
            ExpectedSTGValue::Float((-0.0_f32).to_bits()),
        ),
        (
            STGValueTarget::ScriptParameter(STGParameterTarget {
                script: action,
                parameter: 0,
            }),
            STGValueKind::String,
            ExpectedSTGValue::String("action"),
        ),
        (
            STGValueTarget::ScriptParameter(STGParameterTarget {
                script: action,
                parameter: 1,
            }),
            STGValueKind::Enum,
            ExpectedSTGValue::Enum(-3),
        ),
    ];

    for (target, original_kind, original) in sources {
        for replacement_kind in [
            STGValueKind::Integer,
            STGValueKind::Float,
            STGValueKind::String,
            STGValueKind::Enum,
        ] {
            if replacement_kind == original_kind {
                continue;
            }
            let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
            let undo = changed_structure(
                document
                    .change_value_type(target, replacement_kind)
                    .unwrap(),
            );
            assert_default_stg_value(&document.value(target).unwrap(), replacement_kind);
            changed_structure(document.restore_structure(undo).unwrap());
            original.assert_eq(&document.value(target).unwrap());
        }
    }
}

#[test]
fn stg_structure_supports_multi_entry_and_scalar_interleaved_undo() {
    let mut document = STGDocument::parse(empty_stg_fixture()).unwrap();
    let undo_event = changed_structure(document.insert_event(0, 0).unwrap());
    let script = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let undo_script = changed_structure(document.insert_script(script, 27).unwrap());
    let redo_script = changed_structure(document.restore_structure(undo_script).unwrap());
    let redo_event = changed_structure(document.restore_structure(undo_event).unwrap());
    assert_eq!(document.event_block_count(), Some(0));
    changed_structure(document.restore_structure(redo_event).unwrap());
    changed_structure(document.restore_structure(redo_script).unwrap());
    assert_eq!(document.script(script).unwrap().id, 27);

    let mut scalar_interleaved = STGDocument::parse(empty_stg_fixture()).unwrap();
    let undo_event = changed_structure(scalar_interleaved.insert_event(0, 0).unwrap());
    let id = STGNumberTarget::EventID { block: 0, event: 0 };
    let previous = match scalar_interleaved.set_number(id, 42).unwrap() {
        STGMutation::Changed { previous } => previous,
        STGMutation::Unchanged => panic!("expected scalar change"),
    };
    scalar_interleaved.set_number(id, previous).unwrap();
    changed_structure(scalar_interleaved.restore_structure(undo_event).unwrap());
    assert_eq!(scalar_interleaved.event_block_count(), Some(0));
}

#[test]
fn stg_structure_previews_history_charge_before_allocating_an_inverse() {
    let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let script = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Action,
        script: 0,
    };
    let edit = STGStructuralEdit::RemoveScript { target: script };
    let preview = document.preview_structure(edit).unwrap();
    assert!(preview.is_changed());
    assert_eq!(preview.edit(), edit);
    assert!(preview.retained_bytes() > 0);

    let mut prospective = document.clone();
    let inverse = changed_structure(
        prospective
            .apply_structure_preview(preview.clone())
            .unwrap(),
    );
    assert_eq!(inverse.retained_bytes(), preview.retained_bytes());
    assert_eq!(document.script(script).unwrap().id, 55);

    let string = STGTextTarget::ParameterString {
        value: STGValueTarget::ScriptParameter(STGParameterTarget {
            script,
            parameter: 0,
        }),
    };
    document.set_text(string, "changed".to_owned()).unwrap();
    assert_structural_state_mismatch(
        document.apply_structure_preview(preview),
        STGStructuralLocation::Script(script),
    );
    assert_eq!(document.text(string).unwrap().decoded(), Some("changed"));

    let neutral = document
        .preview_structure(STGStructuralEdit::ChangeScriptType {
            target: script,
            type_id: 55,
        })
        .unwrap();
    assert!(!neutral.is_changed());
    assert_eq!(neutral.retained_bytes(), 0);
    assert_eq!(
        document.apply_structure_preview(neutral).unwrap(),
        STGMutation::Unchanged
    );

    let foreign = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let foreign_preview = foreign.preview_structure(edit).unwrap();
    assert!(matches!(
        document.apply_structure_preview(foreign_preview),
        Err(FormatError::STGStructuralLineageMismatch)
    ));
}

#[test]
fn stg_structure_rejects_stale_images_and_raw_tails_atomically() {
    let mut document = STGDocument::parse(empty_stg_fixture()).unwrap();
    let inverse = changed_structure(document.insert_event(0, 0).unwrap());
    let id_target = STGNumberTarget::EventID { block: 0, event: 0 };
    document.set_number(id_target, 42).unwrap();
    match document.restore_structure(inverse) {
        Err(FormatError::STGStructuralStateMismatch {
            location: STGStructuralLocation::Event { block: 0, event: 0 },
        }) => {}
        Err(other) => panic!("unexpected stale-image error: {other}"),
        Ok(_) => panic!("expected a stale-image rejection"),
    }
    assert_eq!(document.number(id_target).unwrap(), 42);
    assert_eq!(document.event_block(0).unwrap().event_count, 1);

    let mut raw_bytes = stg_prefix_fixture(0);
    raw_bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let mut raw = STGDocument::parse(raw_bytes).unwrap();
    match raw.insert_event(0, 0) {
        Err(FormatError::STGStructureUnavailable {
            location: STGStructuralLocation::Event { block: 0, event: 0 },
        }) => {}
        Err(other) => panic!("unexpected raw-tail structure error: {other}"),
        Ok(_) => panic!("expected raw-tail structural rejection"),
    }
}

#[test]
fn stg_structure_rejects_foreign_and_exact_subtree_mismatches() {
    let mut first = STGDocument::parse(empty_stg_fixture()).unwrap();
    let foreign = changed_structure(first.insert_event(0, 0).unwrap());
    let mut second = STGDocument::parse(empty_stg_fixture()).unwrap();
    changed_structure(second.insert_event(0, 0).unwrap());
    assert!(matches!(
        second.restore_structure(foreign),
        Err(FormatError::STGStructuralLineageMismatch)
    ));
    assert_eq!(second.event_block(0).unwrap().event_count, 1);

    let mut script_document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let script = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let script_inverse = changed_structure(script_document.change_script_type(script, 19).unwrap());
    let added_parameter = STGValueTarget::ScriptParameter(STGParameterTarget {
        script,
        parameter: 2,
    });
    script_document
        .set_number(
            STGNumberTarget::ParameterInteger {
                value: added_parameter,
            },
            91,
        )
        .unwrap();
    assert_structural_state_mismatch(
        script_document.restore_structure(script_inverse),
        STGStructuralLocation::Script(script),
    );
    assert_eq!(
        script_document
            .number(STGNumberTarget::ParameterInteger {
                value: added_parameter,
            })
            .unwrap(),
        91
    );

    let mut value_document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    let value = STGValueTarget::VariableInitial { variable: 0 };
    let value_inverse = changed_structure(
        value_document
            .change_value_type(value, STGValueKind::Float)
            .unwrap(),
    );
    let float_target = STGFloatTarget::Parameter { value };
    let replacement = STGFloatValue::from_bits(0x7fc0_1234);
    value_document.set_float(float_target, replacement).unwrap();
    assert_structural_state_mismatch(
        value_document.restore_structure(value_inverse),
        STGStructuralLocation::Value(value),
    );
    assert_eq!(value_document.float(float_target).unwrap(), replacement);
}

#[test]
fn stg_structure_rejects_every_command_for_an_opaque_tail() {
    let mut bytes = stg_prefix_fixture(0);
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let mut document = STGDocument::parse(bytes).unwrap();
    let event = STGEventTarget { block: 0, event: 0 };
    let script = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let value = STGValueTarget::VariableInitial { variable: 0 };

    let event_location = STGStructuralLocation::Event {
        block: event.block,
        event: event.event,
    };
    assert_structure_unavailable(document.insert_event(0, 0), event_location);
    assert_structure_unavailable(document.remove_event(0, 0), event_location);
    assert_structure_unavailable(
        document.insert_script(script, 9_999),
        STGStructuralLocation::Script(script),
    );
    assert_structure_unavailable(
        document.remove_script(script),
        STGStructuralLocation::Script(script),
    );
    assert_structure_unavailable(
        document.change_script_type(script, 9_999),
        STGStructuralLocation::Script(script),
    );
    assert_structure_unavailable(
        document.change_value_type(value, STGValueKind::String),
        STGStructuralLocation::Value(value),
    );
    assert!(matches!(document.tail_status(), STGTailStatus::Raw { .. }));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the expected diagnostic matrix is easier to audit beside its synthetic corruptions"
)]
fn stg_references_report_duplicate_shapes_text_and_missing_ids() {
    let fixture = complete_stg_fixture();
    let offsets = fixture.offsets;
    let mut bytes = fixture.bytes;

    bytes[offsets.variable_float_type - 4..offsets.variable_float_type]
        .copy_from_slice(&100_u32.to_le_bytes());
    let second_event = offsets.action_enum_type + 8;
    bytes[second_event + 64..second_event + 68].copy_from_slice(&500_u32.to_le_bytes());
    let action_type = offsets.action_parameter_count - 4;
    bytes[action_type..action_type + 4].copy_from_slice(&9_999_u32.to_le_bytes());
    bytes[offsets.action_string_type + 8] = 0xff;

    let duplicate_area = bytes[offsets.area_description..offsets.area_description + 84].to_vec();
    bytes.splice(
        offsets.area_description + 84..offsets.area_description + 84,
        duplicate_area,
    );
    bytes[offsets.area_count..offsets.area_count + 4].copy_from_slice(&2_u32.to_le_bytes());

    let mut document = STGDocument::parse(bytes).unwrap();
    let missing_refs = STGScriptTarget {
        block: 0,
        event: 1,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    changed_structure(document.insert_script(missing_refs, 2).unwrap());
    for parameter in 0..2 {
        document
            .set_number(
                STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::ScriptParameter(STGParameterTarget {
                        script: missing_refs,
                        parameter,
                    }),
                },
                999,
            )
            .unwrap();
    }
    let missing_trigger = STGScriptTarget {
        kind: STGScriptKind::Action,
        script: 0,
        ..missing_refs
    };
    changed_structure(document.insert_script(missing_trigger, 0).unwrap());
    document
        .set_number(
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: missing_trigger,
                    parameter: 0,
                }),
            },
            999,
        )
        .unwrap();

    let diagnostics = document.diagnostics();
    for expected in [
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGNumber(STGNumberTarget::Area {
                area: 0,
                field: STGAreaField::AreaID,
            }),
            message: "Duplicate area ID",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGNumber(STGNumberTarget::VariableID { variable: 0 }),
            message: "Duplicate variable ID",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGNumber(STGNumberTarget::EventID {
                block: 0,
                event: 0,
            }),
            message: "Duplicate event ID",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGScript(STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Condition,
                script: 0,
            }),
            message: "Condition parameter count differs from catalog",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGScript(STGScriptTarget {
                block: 0,
                event: 0,
                kind: STGScriptKind::Action,
                script: 0,
            }),
            message: "Unknown action type",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGText(STGTextTarget::ParameterString {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Action,
                        script: 0,
                    },
                    parameter: 0,
                }),
            }),
            message: "String parameter is not valid CP949",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGNumber(STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: missing_refs,
                    parameter: 0,
                }),
            }),
            message: "Missing troop reference",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGNumber(STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: missing_refs,
                    parameter: 1,
                }),
            }),
            message: "Missing area reference",
        },
        Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGNumber(STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: missing_trigger,
                    parameter: 0,
                }),
            }),
            message: "Missing trigger reference",
        },
    ] {
        assert!(
            diagnostics.contains(&expected),
            "missing diagnostic {expected:?} in {diagnostics:#?}"
        );
    }
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message == "Missing variable reference"
            && diagnostic.location
                == DiagnosticLocation::STGNumber(STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::ScriptParameter(STGParameterTarget {
                        script: STGScriptTarget {
                            block: 0,
                            event: 0,
                            kind: STGScriptKind::Condition,
                            script: 0,
                        },
                        parameter: 0,
                    }),
                })
    }));
}

#[test]
fn stg_references_compare_signed_payloads_as_exact_unsigned_id_bits() {
    let mut document = STGDocument::parse(complete_stg_fixture().bytes).unwrap();
    document
        .set_number(
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::UniqueID,
            },
            i64::from(u32::MAX),
        )
        .unwrap();
    document
        .set_number(
            STGNumberTarget::Area {
                area: 0,
                field: STGAreaField::AreaID,
            },
            i64::from(u32::MAX),
        )
        .unwrap();
    document
        .set_number(
            STGNumberTarget::EventID { block: 0, event: 0 },
            i64::from(u32::MAX),
        )
        .unwrap();

    let condition = STGScriptTarget {
        block: 0,
        event: 1,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    changed_structure(document.insert_script(condition, 2).unwrap());
    for parameter in 0..2 {
        document
            .set_number(
                STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::ScriptParameter(STGParameterTarget {
                        script: condition,
                        parameter,
                    }),
                },
                -1,
            )
            .unwrap();
    }
    let trigger = STGScriptTarget {
        kind: STGScriptKind::Action,
        script: 0,
        ..condition
    };
    changed_structure(document.insert_script(trigger, 0).unwrap());
    document
        .set_number(
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(STGParameterTarget {
                    script: trigger,
                    parameter: 0,
                }),
            },
            -1,
        )
        .unwrap();

    let locations = [
        STGParameterTarget {
            script: condition,
            parameter: 0,
        },
        STGParameterTarget {
            script: condition,
            parameter: 1,
        },
        STGParameterTarget {
            script: trigger,
            parameter: 0,
        },
    ];
    let diagnostics = document.diagnostics();
    assert!(diagnostics.iter().all(|diagnostic| {
        !locations.iter().any(|target| {
            diagnostic.location
                == DiagnosticLocation::STGNumber(STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::ScriptParameter(*target),
                })
                && diagnostic.message.starts_with("Missing ")
        })
    }));
}

#[test]
fn stg_references_warn_when_the_tail_is_preserved_raw() {
    let mut bytes = stg_prefix_fixture(0);
    let tail_start = bytes.len();
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let document = STGDocument::parse(bytes).unwrap();
    assert_eq!(
        document.diagnostics(),
        vec![Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGTail {
                region: STGRegion::Areas,
                offset: tail_start,
            },
            message: "STG tail is preserved as raw bytes",
        }]
    );
}

#[test]
fn stg_references_bound_the_diagnostic_result() {
    let bytes = stg_prefix_fixture(2_500);
    let tail_start = bytes.len();
    let document = STGDocument::parse(bytes).unwrap();
    let diagnostics = document.diagnostics();
    assert_eq!(diagnostics.len(), 4_096);
    assert_eq!(
        diagnostics.get(diagnostics.len() - 2),
        Some(&Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGTail {
                region: STGRegion::Areas,
                offset: tail_start,
            },
            message: "STG tail is preserved as raw bytes",
        })
    );
    assert_eq!(
        diagnostics.last(),
        Some(&Diagnostic {
            severity: Severity::Warning,
            location: DiagnosticLocation::STGDocument,
            message: "Additional STG diagnostics were omitted",
        })
    );
}

fn changed_structure(
    mutation: STGMutation<kufeditor_formats::STGStructuralImage>,
) -> kufeditor_formats::STGStructuralImage {
    match mutation {
        STGMutation::Changed { previous } => previous,
        STGMutation::Unchanged => panic!("expected an STG structural change"),
    }
}

#[derive(Clone, Copy)]
enum ExpectedSTGValue {
    Integer(i32),
    Float(u32),
    String(&'static str),
    Enum(i32),
}

impl ExpectedSTGValue {
    fn assert_eq(self, actual: &STGValue<'_>) {
        match self {
            Self::Integer(expected) => assert_eq!(actual, &STGValue::Integer(expected)),
            Self::Float(expected) => {
                assert_eq!(actual, &STGValue::Float(STGFloatValue::from_bits(expected)));
            }
            Self::String(expected) => {
                assert_eq!(actual, &STGValue::String(STGText::Decoded(expected.into())));
            }
            Self::Enum(expected) => assert_eq!(actual, &STGValue::Enum(expected)),
        }
    }
}

fn assert_default_stg_value(actual: &STGValue<'_>, kind: STGValueKind) {
    match kind {
        STGValueKind::Integer => assert_eq!(actual, &STGValue::Integer(0)),
        STGValueKind::Float => {
            assert_eq!(actual, &STGValue::Float(STGFloatValue::from_bits(0)));
        }
        STGValueKind::String => {
            assert_eq!(actual, &STGValue::String(STGText::Decoded("".into())));
        }
        STGValueKind::Enum => assert_eq!(actual, &STGValue::Enum(0)),
    }
}

fn event_ids(document: &STGDocument, block: usize) -> Vec<u32> {
    let count = document
        .event_block(block)
        .unwrap_or_else(|error| panic!("failed to read test event block {block}: {error}"))
        .event_count;
    (0..count)
        .map(|event| {
            document
                .event(STGEventTarget { block, event })
                .unwrap_or_else(|error| {
                    panic!("failed to read test event {block}:{event}: {error}")
                })
                .id
        })
        .collect()
}

fn assert_structure_unavailable<T>(
    result: Result<T, FormatError>,
    expected_location: STGStructuralLocation,
) {
    match result {
        Err(FormatError::STGStructureUnavailable { location }) => {
            assert_eq!(location, expected_location);
        }
        Err(other) => panic!("unexpected opaque-tail error: {other}"),
        Ok(_) => panic!("expected an opaque-tail structural rejection"),
    }
}

fn assert_structural_state_mismatch<T>(
    result: Result<T, FormatError>,
    expected_location: STGStructuralLocation,
) {
    match result {
        Err(FormatError::STGStructuralStateMismatch { location }) => {
            assert_eq!(location, expected_location);
        }
        Err(other) => panic!("unexpected structural-state error: {other}"),
        Ok(_) => panic!("expected a structural-state rejection"),
    }
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
