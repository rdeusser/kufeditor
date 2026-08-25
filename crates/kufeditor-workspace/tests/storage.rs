#![allow(
    clippy::unwrap_used,
    reason = "tests use controlled temporary paths and fixed-size fixtures"
)]

use std::{fs, path::PathBuf};

#[cfg(unix)]
use std::{ffi::OsString, os::unix::ffi::OsStringExt};

use kufeditor_formats::FormatError;
use kufeditor_workspace::{
    Document, DocumentEdit, DocumentID, SaveDocument, SaveNumberTarget, SkillDocument,
    SkillTextField, TextSOXDocument, TroopDocument, TroopField, Workspace, WorkspaceError,
    load_path,
};
use tempfile::tempdir;

const SAVE_CONTEXT_SIZE: usize = 0x438;
const SAVE_MAIN_SIZE: usize = 0x154;
const SAVE_PADDED_SIZE: usize = 0x8000;
const SAVE_MAP_NAME_OFFSET: usize = 0x20;

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

fn workspace_with_troop() -> (Workspace, kufeditor_workspace::DocumentID) {
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

fn workspace_with_skill() -> (Workspace, kufeditor_workspace::DocumentID) {
    let document = SkillDocument::parse(skill_fixture(&[])).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("SkillInfo.sox"), Document::Skill(document));
    (workspace, id)
}

fn text_sox_fixture(trailing: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&41_u32.to_le_bytes());
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.extend_from_slice(b"Alpha");
    bytes.extend_from_slice(&9001_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(b"Beta");
    bytes.extend_from_slice(trailing);
    bytes
}

fn workspace_with_text_sox() -> (Workspace, kufeditor_workspace::DocumentID) {
    let document = TextSOXDocument::parse(text_sox_fixture(&[])).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(
        PathBuf::from("StringTable.sox"),
        Document::TextSOX(document),
    );
    (workspace, id)
}

fn save_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    append_u32(&mut bytes, 0);
    append_u32(&mut bytes, 0x6e);

    append_u32(&mut bytes, u32::MAX);
    bytes.resize(bytes.len() + SAVE_CONTEXT_SIZE - size_of::<u32>(), 0);

    append_u32(&mut bytes, 0);
    let main = bytes.len();
    bytes.resize(main + SAVE_MAIN_SIZE, 0);
    let map_name = bytes
        .get_mut(main + SAVE_MAP_NAME_OFFSET..main + SAVE_MAP_NAME_OFFSET + 32)
        .unwrap();
    map_name.get_mut(..5).unwrap().copy_from_slice(b"MapA\0");
    map_name
        .get_mut(5..31)
        .unwrap()
        .copy_from_slice(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");

    append_u32(&mut bytes, 0);
    append_i32(&mut bytes, -1);
    append_u32(&mut bytes, 0);
    append_u32(&mut bytes, 0);
    for _ in 0..20 {
        append_u32(&mut bytes, 0);
    }
    append_u32(&mut bytes, 0);

    bytes.resize(SAVE_PADDED_SIZE, 0);
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    let length = u32::try_from(bytes.len()).unwrap();
    patch_u32(&mut bytes, 0, length);
    bytes
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn patch_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes
        .get_mut(offset..offset + size_of::<u32>())
        .unwrap()
        .copy_from_slice(&value.to_le_bytes());
}

