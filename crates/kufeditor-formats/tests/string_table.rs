#![allow(
    clippy::unwrap_used,
    reason = "literal fixtures use statically valid sizes and expected successful parses"
)]

use kufeditor_formats::{
    FormatError, SOXDocument, SOXStringTableDocument, SOXStringTableLayout, StringTableParseError,
    parse_sox,
};

struct LayoutFixture {
    layout: SOXStringTableLayout,
    bytes: Vec<u8>,
    ids: Vec<Option<u32>>,
    fields: Vec<Vec<Vec<u8>>>,
}

fn sequential_fixture() -> LayoutFixture {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 6);
    bytes.extend_from_slice(b"Alpha\xAB");
    push_trailing(&mut bytes);

    LayoutFixture {
        layout: SOXStringTableLayout::Sequential,
        bytes,
        ids: vec![None, None],
        fields: vec![vec![Vec::new()], vec![b"Alpha\xAB".to_vec()]],
    }
}

fn indexed_fixture() -> LayoutFixture {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 3);
    push_u16(&mut bytes, 0);
    push_u32(&mut bytes, 4_096);
    push_u16(&mut bytes, 4);
    bytes.extend_from_slice(b"Beta");
    push_trailing(&mut bytes);

    LayoutFixture {
        layout: SOXStringTableLayout::Indexed,
        bytes,
        ids: vec![Some(3), Some(4_096)],
        fields: vec![vec![Vec::new()], vec![b"Beta".to_vec()]],
    }
}

fn indexed_pair_fixture() -> LayoutFixture {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 7);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 3);
    bytes.extend_from_slice(b"Key");
    push_u32(&mut bytes, 4_096);
    push_u16(&mut bytes, 4);
    bytes.extend_from_slice(b"Name");
    push_u16(&mut bytes, 0);
    push_trailing(&mut bytes);

    LayoutFixture {
        layout: SOXStringTableLayout::IndexedPair,
        bytes,
        ids: vec![Some(7), Some(4_096)],
        fields: vec![
            vec![Vec::new(), b"Key".to_vec()],
            vec![b"Name".to_vec(), Vec::new()],
        ],
    }
}

fn indexed_triple_fixture() -> LayoutFixture {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 11);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 3);
    bytes.extend_from_slice(b"One");
    push_u16(&mut bytes, 3);
    bytes.extend_from_slice(b"Two");
    push_u32(&mut bytes, 4_096);
    push_u16(&mut bytes, 5);
    bytes.extend_from_slice(b"Three");
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 4);
    bytes.extend_from_slice(b"Four");
    push_trailing(&mut bytes);

    LayoutFixture {
        layout: SOXStringTableLayout::IndexedTriple,
        bytes,
        ids: vec![Some(11), Some(4_096)],
        fields: vec![
            vec![Vec::new(), b"One".to_vec(), b"Two".to_vec()],
            vec![b"Three".to_vec(), Vec::new(), b"Four".to_vec()],
        ],
    }
}

fn layout_fixtures() -> [LayoutFixture; 4] {
    [
        sequential_fixture(),
        indexed_fixture(),
        indexed_pair_fixture(),
        indexed_triple_fixture(),
    ]
}

