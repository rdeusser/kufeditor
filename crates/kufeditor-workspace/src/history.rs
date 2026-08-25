use crate::{DocumentEdit, StateID};

#[derive(Clone, Debug)]
pub(crate) struct HistoryEntry {
    pub(crate) forward: DocumentEdit,
    pub(crate) inverse: DocumentEdit,
    pub(crate) before: StateID,
    pub(crate) after: StateID,
}
