#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use lengths that fit their wire fields"
)]

use kufeditor_formats::{
    DiagnosticField, DiagnosticLocation, FormatError, Severity, SkillDocument, SkillField,
    SkillTextField, TroopField,
};

#[derive(Clone, Copy)]
struct SkillFixture<'a> {
    skill_id: i32,
    localization_key: &'a [u8],
    icon_path: &'a [u8],
    skill_type: u32,
    max_level: u32,
}

fn skill_fixture(records: &[SkillFixture<'_>], trailing: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, u32::try_from(records.len()).unwrap());

    for record in records {
        push_i32(&mut bytes, record.skill_id);
        push_bytes(&mut bytes, record.localization_key);
        push_bytes(&mut bytes, record.icon_path);
        push_u32(&mut bytes, record.skill_type);
        push_u32(&mut bytes, record.max_level);
    }

    bytes.extend_from_slice(b"THEND");
    bytes.resize(bytes.len() + 59, b' ');
    bytes.extend_from_slice(trailing);
    bytes
}

fn push_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(value);
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn uppercase_ascii_hex(bytes: &[u8]) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(*DIGITS.get(usize::from(byte >> 4)).unwrap());
        encoded.push(*DIGITS.get(usize::from(byte & 0x0f)).unwrap());
    }
    encoded
}

fn mixed_case_ascii_hex(bytes: &[u8]) -> Vec<u8> {
    uppercase_ascii_hex(bytes)
        .into_iter()
        .enumerate()
        .map(|(index, byte)| {
            if index % 3 == 0 {
                byte.to_ascii_lowercase()
            } else {
                byte
            }
        })
        .collect()
}

fn melee_fixture(trailing: &[u8]) -> Vec<u8> {
    skill_fixture(
        &[SkillFixture {
            skill_id: 0,
            localization_key: b"@(S_Melee)",
            icon_path: b"IL_SKL_Melee.tga",
            skill_type: 1,
            max_level: 50,
        }],
        trailing,
    )
}

#[test]
fn parses_header_and_all_five_skill_fields() {
    let document = SkillDocument::parse(melee_fixture(&[])).unwrap();

    assert_eq!(document.record_count(), 1);
    assert_eq!(document.skill_id(0).unwrap(), 0);
    assert_eq!(
        document.text(0, SkillTextField::LocalizationKey).unwrap(),
        "@(S_Melee)"
    );
    assert_eq!(
        document.text(0, SkillTextField::IconPath).unwrap(),
        "IL_SKL_Melee.tga"
    );
    assert_eq!(document.skill_type(0).unwrap(), 1);
    assert_eq!(document.max_level(0).unwrap(), 50);
}

