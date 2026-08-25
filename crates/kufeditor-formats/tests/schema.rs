#![allow(
    clippy::unwrap_used,
    reason = "literal fixtures use statically valid sizes and expected successful parses"
)]

use std::collections::HashSet;

use kufeditor_formats::{
    FormatError, GeneratedSoxError, SchemaDocument, SoxDocument, SoxSchema, SpecialNameRef,
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
    let schemas: HashSet<_> = SoxSchema::ALL.into_iter().collect();

    assert_eq!(SoxSchema::ALL.len(), 18);
    assert_eq!(schemas.len(), 18);
}

#[test]
fn schemas_expose_canonical_stems_markers_and_display_names() {
    let cases = [
        (SoxSchema::AbilityByJob, "AbilityByJob", 100),
        (SoxSchema::AbilityInfo, "AbilityInfo", 100),
        (SoxSchema::CharInfo, "CharInfo", 100),
        (SoxSchema::CustomRandomTable, "KUF2CustomRandomTable", 100),
        (SoxSchema::ItemAttInfo, "ItemAttInfo", 100),
        (SoxSchema::ItemTypeInfo, "ItemTypeInfo", 2),
        (SoxSchema::JobInfo, "JobInfo", 100),
        (SoxSchema::LeaderGeneration, "LeaderGeneration", 100),
        (SoxSchema::LibraryInfo, "LibraryInfo", 100),
        (SoxSchema::ResistInfo, "ResistInfo", 100),
        (SoxSchema::SkillInfo, "SkillInfo", 100),
        (SoxSchema::SkillPointTable, "SkillPointTable", 100),
        (SoxSchema::SpecialNames, "SpecialNames", 100),
        (SoxSchema::TroopInfo, "TroopInfo", 100),
        (SoxSchema::UnitUvInfo, "UnitUVInfo", 100),
        (SoxSchema::UnitUvid, "UnitUVID", 100),
        (SoxSchema::WorldmapCharInfo, "WorldMap_CharInfo", 100),
        (SoxSchema::WorldmapTroopInfo, "WorldMap_TroopInfo", 100),
    ];

    for (schema, stem, marker) in cases {
        assert_eq!(schema.file_stem(), stem);
        assert_eq!(schema.marker(), marker);
        assert_eq!(schema.to_string(), stem);
    }
}

struct SchemaFixture {
    schema: SoxSchema,
    bytes: Vec<u8>,
    record_count: usize,
}

