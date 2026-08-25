use kufeditor_formats::{FormatError, TroopDocument, TroopField};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocumentId(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StateId(pub(crate) u64);

#[derive(Clone, Debug)]
pub enum Document {
    Troop(TroopDocument),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentEdit {
    SetTroopField {
        record: usize,
        field: TroopField,
        value: i32,
    },
}

impl Document {
    pub(crate) fn apply(&mut self, edit: DocumentEdit) -> Result<DocumentEdit, FormatError> {
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
        }
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, FormatError> {
        match self {
            Self::Troop(document) => document.encode(),
        }
    }

    pub(crate) fn rebase_source(
        &mut self,
        saved: &Self,
        bytes: Vec<u8>,
    ) -> Result<(), FormatError> {
        match (self, saved) {
            (Self::Troop(document), Self::Troop(saved)) => document.rebase_source(saved, bytes),
        }
    }
}
