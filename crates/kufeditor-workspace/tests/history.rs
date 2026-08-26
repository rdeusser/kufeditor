#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use known fixed-size byte ranges"
)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use kufeditor_formats::{DiagnosticField, FormatError};
use kufeditor_workspace::{
    ApplyOutcome, DiagnosticLocation, Document, DocumentEdit, DocumentID, DocumentKind,
    STGDocument, STGEventTarget, STGFloatTarget, STGFloatValue, STGHeaderTextField,
    STGNumberTarget, STGParameterTarget, STGScriptKind, STGScriptTarget, STGStructuralChange,
    STGStructuralEdit, STGTailStatus, STGText, STGTextTarget, STGUnitField, STGUnitFloatField,
    STGValue, STGValueKind, STGValueTarget, SaveDocument, SaveEditor, SaveNumberTarget,
    SaveTextField, SaveUnitField, SkillDocument, SkillTextField, StateID, TextSOXDocument,
    TextSOXField, TroopDocument, TroopField, Workspace, WorkspaceError,
};

#[path = "../../kufeditor-formats/tests/support/stg.rs"]
#[allow(
    dead_code,
    reason = "the shared STG fixture exposes offsets and variants used by format-level tests"
)]
mod stg_support;

const SAVE_CONTEXT_SIZE: usize = 0x438;
const SAVE_MAIN_SIZE: usize = 0x154;
const SAVE_UNIT_SIZE: usize = 483;
const SAVE_MAP_NAME_OFFSET: usize = 0x20;
const SAVE_SKILL_DATA_OFFSET: usize = 71;
const SAVE_UCD_OFFSET: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HistoryState {
    state: StateID,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
}

fn history_state(workspace: &Workspace, id: DocumentID) -> HistoryState {
    HistoryState {
        state: workspace.state_id(id).unwrap(),
        dirty: workspace.is_dirty(id).unwrap(),
        can_undo: workspace.can_undo(id).unwrap(),
        can_redo: workspace.can_redo(id).unwrap(),
    }
}

fn encoded_bytes(workspace: &mut Workspace, id: DocumentID, path: &Path) -> Vec<u8> {
    let request = workspace
        .prepare_save(id, Some(path.to_path_buf()))
        .unwrap();
    let token = request.token();
    request.run().unwrap();
    workspace.finish_save_failure(id, token).unwrap();
    fs::read(path).unwrap()
}

fn assert_unchanged_edit(
    workspace: &mut Workspace,
    id: DocumentID,
    edit: DocumentEdit,
    path: &Path,
) {
    let before_state = history_state(workspace, id);
    let before_bytes = encoded_bytes(workspace, id, path);

    assert_eq!(workspace.apply(id, edit).unwrap(), ApplyOutcome::Unchanged);

    assert_eq!(history_state(workspace, id), before_state);
    assert_eq!(encoded_bytes(workspace, id, path), before_bytes);
}

fn assert_not_save<T>(result: Result<T, WorkspaceError>, id: DocumentID) {
    match result {
        Err(WorkspaceError::NotSave(actual)) => assert_eq!(actual, id),
        Err(error) => panic!("expected NotSave, got {error}"),
        Ok(_) => panic!("expected NotSave, got success"),
    }
}

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

fn workspace_with_troop() -> (Workspace, kufeditor_workspace::DocumentID) {
    let document = TroopDocument::parse(troop_fixture()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("TroopInfo.sox"), Document::Troop(document));
    (workspace, id)
}

fn skill_fixture(localization_key: &[u8], icon_path: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&(-2_i32).to_le_bytes());
    push_skill_text(&mut bytes, localization_key);
    push_skill_text(&mut bytes, icon_path);
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&25_u32.to_le_bytes());
    bytes.extend_from_slice(b"THEND");
    bytes.resize(bytes.len() + 59, b' ');
    bytes
}

fn push_skill_text(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(value);
}

fn workspace_with_skill() -> (Workspace, kufeditor_workspace::DocumentID) {
    let document =
        SkillDocument::parse(skill_fixture(b"@(S_Elemental)", b"IL_SKL_Elem.tga")).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("SkillInfo.sox"), Document::Skill(document));
    (workspace, id)
}

fn text_sox_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&100_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&41_u32.to_le_bytes());
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.extend_from_slice(b"Alpha");
    bytes.extend_from_slice(&41_u32.to_le_bytes());
    bytes.extend_from_slice(&6_u16.to_le_bytes());
    bytes.extend_from_slice(b"Beta\r\n");
    bytes.extend_from_slice(&[0xde, 0xad]);
    bytes
}

fn workspace_with_text_sox() -> (Workspace, kufeditor_workspace::DocumentID) {
    let document = TextSOXDocument::parse(text_sox_fixture()).unwrap();
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

    let context = bytes.len();
    append_u32(&mut bytes, u32::MAX);
    bytes.resize(context + SAVE_CONTEXT_SIZE, 0);
    bytes
        .get_mut(context + 4..context + 9)
        .unwrap()
        .copy_from_slice(b"Alpha");

    append_u32(&mut bytes, 2);
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

    append_u32(&mut bytes, 1);
    let unit = bytes.len();
    bytes.resize(unit + SAVE_UNIT_SIZE, 0);
    patch_u32(&mut bytes, unit + SAVE_UCD_OFFSET, 99);
    bytes
        .get_mut(unit + SAVE_SKILL_DATA_OFFSET..unit + SAVE_SKILL_DATA_OFFSET + 24)
        .unwrap()
        .copy_from_slice(&[
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
            0xae, 0xaf, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
        ]);

    append_i32(&mut bytes, -1);
    append_u32(&mut bytes, 1);
    bytes.extend_from_slice(&[61, 60, 62, 63]);
    append_u32(&mut bytes, 6_400);
    append_u32(&mut bytes, 1);
    append_u32(&mut bytes, 0x0203_0405);
    for slot in 0_i32..20 {
        append_i32(&mut bytes, slot - 1);
    }
    append_i32(&mut bytes, -2);

    bytes.resize(0x8000, 0);
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

fn workspace_with_save() -> (Workspace, DocumentID, Vec<u8>) {
    let source = save_fixture();
    let document = SaveDocument::parse(source.clone()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("campaign.sav"), Document::Save(document));
    (workspace, id, source)
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

#[test]
fn document_kinds_report_both_supported_formats() {
    let (troop_workspace, troop_id) = workspace_with_troop();
    let (skill_workspace, skill_id) = workspace_with_skill();

    assert_eq!(
        troop_workspace.document_kind(troop_id).unwrap(),
        DocumentKind::TroopInfo
    );
    assert_eq!(
        skill_workspace.document_kind(skill_id).unwrap(),
        DocumentKind::SkillInfo
    );
}

#[test]
fn skill_projections_return_the_wire_values() {
    let (workspace, id) = workspace_with_skill();

    assert_eq!(workspace.record_count(id).unwrap(), 1);
    assert_eq!(workspace.skill_id(id, 0).unwrap(), -2);
    assert_eq!(workspace.skill_type(id, 0).unwrap(), 2);
    assert_eq!(workspace.skill_max_level(id, 0).unwrap(), 25);
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::LocalizationKey)
            .unwrap(),
        "@(S_Elemental)"
    );
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::IconPath)
            .unwrap(),
        "IL_SKL_Elem.tga"
    );
    assert!(workspace.diagnostics(id).unwrap().is_empty());
}

