use kufeditor_formats::{
    STGFloatTarget, STGFloatValue, STGNumberTarget, STGStructuralChange, STGStructuralImage,
    STGTextImage, STGTextTarget, SaveTextField, SaveTextImage,
};

use crate::{DocumentEdit, StateID};

pub const DEFAULT_STG_HISTORY_LIMIT: usize = 128 * 1024 * 1024;

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

#[derive(Debug)]
pub(crate) enum STGHistoryAction {
    Number {
        target: STGNumberTarget,
        value: i64,
        retained_bytes: usize,
    },
    Float {
        target: STGFloatTarget,
        value: STGFloatValue,
        retained_bytes: usize,
    },
    Text {
        target: STGTextTarget,
        image: STGTextImage,
        opposite_retained_bytes: usize,
        retained_bytes: usize,
    },
    Structure {
        image: STGStructuralImage,
        opposite_retained_bytes: usize,
        retained_bytes: usize,
    },
}

impl STGHistoryAction {
    pub(crate) const fn retained_bytes(&self) -> usize {
        match self {
            Self::Number { retained_bytes, .. }
            | Self::Float { retained_bytes, .. }
            | Self::Text { retained_bytes, .. }
            | Self::Structure { retained_bytes, .. } => *retained_bytes,
        }
    }
}

#[derive(Debug)]
pub(crate) enum HistoryEntry {
    Standard {
        forward: HistoryAction,
        inverse: HistoryAction,
        before: StateID,
        after: StateID,
    },
    STG {
        action: STGHistoryAction,
        before: StateID,
        after: StateID,
        retained_bytes: usize,
    },
}

impl HistoryEntry {
    pub(crate) const fn retained_bytes(&self) -> usize {
        match self {
            Self::Standard { .. } => 0,
            Self::STG { retained_bytes, .. } => *retained_bytes,
        }
    }

    pub(crate) fn stg_structural_change(&self) -> Option<STGStructuralChange> {
        match self {
            Self::STG {
                action: STGHistoryAction::Structure { image, .. },
                ..
            } => Some(image.change()),
            Self::Standard { .. }
            | Self::STG {
                action:
                    STGHistoryAction::Number { .. }
                    | STGHistoryAction::Float { .. }
                    | STGHistoryAction::Text { .. },
                ..
            } => None,
        }
    }
}
