use thiserror::Error;

use crate::generated::sox_troop_info;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct CleaveError(#[from] sox_troop_info::Error);

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
        source: CleaveError,
    },

    #[error("failed to encode TroopInfo: {0}")]
    TroopEncode(#[source] CleaveError),

    #[error("TroopInfo record {record} is outside the record count {record_count}")]
    RecordOutOfRange { record: usize, record_count: usize },
}
