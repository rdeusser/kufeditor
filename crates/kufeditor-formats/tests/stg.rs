use std::collections::HashSet;

#[path = "support/stg.rs"]
mod stg_support;

use kufeditor_formats::{
    DiagnosticLocation, FormatError, STGAbilityOwner, STGAreaField, STGAreaFloatField,
    STGCleaveError, STGCleaveErrorKind, STGCollection, STGEditor, STGEncodeError, STGFieldAccess,
    STGFloatTarget, STGFloatValue, STGFooterField, STGHeaderTextField, STGMutation,
    STGNumberTarget, STGParameterTarget, STGParseError, STGPreflightError, STGRebaseError,
    STGRegion, STGScriptKind, STGScriptTarget, STGSkillField, STGSkillOwner, STGStructuralLocation,
    STGTailFailure, STGTarget, STGText, STGTextEncoding, STGTextError, STGTextTarget, STGUnitField,
    STGUnitFloatField, STGUnitGroup, STGValueKind, STGValueTarget,
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
