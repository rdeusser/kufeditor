#![allow(
    clippy::unwrap_used,
    reason = "literal test fixtures have wire-field lengths checked at construction"
)]

use kufeditor_formats::{
    DiagnosticField, FormatError, Severity, TextSOXDocument, TextSOXField, TextSOXParseError,
};

fn text_fixture(records: &[(u32, &[u8])], tail: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(records.len()).unwrap().to_le_bytes());

    for &(index, text) in records {
        bytes.extend_from_slice(&index.to_le_bytes());
        bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(text);
    }

    bytes.extend_from_slice(tail);
    bytes
}

fn ascii_hex(bytes: &[u8], mixed_case: bool) -> Vec<u8> {
    const UPPERCASE: &[u8; 16] = b"0123456789ABCDEF";
    const LOWERCASE: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = Vec::with_capacity(bytes.len() * 2);
    for (index, &byte) in bytes.iter().enumerate() {
        let digits = if mixed_case && index % 2 == 1 {
            LOWERCASE
        } else {
            UPPERCASE
        };
        encoded.push(*digits.get(usize::from(byte >> 4)).unwrap());
        encoded.push(*digits.get(usize::from(byte & 0x0f)).unwrap());
    }
    encoded
}

fn parse_error(bytes: Vec<u8>) -> (usize, TextSOXParseError) {
    match TextSOXDocument::parse(bytes).unwrap_err() {
        FormatError::TextSOXParse { offset, source } => (offset, source),
        error => panic!("expected text SOX parse error, got {error:?}"),
    }
}

#[test]
fn parses_a_record_with_its_stored_index_and_initial_byte_budget() {
    let document = TextSOXDocument::parse(text_fixture(&[(42, b"Hello")], b"TAIL")).unwrap();

    assert_eq!(document.record_count(), 1);
    assert_eq!(document.record_index(0).unwrap(), 42);
    assert_eq!(document.max_length(0).unwrap(), 5);
    assert_eq!(document.text(0).unwrap(), "Hello");
}

#[test]
fn preserves_sparse_out_of_order_stored_indices() {
    let document = TextSOXDocument::parse(text_fixture(
        &[(900, b"Alpha"), (2, b"Beta"), (50_000, b"Gamma")],
        b"",
    ))
    .unwrap();

    assert_eq!(document.record_index(0).unwrap(), 900);
    assert_eq!(document.record_index(1).unwrap(), 2);
    assert_eq!(document.record_index(2).unwrap(), 50_000);
}

