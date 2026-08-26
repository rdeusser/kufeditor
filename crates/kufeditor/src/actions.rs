use gpui::{Action, App, KeyBinding, actions};
use kufeditor_workspace::{DocumentID, SaveNumberTarget};

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
        ActivateSaveControl,
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Action)]
#[action(namespace = kufeditor, no_json)]
pub(crate) struct SetSaveChoice {
    pub(crate) document: DocumentID,
    pub(crate) target: SaveNumberTarget,
    pub(crate) value: i64,
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
        KeyBinding::new("enter", ActivateSaveControl, Some("SaveControl")),
        KeyBinding::new("space", ActivateSaveControl, Some("SaveControl")),
    ]);
}
