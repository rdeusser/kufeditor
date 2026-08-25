#![allow(
    clippy::unwrap_used,
    reason = "literal fixtures use controlled temporary paths and statically valid sizes"
)]

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use kufeditor_game::{
    CatalogFileError, CatalogIssue, CatalogLoadError, CatalogRole, NameDictionary,
    load_name_dictionary,
};
use tempfile::TempDir;

const GERALD_CP949: &[u8] = &[0xb0, 0xd4, 0xb7, 0xb2, 0xb5, 0xe5];

struct CatalogTree {
    _temporary: TempDir,
    data: PathBuf,
    sox: PathBuf,
}

impl CatalogTree {
    fn new() -> Self {
        let temporary = TempDir::new().unwrap();
        let data = temporary.path().join("Data");
        let sox = data.join("SOX");
        fs::create_dir_all(sox.join("ENG")).unwrap();
        fs::create_dir_all(data.join("Text/ENG")).unwrap();
        Self {
            _temporary: temporary,
            data,
            sox,
        }
    }

    fn role_path(&self, role: CatalogRole) -> PathBuf {
        match role {
            CatalogRole::WeaponNames => self.data.join(role.relative_path()),
            _ => self.sox.join(role.relative_path()),
        }
    }

    fn write(&self, role: CatalogRole, bytes: &[u8]) {
        fs::write(self.role_path(role), bytes).unwrap();
    }
}

#[test]
fn complete_load_uses_sparse_stored_ids_without_dense_allocation() {
    let tree = complete_catalog_tree();

    let loaded = load_name_dictionary(&tree.sox).unwrap();

    assert!(loaded.issues.is_empty());
    assert_eq!(loaded.dictionary.troop_name(2), Some("Footman"));
    assert_eq!(loaded.dictionary.troop_name(4_096), Some("Archer"));
    assert_eq!(loaded.dictionary.troop_name(u32::MAX), Some("Last Troop"));
    assert_eq!(loaded.dictionary.troop_name(3), None);
    assert_eq!(loaded.dictionary.character_name(0), Some("Gerald"));
    assert_eq!(loaded.dictionary.character_name(200), Some("Human"));
    assert_eq!(loaded.dictionary.character_name(u8::MAX), None);
}

#[test]
fn leader_lookup_splits_unicode_whitespace_and_checks_every_bound() {
    let dictionary = complete_dictionary();

    assert_eq!(dictionary.leader_name(73, 0), Some("Alpha"));
    assert_eq!(dictionary.leader_name(73, 1), Some("Beta"));
    assert_eq!(dictionary.leader_name(73, 2), Some("Gamma"));
    assert_eq!(dictionary.leader_name(73, -1), None);
    assert_eq!(dictionary.leader_name(73, 3), None);
    assert_eq!(dictionary.leader_name(74, 0), None);
}

#[test]
fn special_names_prefer_localized_display_and_fall_back_to_cp949_default() {
    let dictionary = complete_dictionary();

    assert_eq!(
        dictionary.translate("DynamicKey"),
        Some("Localized Hero".to_owned())
    );
    assert_eq!(
        dictionary.translate("FallbackKey"),
        Some("게럴드".to_owned())
    );
}

#[test]
fn item_attributes_and_weapon_names_follow_stored_id_and_fallback_rules() {
    let dictionary = complete_dictionary();

    assert_eq!(dictionary.item_attribute_name(91), Some("Flame"));
    assert_eq!(dictionary.item_attribute_description(91), Some("Adds fire"));
    assert_eq!(dictionary.item_attribute_name(-1), None);
    assert_eq!(dictionary.item_attribute_description(92), None);

    assert_eq!(dictionary.item_type_base_name(0), Some("Sword"));
    assert_eq!(dictionary.item_type_base_name(1), Some("Axe"));
    assert_eq!(dictionary.item_type_base_name(-1), None);
    assert_eq!(dictionary.item_type_base_name(2), None);

    assert_eq!(
        dictionary.weapon_name(0, 1, -1),
        Some("Long Sabre".to_owned())
    );
    assert_eq!(dictionary.weapon_name(0, 26, 4), Some("Sabre".to_owned()));
    assert_eq!(dictionary.weapon_name(0, 500, 4), Some("Sword".to_owned()));
    assert_eq!(dictionary.weapon_name(-1, 0, 0), None);
    assert_eq!(dictionary.weapon_name(2, 0, 0), None);
}

#[test]
fn enhancement_prefixes_use_stored_item_type_ids_and_exact_joining() {
    let dictionary = complete_dictionary();

    assert_eq!(
        dictionary.weapon_name(0, 0, 0),
        Some("Fine Sword".to_owned())
    );
    assert_eq!(
        dictionary.weapon_name(0, 0, 1),
        Some("Rare\u{2003}Sword".to_owned())
    );
    assert_eq!(dictionary.weapon_name(0, 0, 2), Some("Sword".to_owned()));
    assert_eq!(dictionary.weapon_name(0, 0, 3), Some("Sword".to_owned()));
    assert_eq!(dictionary.weapon_name(1, 0, 0), Some("Keen Axe".to_owned()));
}