fn schema_fixtures() -> Vec<SchemaFixture> {
    [
        (SoxSchema::AbilityByJob, standard_fixture(24), 1),
        (SoxSchema::AbilityInfo, standard_fixture(64), 1),
        (SoxSchema::CharInfo, standard_fixture(136), 1),
        (
            SoxSchema::CustomRandomTable,
            custom_random_table_fixture(),
            9,
        ),
        (SoxSchema::ItemAttInfo, standard_fixture(12), 1),
        (SoxSchema::ItemTypeInfo, item_type_info_fixture(), 1),
        (SoxSchema::JobInfo, standard_fixture(72), 1),
        (SoxSchema::LeaderGeneration, standard_fixture(72), 1),
        (SoxSchema::LibraryInfo, standard_fixture(6), 1),
        (SoxSchema::ResistInfo, standard_fixture(12), 1),
        (SoxSchema::SkillInfo, standard_fixture(16), 1),
        (SoxSchema::SkillPointTable, standard_fixture(8), 1),
        (SoxSchema::SpecialNames, standard_fixture(4), 1),
        (SoxSchema::TroopInfo, standard_fixture(148), 1),
        (SoxSchema::UnitUvInfo, standard_fixture(36), 1),
        (SoxSchema::UnitUvid, standard_fixture(72), 1),
        (SoxSchema::WorldmapCharInfo, standard_fixture(28), 1),
        (SoxSchema::WorldmapTroopInfo, standard_fixture(28), 1),
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

    let document = SchemaDocument::parse(SoxSchema::SpecialNames, bytes.clone()).unwrap();

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
        SchemaDocument::parse(SoxSchema::SpecialNames, special_names_fixture()).unwrap();
    let ability_by_job =
        SchemaDocument::parse(SoxSchema::AbilityByJob, standard_fixture(24)).unwrap();

    assert_eq!(special_names.special_name(2), None);
    assert_eq!(ability_by_job.special_name(0), None);
}

#[test]
fn special_name_preserves_mixed_case_ascii_hex_source_and_canonicalizes_case() {
    let raw = special_names_fixture();
    let encoded = mixed_case_ascii_hex(&raw);
    assert!(encoded.iter().any(u8::is_ascii_lowercase));

    let document = SchemaDocument::parse(SoxSchema::SpecialNames, encoded.clone()).unwrap();
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

    let document = SchemaDocument::parse(SoxSchema::AbilityByJob, encoded.clone()).unwrap();

    assert_eq!(document.encode(), encoded);
    assert_eq!(document.canonical_encode().unwrap(), ascii_hex(&raw));
}

#[test]
fn mixed_case_marker_two_ascii_hex_preserves_exact_source() {
    let mut raw = item_type_info_fixture();
    *raw.get_mut(8).unwrap() = 0xab;
    let encoded = mixed_case_ascii_hex(&raw);

    let document = SchemaDocument::parse(SoxSchema::ItemTypeInfo, encoded.clone()).unwrap();

    assert_eq!(document.encode(), encoded);
    assert_eq!(document.canonical_encode().unwrap(), ascii_hex(&raw));
}

#[test]
fn canonical_ascii_hex_uses_only_uppercase_hexadecimal_digits() {
    let mut raw = standard_fixture(24);
    *raw.get_mut(8).unwrap() = 0xab;
    let document =
        SchemaDocument::parse(SoxSchema::AbilityByJob, mixed_case_ascii_hex(&raw)).unwrap();

    let canonical = document.canonical_encode().unwrap();

    assert_eq!(canonical, ascii_hex(&raw));
    assert!(!canonical.iter().any(u8::is_ascii_lowercase));
}

#[test]
fn standard_schema_preserves_bytes_after_the_footer() {
    let mut bytes = standard_fixture(24);
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let document = SchemaDocument::parse(SoxSchema::AbilityByJob, bytes.clone()).unwrap();

    assert_eq!(document.encode(), bytes);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn item_type_info_preserves_bytes_after_the_generated_range() {
    let mut bytes = item_type_info_fixture();
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let document = SchemaDocument::parse(SoxSchema::ItemTypeInfo, bytes.clone()).unwrap();

    assert_eq!(document.encode(), bytes);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn fixed_custom_random_table_ignores_the_header_count() {
    let bytes = custom_random_table_fixture();

    let document = SchemaDocument::parse(SoxSchema::CustomRandomTable, bytes.clone()).unwrap();

    assert_eq!(document.record_count(), 9);
    assert_eq!(document.canonical_encode().unwrap(), bytes);
}

#[test]
fn truncated_custom_random_table_stops_at_the_fixed_layout_preflight() {
    let mut bytes = custom_random_table_fixture();
    bytes.pop();

    let error = SchemaDocument::parse(SoxSchema::CustomRandomTable, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::SchemaParse {
            schema: SoxSchema::CustomRandomTable,
            source: GeneratedSoxError::UnexpectedEof { .. },
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

    let error = SchemaDocument::parse(SoxSchema::AbilityByJob, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::SchemaParse {
            schema: SoxSchema::AbilityByJob,
            offset: 4,
            source: GeneratedSoxError::Validation {
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

    let error = SchemaDocument::parse(SoxSchema::AbilityByJob, bytes).unwrap_err();

    assert!(matches!(
        error,
        FormatError::SchemaParse {
            schema: SoxSchema::AbilityByJob,
            offset: 8,
            source: GeneratedSoxError::InvalidLength {
                field: "records",
                value,
            },
        } if value == i128::from(u32::MAX)
    ));
}

#[test]
fn header_driven_preflights_reject_two_records_with_one_minimum_body() {
    let cases = [
        (SoxSchema::AbilityByJob, 100, 24, 64),
        (SoxSchema::AbilityInfo, 100, 64, 64),
        (SoxSchema::CharInfo, 100, 136, 64),
        (SoxSchema::ItemAttInfo, 100, 12, 64),
        (SoxSchema::ItemTypeInfo, 2, 178, 0),
        (SoxSchema::JobInfo, 100, 72, 64),
        (SoxSchema::LeaderGeneration, 100, 72, 64),
        (SoxSchema::LibraryInfo, 100, 6, 64),
        (SoxSchema::ResistInfo, 100, 12, 64),
        (SoxSchema::SkillInfo, 100, 16, 64),
        (SoxSchema::SkillPointTable, 100, 8, 64),
        (SoxSchema::SpecialNames, 100, 4, 64),
        (SoxSchema::TroopInfo, 100, 148, 64),
        (SoxSchema::UnitUvInfo, 100, 36, 64),
        (SoxSchema::UnitUvid, 100, 72, 64),
        (SoxSchema::WorldmapCharInfo, 100, 28, 64),
        (SoxSchema::WorldmapTroopInfo, 100, 28, 64),
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
                    source: GeneratedSoxError::InvalidLength {
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
        SoxSchema::ItemTypeInfo,
        b"020000000100000000000000000000000".to_vec(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::OddAsciiHexLength { length: 33 }
    ));
}

#[test]
fn invalid_ascii_hex_byte_remains_typed() {
    let error = SchemaDocument::parse(
        SoxSchema::ItemTypeInfo,
        b"0200000001000000Z00000000000000000".to_vec(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        FormatError::InvalidAsciiHexByte { index: 16 }
    ));
}

#[test]
fn automatic_detection_still_returns_only_the_three_existing_document_kinds() {
    let troop = parse_sox(standard_fixture(148)).unwrap();
    let skill = parse_sox(standard_fixture(16)).unwrap();
    let text = parse_sox(text_fixture()).unwrap();

    assert!(matches!(troop, SoxDocument::Troop(_)));
    assert!(matches!(skill, SoxDocument::Skill(_)));
    assert!(matches!(text, SoxDocument::Text(_)));
}
