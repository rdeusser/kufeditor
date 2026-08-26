use kufeditor_formats::{
    FormatError, STGDocument, STGEncodeError, STGFloatTarget, STGFloatValue, STGNumberTarget,
    STGRebaseError, STGStructuralEdit, STGTextTarget, SaveDocument, SaveMutation, SaveNumberTarget,
    SaveTextField, SkillDocument, SkillTextField, TextSOXDocument, TroopDocument, TroopField,
};

use crate::{
    WorkspaceError,
    history::{DocumentMutation, HistoryAction},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocumentID(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StateID(pub(crate) u64);

#[allow(
    clippy::large_enum_variant,
    reason = "the public document model keeps each owned format document inline"
)]
#[derive(Clone, Debug)]
pub enum Document {
    Troop(TroopDocument),
    Skill(SkillDocument),
    TextSOX(TextSOXDocument),
    Save(SaveDocument),
    STG(STGDocument),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    TroopInfo,
    SkillInfo,
    TextSOX,
    CrusadersSave,
    CrusadersSTG,
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
    SetSaveNumber {
        target: SaveNumberTarget,
        value: i64,
    },
    SetSaveText {
        field: SaveTextField,
        value: String,
    },
    SetSTGNumber {
        target: STGNumberTarget,
        value: i64,
    },
    SetSTGFloat {
        target: STGFloatTarget,
        value: STGFloatValue,
    },
    SetSTGText {
        target: STGTextTarget,
        value: String,
    },
    EditSTGStructure {
        edit: STGStructuralEdit,
    },
}

impl DocumentEdit {
    pub(crate) const fn is_stg(&self) -> bool {
        matches!(
            self,
            Self::SetSTGNumber { .. }
                | Self::SetSTGFloat { .. }
                | Self::SetSTGText { .. }
                | Self::EditSTGStructure { .. }
        )
    }
}

impl Document {
    pub(crate) const fn kind(&self) -> DocumentKind {
        match self {
            Self::Troop(_) => DocumentKind::TroopInfo,
            Self::Skill(_) => DocumentKind::SkillInfo,
            Self::TextSOX(_) => DocumentKind::TextSOX,
            Self::Save(_) => DocumentKind::CrusadersSave,
            Self::STG(_) => DocumentKind::CrusadersSTG,
        }
    }

    pub(crate) fn apply(
        &mut self,
        id: DocumentID,
        action: HistoryAction,
    ) -> Result<DocumentMutation, WorkspaceError> {
        match action {
            HistoryAction::Edit(edit) => self.apply_edit(id, edit),
            HistoryAction::RestoreSaveText { field, image } => match self {
                Self::Save(document) => Ok(match document.restore_text(field, image) {
                    SaveMutation::Unchanged => DocumentMutation::Unchanged,
                    SaveMutation::Changed { previous } => DocumentMutation::Changed {
                        inverse: HistoryAction::RestoreSaveText {
                            field,
                            image: previous,
                        },
                    },
                }),
                _ => Err(WorkspaceError::NotSave(id)),
            },
        }
    }

    fn apply_edit(
        &mut self,
        id: DocumentID,
        edit: DocumentEdit,
    ) -> Result<DocumentMutation, WorkspaceError> {
        match edit {
            DocumentEdit::SetTroopField {
                record,
                field,
                value,
            } => self.apply_troop_field(id, record, field, value),
            DocumentEdit::SetSkillID { record, value } => self.apply_skill_id(id, record, value),
            DocumentEdit::SetSkillType { record, value } => {
                self.apply_skill_type(id, record, value)
            }
            DocumentEdit::SetSkillMaxLevel { record, value } => {
                self.apply_skill_max_level(id, record, value)
            }
            DocumentEdit::SetSkillText {
                record,
                field,
                value,
            } => self.apply_skill_text(id, record, field, value),
            DocumentEdit::SetTextSOXText { record, value } => {
                self.apply_text_sox_text(id, record, value)
            }
            DocumentEdit::SetSaveNumber { target, value } => {
                self.apply_save_number(id, target, value)
            }
            DocumentEdit::SetSaveText { field, value } => self.apply_save_text(id, field, value),
            DocumentEdit::SetSTGNumber { .. }
            | DocumentEdit::SetSTGFloat { .. }
            | DocumentEdit::SetSTGText { .. }
            | DocumentEdit::EditSTGStructure { .. } => {
                unreachable!("STG edits use the bounded workspace history path")
            }
        }
    }