#[test]
fn skill_numeric_edits_have_exact_undo_and_redo() {
    let (mut workspace, id) = workspace_with_skill();
    let saved_state = workspace.state_id(id).unwrap();

    workspace
        .apply(
            id,
            DocumentEdit::SetSkillID {
                record: 0,
                value: 8,
            },
        )
        .unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetSkillType {
                record: 0,
                value: 1,
            },
        )
        .unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetSkillMaxLevel {
                record: 0,
                value: 50,
            },
        )
        .unwrap();

    assert_eq!(workspace.skill_id(id, 0).unwrap(), 8);
    assert_eq!(workspace.skill_type(id, 0).unwrap(), 1);
    assert_eq!(workspace.skill_max_level(id, 0).unwrap(), 50);

    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.skill_max_level(id, 0).unwrap(), 25);
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.skill_type(id, 0).unwrap(), 2);
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.skill_id(id, 0).unwrap(), -2);
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());

    for _ in 0..3 {
        assert!(workspace.redo(id).unwrap());
    }
    assert_eq!(workspace.skill_id(id, 0).unwrap(), 8);
    assert_eq!(workspace.skill_type(id, 0).unwrap(), 1);
    assert_eq!(workspace.skill_max_level(id, 0).unwrap(), 50);
}

#[test]
fn skill_owned_text_edits_have_exact_undo_and_redo() {
    let (mut workspace, id) = workspace_with_skill();
    let saved_state = workspace.state_id(id).unwrap();

    workspace
        .apply(
            id,
            DocumentEdit::SetSkillText {
                record: 0,
                field: SkillTextField::LocalizationKey,
                value: "@(S_Fire)".to_owned(),
            },
        )
        .unwrap();
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

    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::LocalizationKey)
            .unwrap(),
        "@(S_Fire)"
    );
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::IconPath)
            .unwrap(),
        "IL_SKL_Fire.tga"
    );

    assert!(workspace.undo(id).unwrap());
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::IconPath)
            .unwrap(),
        "IL_SKL_Elem.tga"
    );
    assert!(workspace.undo(id).unwrap());
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::LocalizationKey)
            .unwrap(),
        "@(S_Elemental)"
    );
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());

    for _ in 0..2 {
        assert!(workspace.redo(id).unwrap());
    }
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::LocalizationKey)
            .unwrap(),
        "@(S_Fire)"
    );
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::IconPath)
            .unwrap(),
        "IL_SKL_Fire.tga"
    );
}

#[test]
fn editing_skill_text_after_undo_discards_the_redo_branch() {
    let (mut workspace, id) = workspace_with_skill();

    workspace
        .apply(
            id,
            DocumentEdit::SetSkillText {
                record: 0,
                field: SkillTextField::LocalizationKey,
                value: "@(S_Fire)".to_owned(),
            },
        )
        .unwrap();
    workspace.undo(id).unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetSkillText {
                record: 0,
                field: SkillTextField::LocalizationKey,
                value: "@(S_Ice)".to_owned(),
            },
        )
        .unwrap();

    assert!(!workspace.can_redo(id).unwrap());
    assert_eq!(
        workspace
            .skill_text(id, 0, SkillTextField::LocalizationKey)
            .unwrap(),
        "@(S_Ice)"
    );
}

#[test]
fn wrong_kind_edits_are_state_neutral() {
    let (mut troop_workspace, troop_id) = workspace_with_troop();
    let troop_state = troop_workspace.state_id(troop_id).unwrap();
    let troop_error = troop_workspace
        .apply(
            troop_id,
            DocumentEdit::SetSkillID {
                record: 0,
                value: 8,
            },
        )
        .unwrap_err();

    assert!(matches!(troop_error, WorkspaceError::NotSkill(id) if id == troop_id));
    assert_eq!(troop_workspace.state_id(troop_id).unwrap(), troop_state);
    assert!(!troop_workspace.can_undo(troop_id).unwrap());
    assert!(!troop_workspace.is_dirty(troop_id).unwrap());

    let (mut skill_workspace, skill_id) = workspace_with_skill();
    let skill_state = skill_workspace.state_id(skill_id).unwrap();
    let skill_error = skill_workspace
        .apply(skill_id, move_speed(175))
        .unwrap_err();

    assert!(matches!(skill_error, WorkspaceError::NotTroop(id) if id == skill_id));
    assert_eq!(skill_workspace.state_id(skill_id).unwrap(), skill_state);
    assert!(!skill_workspace.can_undo(skill_id).unwrap());
    assert!(!skill_workspace.is_dirty(skill_id).unwrap());
}

#[test]
fn invalid_skill_text_projection_stays_typed() {
    let document = SkillDocument::parse(skill_fixture(&[0xff], b"IL_SKL_Elem.tga")).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("SkillInfo.sox"), Document::Skill(document));

    let error = workspace
        .skill_text(id, 0, SkillTextField::LocalizationKey)
        .unwrap_err();

    assert!(matches!(
        error,
        WorkspaceError::Format(FormatError::SkillUTF8 {
            record: 0,
            field: SkillTextField::LocalizationKey,
            ..
        })
    ));
}

