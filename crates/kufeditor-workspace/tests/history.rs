#![allow(
    clippy::unwrap_used,
    reason = "synthetic fixtures use known fixed-size byte ranges"
)]

use std::path::PathBuf;

use kufeditor_formats::{DiagnosticField, FormatError};
use kufeditor_workspace::{
    Document, DocumentEdit, DocumentKind, SkillDocument, SkillTextField, TextSoxDocument,
    TextSoxField, TroopDocument, TroopField, Workspace, WorkspaceError,
};

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

fn workspace_with_skill() -> (Workspace, kufeditor_workspace::DocumentId) {
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

fn workspace_with_text_sox() -> (Workspace, kufeditor_workspace::DocumentId) {
    let document = TextSoxDocument::parse(text_sox_fixture()).unwrap();
    let mut workspace = Workspace::new();
    let id = workspace.open_loaded(
        PathBuf::from("StringTable.sox"),
        Document::TextSox(document),
    );
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
            DocumentEdit::SetSkillId {
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
            DocumentEdit::SetSkillId {
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
        WorkspaceError::Format(FormatError::SkillUtf8 {
            record: 0,
            field: SkillTextField::LocalizationKey,
            ..
        })
    ));
}

#[test]
fn text_sox_kind_count_and_projections_return_wire_values() {
    let (workspace, id) = workspace_with_text_sox();

    assert_eq!(workspace.document_kind(id).unwrap(), DocumentKind::TextSox);
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
            DocumentEdit::SetTextSoxText {
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
            DocumentEdit::SetTextSoxText {
                record: 0,
                value: "Omega".to_owned(),
            },
        )
        .unwrap();
    workspace.undo(id).unwrap();
    workspace
        .apply(
            id,
            DocumentEdit::SetTextSoxText {
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
            FormatError::TextSoxEmptyText { record: 0 },
        ),
        (
            0,
            "Café".to_owned(),
            FormatError::TextSoxInvalidTextByte {
                record: 0,
                index: 3,
                byte: 0xc3,
            },
        ),
        (
            0,
            "Longer".to_owned(),
            FormatError::TextSoxTooLong {
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
                field: DiagnosticField::TextSox(TextSoxField::Text),
            },
        ),
    ];

    for (record, value, expected) in cases {
        let (mut workspace, id) = workspace_with_text_sox();
        let saved_state = workspace.state_id(id).unwrap();

        let error = workspace
            .apply(id, DocumentEdit::SetTextSoxText { record, value })
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
    let text_edit = DocumentEdit::SetTextSoxText {
        record: 0,
        value: "Omega".to_owned(),
    };
    let (mut troop_workspace, troop_id) = workspace_with_troop();
    let troop_state = troop_workspace.state_id(troop_id).unwrap();
    let troop_error = troop_workspace
        .apply(troop_id, text_edit.clone())
        .unwrap_err();
    assert!(matches!(troop_error, WorkspaceError::NotTextSox(id) if id == troop_id));
    assert_eq!(troop_workspace.state_id(troop_id).unwrap(), troop_state);
    assert!(!troop_workspace.can_undo(troop_id).unwrap());

    let (mut skill_workspace, skill_id) = workspace_with_skill();
    let skill_state = skill_workspace.state_id(skill_id).unwrap();
    let skill_error = skill_workspace.apply(skill_id, text_edit).unwrap_err();
    assert!(matches!(skill_error, WorkspaceError::NotTextSox(id) if id == skill_id));
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
            DocumentEdit::SetSkillId {
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
    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.field == DiagnosticField::TextSox(TextSoxField::Index)
        })
    );
}
