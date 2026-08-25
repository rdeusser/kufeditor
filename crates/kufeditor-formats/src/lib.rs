//! Binary format codecs and source-preserving document types.

mod diagnostic;
mod error;
mod generated;
mod schema;
mod skill;
mod sox;
mod text;
mod troop;

pub use diagnostic::{Diagnostic, DiagnosticField, Severity};
pub use error::{
    FormatError, GeneratedSoxError, SkillCleaveError, TextSoxParseError, TroopCleaveError,
};
pub use schema::{SchemaDocument, SoxSchema};
pub use skill::{SkillDocument, SkillField, SkillTextField};
pub use sox::{SoxDocument, parse_sox};
pub use text::{TextSoxDocument, TextSoxField};
pub use troop::{TroopDocument, TroopField, TroopGroup};