#[test]
fn strict_decoding_records_each_bad_field_and_preserves_valid_halves() {
    let tree = complete_catalog_tree();
    tree.write(
        CatalogRole::ItemAttributes,
        &indexed_fields_table(&[
            (91, &[b"Flame", b"Adds fire"]),
            (92, &[b"Broken\xff", b"Valid description"]),
            (93, &[b"Valid name", b"Broken\xff"]),
        ]),
    );
    tree.write(
        CatalogRole::SpecialNameKeys,
        &special_names_table(&[
            (b"ValidKey", b"\xff"),
            (b"FallbackAfterBadLocalized", b"Default display"),
            (b"\xff", b"Unused default"),
        ]),
    );
    tree.write(
        CatalogRole::SpecialDisplayNames,
        &sequential_table(&[b"Localized display", b"\xff", b"Unused localized", b"\xff"]),
    );

    let loaded = load_name_dictionary(&tree.sox).unwrap();

    assert_eq!(loaded.dictionary.item_attribute_name(92), None);
    assert_eq!(
        loaded.dictionary.item_attribute_description(92),
        Some("Valid description")
    );
    assert_eq!(
        loaded.dictionary.item_attribute_name(93),
        Some("Valid name")
    );
    assert_eq!(loaded.dictionary.item_attribute_description(93), None);
    assert_eq!(
        loaded.dictionary.translate("ValidKey"),
        Some("Localized display".to_owned())
    );
    assert_eq!(
        loaded.dictionary.translate("FallbackAfterBadLocalized"),
        Some("Default display".to_owned())
    );
    assert_eq!(loaded.dictionary.translate("�"), None);

    assert_encoding_issue(
        &loaded.issues,
        CatalogRole::ItemAttributes,
        &tree.role_path(CatalogRole::ItemAttributes),
        1,
        0,
    );
    assert_encoding_issue(
        &loaded.issues,
        CatalogRole::ItemAttributes,
        &tree.role_path(CatalogRole::ItemAttributes),
        2,
        1,
    );
    assert_encoding_issue(
        &loaded.issues,
        CatalogRole::SpecialNameKeys,
        &tree.role_path(CatalogRole::SpecialNameKeys),
        0,
        1,
    );
    assert_encoding_issue(
        &loaded.issues,
        CatalogRole::SpecialDisplayNames,
        &tree.role_path(CatalogRole::SpecialDisplayNames),
        1,
        0,
    );
    assert_encoding_issue(
        &loaded.issues,
        CatalogRole::SpecialNameKeys,
        &tree.role_path(CatalogRole::SpecialNameKeys),
        2,
        0,
    );
    assert_encoding_issue(
        &loaded.issues,
        CatalogRole::SpecialDisplayNames,
        &tree.role_path(CatalogRole::SpecialDisplayNames),
        3,
        0,
    );
    assert_eq!(loaded.issues.len(), 6);
}

#[test]
fn empty_item_attribute_halves_are_absent_independently() {
    let tree = complete_catalog_tree();
    tree.write(
        CatalogRole::ItemAttributes,
        &indexed_fields_table(&[
            (94, &[b"", b"Description only"]),
            (95, &[b"Name only", b""]),
        ]),
    );

    let loaded = load_name_dictionary(&tree.sox).unwrap();

    assert_eq!(loaded.dictionary.item_attribute_name(94), None);
    assert_eq!(
        loaded.dictionary.item_attribute_description(94),
        Some("Description only")
    );
    assert_eq!(loaded.dictionary.item_attribute_name(95), Some("Name only"));
    assert_eq!(loaded.dictionary.item_attribute_description(95), None);
}

#[test]
fn partial_load_keeps_dictionary_and_raw_issue_with_exact_path() {
    let tree = complete_catalog_tree();
    let missing_path = tree.role_path(CatalogRole::LeaderPools);
    fs::remove_file(&missing_path).unwrap();

    let loaded = load_name_dictionary(&tree.sox).unwrap();

    assert_eq!(loaded.dictionary.troop_name(2), Some("Footman"));
    assert_eq!(loaded.dictionary.leader_name(73, 0), None);
    assert_eq!(loaded.issues.len(), 1);
    let issue = loaded.issues.first().unwrap();
    assert_eq!(issue.role, CatalogRole::LeaderPools);
    assert_eq!(issue.path, missing_path);
    assert!(matches!(issue.error, CatalogFileError::Read { .. }));
}

