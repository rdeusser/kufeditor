#![allow(
    clippy::unwrap_used,
    reason = "literal fixtures use statically valid sizes and expected successful parses"
)]

use std::collections::HashSet;

use kufeditor_formats::{
    FormatError, GeneratedSOXError, SOXDocument, SOXSchema, SchemaDocument, SpecialNameRef,
    parse_sox,
};

const SPECIAL_NAMES_PREFIX: &[u8] = &[
    0x64, 0x00, 0x00, 0x00, // marker 100
    0x02, 0x00, 0x00, 0x00, // two records
    0x04, 0x00, b'H', b'e', b'r', b'o', // record 0 key
    0x02, 0x00, 0xb0, 0xa1, // record 0 value: CP949 "ga"
    0x03, 0x00, b'N', b'P', b'C', // record 1 key
    0x00, 0x00, // record 1 empty value
];
const SPECIAL_NAMES_FOOTER: &[u8; 64] =
    b"THEND                                                           ";

#[test]
fn registry_contains_each_named_schema_once() {
    let schemas: HashSet<_> = SOXSchema::ALL.into_iter().collect();

    assert_eq!(SOXSchema::ALL.len(), 18);
    assert_eq!(schemas.len(), 18);
}

#[test]
fn schemas_expose_canonical_stems_markers_and_display_names() {
    let cases = [
        (SOXSchema::AbilityByJob, "AbilityByJob", 100),
        (SOXSchema::AbilityInfo, "AbilityInfo", 100),
        (SOXSchema::CharInfo, "CharInfo", 100),
        (SOXSchema::CustomRandomTable, "KUF2CustomRandomTable", 100),
        (SOXSchema::ItemAttInfo, "ItemAttInfo", 100),
        (SOXSchema::ItemTypeInfo, "ItemTypeInfo", 2),
        (SOXSchema::JobInfo, "JobInfo", 100),
        (SOXSchema::LeaderGeneration, "LeaderGeneration", 100),
        (SOXSchema::LibraryInfo, "LibraryInfo", 100),
        (SOXSchema::ResistInfo, "ResistInfo", 100),
        (SOXSchema::SkillInfo, "SkillInfo", 100),
        (SOXSchema::SkillPointTable, "SkillPointTable", 100),
        (SOXSchema::SpecialNames, "SpecialNames", 100),
        (SOXSchema::TroopInfo, "TroopInfo", 100),
        (SOXSchema::UnitUVInfo, "UnitUVInfo", 100),
        (SOXSchema::UnitUVID, "UnitUVID", 100),
        (SOXSchema::WorldmapCharInfo, "WorldMap_CharInfo", 100),
        (SOXSchema::WorldmapTroopInfo, "WorldMap_TroopInfo", 100),
    ];

    for (schema, stem, marker) in cases {
        assert_eq!(schema.file_stem(), stem);
        assert_eq!(schema.marker(), marker);
        assert_eq!(schema.to_string(), stem);
    }
}

struct SchemaFixture {
    schema: SOXSchema,
    bytes: Vec<u8>,
    record_count: usize,
}

fn schema_fixtures() -> Vec<SchemaFixture> {
    [
        (SOXSchema::AbilityByJob, standard_fixture(24), 1),
        (SOXSchema::AbilityInfo, standard_fixture(64), 1),
        (SOXSchema::CharInfo, standard_fixture(136), 1),
        (
            SOXSchema::CustomRandomTable,
            custom_random_table_fixture(),
            9,
        ),
        (SOXSchema::ItemAttInfo, standard_fixture(12), 1),
        (SOXSchema::ItemTypeInfo, item_type_info_fixture(), 1),
        (SOXSchema::JobInfo, standard_fixture(72), 1),
        (SOXSchema::LeaderGeneration, standard_fixture(72), 1),
        (SOXSchema::LibraryInfo, standard_fixture(6), 1),
        (SOXSchema::ResistInfo, standard_fixture(12), 1),
        (SOXSchema::SkillInfo, standard_fixture(16), 1),
        (SOXSchema::SkillPointTable, standard_fixture(8), 1),
        (SOXSchema::SpecialNames, standard_fixture(4), 1),
        (SOXSchema::TroopInfo, standard_fixture(148), 1),
        (SOXSchema::UnitUVInfo, standard_fixture(36), 1),
        (SOXSchema::UnitUVID, standard_fixture(72), 1),
        (SOXSchema::WorldmapCharInfo, standard_fixture(28), 1),
        (SOXSchema::WorldmapTroopInfo, standard_fixture(28), 1),
    ]
    .into_iter()
    .map(|(schema, bytes, record_count)| SchemaFixture {
        schema,
        bytes,
        record_count,
    })
    .collect()
}

fn standard_fixture(record_bytes: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, 1);
    bytes.resize(bytes.len() + record_bytes, 0);
    push_footer(&mut bytes);
    bytes
}

