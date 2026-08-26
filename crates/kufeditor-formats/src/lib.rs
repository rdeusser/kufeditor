//! Binary format codecs and source-preserving document types.

mod diagnostic;
mod error;
mod generated;
mod save;
mod schema;
mod skill;
mod sox;
pub mod stg;
mod string_table;
mod text;
mod troop;

pub use diagnostic::{Diagnostic, DiagnosticField, DiagnosticLocation, Severity};
pub use error::{
    FormatError, GeneratedSOXError, STGCleaveError, STGCleaveErrorKind, STGCollection,
    STGEncodeError, STGParseError, STGPreflightError, STGRebaseError, STGRegion,
    STGStructuralLocation, STGTailFailure, STGTarget, STGTextEncoding, STGTextError, STGValueKind,
    SaveCleaveError, SaveEncodeError, SaveParseError, SaveRegion, SkillCleaveError,
    StringTableEncodeError, StringTableParseError, TextSOXParseError, TroopCleaveError,
};
pub use save::{
    SaveChoice, SaveDocument, SaveEditor, SaveEquipmentField, SaveEquipmentGroup,
    SaveEquipmentSlot, SaveMainField, SaveMutation, SaveNumberTarget, SaveRosterField,
    SaveTextField, SaveTextImage, SaveUnitField, SaveUnitGroup,
};
pub use schema::{SOXSchema, SchemaDocument, SpecialNameRef};
pub use skill::{SkillDocument, SkillField, SkillTextField};
pub use sox::{SOXDocument, parse_sox};
pub use stg::{
    STGAbilityOwner, STGAreaField, STGAreaFloatField, STGChoice, STGDocument, STGEditor,
    STGFieldAccess, STGFloatTarget, STGFloatValue, STGFooterField, STGHeaderTextField, STGMutation,
    STGNumberTarget, STGParameterTarget, STGScriptKind, STGScriptTarget, STGSkillField,
    STGSkillOwner, STGTailStatus, STGText, STGTextTarget, STGUnitField, STGUnitFloatField,
    STGUnitGroup, STGValueTarget,
};
pub use string_table::{SOXStringTableDocument, SOXStringTableLayout};
pub use text::{TextSOXDocument, TextSOXField};
pub use troop::{TroopDocument, TroopField, TroopGroup};