#[test]
fn text_sox_kind_count_and_projections_return_wire_values() {
    let (workspace, id) = workspace_with_text_sox();

    assert_eq!(workspace.document_kind(id).unwrap(), DocumentKind::TextSOX);
    assert_eq!(workspace.record_count(id).unwrap(), 2);
    assert_eq!(workspace.text_sox_index(id, 0).unwrap(), 41);
    assert_eq!(workspace.text_sox_max_length(id, 0).unwrap(), 5);
    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Alpha");
    assert_eq!(workspace.text_sox_index(id, 1).unwrap(), 41);
    assert_eq!(workspace.text_sox_max_length(id, 1).unwrap(), 6);
    assert_eq!(workspace.text_sox_text(id, 1).unwrap(), "Beta\r\n");
}

#[test]
fn text_sox_owned_text_edit_has_exact_undo_and_redo() {
    let (mut workspace, id) = workspace_with_text_sox();
    let saved_state = workspace.state_id(id).unwrap();

    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "Omega".to_owned(),
            },
        )
        .unwrap();

    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Omega");
    assert!(workspace.is_dirty(id).unwrap());
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Alpha");
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());
    assert!(workspace.redo(id).unwrap());
    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Omega");
    assert!(workspace.is_dirty(id).unwrap());
}

#[test]
fn editing_text_sox_after_undo_discards_the_redo_branch() {
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
    workspace.undo(id).unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetTextSOXText {
                record: 0,
                value: "Gamma".to_owned(),
            },
        )
        .unwrap();

    assert!(!workspace.can_redo(id).unwrap());
    assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Gamma");
}

#[test]
fn invalid_text_sox_edits_create_no_state_or_history() {
    let cases = [
        (
            0,
            String::new(),
            FormatError::TextSOXEmptyText { record: 0 },
        ),
        (
            0,
            "Café".to_owned(),
            FormatError::TextSOXInvalidTextByte {
                record: 0,
                index: 3,
                byte: 0xc3,
            },
        ),
        (
            0,
            "Longer".to_owned(),
            FormatError::TextSOXTooLong {
                record: 0,
                length: 6,
                maximum: 5,
            },
        ),
        (
            2,
            "Nope".to_owned(),
            FormatError::RecordOutOfRange {
                record: 2,
                record_count: 2,
                field: DiagnosticField::TextSOX(TextSOXField::Text),
            },
        ),
    ];

    for (record, value, expected) in cases {
        let (mut workspace, id) = workspace_with_text_sox();
        let saved_state = workspace.state_id(id).unwrap();

        let error = workspace
            .apply(id, DocumentEdit::SetTextSOXText { record, value })
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            WorkspaceError::Format(expected).to_string()
        );
        assert_eq!(workspace.state_id(id).unwrap(), saved_state);
        assert!(!workspace.can_undo(id).unwrap());
        assert!(!workspace.can_redo(id).unwrap());
        assert!(!workspace.is_dirty(id).unwrap());
        assert_eq!(workspace.text_sox_text(id, 0).unwrap(), "Alpha");
    }
}

#[test]
fn wrong_kind_text_sox_edits_are_state_neutral() {
    let text_edit = DocumentEdit::SetTextSOXText {
        record: 0,
        value: "Omega".to_owned(),
    };
    let (mut troop_workspace, troop_id) = workspace_with_troop();
    let troop_state = troop_workspace.state_id(troop_id).unwrap();
    let troop_error = troop_workspace
        .apply(troop_id, text_edit.clone())
        .unwrap_err();
    assert!(matches!(troop_error, WorkspaceError::NotTextSOX(id) if id == troop_id));
    assert_eq!(troop_workspace.state_id(troop_id).unwrap(), troop_state);
    assert!(!troop_workspace.can_undo(troop_id).unwrap());

    let (mut skill_workspace, skill_id) = workspace_with_skill();
    let skill_state = skill_workspace.state_id(skill_id).unwrap();
    let skill_error = skill_workspace.apply(skill_id, text_edit).unwrap_err();
    assert!(matches!(skill_error, WorkspaceError::NotTextSOX(id) if id == skill_id));
    assert_eq!(skill_workspace.state_id(skill_id).unwrap(), skill_state);
    assert!(!skill_workspace.can_undo(skill_id).unwrap());

    let (mut text_workspace, text_id) = workspace_with_text_sox();
    let text_state = text_workspace.state_id(text_id).unwrap();
    let troop_error = text_workspace.apply(text_id, move_speed(175)).unwrap_err();
    assert!(matches!(troop_error, WorkspaceError::NotTroop(id) if id == text_id));
    assert_eq!(text_workspace.state_id(text_id).unwrap(), text_state);
    assert!(!text_workspace.can_undo(text_id).unwrap());

    let skill_error = text_workspace
        .apply(
            text_id,
            DocumentEdit::SetSkillID {
                record: 0,
                value: 8,
            },
        )
        .unwrap_err();
    assert!(matches!(skill_error, WorkspaceError::NotSkill(id) if id == text_id));
    assert_eq!(text_workspace.state_id(text_id).unwrap(), text_state);
    assert!(!text_workspace.can_undo(text_id).unwrap());
}

#[test]
fn text_sox_duplicate_index_diagnostics_retain_the_typed_field() {
    let (workspace, id) = workspace_with_text_sox();

    let diagnostics = workspace.diagnostics(id).unwrap();

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().enumerate().all(|(record, diagnostic)| {
        diagnostic.location
            == DiagnosticLocation::Record {
                record,
                field: DiagnosticField::TextSOX(TextSOXField::Index),
            }
    }));
}

#[test]
fn save_numeric_edit_has_one_exact_undo_and_redo() {
    let (mut workspace, id, _) = workspace_with_save();
    let target = SaveNumberTarget::Unit {
        unit: 0,
        field: SaveUnitField::UCD,
    };
    let saved_state = workspace.state_id(id).unwrap();

    assert_eq!(
        workspace
            .apply(id, DocumentEdit::SetSaveNumber { target, value: 2 })
            .unwrap(),
        ApplyOutcome::Changed
    );
    assert_eq!(
        workspace.document_kind(id).unwrap(),
        DocumentKind::CrusadersSave
    );
    assert_eq!(workspace.save_number(id, target).unwrap(), 2);
    assert_ne!(workspace.state_id(id).unwrap(), saved_state);
    assert!(workspace.is_dirty(id).unwrap());
    assert!(workspace.can_undo(id).unwrap());
    assert!(!workspace.can_redo(id).unwrap());

    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.save_number(id, target).unwrap(), 99);
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());
    assert!(!workspace.can_undo(id).unwrap());
    assert!(workspace.can_redo(id).unwrap());

    assert!(workspace.redo(id).unwrap());
    assert_eq!(workspace.save_number(id, target).unwrap(), 2);
    assert!(workspace.is_dirty(id).unwrap());
    assert!(workspace.can_undo(id).unwrap());
    assert!(!workspace.can_redo(id).unwrap());
}

