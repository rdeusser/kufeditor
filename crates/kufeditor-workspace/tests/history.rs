#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use known fixed-size byte ranges"
)]

use std::path::PathBuf;

use kufeditor_workspace::{Document, DocumentEdit, TroopDocument, TroopField, Workspace};

fn troop_fixture() -> Vec<u8> {
    let mut bytes = vec![0_u8; 8 + 148 + 64];
    bytes
        .get_mut(0..4)
        .unwrap()
        .copy_from_slice(&100_u32.to_le_bytes());
    bytes
        .get_mut(4..8)
        .unwrap()
        .copy_from_slice(&1_u32.to_le_bytes());
    bytes
        .get_mut(16..20)
        .unwrap()
        .copy_from_slice(&130_i32.to_le_bytes());
    bytes
        .get_mut(64..68)
        .unwrap()
        .copy_from_slice(&100_i32.to_le_bytes());
    bytes
        .get_mut(108..112)
        .unwrap()
        .copy_from_slice(&800_i32.to_le_bytes());
    bytes
}

fn workspace_with_troop() -> (Workspace, kufeditor_workspace::DocumentId) {
    let document = TroopDocument::parse(troop_fixture()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("TroopInfo.sox"), Document::Troop(document));
    (workspace, id)
}

fn move_speed(value: i32) -> DocumentEdit {
    DocumentEdit::SetTroopField {
        record: 0,
        field: TroopField::MoveSpeed,
        value,
    }
}

#[test]
fn undoing_to_the_saved_state_clears_dirty() {
    let (mut workspace, id) = workspace_with_troop();
    let original_state = workspace.state_id(id).unwrap();

    workspace.apply(id, move_speed(175)).unwrap();

    assert!(workspace.is_dirty(id).unwrap());
    assert_ne!(workspace.state_id(id).unwrap(), original_state);
    assert!(workspace.undo(id).unwrap());
    assert!(!workspace.is_dirty(id).unwrap());
    assert_eq!(
        workspace.troop_value(id, 0, TroopField::MoveSpeed).unwrap(),
        130
    );

    assert!(workspace.redo(id).unwrap());
    assert!(workspace.is_dirty(id).unwrap());
    assert_eq!(
        workspace.troop_value(id, 0, TroopField::MoveSpeed).unwrap(),
        175
    );
}

#[test]
fn editing_after_undo_discards_the_redo_branch() {
    let (mut workspace, id) = workspace_with_troop();

    workspace.apply(id, move_speed(175)).unwrap();
    workspace.undo(id).unwrap();
    workspace.apply(id, move_speed(200)).unwrap();

    assert!(!workspace.can_redo(id).unwrap());
    assert_eq!(
        workspace.troop_value(id, 0, TroopField::MoveSpeed).unwrap(),
        200
    );
}

#[test]
fn failed_edit_does_not_create_history() {
    let (mut workspace, id) = workspace_with_troop();
    let original_state = workspace.state_id(id).unwrap();

    let result = workspace.apply(
        id,
        DocumentEdit::SetTroopField {
            record: 1,
            field: TroopField::MoveSpeed,
            value: 175,
        },
    );

    assert!(result.is_err());
    assert_eq!(workspace.state_id(id).unwrap(), original_state);
    assert!(!workspace.can_undo(id).unwrap());
    assert!(!workspace.is_dirty(id).unwrap());
}

#[test]
fn projections_describe_the_open_document() {
    let (workspace, id) = workspace_with_troop();

    assert_eq!(workspace.path(id).unwrap(), PathBuf::from("TroopInfo.sox"));
    assert_eq!(workspace.title(id).unwrap(), "TroopInfo.sox");
    assert_eq!(workspace.record_count(id).unwrap(), 1);
    assert!(workspace.diagnostics(id).unwrap().is_empty());
}

#[test]
fn document_ids_follow_open_order() {
    let document = TroopDocument::parse(troop_fixture()).unwrap();
    let mut workspace = Workspace::new();
    let first = workspace.open_loaded(
        PathBuf::from("first.sox"),
        Document::Troop(document.clone()),
    );
    let second = workspace.open_loaded(PathBuf::from("second.sox"), Document::Troop(document));

    assert_eq!(workspace.document_ids(), &[first, second]);
}