fn push_trailing(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(b"THEND");
    bytes.extend_from_slice(&[b' '; 59]);
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
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

fn mixed_case_ascii_hex(bytes: &[u8]) -> Vec<u8> {
    let mut encoded = ascii_hex(bytes);
    for (index, byte) in encoded.iter_mut().enumerate() {
        if index.is_multiple_of(2) && matches!(*byte, b'A'..=b'F') {
            *byte = byte.to_ascii_lowercase();
        }
    }
    encoded
}

#[test]
fn literal_layouts_parse_with_exact_records_fields_and_writes() {
    for fixture in layout_fixtures() {
        let document =
            SOXStringTableDocument::parse(fixture.layout, fixture.bytes.clone()).unwrap();

        assert_eq!(document.layout(), fixture.layout);
        assert_eq!(document.record_count(), fixture.fields.len());
        assert_eq!(fixture.ids.len(), fixture.fields.len());
        for (record, expected_fields) in fixture.fields.iter().enumerate() {
            assert_eq!(
                document.record_id(record).unwrap(),
                fixture.ids.get(record).copied().unwrap()
            );
            for (field, expected) in expected_fields.iter().enumerate() {
                assert_eq!(document.field(record, field).unwrap(), expected);
            }
            assert!(matches!(
                document.field(record, expected_fields.len()).unwrap_err(),
                FormatError::StringTableFieldOutOfRange {
                    layout,
                    record: actual_record,
                    field,
                    field_count,
                } if layout == fixture.layout
                    && actual_record == record
                    && field == expected_fields.len()
                    && field_count == expected_fields.len()
            ));
        }
        assert_eq!(document.encode(), fixture.bytes);
        assert_eq!(document.canonical_encode().unwrap(), fixture.bytes);
    }
}

#[test]
fn layout_display_labels_are_stable() {
    let cases = [
        (SOXStringTableLayout::Sequential, "Sequential"),
        (SOXStringTableLayout::Indexed, "Indexed"),
        (SOXStringTableLayout::IndexedPair, "IndexedPair"),
        (SOXStringTableLayout::IndexedTriple, "IndexedTriple"),
    ];

    for (layout, label) in cases {
        assert_eq!(layout.to_string(), label);
    }
}

#[test]
fn mixed_case_ascii_hex_preserves_exact_source_and_canonicalizes_uppercase() {
    let fixture = sequential_fixture();
    let encoded = mixed_case_ascii_hex(&fixture.bytes);

    let document = SOXStringTableDocument::parse(fixture.layout, encoded.clone()).unwrap();

    assert_eq!(document.encode(), encoded);
    assert_eq!(
        document.canonical_encode().unwrap(),
        ascii_hex(&fixture.bytes)
    );
    assert!(
        !document
            .canonical_encode()
            .unwrap()
            .iter()
            .any(u8::is_ascii_lowercase)
    );
}

#[test]
fn wrong_marker_is_a_layout_aware_parse_error() {
    let mut bytes = indexed_pair_fixture().bytes;
    bytes
        .get_mut(..4)
        .unwrap()
        .copy_from_slice(&2_u32.to_le_bytes());

    let error =
        SOXStringTableDocument::parse(SOXStringTableLayout::IndexedPair, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::StringTableParse {
            layout: SOXStringTableLayout::IndexedPair,
            offset: 0,
            source: StringTableParseError::InvalidMarker { marker: 2 },
        }
    ));
}

#[test]
fn odd_ascii_hex_length_remains_typed() {
    let error = SOXStringTableDocument::parse(
        SOXStringTableLayout::Sequential,
        b"6400000001000000A".to_vec(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::OddASCIIHexLength { length: 17 }
    ));
}

#[test]
fn invalid_ascii_hex_byte_remains_typed() {
    let error = SOXStringTableDocument::parse(
        SOXStringTableLayout::Sequential,
        b"6400000001000000Z0".to_vec(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::InvalidASCIIHexByte { index: 16 }
    ));
}

#[test]
fn truncated_header_remains_typed() {
    let error = SOXStringTableDocument::parse(
        SOXStringTableLayout::Sequential,
        vec![100, 0, 0, 0, 1, 0, 0],
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::StringTableParse {
            layout: SOXStringTableLayout::Sequential,
            offset: 7,
            source: StringTableParseError::TruncatedHeader { actual: 7 },
        }
    ));
}

#[test]
fn truncated_stored_id_remains_typed() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 1);
    push_u16(&mut bytes, 5);
    bytes.extend_from_slice(b"First");
    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc]);

    let error = SOXStringTableDocument::parse(SOXStringTableLayout::Indexed, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::StringTableParse {
            layout: SOXStringTableLayout::Indexed,
            offset: 19,
            source: StringTableParseError::TruncatedStoredID {
                record: 1,
                remaining: 3,
            },
        }
    ));
}

#[test]
fn truncated_field_length_remains_typed() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 2);
    push_u16(&mut bytes, 3);
    bytes.extend_from_slice(b"One");
    bytes.push(0xff);

    let error = SOXStringTableDocument::parse(SOXStringTableLayout::Sequential, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::StringTableParse {
            layout: SOXStringTableLayout::Sequential,
            offset: 13,
            source: StringTableParseError::TruncatedFieldLength {
                record: 1,
                field: 0,
                remaining: 1,
            },
        }
    ));
}

#[test]
fn truncated_field_payload_reports_the_full_context() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 9);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 4);
    bytes.push(b'X');

    let error =
        SOXStringTableDocument::parse(SOXStringTableLayout::IndexedPair, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::StringTableParse {
            layout: SOXStringTableLayout::IndexedPair,
            offset: 16,
            source: StringTableParseError::TruncatedFieldPayload {
                record: 0,
                field: 1,
                length: 4,
                remaining: 1,
            },
        }
    ));
}

