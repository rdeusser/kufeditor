use kufeditor_formats::{FormatError, SkillDocument, SkillTextField, TroopDocument, TroopField};

use crate::WorkspaceError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocumentId(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StateId(pub(crate) u64);

#[derive(Clone, Debug)]
pub enum Document {
    Troop(TroopDocument),
    Skill(SkillDocument),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    TroopInfo,
    SkillInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentEdit {
    SetTroopField {
        record: usize,
        field: TroopField,
        value: i32,
    },
    SetSkillId {
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
}

impl Document {
    pub(crate) const fn kind(&self) -> DocumentKind {
        match self {
            Self::Troop(_) => DocumentKind::TroopInfo,
            Self::Skill(_) => DocumentKind::SkillInfo,
        }
    }

    pub(crate) fn apply(
        &mut self,
        id: DocumentId,
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
            (Self::Skill(document), DocumentEdit::SetSkillId { record, value }) => {
                let previous = document.set_skill_id(record, value)?;
                Ok(DocumentEdit::SetSkillId {
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
            (Self::Troop(_), _) => Err(WorkspaceError::NotSkill(id)),
            (Self::Skill(_), DocumentEdit::SetTroopField { .. }) => {
                Err(WorkspaceError::NotTroop(id))
            }
        }
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, FormatError> {
        match self {
            Self::Troop(document) => document.encode(),
            Self::Skill(document) => document.encode(),
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
            _ => Err(FormatError::InconsistentSoxRebase),
        }
    }
}
