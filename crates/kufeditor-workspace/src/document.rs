use kufeditor_formats::{
    FormatError, SkillDocument, SkillTextField, TextSOXDocument, TroopDocument, TroopField,
};

use crate::WorkspaceError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocumentID(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StateID(pub(crate) u64);

#[derive(Clone, Debug)]
pub enum Document {
    Troop(TroopDocument),
    Skill(SkillDocument),
    TextSOX(TextSOXDocument),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    TroopInfo,
    SkillInfo,
    TextSOX,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentEdit {
    SetTroopField {
        record: usize,
        field: TroopField,
        value: i32,
    },
    SetSkillID {
        record: usize,
        value: i32,
    },
    SetSkillType {
        record: usize,
        value: u32,
    },
    SetSkillMaxLevel {
        record: usize,
        value: u32,
    },
    SetSkillText {
        record: usize,
        field: SkillTextField,
        value: String,
    },
    SetTextSOXText {
        record: usize,
        value: String,
    },
}

impl Document {
    pub(crate) const fn kind(&self) -> DocumentKind {
        match self {
            Self::Troop(_) => DocumentKind::TroopInfo,
            Self::Skill(_) => DocumentKind::SkillInfo,
            Self::TextSOX(_) => DocumentKind::TextSOX,
        }
    }

    pub(crate) fn apply(
        &mut self,
        id: DocumentID,
        edit: DocumentEdit,
    ) -> Result<DocumentEdit, WorkspaceError> {
        match (self, edit) {
            (
                Self::Troop(document),
                DocumentEdit::SetTroopField {
                    record,
                    field,
                    value,
                },
            ) => {
                let previous = document.set_value(record, field, value)?;
                Ok(DocumentEdit::SetTroopField {
                    record,
                    field,
                    value: previous,
                })
            }
            (Self::Skill(document), DocumentEdit::SetSkillID { record, value }) => {
                let previous = document.set_skill_id(record, value)?;
                Ok(DocumentEdit::SetSkillID {
                    record,
                    value: previous,
                })
            }
            (Self::Skill(document), DocumentEdit::SetSkillType { record, value }) => {
                let previous = document.set_skill_type(record, value)?;
                Ok(DocumentEdit::SetSkillType {
                    record,
                    value: previous,
                })
            }
            (Self::Skill(document), DocumentEdit::SetSkillMaxLevel { record, value }) => {
                let previous = document.set_max_level(record, value)?;
                Ok(DocumentEdit::SetSkillMaxLevel {
                    record,
                    value: previous,
                })
            }
            (
                Self::Skill(document),
                DocumentEdit::SetSkillText {
                    record,
                    field,
                    value,
                },
            ) => {
                let previous = document.set_text(record, field, value)?;
                Ok(DocumentEdit::SetSkillText {
                    record,
                    field,
                    value: previous,
                })
            }
            (Self::TextSOX(document), DocumentEdit::SetTextSOXText { record, value }) => {
                let previous = document.set_text(record, value)?;
                Ok(DocumentEdit::SetTextSOXText {
                    record,
                    value: previous,
                })
            }
            (_, DocumentEdit::SetTroopField { .. }) => Err(WorkspaceError::NotTroop(id)),
            (
                _,
                DocumentEdit::SetSkillID { .. }
                | DocumentEdit::SetSkillType { .. }
                | DocumentEdit::SetSkillMaxLevel { .. }
                | DocumentEdit::SetSkillText { .. },
            ) => Err(WorkspaceError::NotSkill(id)),
            (_, DocumentEdit::SetTextSOXText { .. }) => Err(WorkspaceError::NotTextSOX(id)),
        }
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, FormatError> {
        match self {
            Self::Troop(document) => document.encode(),
            Self::Skill(document) => document.encode(),
            Self::TextSOX(document) => document.encode(),
        }
    }

    pub(crate) fn rebase_source(
        &mut self,
        saved: &Self,
        bytes: Vec<u8>,
    ) -> Result<(), FormatError> {
        match (self, saved) {
            (Self::Troop(document), Self::Troop(saved)) => document.rebase_source(saved, bytes),
            (Self::Skill(document), Self::Skill(saved)) => document.rebase_source(saved, bytes),
            (Self::TextSOX(document), Self::TextSOX(saved)) => document.rebase_source(saved, bytes),
            _ => Err(FormatError::InconsistentSOXRebase),
        }
    }
}