#[test]
fn impossible_record_count_stops_at_the_handwritten_preflight() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, u32::MAX);
    push_trailing(&mut bytes);
    let remaining = bytes.len() - 8;

    let error =
        SOXStringTableDocument::parse(SOXStringTableLayout::IndexedTriple, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::StringTableParse {
            layout: SOXStringTableLayout::IndexedTriple,
            offset: 8,
            source: StringTableParseError::ImpossibleRecordCount {
                count: u32::MAX,
                minimum_record_size: 10,
                remaining: actual_remaining,
            },
        } if actual_remaining == remaining
    ));
}

#[test]
fn each_layout_rejects_one_byte_less_than_its_minimum_body() {
    let cases = [
        (SOXStringTableLayout::Sequential, 2_usize),
        (SOXStringTableLayout::Indexed, 6),
        (SOXStringTableLayout::IndexedPair, 8),
        (SOXStringTableLayout::IndexedTriple, 10),
    ];

    for (layout, minimum_record_size) in cases {
        let mut bytes = Vec::new();
        push_u32(&mut bytes, 100);
        push_u32(&mut bytes, 2);
        let remaining = minimum_record_size * 2 - 1;
        bytes.resize(bytes.len() + remaining, 0);

        let error = SOXStringTableDocument::parse(layout, bytes).unwrap_err();

        assert!(
            matches!(
                error,
                FormatError::StringTableParse {
                    layout: actual_layout,
                    offset: 8,
                    source: StringTableParseError::ImpossibleRecordCount {
                        count: 2,
                        minimum_record_size: actual_minimum,
                        remaining: actual_remaining,
                    },
                } if actual_layout == layout
                    && actual_minimum == minimum_record_size
                    && actual_remaining == remaining
            ),
            "{layout}: {error}"
        );
    }
}

#[test]
fn record_and_field_access_errors_report_valid_counts() {
    let fixture = indexed_pair_fixture();
    let document = SOXStringTableDocument::parse(fixture.layout, fixture.bytes.clone()).unwrap();

    assert!(matches!(
        document.record_id(2).unwrap_err(),
        FormatError::StringTableRecordOutOfRange {
            layout: SOXStringTableLayout::IndexedPair,
            record: 2,
            record_count: 2,
        }
    ));
    assert!(matches!(
        document.field(2, 0).unwrap_err(),
        FormatError::StringTableRecordOutOfRange {
            layout: SOXStringTableLayout::IndexedPair,
            record: 2,
            record_count: 2,
        }
    ));
    assert!(matches!(
        document.field(0, 2).unwrap_err(),
        FormatError::StringTableFieldOutOfRange {
            layout: SOXStringTableLayout::IndexedPair,
            record: 0,
            field: 2,
            field_count: 2,
        }
    ));
}

fn troop_fixture() -> Vec<u8> {
    let mut bytes = vec![0_u8; 8 + 148 + 64];
    bytes
        .get_mut(..4)
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
    push_u16(&mut bytes, 14);
    bytes.extend_from_slice(b"@(S_Elemental)");
    push_u16(&mut bytes, 14);
    bytes.extend_from_slice(b"IL_SKL_Elem.tga");
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 25);
    bytes.extend_from_slice(b"THEND");
    bytes.extend_from_slice(&[b' '; 59]);
    bytes
}

fn text_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 17);
    push_u16(&mut bytes, 4);
    bytes.extend_from_slice(b"Text");
    bytes
}

fn existing_document_kind(document: &SOXDocument) -> &'static str {
    match document {
        SOXDocument::Troop(_) => "TroopInfo",
        SOXDocument::Skill(_) => "SkillInfo",
        SOXDocument::Text(_) => "text SOX",
    }
}

#[test]
fn automatic_detection_still_has_only_the_three_existing_document_kinds() {
    let troop = parse_sox(troop_fixture()).unwrap();
    let skill = parse_sox(skill_fixture()).unwrap();
    let text = parse_sox(text_fixture()).unwrap();

    assert_eq!(existing_document_kind(&troop), "TroopInfo");
    assert_eq!(existing_document_kind(&skill), "SkillInfo");
    assert_eq!(existing_document_kind(&text), "text SOX");

    let error = parse_sox(sequential_fixture().bytes).unwrap_err();
    assert!(matches!(error, FormatError::UnsupportedSOX));
}