#[test]
fn unchanged_raw_document_encodes_to_the_exact_original_bytes_including_the_tail() {
    let source = text_fixture(&[(4, b"Alpha"), (9, b"Beta")], &[0xde, 0xad, 0xbe, 0xef]);
    let document = TextSOXDocument::parse(source.clone()).unwrap();

    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn unchanged_mixed_case_ascii_hex_document_encodes_to_the_exact_original_bytes() {
    let raw = text_fixture(&[(12, b"Alpha")], b"TAIL");
    let source = ascii_hex(&raw, true);
    let document = TextSOXDocument::parse(source.clone()).unwrap();

    assert!(source.iter().any(u8::is_ascii_lowercase));
    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn rejects_a_marker_other_than_100() {
    let mut source = text_fixture(&[(0, b"A")], b"");
    source
        .get_mut(..4)
        .unwrap()
        .copy_from_slice(&99_u32.to_le_bytes());

    assert_eq!(
        parse_error(source),
        (0, TextSOXParseError::InvalidMarker { marker: 99 })
    );
}

#[test]
fn rejects_zero_and_excessive_record_counts() {
    let zero = text_fixture(&[], b"");
    let mut excessive = text_fixture(&[(0, b"A")], b"");
    excessive
        .get_mut(4..8)
        .unwrap()
        .copy_from_slice(&10_001_u32.to_le_bytes());

    assert_eq!(
        parse_error(zero),
        (4, TextSOXParseError::InvalidRecordCount { count: 0 })
    );
    assert_eq!(
        parse_error(excessive),
        (4, TextSOXParseError::InvalidRecordCount { count: 10_001 })
    );
}

#[test]
fn rejects_an_impossible_record_count_before_allocating_record_storage() {
    let mut source = text_fixture(&[], b"");
    source
        .get_mut(4..8)
        .unwrap()
        .copy_from_slice(&10_000_u32.to_le_bytes());

    assert_eq!(
        parse_error(source),
        (
            8,
            TextSOXParseError::ImpossibleRecordCount {
                count: 10_000,
                maximum: 0,
            },
        )
    );
}

#[test]
fn rejects_a_short_header() {
    assert_eq!(
        parse_error(vec![100, 0, 0]),
        (3, TextSOXParseError::TruncatedHeader { actual: 3 })
    );
}

#[test]
fn rejects_a_truncated_record_header() {
    let mut source = text_fixture(&[], b"");
    source
        .get_mut(4..8)
        .unwrap()
        .copy_from_slice(&1_u32.to_le_bytes());
    source.extend_from_slice(&[1, 0, 0, 0, 2]);

    assert_eq!(
        parse_error(source),
        (
            8,
            TextSOXParseError::TruncatedRecordHeader {
                record: 0,
                remaining: 5,
            },
        )
    );
}

#[test]
fn rejects_empty_text() {
    assert_eq!(
        parse_error(text_fixture(&[(1, b"")], b"")),
        (14, TextSOXParseError::EmptyText { record: 0 })
    );
}

#[test]
fn rejects_a_truncated_text_payload() {
    let mut source = text_fixture(&[], b"");
    source
        .get_mut(4..8)
        .unwrap()
        .copy_from_slice(&1_u32.to_le_bytes());
    source.extend_from_slice(&7_u32.to_le_bytes());
    source.extend_from_slice(&5_u16.to_le_bytes());
    source.extend_from_slice(b"four");

    assert_eq!(
        parse_error(source),
        (
            14,
            TextSOXParseError::TruncatedText {
                record: 0,
                length: 5,
                remaining: 4,
            },
        )
    );
}

#[test]
fn rejects_a_text_byte_outside_the_supported_ascii_set() {
    assert_eq!(
        parse_error(text_fixture(&[(1, b"A\x1fB")], b"")),
        (
            15,
            TextSOXParseError::InvalidTextByte {
                record: 0,
                index: 1,
                byte: 0x1f,
            },
        )
    );
}

#[test]
fn edits_within_the_original_byte_budget_and_preserves_indices_and_tail() {
    let source = text_fixture(&[(900, b"Alpha"), (2, b"Bravo")], b"TAIL");
    let mut document = TextSOXDocument::parse(source).unwrap();

    assert_eq!(document.set_text(0, "A".to_owned()).unwrap(), "Alpha");
    assert_eq!(document.set_text(1, "12345".to_owned()).unwrap(), "Bravo");

    let encoded = document.encode().unwrap();
    assert_eq!(
        encoded,
        text_fixture(&[(900, b"A"), (2, b"12345")], b"TAIL")
    );
    let reparsed = TextSOXDocument::parse(encoded).unwrap();
    assert_eq!(reparsed.record_index(0).unwrap(), 900);
    assert_eq!(reparsed.record_index(1).unwrap(), 2);
    assert_eq!(reparsed.text(0).unwrap(), "A");
    assert_eq!(reparsed.text(1).unwrap(), "12345");
}

#[test]
fn edited_ascii_hex_document_uses_uppercase_hexadecimal_and_reparses() {
    let raw = text_fixture(&[(0, b"Alpha")], b"TAIL");
    let mut document = TextSOXDocument::parse(ascii_hex(&raw, true)).unwrap();

    document.set_text(0, "Bravo".to_owned()).unwrap();
    let encoded = document.encode().unwrap();

    assert_eq!(
        encoded,
        ascii_hex(&text_fixture(&[(0, b"Bravo")], b"TAIL"), false)
    );
    assert!(encoded.iter().all(|byte| !byte.is_ascii_lowercase()));
    assert_eq!(
        TextSOXDocument::parse(encoded).unwrap().text(0).unwrap(),
        "Bravo"
    );
}

#[test]
fn rejects_invalid_edits_without_mutating_the_text_or_encoded_bytes() {
    let source = text_fixture(&[(0, b"Alpha")], b"TAIL");
    let mut document = TextSOXDocument::parse(source.clone()).unwrap();

    assert!(matches!(
        document.set_text(0, String::new()).unwrap_err(),
        FormatError::TextSOXEmptyText { record: 0 }
    ));
    assert!(matches!(
        document.set_text(0, "123456".to_owned()).unwrap_err(),
        FormatError::TextSOXTooLong {
            record: 0,
            length: 6,
            maximum: 5,
        }
    ));
    assert!(matches!(
        document.set_text(0, "é".to_owned()).unwrap_err(),
        FormatError::TextSOXInvalidTextByte {
            record: 0,
            index: 0,
            byte: 0xc3,
        }
    ));
    assert_eq!(document.text(0).unwrap(), "Alpha");
    assert_eq!(document.encode().unwrap(), source);
}

#[test]
fn reports_duplicate_stored_indices_and_ignores_sparse_unique_indices() {
    let duplicate = TextSOXDocument::parse(text_fixture(&[(4, b"A"), (4, b"B")], b"")).unwrap();
    let unique = TextSOXDocument::parse(text_fixture(&[(4, b"A"), (99, b"B")], b"")).unwrap();

    assert_eq!(
        duplicate.diagnostics(),
        vec![
            kufeditor_formats::Diagnostic {
                severity: Severity::Warning,
                record: 0,
                field: DiagnosticField::TextSOX(TextSOXField::Index),
                message: "Stored index is duplicated",
            },
            kufeditor_formats::Diagnostic {
                severity: Severity::Warning,
                record: 1,
                field: DiagnosticField::TextSOX(TextSOXField::Index),
                message: "Stored index is duplicated",
            },
        ]
    );
    assert!(unique.diagnostics().is_empty());
}

#[test]
fn out_of_range_projections_name_the_text_sox_field() {
    let document = TextSOXDocument::parse(text_fixture(&[(0, b"A")], b"")).unwrap();

    assert!(matches!(
        document.record_index(1).unwrap_err(),
        FormatError::RecordOutOfRange {
            record: 1,
            record_count: 1,
            field: DiagnosticField::TextSOX(TextSOXField::Index),
        }
    ));
    assert!(matches!(
        document.text(1).unwrap_err(),
        FormatError::RecordOutOfRange {
            record: 1,
            record_count: 1,
            field: DiagnosticField::TextSOX(TextSOXField::Text),
        }
    ));
}

#[test]
fn rebase_accepts_only_the_exact_saved_snapshot_and_retains_the_initial_budget() {
    let source = text_fixture(&[(7, b"abcdefgh")], b"TAIL");
    let mut saved = TextSOXDocument::parse(source.clone()).unwrap();
    saved.set_text(0, "xyz".to_owned()).unwrap();
    let saved_bytes = saved.encode().unwrap();
    let mut live = TextSOXDocument::parse(source).unwrap();

    assert!(matches!(
        live.rebase_source(&saved, b"not the saved snapshot".to_vec())
            .unwrap_err(),
        FormatError::InconsistentSOXRebase
    ));
    assert_eq!(live.text(0).unwrap(), "abcdefgh");

    live.rebase_source(&saved, saved_bytes.clone()).unwrap();
    assert_eq!(live.max_length(0).unwrap(), 8);
    assert_eq!(live.text(0).unwrap(), "abcdefgh");
    assert_eq!(live.set_text(0, "12345678".to_owned()).unwrap(), "abcdefgh");
    assert_eq!(
        live.encode().unwrap(),
        text_fixture(&[(7, b"12345678")], b"TAIL")
    );
}