fn workspace_with_save(path: PathBuf) -> (Workspace, DocumentID) {
    let document = SaveDocument::parse(save_fixture()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(path, Document::Save(document));
    (workspace, id)
}

fn campaign_index(value: i64) -> DocumentEdit {
    DocumentEdit::SetSaveNumber {
        target: SaveNumberTarget::CampaignIndex,
        value,
    }
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
fn load_supports_save_and_sox_extensions_case_insensitively() {
    let directory = tempdir().unwrap();
    let source = save_fixture();

    for name in ["campaign.sav", "campaign.SAV"] {
        let path = directory.path().join(name);
        fs::write(&path, &source).unwrap();

        let loaded = load_path(path.clone()).unwrap();
        let Document::Save(document) = loaded.document() else {
            panic!("Crusaders save was detected as a SOX document");
        };
        assert_eq!(document.number(SaveNumberTarget::CampaignIndex).unwrap(), 0);

        let mut workspace = Workspace::new();
        let id = workspace.insert_loaded(loaded);
        let saved = workspace.prepare_save(id, None).unwrap().run().unwrap();
        workspace.finish_save(saved).unwrap();
        assert_eq!(fs::read(path).unwrap(), source);
    }

    for name in ["TroopInfo.sox", "TroopInfo.SOX"] {
        let path = directory.path().join(name);
        fs::write(&path, troop_fixture()).unwrap();

        let loaded = load_path(path).unwrap();
        assert!(matches!(loaded.document(), Document::Troop(_)));
    }
}

#[test]
fn load_rejects_an_unknown_save_extension_before_reading() {
    let directory = tempdir().unwrap();
    let paths = [
        directory.path().join("missing.campaign"),
        directory.path().join("missing"),
    ];

    for path in paths {
        let error = load_path(path.clone()).unwrap_err();

        assert!(matches!(error, WorkspaceError::UnsupportedFile { path: found } if found == path));
    }
}

#[cfg(unix)]
#[test]
fn load_rejects_a_non_utf8_save_extension_before_reading() {
    let directory = tempdir().unwrap();
    let path = directory
        .path()
        .join(OsString::from_vec(b"missing.\xff".to_vec()));

    let error = load_path(path.clone()).unwrap_err();

    assert!(matches!(error, WorkspaceError::UnsupportedFile { path: found } if found == path));
}

#[test]
fn save_as_without_an_extension_appends_the_document_extension_and_writes_there() {
    let directory = tempdir().unwrap();
    let cases = vec![
        (
            "campaign-copy",
            Document::Save(SaveDocument::parse(save_fixture()).unwrap()),
            "sav",
        ),
        (
            "troop-copy",
            Document::Troop(TroopDocument::parse(troop_fixture()).unwrap()),
            "sox",
        ),
        (
            "skill-copy",
            Document::Skill(SkillDocument::parse(skill_fixture(&[])).unwrap()),
            "sox",
        ),
        (
            "text-copy",
            Document::TextSOX(TextSOXDocument::parse(text_sox_fixture(&[])).unwrap()),
            "sox",
        ),
    ];

    for (name, document, extension) in cases {
        let target = directory.path().join(name);
        let expected = target.with_extension(extension);
        let mut workspace = Workspace::new();
        let id = workspace.open_loaded(PathBuf::from("source"), document);

        let saved = workspace
            .prepare_save(id, Some(target.clone()))
            .unwrap()
            .run()
            .unwrap();
        workspace.finish_save(saved).unwrap();

        assert_eq!(workspace.path(id).unwrap(), expected);
        assert!(expected.is_file());
        assert!(!target.exists());
    }
}

#[test]
fn save_as_rejects_wrong_explicit_extensions_without_starting_a_save() {
    let directory = tempdir().unwrap();
    let cases = vec![
        (
            "campaign",
            Document::Save(SaveDocument::parse(save_fixture()).unwrap()),
            "sox",
            "sav",
        ),
        (
            "troop",
            Document::Troop(TroopDocument::parse(troop_fixture()).unwrap()),
            "sav",
            "sox",
        ),
        (
            "skill",
            Document::Skill(SkillDocument::parse(skill_fixture(&[])).unwrap()),
            "sav",
            "sox",
        ),
        (
            "text",
            Document::TextSOX(TextSOXDocument::parse(text_sox_fixture(&[])).unwrap()),
            "sav",
            "sox",
        ),
    ];

    for (name, document, actual, expected) in cases {
        let path = directory.path().join(format!("{name}.{actual}"));
        let mut workspace = Workspace::new();
        let id = workspace.open_loaded(PathBuf::from("source"), document);

        let error = workspace.prepare_save(id, Some(path.clone())).unwrap_err();
        let display = error.to_string();

        assert!(matches!(
            error,
            WorkspaceError::WrongExtension {
                path: found,
                expected: found_expected,
                actual: found_actual,
            } if found == path && found_expected == expected && found_actual == actual
        ));
        assert!(display.contains(&format!(".{expected}")));
        assert!(display.contains(&format!(".{actual}")));
        assert!(!workspace.save_in_progress(id).unwrap());
    }
}

#[test]
fn save_as_accepts_uppercase_extensions_without_rewriting_the_path() {
    let directory = tempdir().unwrap();
    let cases = vec![
        (
            "campaign.SAV",
            Document::Save(SaveDocument::parse(save_fixture()).unwrap()),
        ),
        (
            "troop.SOX",
            Document::Troop(TroopDocument::parse(troop_fixture()).unwrap()),
        ),
        (
            "skill.SOX",
            Document::Skill(SkillDocument::parse(skill_fixture(&[])).unwrap()),
        ),
        (
            "text.SOX",
            Document::TextSOX(TextSOXDocument::parse(text_sox_fixture(&[])).unwrap()),
        ),
    ];

    for (name, document) in cases {
        let target = directory.path().join(name);
        let mut workspace = Workspace::new();
        let id = workspace.open_loaded(PathBuf::from("source"), document);

        let saved = workspace
            .prepare_save(id, Some(target.clone()))
            .unwrap()
            .run()
            .unwrap();
        workspace.finish_save(saved).unwrap();

        assert_eq!(workspace.path(id).unwrap(), target);
        assert!(target.is_file());
    }
}

#[cfg(unix)]
#[test]
fn save_as_wrong_non_utf8_extension_retains_the_original_path() {
    let directory = tempdir().unwrap();
    let target = directory
        .path()
        .join(OsString::from_vec(b"campaign.\xff".to_vec()));
    let (mut workspace, id) = workspace_with_save(PathBuf::from("campaign.sav"));

    let error = workspace
        .prepare_save(id, Some(target.clone()))
        .unwrap_err();

    assert!(matches!(
        error,
        WorkspaceError::WrongExtension {
            path: found,
            expected: "sav",
            actual,
        } if found == target && actual == "\u{fffd}"
    ));
    assert!(!workspace.save_in_progress(id).unwrap());
}

#[test]
fn normal_save_keeps_the_current_path_exactly() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("legacy-campaign-name");
    let source = save_fixture();
    let (mut workspace, id) = workspace_with_save(path.clone());

    let saved = workspace.prepare_save(id, None).unwrap().run().unwrap();
    workspace.finish_save(saved).unwrap();

    assert_eq!(workspace.path(id).unwrap(), path);
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn concurrent_save_rebases_the_live_document_and_preserves_history() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("campaign.sav");
    let (mut workspace, id) = workspace_with_save(PathBuf::from("original.sav"));

    workspace.apply(id, campaign_index(1)).unwrap();
    let request = workspace.prepare_save(id, Some(target.clone())).unwrap();
    let saved = request.run().unwrap();
    let committed = fs::read(&target).unwrap();
    workspace.apply(id, campaign_index(2)).unwrap();

    workspace.finish_save(saved).unwrap();

    assert_eq!(
        workspace
            .save_number(id, SaveNumberTarget::CampaignIndex)
            .unwrap(),
        2
    );
    assert!(workspace.is_dirty(id).unwrap());

    assert!(workspace.undo(id).unwrap());
    assert_eq!(
        workspace
            .save_number(id, SaveNumberTarget::CampaignIndex)
            .unwrap(),
        1
    );
    assert!(!workspace.is_dirty(id).unwrap());

    let round_trip = directory.path().join("round-trip.sav");
    let request = workspace
        .prepare_save(id, Some(round_trip.clone()))
        .unwrap();
    let token = request.token();
    request.run().unwrap();
    workspace.finish_save_failure(id, token).unwrap();
    assert_eq!(fs::read(round_trip).unwrap(), committed);

    assert!(workspace.redo(id).unwrap());
    assert_eq!(
        workspace
            .save_number(id, SaveNumberTarget::CampaignIndex)
            .unwrap(),
        2
    );
    assert!(workspace.is_dirty(id).unwrap());
}

#[test]
fn stale_save_completion_keeps_the_current_save_token() {
    let directory = tempdir().unwrap();
    let original_path = PathBuf::from("original.sav");
    let (mut workspace, id) = workspace_with_save(original_path.clone());

    let first = workspace
        .prepare_save(id, Some(directory.path().join("first.sav")))
        .unwrap();
    let first_token = first.token();
    let stale = first.run().unwrap();
    workspace.finish_save_failure(id, first_token).unwrap();

    let current = workspace
        .prepare_save(id, Some(directory.path().join("current.sav")))
        .unwrap();
    let current_token = current.token();

    let error = workspace.finish_save(stale).unwrap_err();

    assert!(matches!(
        error,
        WorkspaceError::StaleSave { document, token }
            if document == id && token == first_token
    ));
    assert!(workspace.save_in_progress(id).unwrap());
    assert_eq!(workspace.path(id).unwrap(), original_path);
    workspace.finish_save_failure(id, current_token).unwrap();
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
            source: FormatError::UnsupportedSOX,
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

#[test]
fn load_detects_raw_and_ascii_hex_text_sox() {
    let directory = tempdir().unwrap();
    let raw = text_sox_fixture(&[0xde, 0xad]);
    let cases = [("raw.sox", raw.clone()), ("encoded.sox", ascii_hex(&raw))];

    for (name, bytes) in cases {
        let path = directory.path().join(name);
        fs::write(&path, bytes).unwrap();

        let loaded = load_path(path).unwrap();
        let Document::TextSOX(document) = loaded.document() else {
            panic!("text SOX was detected as a named schema");
        };
        assert_eq!(document.record_count(), 2);
        assert_eq!(document.record_index(0).unwrap(), 41);
        assert_eq!(document.record_index(1).unwrap(), 9001);
        assert_eq!(document.text(0).unwrap(), "Alpha");
        assert_eq!(document.text(1).unwrap(), "Beta");
    }
}

#[test]
fn text_sox_save_as_is_reparsable_and_marks_the_snapshot_clean() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("StringTable.sox");
    fs::write(&target, b"old target").unwrap();
    let (mut workspace, id) = workspace_with_text_sox();
    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "Omega".to_owned(),
            },
        )
        .unwrap();

    let saved = workspace
        .prepare_save(id, Some(target.clone()))
        .unwrap()
        .run()
        .unwrap();
    workspace.finish_save(saved).unwrap();

    assert_eq!(workspace.path(id).unwrap(), target);
    assert!(!workspace.is_dirty(id).unwrap());
    let loaded = load_path(target).unwrap();
    let Document::TextSOX(document) = loaded.document() else {
        panic!("saved text SOX was detected as a named schema");
    };
    assert_eq!(document.text(0).unwrap(), "Omega");
}

