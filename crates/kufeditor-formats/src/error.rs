use thiserror::Error;

use crate::generated::sox_troop_info;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct CleaveError(#[from] sox_troop_info::Error);

#[derive(Debug, Error)]
pub enum FormatError {
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
