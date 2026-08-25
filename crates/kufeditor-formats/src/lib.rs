//! Binary format codecs and source-preserving document types.

mod diagnostic;
mod error;
mod generated;
mod schema;
mod skill;
mod sox;
mod string_table;
mod text;
mod troop;

pub use diagnostic::{Diagnostic, DiagnosticField, Severity};
pub use error::{
    FormatError, GeneratedSOXError, SkillCleaveError, StringTableEncodeError,
    StringTableParseError, TextSOXParseError, TroopCleaveError,
};
pub use schema::{SOXSchema, SchemaDocument, SpecialNameRef};
pub use skill::{SkillDocument, SkillField, SkillTextField};
pub use sox::{SOXDocument, parse_sox};
pub use string_table::{SOXStringTableDocument, SOXStringTableLayout};
pub use text::{TextSOXDocument, TextSOXField};
pub use troop::{TroopDocument, TroopField, TroopGroup};