#[test]
fn raw_fatal_load_retains_all_role_issues() {
    let tree = CatalogTree::new();

    let error = load_name_dictionary(&tree.sox).unwrap_err();

    let CatalogLoadError::NoUsableCatalogs { issues } = error else {
        panic!("expected no usable catalogs");
    };
    assert_eq!(issues.len(), 8);
    let roles = issues
        .iter()
        .map(|issue| issue.role)
        .collect::<HashSet<_>>();
    assert_eq!(roles.len(), 8);
}

#[test]
fn post_decode_fatal_load_retains_raw_and_decoding_issues() {
    let tree = CatalogTree::new();
    tree.write(
        CatalogRole::TroopNames,
        &indexed_table(&[(1, b""), (2, b"\xff")]),
    );
    tree.write(CatalogRole::CharacterNames, &indexed_table(&[(3, b"")]));
    tree.write(
        CatalogRole::SpecialNameKeys,
        &special_names_table(&[(b"\xff", b"\xff")]),
    );
    tree.write(
        CatalogRole::SpecialDisplayNames,
        &sequential_table(&[b"\xff"]),
    );

    let error = load_name_dictionary(&tree.sox).unwrap_err();

    let CatalogLoadError::NoUsableCatalogs { issues } = error else {
        panic!("expected post-decode fatal load");
    };
    assert_encoding_issue(
        &issues,
        CatalogRole::TroopNames,
        &tree.role_path(CatalogRole::TroopNames),
        1,
        0,
    );
    assert_encoding_issue(
        &issues,
        CatalogRole::SpecialNameKeys,
        &tree.role_path(CatalogRole::SpecialNameKeys),
        0,
        0,
    );
    assert_encoding_issue(
        &issues,
        CatalogRole::SpecialNameKeys,
        &tree.role_path(CatalogRole::SpecialNameKeys),
        0,
        1,
    );
    assert_encoding_issue(
        &issues,
        CatalogRole::SpecialDisplayNames,
        &tree.role_path(CatalogRole::SpecialDisplayNames),
        0,
        0,
    );
    assert_eq!(issues.len(), 8);
}

#[test]
fn fatal_load_decodes_optional_and_extra_localized_fields_before_core_decision() {
    let tree = CatalogTree::new();
    tree.write(CatalogRole::TroopNames, &indexed_table(&[]));
    tree.write(CatalogRole::CharacterNames, &indexed_table(&[]));
    tree.write(CatalogRole::SpecialNameKeys, &special_names_table(&[]));
    tree.write(
        CatalogRole::SpecialDisplayNames,
        &sequential_table(&[b"\xff"]),
    );
    tree.write(
        CatalogRole::ItemAttributes,
        &indexed_fields_table(&[(77, &[b"\xff", b"Valid description"])]),
    );

    let error = load_name_dictionary(&tree.sox).unwrap_err();

    let CatalogLoadError::NoUsableCatalogs { issues } = error else {
        panic!("expected decoded-core fatal load");
    };
    assert_encoding_issue(
        &issues,
        CatalogRole::ItemAttributes,
        &tree.role_path(CatalogRole::ItemAttributes),
        0,
        0,
    );
    assert_encoding_issue(
        &issues,
        CatalogRole::SpecialDisplayNames,
        &tree.role_path(CatalogRole::SpecialDisplayNames),
        0,
        0,
    );
    let raw_issue = issues
        .iter()
        .find(|issue| issue.role == CatalogRole::LeaderPools)
        .unwrap();
    assert_eq!(raw_issue.path, tree.role_path(CatalogRole::LeaderPools));
    assert!(matches!(raw_issue.error, CatalogFileError::Read { .. }));
    assert_eq!(issues.len(), 5);
}

#[test]
fn forward_translation_is_normalized_prioritized_and_nonempty() {
    let dictionary = complete_dictionary();

    assert_eq!(dictionary.translate("게럴드"), Some("Gerald".to_owned()));
    assert_eq!(
        dictionary.translate("DynamicKey"),
        Some("Localized Hero".to_owned())
    );
    assert_eq!(
        dictionary.translate("----DynamicKey----"),
        Some("Localized Hero".to_owned())
    );
    assert_eq!(
        dictionary.translate("DynamicKey123"),
        Some("Localized Hero".to_owned())
    );
    assert_eq!(
        dictionary.translate("ascii 게럴드 ready"),
        Some("ascii Gerald ready".to_owned())
    );
    assert_eq!(
        dictionary.translate("WrappedKey"),
        Some("Wrapped Value".to_owned())
    );
    assert_eq!(dictionary.translate(""), None);
    assert_eq!(dictionary.translate("unknown"), None);
}

