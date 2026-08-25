use thiserror::Error;

use crate::{
    diagnostic::DiagnosticField,
    generated::{sox_skill_info, sox_troop_info},
    skill::SkillTextField,
};

#[derive(Debug, Error)]
#[error(transparent)]
pub struct TroopCleaveError(#[from] sox_troop_info::Error);

#[derive(Debug, Error)]
#[error(transparent)]
pub struct SkillCleaveError(#[from] sox_skill_info::Error);

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("ASCII-hex SOX input has an odd length of {length} bytes")]
    OddAsciiHexLength { length: usize },

    #[error("ASCII-hex SOX input has a non-hexadecimal byte at index {index}")]
    InvalidAsciiHexByte { index: usize },

    #[error("saved SOX source image has an inconsistent encoding envelope")]
    InconsistentSoxRebase,

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
