//! Binary format codecs and source-preserving document types.

mod diagnostic;
mod error;
mod generated;
mod sox;
mod troop;

pub use diagnostic::{Diagnostic, Severity};
pub use error::{CleaveError, FormatError};
pub use troop::{TroopDocument, TroopField, TroopGroup};