fn item_type_info_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 1);
    bytes.resize(bytes.len() + 44 * 4, 0);
    push_u16(&mut bytes, 0);
    bytes
}

fn custom_random_table_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, u32::MAX);
    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 0);
    for _ in 0..7 {
        bytes.extend_from_slice(&[0; 4 * 4]);
    }
    push_u32(&mut bytes, 0);
    for _ in 0..9 {
        bytes.extend_from_slice(&[0; 30 * 4]);
    }
    push_footer(&mut bytes);
    assert_eq!(bytes.len(), 1_272);
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

fn push_footer(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(b"THEND");
    bytes.extend_from_slice(&[b' '; 59]);
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

fn special_names_fixture() -> Vec<u8> {
    let mut bytes = SPECIAL_NAMES_PREFIX.to_vec();
    bytes.extend_from_slice(SPECIAL_NAMES_FOOTER);
    bytes
}

#[test]
fn special_name_projects_literal_records_and_preserves_raw_source() {
    let bytes = special_names_fixture();

    let document = SchemaDocument::parse(SOXSchema::SpecialNames, bytes.clone()).unwrap();

    assert_eq!(
        document.special_name(0),
        Some(SpecialNameRef {
            key: b"Hero",
            value: &[0xb0, 0xa1],
        })
    );
    assert_eq!(
        document.special_name(1),
        Some(SpecialNameRef {
            key: b"NPC",
            value: b"",
        })
    );
    assert_eq!(document.encode(), bytes);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn special_name_is_none_for_wrong_schema_or_record() {
    let special_names =
        SchemaDocument::parse(SOXSchema::SpecialNames, special_names_fixture()).unwrap();
    let ability_by_job =
        SchemaDocument::parse(SOXSchema::AbilityByJob, standard_fixture(24)).unwrap();

    assert_eq!(special_names.special_name(2), None);
    assert_eq!(ability_by_job.special_name(0), None);
}

#[test]
fn special_name_preserves_mixed_case_ascii_hex_source_and_canonicalizes_case() {
    let raw = special_names_fixture();
    let encoded = mixed_case_ascii_hex(&raw);
    assert!(encoded.iter().any(u8::is_ascii_lowercase));

    let document = SchemaDocument::parse(SOXSchema::SpecialNames, encoded.clone()).unwrap();
    let canonical = document.canonical_encode().unwrap();

    assert_eq!(document.encode(), encoded);
    assert_eq!(canonical, ascii_hex(&raw));
    assert!(!canonical.iter().any(u8::is_ascii_lowercase));
}

#[test]
fn literal_fixtures_parse_and_write_every_generated_schema() {
    for fixture in schema_fixtures() {
        let document = SchemaDocument::parse(fixture.schema, fixture.bytes.clone()).unwrap();

        assert_eq!(document.schema(), fixture.schema);
        assert_eq!(document.record_count(), fixture.record_count);
        assert_eq!(document.encode(), fixture.bytes);
        assert_eq!(document.canonical_encode().unwrap(), fixture.bytes);
    }
}

#[test]
fn mixed_case_standard_ascii_hex_preserves_exact_source() {
    let mut raw = standard_fixture(24);
    *raw.get_mut(8).unwrap() = 0xab;
    let encoded = mixed_case_ascii_hex(&raw);

    let document = SchemaDocument::parse(SOXSchema::AbilityByJob, encoded.clone()).unwrap();

    assert_eq!(document.encode(), encoded);
    assert_eq!(document.canonical_encode().unwrap(), ascii_hex(&raw));
}

#[test]
fn mixed_case_marker_two_ascii_hex_preserves_exact_source() {
    let mut raw = item_type_info_fixture();
    *raw.get_mut(8).unwrap() = 0xab;
    let encoded = mixed_case_ascii_hex(&raw);

    let document = SchemaDocument::parse(SOXSchema::ItemTypeInfo, encoded.clone()).unwrap();

    assert_eq!(document.encode(), encoded);
    assert_eq!(document.canonical_encode().unwrap(), ascii_hex(&raw));
}

#[test]
fn canonical_ascii_hex_uses_only_uppercase_hexadecimal_digits() {
    let mut raw = standard_fixture(24);
    *raw.get_mut(8).unwrap() = 0xab;
    let document =
        SchemaDocument::parse(SOXSchema::AbilityByJob, mixed_case_ascii_hex(&raw)).unwrap();

    let canonical = document.canonical_encode().unwrap();

    assert_eq!(canonical, ascii_hex(&raw));
    assert!(!canonical.iter().any(u8::is_ascii_lowercase));
}

#[test]
fn standard_schema_preserves_bytes_after_the_footer() {
    let mut bytes = standard_fixture(24);
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let document = SchemaDocument::parse(SOXSchema::AbilityByJob, bytes.clone()).unwrap();

    assert_eq!(document.encode(), bytes);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn item_type_info_preserves_bytes_after_the_generated_range() {
    let mut bytes = item_type_info_fixture();
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let document = SchemaDocument::parse(SOXSchema::ItemTypeInfo, bytes.clone()).unwrap();

    assert_eq!(document.encode(), bytes);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn fixed_custom_random_table_ignores_the_header_count() {
    let bytes = custom_random_table_fixture();

    let document = SchemaDocument::parse(SOXSchema::CustomRandomTable, bytes.clone()).unwrap();

    assert_eq!(document.record_count(), 9);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn truncated_custom_random_table_stops_at_the_fixed_layout_preflight() {
    let mut bytes = custom_random_table_fixture();
    bytes.pop();

    let error = SchemaDocument::parse(SOXSchema::CustomRandomTable, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::SchemaParse {
            schema: SOXSchema::CustomRandomTable,
            source: GeneratedSOXError::UnexpectedEOF { .. },
            ..
        }
    ));
}

#[test]
fn wrong_marker_is_a_schema_aware_parse_error() {
    let mut bytes = standard_fixture(24);
    bytes
        .get_mut(0..4)
        .unwrap()
        .copy_from_slice(&2_u32.to_le_bytes());

    let error = SchemaDocument::parse(SOXSchema::AbilityByJob, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::SchemaParse {
            schema: SOXSchema::AbilityByJob,
            offset: 4,
            source: GeneratedSOXError::Validation {
                id: "marker.check",
                field: "marker",
                ..
            }
        }
    ));
}

#[test]
fn impossible_record_count_stops_at_the_handwritten_preflight() {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, u32::MAX);
    push_footer(&mut bytes);

    let error = SchemaDocument::parse(SOXSchema::AbilityByJob, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::SchemaParse {
            schema: SOXSchema::AbilityByJob,
            offset: 8,
            source: GeneratedSOXError::InvalidLength {
                field: "records",
                value,
            },
        } if value == i128::from(u32::MAX)
    ));
}

#[test]
fn header_driven_preflights_reject_two_records_with_one_minimum_body() {
    let cases = [
        (SOXSchema::AbilityByJob, 100, 24, 64),
        (SOXSchema::AbilityInfo, 100, 64, 64),
        (SOXSchema::CharInfo, 100, 136, 64),
        (SOXSchema::ItemAttInfo, 100, 12, 64),
        (SOXSchema::ItemTypeInfo, 2, 178, 0),
        (SOXSchema::JobInfo, 100, 72, 64),
        (SOXSchema::LeaderGeneration, 100, 72, 64),
        (SOXSchema::LibraryInfo, 100, 6, 64),
        (SOXSchema::ResistInfo, 100, 12, 64),
        (SOXSchema::SkillInfo, 100, 16, 64),
        (SOXSchema::SkillPointTable, 100, 8, 64),
        (SOXSchema::SpecialNames, 100, 4, 64),
        (SOXSchema::TroopInfo, 100, 148, 64),
        (SOXSchema::UnitUVInfo, 100, 36, 64),
        (SOXSchema::UnitUVID, 100, 72, 64),
        (SOXSchema::WorldmapCharInfo, 100, 28, 64),
        (SOXSchema::WorldmapTroopInfo, 100, 28, 64),
    ];

    for (schema, marker, minimum_record_bytes, footer_bytes) in cases {
        let mut bytes = Vec::new();
        push_u32(&mut bytes, marker);
        push_u32(&mut bytes, 2);
        bytes.resize(bytes.len() + minimum_record_bytes, 0);
        match footer_bytes {
            0 => {}
            64 => push_footer(&mut bytes),
            _ => unreachable!("fixture table contains an unsupported footer size"),
        }

        let error = SchemaDocument::parse(schema, bytes).unwrap_err();

        assert!(
            matches!(
                error,
                FormatError::SchemaParse {
                    schema: actual_schema,
                    offset: 8,
                    source: GeneratedSOXError::InvalidLength {
                        field: "records",
                        value: 2,
                    },
                } if actual_schema == schema
            ),
            "{schema}: {error}"
        );
    }
}

#[test]
fn odd_ascii_hex_length_remains_typed() {
    let error = SchemaDocument::parse(
        SOXSchema::ItemTypeInfo,
        b"020000000100000000000000000000000".to_vec(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::OddASCIIHexLength { length: 33 }
    ));
}

#[test]
fn invalid_ascii_hex_byte_remains_typed() {
    let error = SchemaDocument::parse(
        SOXSchema::ItemTypeInfo,
        b"0200000001000000Z00000000000000000".to_vec(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::InvalidASCIIHexByte { index: 16 }
    ));
}

#[test]
fn automatic_detection_still_returns_only_the_three_existing_document_kinds() {
    let troop = parse_sox(standard_fixture(148)).unwrap();
    let skill = parse_sox(standard_fixture(16)).unwrap();
    let text = parse_sox(text_fixture()).unwrap();

    assert!(matches!(troop, SOXDocument::Troop(_)));
    assert!(matches!(skill, SOXDocument::Skill(_)));
    assert!(matches!(text, SOXDocument::Text(_)));
}