#[test]
fn edited_ascii_hex_text_sox_is_uppercase_and_preserves_indices_and_tail() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("StringTable.sox");
    let tail = [0xde, 0xad, 0xbe, 0xef];
    let mut encoded = ascii_hex(&text_sox_fixture(&tail));
    for byte in &mut encoded {
        byte.make_ascii_lowercase();
    }
    fs::write(&path, encoded).unwrap();
    let loaded = load_path(path.clone()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.insert_loaded(loaded);
    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "Omega".to_owned(),
            },
        )
        .unwrap();

    let saved = workspace.prepare_save(id, None).unwrap().run().unwrap();
    workspace.finish_save(saved).unwrap();

    let encoded = fs::read(&path).unwrap();
    assert!(
        encoded
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'A'..=b'F'))
    );
    assert!(decode_ascii_hex(&encoded).ends_with(&tail));
    let loaded = load_path(path).unwrap();
    let Document::TextSOX(document) = loaded.document() else {
        panic!("saved text SOX was detected as a named schema");
    };
    assert_eq!(document.record_index(0).unwrap(), 41);
    assert_eq!(document.record_index(1).unwrap(), 9001);
    assert_eq!(document.text(0).unwrap(), "Omega");
}

#[test]
fn an_edit_after_a_text_sox_save_started_remains_dirty() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("StringTable.sox");
    let (mut workspace, id) = workspace_with_text_sox();

    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "First".to_owned(),
            },
        )
        .unwrap();
    let request = workspace.prepare_save(id, Some(target.clone())).unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "Later".to_owned(),
            },
        )
        .unwrap();
    let saved = request.run().unwrap();
    workspace.finish_save(saved).unwrap();

    assert!(workspace.is_dirty(id).unwrap());
    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Later");
    let loaded = load_path(target).unwrap();
    let Document::TextSOX(document) = loaded.document() else {
        panic!("saved text SOX was detected as a named schema");
    };
    assert_eq!(document.text(0).unwrap(), "First");
}

#[test]
fn text_sox_shortening_save_preserves_the_initial_session_budget() {
    let directory = tempdir().unwrap();
    let target = directory.path().join("StringTable.sox");
    let (mut workspace, id) = workspace_with_text_sox();
    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "One".to_owned(),
            },
        )
        .unwrap();

    let saved = workspace
        .prepare_save(id, Some(target))
        .unwrap()
        .run()
        .unwrap();
    workspace.finish_save(saved).unwrap();

    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "One");
    assert_eq!(workspace.text_sox_max_length(id, 0).unwrap(), 5);
}
