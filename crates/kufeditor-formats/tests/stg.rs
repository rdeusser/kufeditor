#![allow(
    clippy::indexing_slicing,
    reason = "synthetic STG fixtures expose checked offsets for direct wire corruption"
)]

use std::collections::HashSet;

#[path = "support/stg.rs"]
mod stg_support;

use kufeditor_formats::{
    DiagnosticLocation, FormatError, STGAbilityOwner, STGAreaField, STGAreaFloatField,
    STGCleaveError, STGCleaveErrorKind, STGCollection, STGDocument, STGEditor, STGEncodeError,
    STGFieldAccess, STGFloatTarget, STGFloatValue, STGFooterField, STGHeaderTextField, STGMutation,
    STGNumberTarget, STGParameterTarget, STGParseError, STGPreflightError, STGRebaseError,
    STGRegion, STGScriptKind, STGScriptTarget, STGSkillField, STGSkillOwner, STGStructuralLocation,
    STGTailFailure, STGTailStatus, STGTarget, STGText, STGTextEncoding, STGTextError,
    STGTextTarget, STGUnitField, STGUnitFloatField, STGUnitGroup, STGValueKind, STGValueTarget,
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
    assert_eq!(editor.storage_bounds(), (0, i64::from(u8::MAX)));

    assert_eq!(STGText::Decoded("hello").decoded(), Some("hello"));
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