#[test]
fn save_text_undo_and_redo_restore_complete_images() {
    let directory = tempfile::tempdir().unwrap();
    let (mut workspace, id, source) = workspace_with_save();
    let main = size_of::<u32>() + size_of::<u32>() + SAVE_CONTEXT_SIZE + size_of::<u32>();
    let text_range = main + SAVE_MAP_NAME_OFFSET..main + SAVE_MAP_NAME_OFFSET + 32;

    assert_eq!(
        workspace
            .apply(
                id,
                DocumentEdit::SetSaveText {
                    field: SaveTextField::MapName,
                    value: "MapB".to_owned(),
                },
            )
            .unwrap(),
        ApplyOutcome::Changed
    );
    let edited = encoded_bytes(&mut workspace, id, &directory.path().join("edited.sav"));
    let mut edited_image = [0_u8; 32];
    edited_image.get_mut(..4).unwrap().copy_from_slice(b"MapB");
    assert_eq!(
        edited.get(text_range.clone()),
        Some(edited_image.as_slice())
    );

    assert!(workspace.undo(id).unwrap());
    let undone = encoded_bytes(&mut workspace, id, &directory.path().join("undone.sav"));
    assert_eq!(undone, source);

    assert!(workspace.redo(id).unwrap());
    let redone = encoded_bytes(&mut workspace, id, &directory.path().join("redone.sav"));
    assert_eq!(redone, edited);
}

#[test]
fn equal_save_edits_preserve_state_history_and_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let (mut workspace, id, _) = workspace_with_save();
    let target = SaveNumberTarget::Unit {
        unit: 0,
        field: SaveUnitField::UCD,
    };

    assert_unchanged_edit(
        &mut workspace,
        id,
        DocumentEdit::SetSaveNumber { target, value: 99 },
        &directory.path().join("equal-number.sav"),
    );
    assert_unchanged_edit(
        &mut workspace,
        id,
        DocumentEdit::SetSaveText {
            field: SaveTextField::MapName,
            value: "MapA".to_owned(),
        },
        &directory.path().join("equal-text.sav"),
    );
}

#[test]
fn equal_save_edit_preserves_an_existing_redo_branch() {
    let directory = tempfile::tempdir().unwrap();
    let (mut workspace, id, _) = workspace_with_save();
    let target = SaveNumberTarget::Unit {
        unit: 0,
        field: SaveUnitField::UCD,
    };

    assert_eq!(
        workspace
            .apply(id, DocumentEdit::SetSaveNumber { target, value: 2 })
            .unwrap(),
        ApplyOutcome::Changed
    );
    assert!(workspace.undo(id).unwrap());
    assert!(workspace.can_redo(id).unwrap());

    assert_unchanged_edit(
        &mut workspace,
        id,
        DocumentEdit::SetSaveNumber { target, value: 99 },
        &directory.path().join("redo-branch.sav"),
    );

    assert!(workspace.redo(id).unwrap());
    assert_eq!(workspace.save_number(id, target).unwrap(), 2);
}

#[test]
fn save_edits_on_a_non_save_document_are_state_neutral() {
    let directory = tempfile::tempdir().unwrap();
    let (mut workspace, id) = workspace_with_troop();
    let before_state = history_state(&workspace, id);
    let path = directory.path().join("TroopInfo.sox");
    let before_bytes = encoded_bytes(&mut workspace, id, &path);

    assert_not_save(
        workspace.apply(
            id,
            DocumentEdit::SetSaveNumber {
                target: SaveNumberTarget::CampaignIndex,
                value: 1,
            },
        ),
        id,
    );
    assert_not_save(
        workspace.apply(
            id,
            DocumentEdit::SetSaveText {
                field: SaveTextField::MapName,
                value: "MapB".to_owned(),
            },
        ),
        id,
    );

    assert_eq!(history_state(&workspace, id), before_state);
    assert_eq!(encoded_bytes(&mut workspace, id, &path), before_bytes);
}

#[test]
fn save_projections_are_checked_and_complete() {
    let (workspace, id, _) = workspace_with_save();
    let target = SaveNumberTarget::Unit {
        unit: 0,
        field: SaveUnitField::UCD,
    };

    assert!(workspace.save_has_size_prefix(id).unwrap());
    assert!(workspace.save_has_context(id).unwrap());
    assert_eq!(workspace.save_context_text(id).unwrap(), ["Alpha"]);
    assert_eq!(workspace.save_unit_count(id).unwrap(), 1);
    assert_eq!(workspace.save_roster_count(id).unwrap(), 1);
    assert_eq!(workspace.save_second_array_count(id).unwrap(), 1);
    assert_eq!(workspace.record_count(id).unwrap(), 1);
    assert_eq!(workspace.save_number(id, target).unwrap(), 99);
    assert_eq!(
        workspace.save_number_storage_bounds(id, target).unwrap(),
        (0, i64::from(u32::MAX))
    );
    assert_eq!(
        workspace.save_number_editor(id, target).unwrap(),
        SaveEditor::UCD
    );
    assert_eq!(
        workspace.save_text(id, SaveTextField::MapName).unwrap(),
        "MapA"
    );
    assert_eq!(
        workspace.save_unit_skill_data(id, 0).unwrap(),
        [
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
            0xae, 0xaf, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
        ]
    );
    let diagnostics = workspace.save_diagnostics(id).unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.location == DiagnosticLocation::Save(target) })
    );
    assert_eq!(workspace.diagnostics(id).unwrap().len(), diagnostics.len());

    let (troop_workspace, troop_id) = workspace_with_troop();
    assert_not_save(troop_workspace.save_has_size_prefix(troop_id), troop_id);
    assert_not_save(troop_workspace.save_has_context(troop_id), troop_id);
    assert_not_save(troop_workspace.save_context_text(troop_id), troop_id);
    assert_not_save(troop_workspace.save_unit_count(troop_id), troop_id);
    assert_not_save(troop_workspace.save_roster_count(troop_id), troop_id);
    assert_not_save(troop_workspace.save_second_array_count(troop_id), troop_id);
    assert_not_save(troop_workspace.save_number(troop_id, target), troop_id);
    assert_not_save(
        troop_workspace.save_number_storage_bounds(troop_id, target),
        troop_id,
    );
    assert_not_save(
        troop_workspace.save_number_editor(troop_id, target),
        troop_id,
    );
    assert_not_save(
        troop_workspace.save_text(troop_id, SaveTextField::MapName),
        troop_id,
    );
    assert_not_save(troop_workspace.save_unit_skill_data(troop_id, 0), troop_id);
    assert_not_save(troop_workspace.save_diagnostics(troop_id), troop_id);
}

