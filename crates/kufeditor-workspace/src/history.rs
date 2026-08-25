use crate::{DocumentEdit, StateId};

#[derive(Clone, Debug)]
pub(crate) struct HistoryEntry {
    pub(crate) forward: DocumentEdit,
    pub(crate) inverse: DocumentEdit,
    pub(crate) before: StateId,
    pub(crate) after: StateId,
}
