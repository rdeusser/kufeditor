#![allow(
    clippy::unwrap_used,
    reason = "tests use controlled temporary paths and fixed-size fixtures"
)]

use std::{fs, path::PathBuf};

use kufeditor_workspace::{
    Document, DocumentEdit, TroopDocument, TroopField, Workspace, WorkspaceError, load_path,
};
use tempfile::tempdir;

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
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
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
fn save_as_replaces_the_target_and_marks_the_snapshot_clean() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("TroopInfo.sox");
    fs::write(&target, b"old target").unwrap();
    let (mut workspace, id) = workspace_with_troop();
    workspace.apply(id, move_speed(175)).unwrap();

    let request = workspace.prepare_save(id, Some(target.clone())).unwrap();
    let saved = request.run().unwrap();
    workspace.finish_save(saved).unwrap();

    assert_eq!(workspace.path(id).unwrap(), target);
    assert!(!workspace.is_dirty(id).unwrap());
    let loaded = load_path(target).unwrap();
    let Document::Troop(document) = loaded.document();
    assert_eq!(document.value(0, TroopField::MoveSpeed).unwrap(), 175);
}

#[test]
fn an_edit_after_save_started_remains_dirty() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("TroopInfo.sox");
    let (mut workspace, id) = workspace_with_troop();

    workspace.apply(id, move_speed(175)).unwrap();
    let request = workspace.prepare_save(id, Some(target.clone())).unwrap();
    workspace.apply(id, move_speed(200)).unwrap();
    let saved = request.run().unwrap();
    workspace.finish_save(saved).unwrap();

    assert!(workspace.is_dirty(id).unwrap());
    assert_eq!(
        workspace.troop_value(id, 0, TroopField::MoveSpeed).unwrap(),
        200
    );
    let loaded = load_path(target).unwrap();
    let Document::Troop(document) = loaded.document();
    assert_eq!(document.value(0, TroopField::MoveSpeed).unwrap(), 175);
}

#[test]
fn failed_save_keeps_the_old_path_and_dirty_state() {
    let directory = tempdir().unwrap();
    let impossible = directory.path().join("missing").join("TroopInfo.sox");
    let (mut workspace, id) = workspace_with_troop();
    let old_path = workspace.path(id).unwrap().to_path_buf();
    workspace.apply(id, move_speed(175)).unwrap();

    let request = workspace.prepare_save(id, Some(impossible)).unwrap();
    let token = request.token();
    assert!(request.run().is_err());
    workspace.finish_save_failure(id, token).unwrap();

    assert_eq!(workspace.path(id).unwrap(), old_path);
    assert!(workspace.is_dirty(id).unwrap());
}

#[test]
fn only_one_save_can_be_in_flight_for_a_document() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("TroopInfo.sox");
    let (mut workspace, id) = workspace_with_troop();

    let request = workspace.prepare_save(id, Some(target)).unwrap();
    let second = workspace.prepare_save(id, None).unwrap_err();

    assert!(matches!(second, WorkspaceError::SaveInProgress(found) if found == id));
    workspace.finish_save_failure(id, request.token()).unwrap();
}

#[test]
fn load_rejects_a_non_sox_file() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("TroopInfo.txt");
    fs::write(&path, troop_fixture()).unwrap();

    let error = load_path(path.clone()).unwrap_err();

    assert!(matches!(error, WorkspaceError::UnsupportedFile { path: found } if found == path));
}