#[test]
fn every_existing_sox_edit_variant_treats_equal_values_as_no_ops() {
    let directory = tempfile::tempdir().unwrap();

    let (mut troop_workspace, troop_id) = workspace_with_troop();
    assert_unchanged_edit(
        &mut troop_workspace,
        troop_id,
        move_speed(130),
        &directory.path().join("troop.sox"),
    );

    let (mut skill_workspace, skill_id) = workspace_with_skill();
    for (index, edit) in [
        DocumentEdit::SetSkillID {
            record: 0,
            value: -2,
        },
        DocumentEdit::SetSkillType {
            record: 0,
            value: 2,
        },
        DocumentEdit::SetSkillMaxLevel {
            record: 0,
            value: 25,
        },
        DocumentEdit::SetSkillText {
            record: 0,
            field: SkillTextField::LocalizationKey,
            value: "@(S_Elemental)".to_owned(),
        },
        DocumentEdit::SetSkillText {
            record: 0,
            field: SkillTextField::IconPath,
            value: "IL_SKL_Elem.tga".to_owned(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        assert_unchanged_edit(
            &mut skill_workspace,
            skill_id,
            edit,
            &directory.path().join(format!("skill-{index}.sox")),
        );
    }

    let (mut text_workspace, text_id) = workspace_with_text_sox();
    assert_unchanged_edit(
        &mut text_workspace,
        text_id,
        DocumentEdit::SetTextSOXText {
            record: 0,
            value: "Alpha".to_owned(),
        },
        &directory.path().join("text.sox"),
    );
}

fn workspace_with_stg() -> (Workspace, DocumentID) {
    workspace_with_stg_bytes(stg_support::complete_stg_fixture().bytes)
}

fn workspace_with_stg_bytes(bytes: Vec<u8>) -> (Workspace, DocumentID) {
    let document = STGDocument::parse(bytes).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(PathBuf::from("campaign.stg"), Document::STG(document));
    (workspace, id)
}

fn stg_event_id_target() -> STGNumberTarget {
    STGNumberTarget::EventID { block: 0, event: 0 }
}

fn stg_action_string_target() -> STGValueTarget {
    STGValueTarget::ScriptParameter(STGParameterTarget {
        script: STGScriptTarget {
            block: 0,
            event: 0,
            kind: STGScriptKind::Action,
            script: 0,
        },
        parameter: 0,
    })
}

#[test]
fn stg_document_kind_and_projections_cover_the_typed_document() {
    let (workspace, id) = workspace_with_stg();

    assert_eq!(
        workspace.document_kind(id).unwrap(),
        DocumentKind::CrusadersSTG
    );
    assert_eq!(workspace.record_count(id).unwrap(), 1);
    assert_eq!(workspace.stg_unit_count(id).unwrap(), 1);
    assert_eq!(workspace.stg_area_count(id).unwrap(), Some(1));
    assert_eq!(workspace.stg_variable_count(id).unwrap(), Some(4));
    assert_eq!(workspace.stg_event_block_count(id).unwrap(), Some(2));
    assert_eq!(workspace.stg_footer_count(id).unwrap(), Some(2));
    assert!(matches!(
        workspace.stg_tail_status(id).unwrap(),
        STGTailStatus::Parsed { suffix } if suffix == [0xf0, 0x0d, 0xca, 0xfe]
    ));

    let block = workspace.stg_event_block(id, 0).unwrap();
    assert_eq!(block.header, 0x0102_0304);
    assert_eq!(block.event_count, 2);
    let event_target = STGEventTarget { block: 0, event: 0 };
    let event = workspace.stg_event(id, event_target).unwrap();
    assert_eq!(event.id, 500);
    assert_eq!(event.description.decoded(), Some("Primary Event"));
    assert_eq!(event.condition_count, 1);
    assert_eq!(event.action_count, 1);

    let condition_target = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let condition = workspace.stg_script(id, condition_target).unwrap();
    assert_eq!(condition.id, 19);
    assert_eq!(condition.parameter_count, 2);
    let parameter_target = STGParameterTarget {
        script: condition_target,
        parameter: 0,
    };
    let parameter = workspace.stg_parameter(id, parameter_target).unwrap();
    assert_eq!(parameter.value, STGValue::Integer(23));
    assert_eq!(
        workspace
            .stg_value(id, STGValueTarget::ScriptParameter(parameter_target),)
            .unwrap(),
        STGValue::Integer(23)
    );

    assert_eq!(
        workspace
            .stg_number(
                id,
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UCD,
                },
            )
            .unwrap(),
        0
    );
    assert_eq!(
        workspace
            .stg_float(
                id,
                STGFloatTarget::Unit {
                    unit: 0,
                    field: STGUnitFloatField::LeaderHPOverride,
                },
            )
            .unwrap(),
        STGFloatValue::from_bits(0)
    );
    assert_eq!(
        workspace
            .stg_text(id, STGTextTarget::Header(STGHeaderTextField::MapFilename),)
            .unwrap(),
        STGText::Decoded("".into())
    );
    assert!(!workspace.diagnostics(id).unwrap().is_empty());
}

#[test]
fn stg_scalar_edits_have_one_state_each_and_exact_undo_redo() {
    let (mut workspace, id) = workspace_with_stg();
    let saved_state = workspace.state_id(id).unwrap();
    let number_target = stg_event_id_target();
    let float_target = STGFloatTarget::Unit {
        unit: 0,
        field: STGUnitFloatField::LeaderHPOverride,
    };
    let fixed_text_target = STGTextTarget::Header(STGHeaderTextField::MapFilename);
    let dynamic_text_target = STGTextTarget::ParameterString {
        value: stg_action_string_target(),
    };
    let edits = [
        DocumentEdit::SetSTGNumber {
            target: number_target,
            value: 900,
        },
        DocumentEdit::SetSTGFloat {
            target: float_target,
            value: STGFloatValue::from_bits(0x7fc0_0002),
        },
        DocumentEdit::SetSTGText {
            target: fixed_text_target,
            value: "Map B".to_owned(),
        },
        DocumentEdit::SetSTGText {
            target: dynamic_text_target,
            value: "changed action".to_owned(),
        },
    ];

    let mut states = vec![saved_state];
    for edit in edits {
        assert_eq!(workspace.apply(id, edit).unwrap(), ApplyOutcome::Changed);
        states.push(workspace.state_id(id).unwrap());
    }
    states.sort_unstable_by_key(|state| format!("{state:?}"));
    states.dedup();
    assert_eq!(states.len(), 5);
    let retained = workspace.history_retained_bytes(id).unwrap();
    assert!(retained > 0);
    assert_eq!(workspace.stg_number(id, number_target).unwrap(), 900);
    assert_eq!(
        workspace.stg_float(id, float_target).unwrap(),
        STGFloatValue::from_bits(0x7fc0_0002)
    );
    assert_eq!(
        workspace.stg_text(id, fixed_text_target).unwrap().decoded(),
        Some("Map B")
    );
    assert_eq!(
        workspace
            .stg_text(id, dynamic_text_target)
            .unwrap()
            .decoded(),
        Some("changed action")
    );

    for _ in 0..4 {
        assert!(workspace.undo(id).unwrap());
    }
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());
    assert_eq!(workspace.stg_number(id, number_target).unwrap(), 500);
    assert_eq!(
        workspace.stg_float(id, float_target).unwrap(),
        STGFloatValue::from_bits(0)
    );
    assert_eq!(
        workspace.stg_text(id, fixed_text_target).unwrap().decoded(),
        Some("")
    );
    assert_eq!(
        workspace
            .stg_text(id, dynamic_text_target)
            .unwrap()
            .decoded(),
        Some("action")
    );
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);

    for _ in 0..4 {
        assert!(workspace.redo(id).unwrap());
    }
    assert!(workspace.is_dirty(id).unwrap());
    assert_eq!(workspace.stg_number(id, number_target).unwrap(), 900);
    assert_eq!(
        workspace.stg_float(id, float_target).unwrap(),
        STGFloatValue::from_bits(0x7fc0_0002)
    );
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);
}

