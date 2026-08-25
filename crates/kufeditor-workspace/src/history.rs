use kufeditor_formats::{SaveTextField, SaveTextImage};

use crate::{DocumentEdit, StateID};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HistoryAction {
    Edit(DocumentEdit),
    RestoreSaveText {
        field: SaveTextField,
        image: SaveTextImage,
    },
}

pub(crate) enum DocumentMutation {
    Unchanged,
    Changed { inverse: HistoryAction },
}

#[derive(Clone, Debug)]
pub(crate) struct HistoryEntry {
    pub(crate) forward: HistoryAction,
    pub(crate) inverse: HistoryAction,
    pub(crate) before: StateID,
    pub(crate) after: StateID,
}
