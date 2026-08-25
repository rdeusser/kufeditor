#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use known fixed-size byte ranges"
)]

use kufeditor_formats::{Severity, TroopDocument, TroopField};

fn troop_fixture() -> Vec<u8> {
    let mut bytes = vec![0_u8; 8 + 148 + 64];
    bytes
        .get_mut(0..4)
        .unwrap()
        .copy_from_slice(&100_u32.to_le_bytes());
    bytes
        .get_mut(4..8)
        .unwrap()
        .copy_from_slice(&1_u32.to_le_bytes());
    bytes
        .get_mut(16..20)
        .unwrap()
        .copy_from_slice(&130_i32.to_le_bytes());
    bytes
        .get_mut(64..68)
        .unwrap()
        .copy_from_slice(&100_i32.to_le_bytes());
    bytes
        .get_mut(108..112)
        .unwrap()
        .copy_from_slice(&800_i32.to_le_bytes());
    for (index, byte) in bytes.get_mut(156..220).unwrap().iter_mut().enumerate() {
        *byte = u8::try_from(index).unwrap();
    }
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    bytes
}

#[test]
fn unchanged_encode_is_byte_identical() {
    let source = troop_fixture();
    let document = TroopDocument::parse(source.clone()).unwrap();

    assert_eq!(document.record_count(), 1);
    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn edit_round_trip_preserves_footer_and_tail() {
    let source = troop_fixture();
    let mut document = TroopDocument::parse(source.clone()).unwrap();
    assert_eq!(
        document.set_value(0, TroopField::MoveSpeed, 175).unwrap(),
        130
    );

    let encoded = document.encode().unwrap();
    assert_eq!(encoded.get(156..), source.get(156..));

    let reparsed = TroopDocument::parse(encoded).unwrap();
    assert_eq!(reparsed.value(0, TroopField::MoveSpeed).unwrap(), 175);
}

#[test]
fn invalid_resistance_and_hp_are_diagnostics() {
    let mut document = TroopDocument::parse(troop_fixture()).unwrap();
    document.set_value(0, TroopField::ResistMelee, 501).unwrap();
    document.set_value(0, TroopField::DefaultUnitHp, 0).unwrap();

    let diagnostics = document.diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|item| item.severity == Severity::Warning)
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.severity == Severity::Error)
    );
}

#[test]
fn truncated_input_returns_an_error() {
    let error = TroopDocument::parse(troop_fixture().get(..107).unwrap().to_vec()).unwrap_err();

    assert!(error.to_string().contains("offset"));
}

#[test]
fn every_wire_field_has_editor_metadata() {
    assert_eq!(TroopField::ALL.len(), 37);
    assert!(
        TroopField::ALL
            .iter()
            .all(|field| !field.label().is_empty())
    );
}