#[test]
fn stg_structural_edits_are_single_exact_history_entries() {
    let (mut workspace, id) = workspace_with_stg();
    let saved_state = workspace.state_id(id).unwrap();
    let insert = STGStructuralEdit::InsertEvent {
        target: STGEventTarget { block: 0, event: 1 },
    };
    assert_eq!(
        workspace
            .apply(id, DocumentEdit::EditSTGStructure { edit: insert })
            .unwrap(),
        ApplyOutcome::Changed
    );
    assert_eq!(workspace.stg_event_block(id, 0).unwrap().event_count, 3);
    assert_eq!(
        workspace.pending_undo_stg_change(id).unwrap(),
        Some(STGStructuralChange::RemoveEvent {
            target: STGEventTarget { block: 0, event: 1 },
        })
    );
    assert_eq!(workspace.pending_redo_stg_change(id).unwrap(), None);
    let after_insert = workspace.state_id(id).unwrap();
    assert_ne!(after_insert, saved_state);

    let value_target = stg_action_string_target();
    assert_eq!(
        workspace
            .apply(
                id,
                DocumentEdit::EditSTGStructure {
                    edit: STGStructuralEdit::ChangeValueType {
                        target: value_target,
                        kind: STGValueKind::Float,
                    },
                },
            )
            .unwrap(),
        ApplyOutcome::Changed
    );
    assert_eq!(
        workspace.stg_value(id, value_target).unwrap(),
        STGValue::Float(STGFloatValue::from_bits(0))
    );
    assert_eq!(
        workspace.pending_undo_stg_change(id).unwrap(),
        Some(STGStructuralChange::ReplaceValue {
            target: value_target,
        })
    );
    let retained = workspace.history_retained_bytes(id).unwrap();

    assert!(workspace.undo(id).unwrap());
    assert_eq!(
        workspace.stg_value(id, value_target).unwrap(),
        STGValue::String(STGText::Decoded("action".into()))
    );
    assert_eq!(
        workspace.pending_redo_stg_change(id).unwrap(),
        Some(STGStructuralChange::ReplaceValue {
            target: value_target,
        })
    );
    assert_eq!(
        workspace.pending_undo_stg_change(id).unwrap(),
        Some(STGStructuralChange::RemoveEvent {
            target: STGEventTarget { block: 0, event: 1 },
        })
    );
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.stg_event_block(id, 0).unwrap().event_count, 2);
    assert_eq!(
        workspace.pending_redo_stg_change(id).unwrap(),
        Some(STGStructuralChange::InsertEvent {
            target: STGEventTarget { block: 0, event: 1 },
        })
    );
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);

    assert!(workspace.redo(id).unwrap());
    assert_eq!(workspace.stg_event_block(id, 0).unwrap().event_count, 3);
    assert_eq!(
        workspace.pending_undo_stg_change(id).unwrap(),
        Some(STGStructuralChange::RemoveEvent {
            target: STGEventTarget { block: 0, event: 1 },
        })
    );
    assert!(workspace.redo(id).unwrap());
    assert_eq!(
        workspace.stg_value(id, value_target).unwrap(),
        STGValue::Float(STGFloatValue::from_bits(0))
    );
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);
}

