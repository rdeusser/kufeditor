#![allow(
    clippy::unwrap_used,
    reason = "tests use controlled temporary paths and fixed-size fixtures"
)]

use std::{fs, path::PathBuf};

use kufeditor_formats::FormatError;
use kufeditor_workspace::{
    Document, DocumentEdit, SkillDocument, SkillTextField, TroopDocument, TroopField, Workspace,
    WorkspaceError, load_path,
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

fn skill_fixture(trailing: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&(-2_i32).to_le_bytes());
    push_skill_text(&mut bytes, b"@(S_Elemental)");
    push_skill_text(&mut bytes, b"IL_SKL_Elem.tga");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&25_u32.to_le_bytes());
    bytes.extend_from_slice(b"THEND");
    bytes.resize(bytes.len() + 59, b' ');
    bytes.extend_from_slice(trailing);
    bytes
}

fn push_skill_text(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(value);
}

fn ascii_hex(bytes: &[u8]) -> Vec<u8> {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = Vec::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(*DIGITS.get(usize::from(byte >> 4)).unwrap());
        encoded.push(*DIGITS.get(usize::from(byte & 0x0f)).unwrap());
    }
    encoded
}

fn decode_ascii_hex(bytes: &[u8]) -> Vec<u8> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|&[high, low]| (hex_value(high) << 4) | hex_value(low))
        .collect()
}

const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("fixture contains a non-hexadecimal byte"),
    }
}

fn workspace_with_skill() -> (Workspace, kufeditor_workspace::DocumentId) {
    let document = SkillDocument::parse(skill_fixture(&[])).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("SkillInfo.sox"), Document::Skill(document));
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
    let Document::Troop(document) = loaded.document() else {
        panic!("saved TroopInfo was detected as SkillInfo");
    };
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
    let Document::Troop(document) = loaded.document() else {
        panic!("saved TroopInfo was detected as SkillInfo");
    };
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

#[test]
fn load_detects_raw_and_ascii_hex_skill_info() {
    let directory = tempdir().unwrap();
    let raw = skill_fixture(&[0xde, 0xad]);
    let cases = [("raw.sox", raw.clone()), ("encoded.sox", ascii_hex(&raw))];

    for (name, bytes) in cases {
        let path = directory.path().join(name);
        fs::write(&path, bytes).unwrap();

        let loaded = load_path(path).unwrap();
        let Document::Skill(document) = loaded.document() else {
            panic!("SkillInfo was detected as TroopInfo");
        };
        assert_eq!(document.skill_id(0).unwrap(), -2);
        assert_eq!(
            document.text(0, SkillTextField::LocalizationKey).unwrap(),
            "@(S_Elemental)"
        );
    }
}

#[test]
fn supported_extension_with_unknown_sox_content_is_typed() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("Unknown.sox");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.resize(8 + 64, 0);
    fs::write(&path, bytes).unwrap();

    let error = load_path(path.clone()).unwrap_err();

    assert!(matches!(
        error,
        WorkspaceError::Parse {
            path: found,
            source: FormatError::UnsupportedSox,
        } if found == path
    ));
}

#[test]
fn saving_edited_ascii_hex_skill_preserves_the_envelope_and_tail() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("SkillInfo.sox");
    let tail = [0xde, 0xad, 0xbe, 0xef];
    fs::write(&path, ascii_hex(&skill_fixture(&tail))).unwrap();
    let loaded = load_path(path.clone()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.insert_loaded(loaded);

    workspace
        .apply(
            id,
            DocumentEdit::SetSkillText {
                record: 0,
                field: SkillTextField::IconPath,
                value: "IL_SKL_Fire.tga".to_owned(),
            },
        )
        .unwrap();
    let saved = workspace.prepare_save(id, None).unwrap().run().unwrap();
    workspace.finish_save(saved).unwrap();

    let encoded = fs::read(&path).unwrap();
    assert!(encoded.iter().all(u8::is_ascii_hexdigit));
    assert!(decode_ascii_hex(&encoded).ends_with(&tail));
    let loaded = load_path(path).unwrap();
    let Document::Skill(document) = loaded.document() else {
        panic!("saved SkillInfo was detected as TroopInfo");
    };
    assert_eq!(
        document.text(0, SkillTextField::IconPath).unwrap(),
        "IL_SKL_Fire.tga"
    );
}

#[test]
fn an_edit_after_a_skill_save_started_remains_dirty() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("SkillInfo.sox");
    let (mut workspace, id) = workspace_with_skill();

    workspace
        .apply(
            id,
            DocumentEdit::SetSkillMaxLevel {
                record: 0,
                value: 50,
            },
        )
        .unwrap();
    let request = workspace.prepare_save(id, Some(target.clone())).unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetSkillMaxLevel {
                record: 0,
                value: 75,
            },
        )
        .unwrap();
    let saved = request.run().unwrap();
    workspace.finish_save(saved).unwrap();

    assert!(workspace.is_dirty(id).unwrap());
    assert_eq!(workspace.skill_max_level(id, 0).unwrap(), 75);
    let loaded = load_path(target).unwrap();
    let Document::Skill(document) = loaded.document() else {
        panic!("saved SkillInfo was detected as TroopInfo");
    };
    assert_eq!(document.max_level(0).unwrap(), 50);
}
