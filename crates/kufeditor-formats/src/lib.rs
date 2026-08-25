//! Binary format codecs and source-preserving document types.

mod diagnostic;
mod error;
mod generated;
mod skill;
mod sox;
mod troop;

pub use diagnostic::{Diagnostic, DiagnosticField, Severity};
pub use error::{FormatError, SkillCleaveError, TroopCleaveError};
pub use skill::{SkillDocument, SkillField, SkillTextField};
pub use sox::{SoxDocument, parse_sox};
pub use troop::{TroopDocument, TroopField, TroopGroup};