#[test]
fn stg_history_dispatches_every_structural_action_family() {
    let (mut workspace, id) = workspace_with_stg();
    let saved_state = workspace.state_id(id).unwrap();
    let condition = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let event_one_condition = STGScriptTarget {
        block: 0,
        event: 1,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    let event_one_action = STGScriptTarget {
        kind: STGScriptKind::Action,
        ..event_one_condition
    };
    let value = STGValueTarget::ScriptParameter(STGParameterTarget {
        script: condition,
        parameter: 1,
    });
    let edits = [
        STGStructuralEdit::ChangeScriptType {
            target: condition,
            type_id: 19,
        },
        STGStructuralEdit::ChangeValueType {
            target: value,
            kind: STGValueKind::String,
        },
        STGStructuralEdit::InsertScript {
            target: event_one_condition,
            type_id: 27,
        },
        STGStructuralEdit::RemoveScript {
            target: event_one_condition,
        },
        STGStructuralEdit::InsertScript {
            target: event_one_action,
            type_id: 7,
        },
        STGStructuralEdit::RemoveScript {
            target: event_one_action,
        },
        STGStructuralEdit::InsertEvent {
            target: STGEventTarget { block: 0, event: 1 },
        },
        STGStructuralEdit::RemoveEvent {
            target: STGEventTarget { block: 0, event: 1 },
        },
    ];

    let mut previous_state = saved_state;
    for edit in edits {
        assert_eq!(
            workspace
                .apply(id, DocumentEdit::EditSTGStructure { edit })
                .unwrap(),
            ApplyOutcome::Changed
        );
        let state = workspace.state_id(id).unwrap();
        assert_ne!(state, previous_state);
        previous_state = state;
    }
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        3
    );
    assert_eq!(
        workspace.stg_value(id, value).unwrap(),
        STGValue::String(STGText::Decoded("".into()))
    );
    assert_eq!(workspace.stg_event_block(id, 0).unwrap().event_count, 2);
    let retained = workspace.history_retained_bytes(id).unwrap();

    for _ in 0..edits.len() {
        assert!(workspace.undo(id).unwrap());
    }
    assert_eq!(workspace.state_id(id).unwrap(), saved_state);
    assert!(!workspace.is_dirty(id).unwrap());
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        2
    );
    assert_eq!(
        workspace.stg_value(id, value).unwrap(),
        STGValue::Float(STGFloatValue::from_bits((-0.0_f32).to_bits()))
    );
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);

    for _ in 0..edits.len() {
        assert!(workspace.redo(id).unwrap());
    }
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        3
    );
    assert_eq!(
        workspace.stg_value(id, value).unwrap(),
        STGValue::String(STGText::Decoded("".into()))
    );
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);
}

#[test]
fn stg_history_repairs_a_longer_same_id_script_exactly() {
    let fixture = stg_support::complete_stg_fixture();
    let mut bytes = fixture.bytes;
    bytes
        .get_mut(
            fixture.offsets.condition_parameter_count
                ..fixture.offsets.condition_parameter_count + size_of::<u32>(),
        )
        .unwrap()
        .copy_from_slice(&4_u32.to_le_bytes());
    let mut extra = Vec::new();
    append_u32(&mut extra, 0);
    append_i32(&mut extra, 77);
    append_u32(&mut extra, 0);
    append_i32(&mut extra, 88);
    bytes.splice(
        fixture.offsets.action_count..fixture.offsets.action_count,
        extra,
    );

    let (mut workspace, id) = workspace_with_stg_bytes(bytes);
    let condition = STGScriptTarget {
        block: 0,
        event: 0,
        kind: STGScriptKind::Condition,
        script: 0,
    };
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        4
    );
    workspace
        .apply(
            id,
            DocumentEdit::EditSTGStructure {
                edit: STGStructuralEdit::ChangeScriptType {
                    target: condition,
                    type_id: 19,
                },
            },
        )
        .unwrap();
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        3
    );
    assert_eq!(
        workspace
            .stg_parameter(
                id,
                STGParameterTarget {
                    script: condition,
                    parameter: 2,
                },
            )
            .unwrap()
            .value,
        STGValue::Integer(77)
    );
    assert!(workspace.undo(id).unwrap());
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        4
    );
    assert!(workspace.redo(id).unwrap());
    assert_eq!(
        workspace.stg_script(id, condition).unwrap().parameter_count,
        3
    );
}

#[test]
fn stg_neutral_and_wrong_kind_edits_preserve_history() {
    let (mut workspace, id) = workspace_with_stg();
    let target = stg_event_id_target();
    workspace
        .apply(id, DocumentEdit::SetSTGNumber { target, value: 900 })
        .unwrap();
    workspace.undo(id).unwrap();
    let before = history_state(&workspace, id);
    let retained = workspace.history_retained_bytes(id).unwrap();

    assert_eq!(
        workspace
            .apply(id, DocumentEdit::SetSTGNumber { target, value: 500 },)
            .unwrap(),
        ApplyOutcome::Unchanged
    );
    assert_eq!(history_state(&workspace, id), before);
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), retained);

    let error = workspace.apply(id, move_speed(175)).unwrap_err();
    assert!(matches!(error, WorkspaceError::NotTroop(actual) if actual == id));
    assert_eq!(history_state(&workspace, id), before);

    let (mut troop_workspace, troop_id) = workspace_with_troop();
    let troop_before = history_state(&troop_workspace, troop_id);
    let error = troop_workspace
        .apply(troop_id, DocumentEdit::SetSTGNumber { target, value: 900 })
        .unwrap_err();
    assert!(matches!(error, WorkspaceError::NotSTG(actual) if actual == troop_id));
    assert_eq!(history_state(&troop_workspace, troop_id), troop_before);
}