    fn apply_troop_field(
        &mut self,
        id: DocumentID,
        record: usize,
        field: TroopField,
        value: i32,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Troop(document) = self else {
            return Err(WorkspaceError::NotTroop(id));
        };
        if document.value(record, field)? == value {
            return Ok(DocumentMutation::Unchanged);
        }
        let previous = document.set_value(record, field, value)?;
        Ok(inverse_edit(DocumentEdit::SetTroopField {
            record,
            field,
            value: previous,
        }))
    }

    fn apply_skill_id(
        &mut self,
        id: DocumentID,
        record: usize,
        value: i32,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Skill(document) = self else {
            return Err(WorkspaceError::NotSkill(id));
        };
        if document.skill_id(record)? == value {
            return Ok(DocumentMutation::Unchanged);
        }
        let previous = document.set_skill_id(record, value)?;
        Ok(inverse_edit(DocumentEdit::SetSkillID {
            record,
            value: previous,
        }))
    }

    fn apply_skill_type(
        &mut self,
        id: DocumentID,
        record: usize,
        value: u32,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Skill(document) = self else {
            return Err(WorkspaceError::NotSkill(id));
        };
        if document.skill_type(record)? == value {
            return Ok(DocumentMutation::Unchanged);
        }
        let previous = document.set_skill_type(record, value)?;
        Ok(inverse_edit(DocumentEdit::SetSkillType {
            record,
            value: previous,
        }))
    }

    fn apply_skill_max_level(
        &mut self,
        id: DocumentID,
        record: usize,
        value: u32,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Skill(document) = self else {
            return Err(WorkspaceError::NotSkill(id));
        };
        if document.max_level(record)? == value {
            return Ok(DocumentMutation::Unchanged);
        }
        let previous = document.set_max_level(record, value)?;
        Ok(inverse_edit(DocumentEdit::SetSkillMaxLevel {
            record,
            value: previous,
        }))
    }

    fn apply_skill_text(
        &mut self,
        id: DocumentID,
        record: usize,
        field: SkillTextField,
        value: String,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Skill(document) = self else {
            return Err(WorkspaceError::NotSkill(id));
        };
        if document.text(record, field)? == value {
            return Ok(DocumentMutation::Unchanged);
        }
        let previous = document.set_text(record, field, value)?;
        Ok(inverse_edit(DocumentEdit::SetSkillText {
            record,
            field,
            value: previous,
        }))
    }

    fn apply_text_sox_text(
        &mut self,
        id: DocumentID,
        record: usize,
        value: String,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::TextSOX(document) = self else {
            return Err(WorkspaceError::NotTextSOX(id));
        };
        if document.text(record)? == value {
            return Ok(DocumentMutation::Unchanged);
        }
        let previous = document.set_text(record, value)?;
        Ok(inverse_edit(DocumentEdit::SetTextSOXText {
            record,
            value: previous,
        }))
    }

    fn apply_save_number(
        &mut self,
        id: DocumentID,
        target: SaveNumberTarget,
        value: i64,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Save(document) = self else {
            return Err(WorkspaceError::NotSave(id));
        };
        Ok(match document.set_number(target, value)? {
            SaveMutation::Unchanged => DocumentMutation::Unchanged,
            SaveMutation::Changed { previous } => inverse_edit(DocumentEdit::SetSaveNumber {
                target,
                value: previous,
            }),
        })
    }

    fn apply_save_text(
        &mut self,
        id: DocumentID,
        field: SaveTextField,
        value: String,
    ) -> Result<DocumentMutation, WorkspaceError> {
        let Self::Save(document) = self else {
            return Err(WorkspaceError::NotSave(id));
        };
        Ok(match document.set_text(field, value)? {
            SaveMutation::Unchanged => DocumentMutation::Unchanged,
            SaveMutation::Changed { previous } => DocumentMutation::Changed {
                inverse: HistoryAction::RestoreSaveText {
                    field,
                    image: previous,
                },
            },
        })
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, FormatError> {
        match self {
            Self::Troop(document) => document.encode(),
            Self::Skill(document) => document.encode(),
            Self::TextSOX(document) => document.encode(),
            Self::Save(document) => document.encode(),
            Self::STG(_) => Err(FormatError::STGEncode(
                STGEncodeError::DirectSinkUnavailable,
            )),
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
            (Self::Save(document), Self::Save(saved)) => document.rebase_source(saved, bytes),
            (Self::STG(_), _) | (_, Self::STG(_)) => {
                Err(FormatError::STGRebase(STGRebaseError::InconsistentImage))
            }
            (Self::Save(_), _) | (_, Self::Save(_)) => Err(FormatError::InconsistentSaveRebase),
            _ => Err(FormatError::InconsistentSOXRebase),
        }
    }
}

fn inverse_edit(edit: DocumentEdit) -> DocumentMutation {
    DocumentMutation::Changed {
        inverse: HistoryAction::Edit(edit),
    }
}