#[test]
fn preserves_a_negative_skill_id() {
    let source = skill_fixture(
        &[SkillFixture {
            skill_id: -2,
            localization_key: b"@(S_Elemental)",
            icon_path: b"IL_SKL_Elem.tga",
            skill_type: 2,
            max_level: 25,
        }],
        &[],
    );
    let document = SkillDocument::parse(source.clone()).unwrap();

    assert_eq!(document.skill_id(0).unwrap(), -2);
    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn parses_empty_strings_and_multiple_records() {
    let source = skill_fixture(
        &[
            SkillFixture {
                skill_id: 0,
                localization_key: b"",
                icon_path: b"",
                skill_type: 1,
                max_level: 50,
            },
            SkillFixture {
                skill_id: 8,
                localization_key: b"@(S_Fire)",
                icon_path: b"IL_SKL_Fire.tga",
                skill_type: 2,
                max_level: 25,
            },
        ],
        &[],
    );
    let document = SkillDocument::parse(source).unwrap();

    assert_eq!(document.record_count(), 2);
    assert_eq!(
        document.text(0, SkillTextField::LocalizationKey).unwrap(),
        ""
    );
    assert_eq!(document.text(0, SkillTextField::IconPath).unwrap(), "");
    assert_eq!(document.skill_id(1).unwrap(), 8);
    assert_eq!(
        document.text(1, SkillTextField::LocalizationKey).unwrap(),
        "@(S_Fire)"
    );
    assert_eq!(document.skill_type(1).unwrap(), 2);
    assert_eq!(document.max_level(1).unwrap(), 25);
}

#[test]
fn rejects_truncated_data() {
    let mut truncated = melee_fixture(&[]);
    truncated.pop();
    let truncated_error = SkillDocument::parse(truncated).unwrap_err();
    assert!(matches!(truncated_error, FormatError::SkillParse { .. }));
}

#[test]
fn rejects_an_impossible_large_record_count_without_allocating() {
    let source = vec![0x64, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff];

    let error = SkillDocument::parse(source).unwrap_err();

    assert!(matches!(&error, FormatError::SkillParse { .. }));
    assert_eq!(
        error.to_string(),
        "failed to parse SkillInfo at offset 8: invalid length 4294967295 for records"
    );
}

#[test]
fn rejects_a_bad_marker() {
    let mut bad_marker = melee_fixture(&[]);
    bad_marker
        .get_mut(..4)
        .unwrap()
        .copy_from_slice(&200_u32.to_le_bytes());
    let marker_error = SkillDocument::parse(bad_marker).unwrap_err();
    assert!(matches!(marker_error, FormatError::SkillParse { .. }));
}

#[test]
fn unchanged_raw_file_encodes_to_the_exact_source_bytes() {
    let source = skill_fixture(
        &[
            SkillFixture {
                skill_id: 0,
                localization_key: b"@(S_Melee)",
                icon_path: b"IL_SKL_Melee.tga",
                skill_type: 1,
                max_level: 50,
            },
            SkillFixture {
                skill_id: 8,
                localization_key: b"@(S_Fire)",
                icon_path: b"IL_SKL_Fire.tga",
                skill_type: 2,
                max_level: 25,
            },
        ],
        &[0xde, 0xad, 0xbe, 0xef],
    );
    let document = SkillDocument::parse(source.clone()).unwrap();

    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn edits_every_field_and_preserves_the_footer_and_trailing_bytes() {
    let trailing = [0xde, 0xad, 0xbe, 0xef];
    let source = melee_fixture(&trailing);
    let mut document = SkillDocument::parse(source).unwrap();

    assert_eq!(document.set_skill_id(0, -7).unwrap(), 0);
    assert_eq!(
        document
            .set_text(
                0,
                SkillTextField::LocalizationKey,
                "@(S_Changed)".to_owned(),
            )
            .unwrap(),
        "@(S_Melee)"
    );
    assert_eq!(
        document
            .set_text(0, SkillTextField::IconPath, "changed.tga".to_owned())
            .unwrap(),
        "IL_SKL_Melee.tga"
    );
    assert_eq!(document.set_skill_type(0, 2).unwrap(), 1);
    assert_eq!(document.set_max_level(0, 65_535).unwrap(), 50);

    let encoded = document.encode().unwrap();
    let expected_suffix = skill_fixture(&[], &trailing);
    assert!(encoded.ends_with(expected_suffix.get(8..).unwrap()));

    let reparsed = SkillDocument::parse(encoded).unwrap();
    assert_eq!(reparsed.skill_id(0).unwrap(), -7);
    assert_eq!(
        reparsed.text(0, SkillTextField::LocalizationKey).unwrap(),
        "@(S_Changed)"
    );
    assert_eq!(
        reparsed.text(0, SkillTextField::IconPath).unwrap(),
        "changed.tga"
    );
    assert_eq!(reparsed.skill_type(0).unwrap(), 2);
    assert_eq!(reparsed.max_level(0).unwrap(), 65_535);
}

#[test]
fn reports_the_legacy_skill_warnings() {
    let source = skill_fixture(
        &[
            SkillFixture {
                skill_id: 0,
                localization_key: b"",
                icon_path: b"",
                skill_type: 5,
                max_level: 0,
            },
            SkillFixture {
                skill_id: 8,
                localization_key: b"@(S_Fire)",
                icon_path: b"IL_SKL_Fire.tga",
                skill_type: 2,
                max_level: 65_536,
            },
        ],
        &[],
    );
    let document = SkillDocument::parse(source).unwrap();

    let observed: Vec<_> = document
        .diagnostics()
        .into_iter()
        .map(|diagnostic| (diagnostic.severity, diagnostic.location, diagnostic.message))
        .collect();
    assert_eq!(
        observed,
        vec![
            (
                Severity::Warning,
                DiagnosticLocation::Record {
                    record: 0,
                    field: DiagnosticField::Skill(SkillField::SkillType),
                },
                "Skill type should be 1 (Combat) or 2 (Magic)",
            ),
            (
                Severity::Warning,
                DiagnosticLocation::Record {
                    record: 0,
                    field: DiagnosticField::Skill(SkillField::MaxLevel),
                },
                "Max level is 0 or exceeds 65535",
            ),
            (
                Severity::Warning,
                DiagnosticLocation::Record {
                    record: 0,
                    field: DiagnosticField::Skill(SkillField::LocalizationKey),
                },
                "Localization key is empty",
            ),
            (
                Severity::Warning,
                DiagnosticLocation::Record {
                    record: 0,
                    field: DiagnosticField::Skill(SkillField::IconPath),
                },
                "Icon path is empty",
            ),
            (
                Severity::Warning,
                DiagnosticLocation::Record {
                    record: 1,
                    field: DiagnosticField::Skill(SkillField::MaxLevel),
                },
                "Max level is 0 or exceeds 65535",
            ),
        ]
    );
}

#[test]
fn invalid_utf8_stays_unchanged_when_another_field_is_edited() {
    let source = skill_fixture(
        &[SkillFixture {
            skill_id: 0,
            localization_key: &[0xff, 0xfe],
            icon_path: b"IL_SKL_Melee.tga",
            skill_type: 1,
            max_level: 50,
        }],
        &[0xde, 0xad],
    );
    let mut document = SkillDocument::parse(source).unwrap();

    let projection_error = document
        .text(0, SkillTextField::LocalizationKey)
        .unwrap_err();
    assert!(matches!(
        projection_error,
        FormatError::SkillUTF8 {
            record: 0,
            field: SkillTextField::LocalizationKey,
            ..
        }
    ));

    let replacement_error = document
        .set_text(0, SkillTextField::LocalizationKey, "replacement".to_owned())
        .unwrap_err();
    assert!(matches!(
        replacement_error,
        FormatError::SkillUTF8 {
            record: 0,
            field: SkillTextField::LocalizationKey,
            ..
        }
    ));
    assert!(document.diagnostics().iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error
            && diagnostic.location
                == DiagnosticLocation::Record {
                    record: 0,
                    field: DiagnosticField::Skill(SkillField::LocalizationKey),
                }
    }));

    document.set_skill_type(0, 2).unwrap();
    let encoded = document.encode().unwrap();
    let expected = skill_fixture(
        &[SkillFixture {
            skill_id: 0,
            localization_key: &[0xff, 0xfe],
            icon_path: b"IL_SKL_Melee.tga",
            skill_type: 2,
            max_level: 50,
        }],
        &[0xde, 0xad],
    );
    assert_eq!(encoded, expected);
}

