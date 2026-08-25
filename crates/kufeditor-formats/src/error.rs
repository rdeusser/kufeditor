use thiserror::Error;

use crate::{
    diagnostic::DiagnosticField,
    generated::{sox_skill_info, sox_troop_info},
    skill::SkillTextField,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TextSoxParseError {
    #[error("text SOX header is truncated")]
    TruncatedHeader { actual: usize },

    #[error("text SOX marker is invalid")]
    InvalidMarker { marker: u32 },

    #[error("text SOX record count is invalid")]
    InvalidRecordCount { count: u32 },

    #[error("text SOX record count cannot fit the source")]
    ImpossibleRecordCount { count: u32, maximum: usize },

    #[error("text SOX record header is truncated")]
    TruncatedRecordHeader { record: usize, remaining: usize },

    #[error("text SOX text is empty")]
    EmptyText { record: usize },

    #[error("text SOX text payload is truncated")]
    TruncatedText {
        record: usize,
        length: u16,
        remaining: usize,
    },

    #[error("text SOX text contains an unsupported byte")]
    InvalidTextByte {
        record: usize,
        index: usize,
        byte: u8,
    },
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct TroopCleaveError(#[from] sox_troop_info::Error);

#[derive(Debug, Error)]
#[error(transparent)]
pub struct SkillCleaveError(#[from] sox_skill_info::Error);

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("SOX input is neither a TroopInfo nor SkillInfo document")]
    UnsupportedSox,

    #[error("ASCII-hex SOX input has an odd length of {length} bytes")]
    OddAsciiHexLength { length: usize },

    #[error("ASCII-hex SOX input has a non-hexadecimal byte at index {index}")]
    InvalidAsciiHexByte { index: usize },

    #[error("saved SOX source image has an inconsistent encoding envelope")]
    InconsistentSoxRebase,

    #[error("failed to parse text SOX at offset {offset}: {source}")]
    TextSoxParse {
        offset: usize,
        #[source]
        source: TextSoxParseError,
    },

    #[error("text SOX record {record} is empty")]
    TextSoxEmptyText { record: usize },

    #[error("text SOX record {record} exceeds its byte budget")]
    TextSoxTooLong {
        record: usize,
        length: usize,
        maximum: u16,
    },

    #[error("text SOX record {record} contains an unsupported byte")]
    TextSoxInvalidTextByte {
        record: usize,
        index: usize,
        byte: u8,
    },

    #[error("failed to parse TroopInfo at offset {offset}: {source}")]
    TroopParse {
        offset: usize,
        #[source]
        source: TroopCleaveError,
    },

    #[error("failed to encode TroopInfo: {0}")]
    TroopEncode(#[source] TroopCleaveError),

    #[error("failed to parse SkillInfo at offset {offset}: {source}")]
    SkillParse {
        offset: usize,
        #[source]
        source: SkillCleaveError,
    },

    #[error("failed to encode SkillInfo: {0}")]
    SkillEncode(#[source] SkillCleaveError),

    #[error("record {record} field {field} is outside the record count {record_count}")]
    RecordOutOfRange {
        record: usize,
        record_count: usize,
        field: DiagnosticField,
    },

    #[error("SkillInfo record {record} field {field} is not valid UTF-8: {source}")]
    SkillUtf8 {
        record: usize,
        field: SkillTextField,
        #[source]
        source: std::str::Utf8Error,
    },
}
