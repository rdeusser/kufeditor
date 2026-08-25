#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use lengths that fit their wire fields"
)]

use kufeditor_formats::{FormatError, SOXDocument, parse_sox};

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
}

fn skill_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 1);
    bytes.extend_from_slice(&(-2_i32).to_le_bytes());
    push_bytes(&mut bytes, b"@(S_Elemental)");
    push_bytes(&mut bytes, b"IL_SKL_Elem.tga");
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 25);
    bytes.extend_from_slice(b"THEND");
    bytes.resize(bytes.len() + 59, b' ');
    bytes
}

fn text_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 17);
    push_bytes(&mut bytes, b"First");
    push_u32(&mut bytes, 4_096);
    push_bytes(&mut bytes, b"Second");
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    bytes
}

fn push_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(value);
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn ascii_hex(bytes: &[u8]) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = Vec::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(*DIGITS.get(usize::from(byte >> 4)).unwrap());
        encoded.push(*DIGITS.get(usize::from(byte & 0x0f)).unwrap());
    }
    encoded
}

#[test]
fn detects_raw_troop_info() {
    let document = parse_sox(troop_fixture()).unwrap();

    assert!(matches!(document, SOXDocument::Troop(_)));
}

#[test]
fn detects_ascii_hex_troop_info() {
    let document = parse_sox(ascii_hex(&troop_fixture())).unwrap();

    assert!(matches!(document, SOXDocument::Troop(_)));
}

#[test]
fn detects_raw_skill_info() {
    let document = parse_sox(skill_fixture()).unwrap();

    assert!(matches!(document, SOXDocument::Skill(_)));
}

#[test]
fn detects_ascii_hex_skill_info() {
    let document = parse_sox(ascii_hex(&skill_fixture())).unwrap();

    assert!(matches!(document, SOXDocument::Skill(_)));
}

#[test]
fn detects_raw_text_sox() {
    let document = parse_sox(text_fixture()).unwrap();

    assert!(matches!(document, SOXDocument::Text(_)));
}

#[test]
fn detects_ascii_hex_text_sox() {
    let document = parse_sox(ascii_hex(&text_fixture())).unwrap();

    assert!(matches!(document, SOXDocument::Text(_)));
}

#[test]
fn rejects_a_sox_marker_without_a_supported_body() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 1);
    bytes.resize(8 + 64, 0);

    let error = parse_sox(bytes).unwrap_err();

    assert!(matches!(error, FormatError::UnsupportedSOX));
}

#[test]
fn rejects_text_shaped_payload_with_a_nul_byte() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 73);
    push_bytes(&mut bytes, b"ok\0");

    let error = parse_sox(bytes).unwrap_err();

    assert!(matches!(error, FormatError::UnsupportedSOX));
}

#[test]
fn malformed_ascii_hex_candidate_keeps_the_encoding_error() {
    let error = parse_sox(b"6400000001000000Z0".to_vec()).unwrap_err();

    assert!(matches!(
        error,
        FormatError::InvalidASCIIHexByte { index: 16 }
    ));
}