#[test]
fn stg_history_limit_evicts_oldest_entries_and_rejects_oversized_entries() {
    let target = stg_event_id_target();
    let (mut probe, probe_id) = workspace_with_stg();
    probe
        .apply(probe_id, DocumentEdit::SetSTGNumber { target, value: 600 })
        .unwrap();
    let one_entry = probe.history_retained_bytes(probe_id).unwrap();
    assert!(one_entry > 0);

    let document = STGDocument::parse(stg_support::complete_stg_fixture().bytes).unwrap();
    let mut oldest_first = Workspace::with_stg_history_limit(one_entry * 2);
    let oldest_first_id =
        oldest_first.open_loaded(PathBuf::from("oldest-first.stg"), Document::STG(document));
    for value in [600, 700, 800] {
        oldest_first
            .apply(
                oldest_first_id,
                DocumentEdit::SetSTGNumber { target, value },
            )
            .unwrap();
    }
    assert_eq!(
        oldest_first
            .history_retained_bytes(oldest_first_id)
            .unwrap(),
        one_entry * 2
    );
    assert!(oldest_first.undo(oldest_first_id).unwrap());
    assert_eq!(
        oldest_first.stg_number(oldest_first_id, target).unwrap(),
        700
    );
    assert!(oldest_first.undo(oldest_first_id).unwrap());
    assert_eq!(
        oldest_first.stg_number(oldest_first_id, target).unwrap(),
        600
    );
    assert!(!oldest_first.can_undo(oldest_first_id).unwrap());

    let document = STGDocument::parse(stg_support::complete_stg_fixture().bytes).unwrap();
    let mut workspace = Workspace::with_stg_history_limit(one_entry);
    let id = workspace.open_loaded(PathBuf::from("bounded.stg"), Document::STG(document));
    workspace
        .apply(id, DocumentEdit::SetSTGNumber { target, value: 600 })
        .unwrap();
    workspace
        .apply(id, DocumentEdit::SetSTGNumber { target, value: 700 })
        .unwrap();
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), one_entry);
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.stg_number(id, target).unwrap(), 600);
    assert!(!workspace.can_undo(id).unwrap());
    assert!(workspace.is_dirty(id).unwrap());
    assert!(workspace.redo(id).unwrap());
    assert_eq!(workspace.stg_number(id, target).unwrap(), 700);
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), one_entry);

    let document = STGDocument::parse(stg_support::complete_stg_fixture().bytes).unwrap();
    let mut rejected = Workspace::with_stg_history_limit(one_entry - 1);
    let rejected_id = rejected.open_loaded(PathBuf::from("rejected.stg"), Document::STG(document));
    let before = history_state(&rejected, rejected_id);
    let error = rejected
        .apply(
            rejected_id,
            DocumentEdit::SetSTGNumber { target, value: 600 },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        WorkspaceError::HistoryBudgetExceeded { requested, maximum }
            if requested == one_entry && maximum == one_entry - 1
    ));
    assert_eq!(history_state(&rejected, rejected_id), before);
    assert_eq!(rejected.stg_number(rejected_id, target).unwrap(), 500);
    assert_eq!(rejected.history_retained_bytes(rejected_id).unwrap(), 0);
}

#[test]
fn stg_history_limit_clears_redo_before_deterministic_eviction() {
    let target = stg_event_id_target();
    let (mut probe, probe_id) = workspace_with_stg();
    probe
        .apply(probe_id, DocumentEdit::SetSTGNumber { target, value: 600 })
        .unwrap();
    let one_entry = probe.history_retained_bytes(probe_id).unwrap();

    let document = STGDocument::parse(stg_support::complete_stg_fixture().bytes).unwrap();
    let mut workspace = Workspace::with_stg_history_limit(one_entry * 2);
    let id = workspace.open_loaded(PathBuf::from("branch.stg"), Document::STG(document));
    for value in [600, 700] {
        workspace
            .apply(id, DocumentEdit::SetSTGNumber { target, value })
            .unwrap();
    }
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), one_entry * 2);
    assert!(workspace.undo(id).unwrap());
    assert!(workspace.can_redo(id).unwrap());

    workspace
        .apply(id, DocumentEdit::SetSTGNumber { target, value: 800 })
        .unwrap();
    assert!(!workspace.can_redo(id).unwrap());
    assert_eq!(workspace.history_retained_bytes(id).unwrap(), one_entry * 2);
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.stg_number(id, target).unwrap(), 600);
    assert!(workspace.undo(id).unwrap());
    assert_eq!(workspace.stg_number(id, target).unwrap(), 500);
    assert!(!workspace.is_dirty(id).unwrap());
}

#[test]
fn stg_text_entry_is_rejected_before_document_or_history_changes() {
    let target = STGTextTarget::ParameterString {
        value: stg_action_string_target(),
    };
    let (mut probe, probe_id) = workspace_with_stg();
    probe
        .apply(
            probe_id,
            DocumentEdit::SetSTGText {
                target,
                value: "a much longer action".to_owned(),
            },
        )
        .unwrap();
    let one_entry = probe.history_retained_bytes(probe_id).unwrap();

    let document = STGDocument::parse(stg_support::complete_stg_fixture().bytes).unwrap();
    let mut workspace = Workspace::with_stg_history_limit(one_entry - 1);
    let id = workspace.open_loaded(PathBuf::from("text-limit.stg"), Document::STG(document));
    workspace
        .apply(
            id,
            DocumentEdit::SetSTGNumber {
                target: stg_event_id_target(),
                value: 600,
            },
        )
        .unwrap();
    assert!(workspace.undo(id).unwrap());
    let before = history_state(&workspace, id);
    let retained_before = workspace.history_retained_bytes(id).unwrap();
    assert!(before.can_redo);
    let error = workspace
        .apply(
            id,
            DocumentEdit::SetSTGText {
                target,
                value: "a much longer action".to_owned(),
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        WorkspaceError::HistoryBudgetExceeded { requested, maximum }
            if requested == one_entry && maximum == one_entry - 1
    ));
    assert_eq!(history_state(&workspace, id), before);
    assert_eq!(
        workspace.stg_text(id, target).unwrap().decoded(),
        Some("action")
    );
    assert_eq!(
        workspace.history_retained_bytes(id).unwrap(),
        retained_before
    );
}

#[test]
fn stg_structural_entry_is_rejected_before_document_or_history_changes() {
    let edit = STGStructuralEdit::RemoveEvent {
        target: STGEventTarget { block: 0, event: 0 },
    };
    let (mut probe, probe_id) = workspace_with_stg();
    probe
        .apply(probe_id, DocumentEdit::EditSTGStructure { edit })
        .unwrap();
    let structural_entry = probe.history_retained_bytes(probe_id).unwrap();

    let document = STGDocument::parse(stg_support::complete_stg_fixture().bytes).unwrap();
    let mut workspace = Workspace::with_stg_history_limit(structural_entry - 1);
    let id = workspace.open_loaded(
        PathBuf::from("structure-limit.stg"),
        Document::STG(document),
    );
    workspace
        .apply(
            id,
            DocumentEdit::SetSTGNumber {
                target: stg_event_id_target(),
                value: 600,
            },
        )
        .unwrap();
    assert!(workspace.undo(id).unwrap());
    let before = history_state(&workspace, id);
    let retained_before = workspace.history_retained_bytes(id).unwrap();
    assert!(before.can_redo);

    let error = workspace
        .apply(id, DocumentEdit::EditSTGStructure { edit })
        .unwrap_err();
    assert!(matches!(
        error,
        WorkspaceError::HistoryBudgetExceeded { requested, maximum }
            if requested == structural_entry && maximum == structural_entry - 1
    ));
    assert_eq!(history_state(&workspace, id), before);
    assert_eq!(workspace.stg_event_block(id, 0).unwrap().event_count, 2);
    assert_eq!(
        workspace.history_retained_bytes(id).unwrap(),
        retained_before
    );
}