#[test]
fn reverse_translation_is_deterministic_non_cascading_and_nonempty() {
    let dictionary = complete_dictionary();

    assert_eq!(
        dictionary.reverse_translate("Localized Hero"),
        Some("DynamicKey".to_owned())
    );
    assert_eq!(
        dictionary.reverse_translate("x Alpha Beta Alpha y"),
        Some("x long-key short-key y".to_owned())
    );
    assert_eq!(
        dictionary.reverse_translate("x zz cascade source zz y"),
        Some("x Alpha y".to_owned())
    );
    assert_eq!(
        dictionary.reverse_translate("Appears"),
        Some("가 나타난다".to_owned())
    );
    assert_eq!(
        dictionary.reverse_translate("Wrapped Value"),
        Some("WrappedKey".to_owned())
    );
    assert_eq!(dictionary.reverse_translate(""), None);
    assert_eq!(dictionary.reverse_translate("unknown"), None);
}

fn complete_dictionary() -> NameDictionary {
    let tree = complete_catalog_tree();
    load_name_dictionary(&tree.sox).unwrap().dictionary
}

fn complete_catalog_tree() -> CatalogTree {
    let tree = CatalogTree::new();
    tree.write(
        CatalogRole::TroopNames,
        &indexed_table(&[
            (2, b"Footman"),
            (4_096, b"Archer"),
            (u32::MAX, b"Last Troop"),
        ]),
    );
    tree.write(
        CatalogRole::CharacterNames,
        &indexed_table(&[(0, b"Gerald"), (200, b"Human"), (u32::MAX, b"Unreachable")]),
    );
    tree.write(
        CatalogRole::LeaderPools,
        &indexed_table(&[(73, "Alpha\u{2003}Beta\tGamma".as_bytes())]),
    );
    tree.write(
        CatalogRole::SpecialNameKeys,
        &special_names_table(&[
            (b"DynamicKey", b"Default Hero"),
            (b"FallbackKey", GERALD_CP949),
            (GERALD_CP949, b"Default Gerald"),
            (b"long-key", b"Alpha Beta"),
            (b"short-key", b"Alpha"),
            (b"Alpha", b"zz cascade source zz"),
            (b"--WrappedKey--", b"--Default Wrapped--"),
        ]),
    );
    tree.write(
        CatalogRole::SpecialDisplayNames,
        &sequential_table(&[
            b"Localized Hero",
            b"",
            b"Dynamic Gerald",
            b"Alpha Beta",
            b"Alpha",
            b"zz cascade source zz",
            b"--Wrapped Value--",
        ]),
    );
    tree.write(
        CatalogRole::ItemAttributes,
        &indexed_fields_table(&[(91, &[b"Flame", b"Adds fire"])]),
    );
    tree.write(
        CatalogRole::ItemTypePrefixes,
        &indexed_fields_table(&[
            (1, &[b"Keen", b"", b""]),
            (0, &[b"Fine", "Rare\u{2003}".as_bytes(), b""]),
        ]),
    );
    tree.write(
        CatalogRole::WeaponNames,
        b"2\n\n2\t// swords\n9\nSword\nLong Sword\n27\nSabre\nLong Sabre\n  \n1 // axes\n3\nAxe\nWar Axe\n",
    );
    tree
}

fn assert_encoding_issue(
    issues: &[CatalogIssue],
    role: CatalogRole,
    path: &Path,
    record: usize,
    field: usize,
) {
    assert!(issues.iter().any(|issue| {
        issue.role == role
            && issue.path == path
            && matches!(
                issue.error,
                CatalogFileError::InvalidFieldEncoding {
                    role: error_role,
                    record: error_record,
                    field: error_field,
                } if error_role == role && error_record == record && error_field == field
            )
    }));
}

fn indexed_table(records: &[(u32, &[u8])]) -> Vec<u8> {
    let mut bytes = table_header(records.len());
    for (id, value) in records {
        push_u32(&mut bytes, *id);
        push_field(&mut bytes, value);
    }
    bytes
}

fn sequential_table(records: &[&[u8]]) -> Vec<u8> {
    let mut bytes = table_header(records.len());
    for record in records {
        push_field(&mut bytes, record);
    }
    bytes
}

fn indexed_fields_table(records: &[(u32, &[&[u8]])]) -> Vec<u8> {
    let mut bytes = table_header(records.len());
    for (id, fields) in records {
        push_u32(&mut bytes, *id);
        for field in *fields {
            push_field(&mut bytes, field);
        }
    }
    bytes
}

fn special_names_table(records: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut bytes = table_header(records.len());
    for (key, default_value) in records {
        push_field(&mut bytes, key);
        push_field(&mut bytes, default_value);
    }
    bytes.extend_from_slice(&[b' '; 64]);
    bytes
}

fn table_header(record_count: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 100);
    push_u32(&mut bytes, u32::try_from(record_count).unwrap());
    bytes
}

fn push_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&u16::try_from(field.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(field);
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
