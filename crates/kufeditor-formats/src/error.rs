use thiserror::Error;

use crate::{
    diagnostic::DiagnosticField,
    generated::{
        sox_ability_by_job, sox_ability_info, sox_char_info, sox_custom_random_table,
        sox_item_att_info, sox_item_type_info, sox_job_info, sox_leader_generation,
        sox_library_info, sox_resist_info, sox_skill_info, sox_skill_point_table,
        sox_special_names, sox_troop_info, sox_unit_uv_info, sox_unit_uvid, sox_worldmap_char_info,
        sox_worldmap_troop_info,
    },
    schema::SoxSchema,
    skill::SkillTextField,
    string_table::SoxStringTableLayout,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GeneratedSoxError {
    #[error("unexpected end of input at offset {offset}: need {needed} bytes, have {remaining}")]
    UnexpectedEof {
        offset: usize,
        needed: usize,
        remaining: usize,
    },

    #[error("invalid {enum_name} value {value}")]
    InvalidEnum {
        enum_name: &'static str,
        value: i128,
    },

    #[error("unknown tag {value} for {struct_name}.{field}")]
    UnknownTag {
        struct_name: &'static str,
        field: &'static str,
        value: i128,
    },

    #[error("validation {id} failed for {field}: {message}")]
    Validation {
        id: &'static str,
        message: &'static str,
        field: &'static str,
    },

    #[error("unsupported encoding {encoding}")]
    UnsupportedEncoding { encoding: &'static str },

    #[error("invalid {encoding} text")]
    InvalidEncoding { encoding: &'static str },

    #[error("invalid regular expression {pattern:?}: {message}")]
    InvalidRegex { pattern: String, message: String },

    #[error("invalid length {value} for {field}")]
    InvalidLength { field: &'static str, value: i128 },

    #[error("length {value} for {field} does not fit {target}")]
    LengthOverflow {
        field: &'static str,
        value: String,
        target: &'static str,
    },

    #[error("{field} has length {actual}, expected {expected}")]
    FixedSize {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("payload for {field} does not match tag {tag}")]
    MatchType { field: &'static str, tag: i128 },

    #[error("parsing {field} made no progress at offset {offset}")]
    NoProgress { field: &'static str, offset: usize },
}

macro_rules! impl_generated_sox_error {
    ($($module:ident),+ $(,)?) => {
        $(
            impl From<$module::Error> for GeneratedSoxError {
                fn from(error: $module::Error) -> Self {
                    match error {
                        $module::Error::UnexpectedEof {
                            offset,
                            needed,
                            remaining,
                        } => Self::UnexpectedEof {
                            offset,
                            needed,
                            remaining,
                        },
                        $module::Error::InvalidEnum { enum_name, value } => {
                            Self::InvalidEnum { enum_name, value }
                        }
                        $module::Error::UnknownTag {
                            struct_name,
                            field,
                            value,
                        } => Self::UnknownTag {
                            struct_name,
                            field,
                            value,
                        },
                        $module::Error::Validation { id, message, field } => {
                            Self::Validation { id, message, field }
                        }
                        $module::Error::UnsupportedEncoding { encoding } => {
                            Self::UnsupportedEncoding { encoding }
                        }
                        $module::Error::InvalidEncoding { encoding } => {
                            Self::InvalidEncoding { encoding }
                        }
                        $module::Error::InvalidRegex { pattern, message } => {
                            Self::InvalidRegex { pattern, message }
                        }
                        $module::Error::InvalidLength { field, value } => {
                            Self::InvalidLength { field, value }
                        }
                        $module::Error::LengthOverflow {
                            field,
                            value,
                            target,
                        } => Self::LengthOverflow {
                            field,
                            value,
                            target,
                        },
                        $module::Error::FixedSize {
                            field,
                            expected,
                            actual,
                        } => Self::FixedSize {
                            field,
                            expected,
                            actual,
                        },
                        $module::Error::MatchType { field, tag } => {
                            Self::MatchType { field, tag }
                        }
                        $module::Error::NoProgress { field, offset } => {
                            Self::NoProgress { field, offset }
                        }
                    }
                }
            }
        )+
    };
}

impl_generated_sox_error!(
    sox_ability_by_job,
    sox_ability_info,
    sox_char_info,
    sox_custom_random_table,
    sox_item_att_info,
    sox_item_type_info,
    sox_job_info,
    sox_leader_generation,
    sox_library_info,
    sox_resist_info,
    sox_skill_info,
    sox_skill_point_table,
    sox_special_names,
    sox_troop_info,
    sox_unit_uv_info,
    sox_unit_uvid,
    sox_worldmap_char_info,
    sox_worldmap_troop_info,
);

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

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StringTableParseError {
    #[error("string-table header is truncated: have {actual} bytes")]
    TruncatedHeader { actual: usize },

    #[error("string-table marker {marker} is invalid")]
    InvalidMarker { marker: u32 },

    #[error(
        "record count {count} cannot fit the source: each record needs at least {minimum_record_size} bytes, and {remaining} bytes remain"
    )]
    ImpossibleRecordCount {
        count: u32,
        minimum_record_size: usize,
        remaining: usize,
    },

    #[error("stored ID for record {record} is truncated: {remaining} bytes remain")]
    TruncatedStoredId { record: usize, remaining: usize },

    #[error("length for record {record} field {field} is truncated: {remaining} bytes remain")]
    TruncatedFieldLength {
        record: usize,
        field: usize,
        remaining: usize,
    },

    #[error(
        "payload for record {record} field {field} is truncated: declared {length} bytes, and {remaining} bytes remain"
    )]
    TruncatedFieldPayload {
        record: usize,
        field: usize,
        length: u16,
        remaining: usize,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StringTableEncodeError {
    #[error("record count {count} exceeds the maximum {maximum}")]
    RecordCountOverflow { count: usize, maximum: u32 },

    #[error("record {record} field {field} length {length} exceeds the maximum {maximum}")]
    FieldLengthOverflow {
        record: usize,
        field: usize,
        length: usize,
        maximum: u16,
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
    #[error("SOX input is neither a TroopInfo, SkillInfo, nor text SOX document")]
    UnsupportedSox,

    #[error("ASCII-hex SOX input has an odd length of {length} bytes")]
    OddAsciiHexLength { length: usize },

    #[error("ASCII-hex SOX input has a non-hexadecimal byte at index {index}")]
    InvalidAsciiHexByte { index: usize },

    #[error("saved SOX source image has an inconsistent encoding envelope")]
    InconsistentSoxRebase,

    #[error("failed to parse {layout} string table at offset {offset}: {source}")]
    StringTableParse {
        layout: SoxStringTableLayout,
        offset: usize,
        #[source]
        source: StringTableParseError,
    },

    #[error("failed to encode {layout} string table: {source}")]
    StringTableEncode {
        layout: SoxStringTableLayout,
        #[source]
        source: StringTableEncodeError,
    },

    #[error("{layout} string-table record {record} is outside the record count {record_count}")]
    StringTableRecordOutOfRange {
        layout: SoxStringTableLayout,
        record: usize,
        record_count: usize,
    },

    #[error(
        "{layout} string-table record {record} field {field} is outside the field count {field_count}"
    )]
    StringTableFieldOutOfRange {
        layout: SoxStringTableLayout,
        record: usize,
        field: usize,
        field_count: usize,
    },

    #[error("failed to parse {schema} at offset {offset}: {source}")]
    SchemaParse {
        schema: SoxSchema,
        offset: usize,
        #[source]
        source: GeneratedSoxError,
    },

    #[error("failed to encode {schema}: {source}")]
    SchemaEncode {
        schema: SoxSchema,
        #[source]
        source: GeneratedSoxError,
    },

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
