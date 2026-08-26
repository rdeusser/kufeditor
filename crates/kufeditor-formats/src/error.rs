use thiserror::Error;

use crate::{
    SaveNumberTarget, SaveTextField,
    diagnostic::DiagnosticField,
    generated::{
        kuf_save, kuf_stg, sox_ability_by_job, sox_ability_info, sox_char_info,
        sox_custom_random_table, sox_item_att_info, sox_item_type_info, sox_job_info,
        sox_leader_generation, sox_library_info, sox_resist_info, sox_skill_info,
        sox_skill_point_table, sox_special_names, sox_troop_info, sox_unit_uv_info, sox_unit_uvid,
        sox_worldmap_char_info, sox_worldmap_troop_info,
    },
    schema::SOXSchema,
    skill::SkillTextField,
    stg::{
        STGFloatTarget, STGNumberTarget, STGParameterTarget, STGScriptKind, STGScriptTarget,
        STGTextTarget, STGValueTarget,
    },
    string_table::SOXStringTableLayout,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GeneratedSOXError {
    #[error("unexpected end of input at offset {offset}: need {needed} bytes, have {remaining}")]
    UnexpectedEOF {
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
            impl From<$module::Error> for GeneratedSOXError {
                fn from(error: $module::Error) -> Self {
                    match error {
                        $module::Error::UnexpectedEof {
                            offset,
                            needed,
                            remaining,
                        } => Self::UnexpectedEOF {
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
pub enum TextSOXParseError {
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
    TruncatedStoredID { record: usize, remaining: usize },

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveRegion {
    Envelope,
    Units,
    Roster,
    SecondArray,
    Missions,
}

impl std::fmt::Display for SaveRegion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Envelope => "envelope",
            Self::Units => "units",
            Self::Roster => "roster",
            Self::SecondArray => "second array",
            Self::Missions => "missions",
        })
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct SaveCleaveError(#[from] kuf_save::Error);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGCleaveErrorKind {
    UnexpectedEOF,
    InvalidEnum,
    UnknownTag,
    Validation,
    UnsupportedEncoding,
    InvalidEncoding,
    InvalidRegex,
    InvalidLength,
    LengthOverflow,
    FixedSize,
    MatchType,
    NoProgress,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum STGCleaveError {
    #[error("unexpected end of input at offset {offset}: need {needed} bytes, have {remaining}")]
    UnexpectedEOF {
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

impl STGCleaveError {
    pub const fn kind(&self) -> STGCleaveErrorKind {
        match self {
            Self::UnexpectedEOF { .. } => STGCleaveErrorKind::UnexpectedEOF,
            Self::InvalidEnum { .. } => STGCleaveErrorKind::InvalidEnum,
            Self::UnknownTag { .. } => STGCleaveErrorKind::UnknownTag,
            Self::Validation { .. } => STGCleaveErrorKind::Validation,
            Self::UnsupportedEncoding { .. } => STGCleaveErrorKind::UnsupportedEncoding,
            Self::InvalidEncoding { .. } => STGCleaveErrorKind::InvalidEncoding,
            Self::InvalidRegex { .. } => STGCleaveErrorKind::InvalidRegex,
            Self::InvalidLength { .. } => STGCleaveErrorKind::InvalidLength,
            Self::LengthOverflow { .. } => STGCleaveErrorKind::LengthOverflow,
            Self::FixedSize { .. } => STGCleaveErrorKind::FixedSize,
            Self::MatchType { .. } => STGCleaveErrorKind::MatchType,
            Self::NoProgress { .. } => STGCleaveErrorKind::NoProgress,
        }
    }

    pub const fn offset(&self) -> Option<usize> {
        match self {
            Self::UnexpectedEOF { offset, .. } | Self::NoProgress { offset, .. } => Some(*offset),
            Self::InvalidEnum { .. }
            | Self::UnknownTag { .. }
            | Self::Validation { .. }
            | Self::UnsupportedEncoding { .. }
            | Self::InvalidEncoding { .. }
            | Self::InvalidRegex { .. }
            | Self::InvalidLength { .. }
            | Self::LengthOverflow { .. }
            | Self::FixedSize { .. }
            | Self::MatchType { .. } => None,
        }
    }
}

impl From<kuf_stg::Error> for STGCleaveError {
    fn from(error: kuf_stg::Error) -> Self {
        match error {
            kuf_stg::Error::UnexpectedEof {
                offset,
                needed,
                remaining,
            } => Self::UnexpectedEOF {
                offset,
                needed,
                remaining,
            },
            kuf_stg::Error::InvalidEnum { enum_name, value } => {
                Self::InvalidEnum { enum_name, value }
            }
            kuf_stg::Error::UnknownTag {
                struct_name,
                field,
                value,
            } => Self::UnknownTag {
                struct_name,
                field,
                value,
            },
            kuf_stg::Error::Validation { id, message, field } => {
                Self::Validation { id, message, field }
            }
            kuf_stg::Error::UnsupportedEncoding { encoding } => {
                Self::UnsupportedEncoding { encoding }
            }
            kuf_stg::Error::InvalidEncoding { encoding } => Self::InvalidEncoding { encoding },
            kuf_stg::Error::InvalidRegex { pattern, message } => {
                Self::InvalidRegex { pattern, message }
            }
            kuf_stg::Error::InvalidLength { field, value } => Self::InvalidLength { field, value },
            kuf_stg::Error::LengthOverflow {
                field,
                value,
                target,
            } => Self::LengthOverflow {
                field,
                value,
                target,
            },
            kuf_stg::Error::FixedSize {
                field,
                expected,
                actual,
            } => Self::FixedSize {
                field,
                expected,
                actual,
            },
            kuf_stg::Error::MatchType { field, tag } => Self::MatchType { field, tag },
            kuf_stg::Error::NoProgress { field, offset } => Self::NoProgress { field, offset },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGRegion {
    Source,
    Magic,
    Header,
    Units,
    Areas,
    Variables,
    EventBlocks,
    Events,
    Conditions,
    Actions,
    Parameters,
    Footer,
    Suffix,
}

impl std::fmt::Display for STGRegion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Source => "source",
            Self::Magic => "magic",
            Self::Header => "header",
            Self::Units => "units",
            Self::Areas => "areas",
            Self::Variables => "variables",
            Self::EventBlocks => "event blocks",
            Self::Events => "events",
            Self::Conditions => "conditions",
            Self::Actions => "actions",
            Self::Parameters => "parameters",
            Self::Footer => "footer",
            Self::Suffix => "suffix",
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum STGPreflightError {
    #[error("STG {region} is truncated at offset {offset}: need {needed} bytes, have {remaining}")]
    Truncated {
        region: STGRegion,
        offset: usize,
        needed: usize,
        remaining: usize,
    },

    #[error(
        "STG {region} count {count} at offset {offset} cannot fit {remaining} remaining bytes with at least {minimum_item_size} bytes per item"
    )]
    ImpossibleCount {
        region: STGRegion,
        offset: usize,
        count: u32,
        minimum_item_size: usize,
        remaining: usize,
    },

    #[error("STG parameter at offset {offset} has unknown type tag {tag}")]
    UnknownParameterType { offset: usize, tag: u32 },

    #[error(
        "STG {region} allocation estimate {requested} with {retained} retained bytes exceeds the {maximum}-byte budget"
    )]
    AllocationBudgetExceeded {
        region: STGRegion,
        offset: usize,
        retained: usize,
        requested: usize,
        maximum: usize,
    },

    #[error("STG {region} size arithmetic overflowed at offset {offset}")]
    ArithmeticOverflow { region: STGRegion, offset: usize },
}

impl STGPreflightError {
    pub const fn region(&self) -> STGRegion {
        match self {
            Self::Truncated { region, .. }
            | Self::ImpossibleCount { region, .. }
            | Self::AllocationBudgetExceeded { region, .. }
            | Self::ArithmeticOverflow { region, .. } => *region,
            Self::UnknownParameterType { .. } => STGRegion::Parameters,
        }
    }

    pub const fn offset(&self) -> usize {
        match self {
            Self::Truncated { offset, .. }
            | Self::ImpossibleCount { offset, .. }
            | Self::UnknownParameterType { offset, .. }
            | Self::AllocationBudgetExceeded { offset, .. }
            | Self::ArithmeticOverflow { offset, .. } => *offset,
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum STGParseError {
    #[error("STG source length {length} exceeds the {maximum}-byte limit")]
    SourceTooLarge { length: usize, maximum: usize },

    #[error("STG source capacity {capacity} exceeds the {maximum}-byte limit")]
    SourceCapacityTooLarge { capacity: usize, maximum: usize },

    #[error("invalid STG magic at offset {offset}: found {actual:#010X}")]
    InvalidMagic { offset: usize, actual: u32 },

    #[error("STG prefix preflight failed: {0}")]
    PrefixPreflight(#[source] STGPreflightError),

    #[error("STG prefix parse failed in {region} at offset {offset}: {source}")]
    PrefixCleave {
        region: STGRegion,
        offset: usize,
        #[source]
        source: STGCleaveError,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum STGTailFailure {
    #[error("STG tail preflight failed: {0}")]
    Preflight(#[source] STGPreflightError),

    #[error("STG tail parse failed in {region} at offset {offset}: {source}")]
    Cleave {
        region: STGRegion,
        offset: usize,
        #[source]
        source: STGCleaveError,
    },
}

impl STGTailFailure {
    pub const fn region(&self) -> STGRegion {
        match self {
            Self::Preflight(error) => error.region(),
            Self::Cleave { region, .. } => *region,
        }
    }

    pub const fn offset(&self) -> usize {
        match self {
            Self::Preflight(error) => error.offset(),
            Self::Cleave { offset, .. } => *offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGCollection {
    Unit,
    Skill,
    Ability,
    StatOverride,
    Area,
    Variable,
    EventBlock,
    Event,
    Condition,
    Action,
    Parameter,
    FooterEntry,
}

impl std::fmt::Display for STGCollection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unit => "unit",
            Self::Skill => "skill",
            Self::Ability => "ability",
            Self::StatOverride => "stat override",
            Self::Area => "area",
            Self::Variable => "variable",
            Self::EventBlock => "event block",
            Self::Event => "event",
            Self::Condition => "condition",
            Self::Action => "action",
            Self::Parameter => "parameter",
            Self::FooterEntry => "footer entry",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGTarget {
    Number(STGNumberTarget),
    Float(STGFloatTarget),
    Text(STGTextTarget),
    Script(STGScriptTarget),
    Parameter(STGParameterTarget),
    Value(STGValueTarget),
    Structure(STGStructuralLocation),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGValueKind {
    Integer,
    Float,
    String,
    Enum,
}

impl std::fmt::Display for STGValueKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Integer => "integer",
            Self::Float => "float",
            Self::String => "string",
            Self::Enum => "enum",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGTextEncoding {
    UTF8,
    CP949,
}

impl std::fmt::Display for STGTextEncoding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UTF8 => "UTF8",
            Self::CP949 => "CP949",
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum STGTextError {
    #[error("stored bytes are not valid {encoding}")]
    InvalidStoredEncoding { encoding: STGTextEncoding },

    #[error("text cannot be encoded as {encoding}")]
    Unencodable { encoding: STGTextEncoding },

    #[error("text contains a zero byte at index {index}")]
    ContainsZero { index: usize },

    #[error("encoded text length {length} exceeds the maximum {maximum}")]
    TooLong { length: usize, maximum: usize },

    #[error("dynamic text length {length} exceeds the wire maximum {maximum}")]
    DynamicLengthOverflow { length: usize, maximum: u32 },

    #[error("text image kind does not match its target")]
    ImageKindMismatch,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGStructuralLocation {
    EventBlock { block: usize },
    Event { block: usize, event: usize },
    Script(STGScriptTarget),
    Parameter(STGParameterTarget),
    Value(STGValueTarget),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum STGEncodeError {
    #[error("direct-sink STG encoding is not available in the generated Rust codec")]
    DirectSinkUnavailable,

    #[error("generated STG encoding failed: {0}")]
    Cleave(#[source] STGCleaveError),

    #[error("encoded STG length {length} exceeds the {maximum}-byte limit")]
    LengthOverflow { length: usize, maximum: usize },

    #[error("failed to allocate {requested} bytes for encoded STG")]
    Allocation { requested: usize },

    #[error("encoded STG cursor ended at {actual}, expected {expected}")]
    CursorMismatch { expected: usize, actual: usize },

    #[error("actual encoded STG length {actual} exceeds the projected length {projected}")]
    LengthProjectionMismatch { projected: usize, actual: usize },

    #[error("STG model retains {retained} bytes, exceeding the {maximum}-byte budget")]
    ModelBudgetExceeded { retained: usize, maximum: usize },

    #[error("actual STG model size {actual} exceeds the projected size {projected}")]
    ModelProjectionMismatch { projected: usize, actual: usize },

    #[error("STG tail layout is inconsistent with the source image")]
    InvalidTailLayout,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, PartialEq)]
pub enum STGRebaseError {
    #[error("committed STG image belongs to another document lineage")]
    ForeignLineage,

    #[error("committed STG image has an invalid layout")]
    InvalidLayout,

    #[error("committed STG image is inconsistent with the saved snapshot")]
    InconsistentImage,
}

#[derive(Debug, Error)]
pub enum SaveParseError {
    #[error("save {region} is truncated at offset {offset}: need {needed} bytes, have {remaining}")]
    Truncated {
        region: SaveRegion,
        offset: usize,
        needed: usize,
        remaining: usize,
    },

    #[error("invalid save magic at offset {offset}: found {actual:#010X}")]
    InvalidMagic { offset: usize, actual: u32 },

    #[error("save envelope does not contain a valid campaign location")]
    InvalidEnvelope,

    #[error("canonical save length does not fit the wire format")]
    CanonicalLengthOverflow,

    #[error("failed to reserve {requested} bytes for the canonical save")]
    Allocation { requested: usize },

    #[error(
        "save {region} count {count} at offset {offset} cannot fit {remaining} remaining bytes with {item_size} bytes per item"
    )]
    ImpossibleCount {
        region: SaveRegion,
        offset: usize,
        count: u32,
        item_size: usize,
        remaining: usize,
    },

    #[error("failed to parse the canonical save at offset {offset}: {source}")]
    Cleave {
        offset: usize,
        #[source]
        source: SaveCleaveError,
    },
}

#[derive(Debug, Error)]
pub enum SaveEncodeError {
    #[error("failed to encode the canonical save: {0}")]
    Cleave(#[source] SaveCleaveError),

    #[error("encoded save length {length} does not fit the wire format")]
    LengthOverflow { length: usize },

    #[error("failed to reserve {requested} bytes for the encoded save")]
    Allocation { requested: usize },

    #[error("canonical save image has length {length}, but needs at least {minimum} bytes")]
    InvalidCanonicalShape { length: usize, minimum: usize },
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("failed to parse STG: {0}")]
    STGParse(#[source] STGParseError),

    #[error("failed to encode STG: {0}")]
    STGEncode(#[source] STGEncodeError),

    #[error("failed to rebase STG: {0}")]
    STGRebase(#[source] STGRebaseError),

    #[error("STG {collection} {index} is outside the count {count} for {target:?}")]
    STGTargetOutOfRange {
        target: STGTarget,
        collection: STGCollection,
        index: usize,
        count: usize,
    },

    #[error("STG value target {target:?} has kind {actual}; expected {expected}")]
    STGValueKindMismatch {
        target: STGValueTarget,
        expected: STGValueKind,
        actual: STGValueKind,
    },

    #[error(
        "STG numeric target {target:?} cannot store {value}; expected {minimum} through {maximum}"
    )]
    STGNumberOutOfRange {
        target: STGNumberTarget,
        value: i64,
        minimum: i64,
        maximum: i64,
    },

    #[error("STG text target {target:?} is invalid: {source}")]
    STGText {
        target: STGTextTarget,
        #[source]
        source: STGTextError,
    },

    #[error("STG structural image location {actual:?} does not match {expected:?}")]
    STGStructuralLocationMismatch {
        expected: STGStructuralLocation,
        actual: STGStructuralLocation,
    },

    #[error("STG structural state no longer matches the image at {location:?}")]
    STGStructuralStateMismatch { location: STGStructuralLocation },

    #[error("STG structural image belongs to another document lineage")]
    STGStructuralLineageMismatch,

    #[error(
        "STG structural history charge changed after preview: projected {projected} bytes, actual {actual} bytes"
    )]
    STGStructuralChargeMismatch { projected: usize, actual: usize },

    #[error("STG structural editing is unavailable for the raw tail at {location:?}")]
    STGStructureUnavailable { location: STGStructuralLocation },

    #[error("unknown STG {kind} type ID {id}")]
    STGUnknownScriptType { kind: STGScriptKind, id: u32 },

    #[error("STG event IDs are exhausted")]
    STGEventIDExhausted,

    #[error("STG target {target:?} is read-only")]
    STGReadOnlyTarget { target: STGTarget },

    #[error("failed to parse Crusaders save: {0}")]
    SaveParse(#[source] SaveParseError),

    #[error("failed to encode Crusaders save: {0}")]
    SaveEncode(#[source] SaveEncodeError),

    #[error("save numeric target {target:?} is outside the record count {record_count}")]
    SaveTargetOutOfRange {
        target: SaveNumberTarget,
        record_count: usize,
    },

    #[error(
        "save numeric target {target:?} cannot store {value}; expected {minimum} through {maximum}"
    )]
    SaveValueOutOfRange {
        target: SaveNumberTarget,
        value: i64,
        minimum: i64,
        maximum: i64,
    },

    #[error(
        "save text field {field:?} contains non-ASCII stored byte {byte:#04x} at index {index}"
    )]
    SaveInvalidStoredText {
        field: SaveTextField,
        index: usize,
        byte: u8,
    },

    #[error("save text field {field:?} contains non-ASCII byte {byte:#04x} at index {index}")]
    SaveInvalidTextByte {
        field: SaveTextField,
        index: usize,
        byte: u8,
    },

    #[error("save text field {field:?} contains a zero byte at index {index}")]
    SaveTextContainsZero { field: SaveTextField, index: usize },

    #[error("save text field {field:?} has length {length}, but the maximum length is {maximum}")]
    SaveTextTooLong {
        field: SaveTextField,
        length: usize,
        maximum: usize,
    },

    #[error("save unit {unit} is outside the unit count {unit_count}")]
    SaveUnitOutOfRange { unit: usize, unit_count: usize },

    #[error("saved Crusaders source image is inconsistent with the saved snapshot")]
    InconsistentSaveRebase,

    #[error("SOX input is neither a TroopInfo, SkillInfo, nor text SOX document")]
    UnsupportedSOX,

    #[error("ASCII-hex SOX input has an odd length of {length} bytes")]
    OddASCIIHexLength { length: usize },

    #[error("ASCII-hex SOX input has a non-hexadecimal byte at index {index}")]
    InvalidASCIIHexByte { index: usize },

    #[error("saved SOX source image has an inconsistent encoding envelope")]
    InconsistentSOXRebase,

    #[error("failed to parse {layout} string table at offset {offset}: {source}")]
    StringTableParse {
        layout: SOXStringTableLayout,
        offset: usize,
        #[source]
        source: StringTableParseError,
    },

    #[error("failed to encode {layout} string table: {source}")]
    StringTableEncode {
        layout: SOXStringTableLayout,
        #[source]
        source: StringTableEncodeError,
    },

    #[error("{layout} string-table record {record} is outside the record count {record_count}")]
    StringTableRecordOutOfRange {
        layout: SOXStringTableLayout,
        record: usize,
        record_count: usize,
    },

    #[error(
        "{layout} string-table record {record} field {field} is outside the field count {field_count}"
    )]
    StringTableFieldOutOfRange {
        layout: SOXStringTableLayout,
        record: usize,
        field: usize,
        field_count: usize,
    },

    #[error("failed to parse {schema} at offset {offset}: {source}")]
    SchemaParse {
        schema: SOXSchema,
        offset: usize,
        #[source]
        source: GeneratedSOXError,
    },

    #[error("failed to encode {schema}: {source}")]
    SchemaEncode {
        schema: SOXSchema,
        #[source]
        source: GeneratedSOXError,
    },

    #[error("failed to parse text SOX at offset {offset}: {source}")]
    TextSOXParse {
        offset: usize,
        #[source]
        source: TextSOXParseError,
    },

    #[error("text SOX record {record} is empty")]
    TextSOXEmptyText { record: usize },

    #[error("text SOX record {record} exceeds its byte budget")]
    TextSOXTooLong {
        record: usize,
        length: usize,
        maximum: u16,
    },

    #[error("text SOX record {record} contains an unsupported byte")]
    TextSOXInvalidTextByte {
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
    SkillUTF8 {
        record: usize,
        field: SkillTextField,
        #[source]
        source: std::str::Utf8Error,
    },
}