#[test]
fn overlong_text_fails_during_skill_encoding() {
    let mut document = SkillDocument::parse(melee_fixture(&[])).unwrap();
    let overlong = "x".repeat(usize::from(u16::MAX) + 1);

    assert_eq!(
        document
            .set_text(0, SkillTextField::LocalizationKey, overlong)
            .unwrap(),
        "@(S_Melee)"
    );
    assert!(matches!(
        document.encode().unwrap_err(),
        FormatError::SkillEncode(_)
    ));
}

#[test]
fn preserves_and_edits_an_ascii_hex_skill_fixture() {
    let raw = melee_fixture(&[0xde, 0xad]);
    let source = mixed_case_ascii_hex(&raw);
    let mut document = SkillDocument::parse(source.clone()).unwrap();

    assert_ne!(source, uppercase_ascii_hex(&raw));
    assert_eq!(document.encode().unwrap(), source);
    assert_eq!(
        document
            .set_text(0, SkillTextField::IconPath, "IL_SKL_Fire.tga".to_owned())
            .unwrap(),
        "IL_SKL_Melee.tga"
    );

    let encoded = document.encode().unwrap();
    let expected = uppercase_ascii_hex(&skill_fixture(
        &[SkillFixture {
            skill_id: 0,
            localization_key: b"@(S_Melee)",
            icon_path: b"IL_SKL_Fire.tga",
            skill_type: 1,
            max_level: 50,
        }],
        &[0xde, 0xad],
    ));
    assert_eq!(encoded, expected);

    let reparsed = SkillDocument::parse(encoded).unwrap();
    assert_eq!(
        reparsed.text(0, SkillTextField::IconPath).unwrap(),
        "IL_SKL_Fire.tga"
    );
}

#[test]
fn record_range_errors_include_the_requested_field() {
    let document = SkillDocument::parse(melee_fixture(&[])).unwrap();

    let numeric_error = document.skill_id(1).unwrap_err();
    assert!(matches!(
        numeric_error,
        FormatError::RecordOutOfRange {
            record: 1,
            record_count: 1,
            field: DiagnosticField::Skill(SkillField::SkillID),
        }
    ));

    let text_error = document.text(1, SkillTextField::IconPath).unwrap_err();
    assert!(matches!(
        text_error,
        FormatError::RecordOutOfRange {
            record: 1,
            record_count: 1,
            field: DiagnosticField::Skill(SkillField::IconPath),
        }
    ));
}

#[test]
fn diagnostic_field_labels_delegate_to_the_format_field() {
    assert_eq!(
        DiagnosticField::Troop(TroopField::MoveSpeed).label(),
        "Move Speed"
    );
    assert_eq!(
        DiagnosticField::Skill(SkillField::IconPath).label(),
        "Icon Path"
    );
}

#[test]
fn rebase_accepts_the_exact_encoded_skill_snapshot() {
    let mut saved = SkillDocument::parse(melee_fixture(&[])).unwrap();
    saved.set_max_level(0, 75).unwrap();
    let bytes = saved.encode().unwrap();
    let mut document = saved.clone();

    document.rebase_source(&saved, bytes.clone()).unwrap();

    assert_eq!(document.encode().unwrap(), bytes);
}
