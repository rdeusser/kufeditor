#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use known fixed-size byte ranges"
)]

use kufeditor_formats::{FormatError, Severity, TroopDocument, TroopField};

const MIXED_CASE_ASCII_HEX_TROOP: &[u8] = concat!(
    "64000000010000000000000000000000",
    "82000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "000000000000000000000000dEaDbEeF",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "000000000000000000000000"
)
.as_bytes();

const UPPERCASE_ASCII_HEX_EDITED_TROOP: &[u8] = concat!(
    "64000000010000000000000000000000",
    "AF000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "000000000000000000000000DEADBEEF",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "00000000000000000000000000000000",
    "000000000000000000000000"
)
.as_bytes();

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
fn ascii_hex_mixed_case_fixture_parses_the_move_speed() {
    let document = TroopDocument::parse(MIXED_CASE_ASCII_HEX_TROOP.to_vec()).unwrap();

    assert_eq!(document.value(0, TroopField::MoveSpeed).unwrap(), 130);
}

#[test]
fn ascii_hex_unchanged_encode_preserves_the_original_letter_case() {
    let document = TroopDocument::parse(MIXED_CASE_ASCII_HEX_TROOP.to_vec()).unwrap();

    assert_eq!(document.encode().unwrap(), MIXED_CASE_ASCII_HEX_TROOP);
}

#[test]
fn ascii_hex_edit_emits_uppercase_hex_and_round_trips() {
    let mut document = TroopDocument::parse(MIXED_CASE_ASCII_HEX_TROOP.to_vec()).unwrap();
    document.set_value(0, TroopField::MoveSpeed, 175).unwrap();

    let encoded = document.encode().unwrap();
    assert_eq!(encoded, UPPERCASE_ASCII_HEX_EDITED_TROOP);

    let reparsed = TroopDocument::parse(encoded).unwrap();
    assert_eq!(reparsed.value(0, TroopField::MoveSpeed).unwrap(), 175);
}

#[test]
fn ascii_hex_odd_length_returns_a_typed_error() {
    let error = TroopDocument::parse(b"6400000001000000F".to_vec()).unwrap_err();

    assert!(matches!(
        error,
        FormatError::OddASCIIHexLength { length: 17 }
    ));
}

#[test]
fn ascii_hex_non_hex_byte_after_a_valid_prefix_returns_a_typed_error() {
    let error = TroopDocument::parse(b"6400000001000000Z0".to_vec()).unwrap_err();

    assert!(matches!(
        error,
        FormatError::InvalidASCIIHexByte { index: 16 }
    ));
}

#[test]
fn ascii_prefix_that_is_not_a_sox_candidate_uses_the_raw_parser() {
    let error = TroopDocument::parse(b"0000000001000000raw SOX source".to_vec()).unwrap_err();

    assert!(matches!(error, FormatError::TroopParse { .. }));
}

#[test]
fn rebase_rejects_a_saved_source_with_a_different_envelope() {
    let saved = TroopDocument::parse(MIXED_CASE_ASCII_HEX_TROOP.to_vec()).unwrap();
    let mut document = saved.clone();

    let error = document.rebase_source(&saved, troop_fixture()).unwrap_err();

    assert!(matches!(error, FormatError::InconsistentSOXRebase));
}

#[test]
fn rebase_rejects_same_envelope_source_that_does_not_match_the_saved_snapshot() {
    let source = troop_fixture();
    let saved = TroopDocument::parse(source.clone()).unwrap();
    let mut document = saved.clone();
    let mut inconsistent = source.clone();
    inconsistent
        .get_mut(16..20)
        .unwrap()
        .copy_from_slice(&175_i32.to_le_bytes());

    let error = document.rebase_source(&saved, inconsistent).unwrap_err();

    assert!(matches!(error, FormatError::InconsistentSOXRebase));
    assert_eq!(document.encode().unwrap(), source);
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
    document.set_value(0, TroopField::DefaultUnitHP, 0).unwrap();

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
fn impossible_large_record_count_returns_a_typed_parse_error() {
    let source = vec![0x64, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff];

    let error = TroopDocument::parse(source).unwrap_err();

    assert!(matches!(&error, FormatError::TroopParse { .. }));
    assert_eq!(
        error.to_string(),
        "failed to parse TroopInfo at offset 8: invalid length 4294967295 for records"
    );
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
