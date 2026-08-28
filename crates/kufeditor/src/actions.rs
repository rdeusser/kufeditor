use gpui::{Action, App, KeyBinding, actions};
use kufeditor_workspace::{
    DocumentID, STGNumberTarget, STGParameterTarget, STGReferenceKind, STGStructuralEdit,
    SaveNumberTarget,
};

use crate::state::{STGReferenceCursor, STGSection};

actions!(
    kufeditor,
    [
        OpenFile,
        Save,
        SaveAll,
        SaveAs,
        Undo,
        Redo,
        FocusNextSaveControl,
        FocusPreviousSaveControl,
        FocusNextSTGControl,
        FocusPreviousSTGControl,
        FocusNextModControl,
        FocusPreviousModControl,
        CancelModOperation,
        FocusNextPatchControl,
        FocusPreviousPatchControl,
        DismissPatchConfirmation,
        MoveSaveListUp,
        MoveSaveListDown,
        MoveSaveListHome,
        MoveSaveListEnd,
        MoveSaveListPageUp,
        MoveSaveListPageDown,
        MoveSaveListLeft,
        MoveSaveListRight,
        MoveSTGListUp,
        MoveSTGListDown,
        MoveSTGListHome,
        MoveSTGListEnd,
        MoveSTGListPageUp,
        MoveSTGListPageDown,
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Action)]
#[action(namespace = kufeditor, no_json)]
pub(crate) struct SetSaveChoice {
    pub(crate) document: DocumentID,
    pub(crate) target: SaveNumberTarget,
    pub(crate) value: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Action)]
#[action(namespace = kufeditor, no_json)]
pub(crate) struct SetSTGChoice {
    pub(crate) document: DocumentID,
    pub(crate) section: STGSection,
    pub(crate) generation: u64,
    pub(crate) target: STGNumberTarget,
    pub(crate) value: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Action)]
#[action(namespace = kufeditor, no_json)]
pub(crate) struct ApplySTGStructuralEdit {
    pub(crate) document: DocumentID,
    pub(crate) section: STGSection,
    pub(crate) generation: u64,
    pub(crate) edit: STGStructuralEdit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Action)]
#[action(namespace = kufeditor, no_json)]
pub(crate) struct SelectSTGReference {
    pub(crate) document: DocumentID,
    pub(crate) section: STGSection,
    pub(crate) generation: u64,
    pub(crate) target: STGParameterTarget,
    pub(crate) kind: STGReferenceKind,
    pub(crate) cursor: STGReferenceCursor,
    pub(crate) position: usize,
    pub(crate) value: i32,
}

pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-o", OpenFile, None),
        KeyBinding::new("ctrl-o", OpenFile, None),
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("ctrl-s", Save, None),
        KeyBinding::new("cmd-shift-s", SaveAs, None),
        KeyBinding::new("ctrl-shift-s", SaveAs, None),
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("ctrl-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
        KeyBinding::new("ctrl-shift-z", Redo, None),
        KeyBinding::new("tab", FocusNextSaveControl, Some("SaveEditor")),
        KeyBinding::new("shift-tab", FocusPreviousSaveControl, Some("SaveEditor")),
        KeyBinding::new("tab", FocusNextSTGControl, Some("STGEditor")),
        KeyBinding::new("shift-tab", FocusPreviousSTGControl, Some("STGEditor")),
        KeyBinding::new("tab", FocusNextModControl, Some("Mods")),
        KeyBinding::new("shift-tab", FocusPreviousModControl, Some("Mods")),
        KeyBinding::new("escape", CancelModOperation, Some("Mods")),
        KeyBinding::new("tab", FocusNextPatchControl, Some("Patches")),
        KeyBinding::new("shift-tab", FocusPreviousPatchControl, Some("Patches")),
        KeyBinding::new("escape", DismissPatchConfirmation, Some("Patches")),
        KeyBinding::new("up", MoveSaveListUp, Some("SaveVirtualList")),
        KeyBinding::new("down", MoveSaveListDown, Some("SaveVirtualList")),
        KeyBinding::new("home", MoveSaveListHome, Some("SaveVirtualList")),
        KeyBinding::new("end", MoveSaveListEnd, Some("SaveVirtualList")),
        KeyBinding::new("pageup", MoveSaveListPageUp, Some("SaveVirtualList")),
        KeyBinding::new("pagedown", MoveSaveListPageDown, Some("SaveVirtualList")),
        KeyBinding::new("left", MoveSaveListLeft, Some("SaveVirtualList")),
        KeyBinding::new("right", MoveSaveListRight, Some("SaveVirtualList")),
        KeyBinding::new("up", MoveSTGListUp, Some("STGVirtualList")),
        KeyBinding::new("down", MoveSTGListDown, Some("STGVirtualList")),
        KeyBinding::new("home", MoveSTGListHome, Some("STGVirtualList")),
        KeyBinding::new("end", MoveSTGListEnd, Some("STGVirtualList")),
        KeyBinding::new("pageup", MoveSTGListPageUp, Some("STGVirtualList")),
        KeyBinding::new("pagedown", MoveSTGListPageDown, Some("STGVirtualList")),
        KeyBinding::new("up", MoveSTGListUp, Some("STGReferenceList")),
        KeyBinding::new("down", MoveSTGListDown, Some("STGReferenceList")),
        KeyBinding::new("home", MoveSTGListHome, Some("STGReferenceList")),
        KeyBinding::new("end", MoveSTGListEnd, Some("STGReferenceList")),
        KeyBinding::new("pageup", MoveSTGListPageUp, Some("STGReferenceList")),
        KeyBinding::new("pagedown", MoveSTGListPageDown, Some("STGReferenceList")),
    ]);
}
