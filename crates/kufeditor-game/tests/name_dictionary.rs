#![allow(
    clippy::unwrap_used,
    reason = "literal fixtures use controlled temporary paths and statically valid sizes"
)]

use std::{
    borrow::Cow,
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
fn unit_name_negative_leader_prefers_character_to_troop() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(-1, 73, 7), Some("Character Seven"));
}

#[test]
fn unit_name_nonnegative_leader_prefers_selected_pool_name_to_job_fallback() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(1, 73, 7), Some("LeaderOne"));
}

#[test]
fn unit_name_missing_or_invalid_leader_falls_back_to_bounded_troop() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(0, 74, 0), Some("Troop Zero"));
    assert_eq!(dictionary.unit_name(2, 73, 42), Some("Troop Forty Two"));
}

#[test]
fn unit_name_failed_leader_pool_conversion_continues_to_troop() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(0, -1, 0), Some("Troop Zero"));
}

#[test]
fn unit_name_missing_initial_character_continues_to_troop() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(-1, 73, 42), Some("Troop Forty Two"));
}

#[test]
fn unit_name_uses_character_fallback_after_leader_and_troop_miss() {
    let dictionary = unit_name_dictionary();

    assert_eq!(
        dictionary.unit_name(0, 74, 200),
        Some("Character Two Hundred")
    );
}

#[test]
fn unit_name_missing_standard_troop_continues_to_character() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(0, 74, 8), Some("Character Eight"));
}

#[test]
fn unit_name_negative_troop_pool_does_not_wrap() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(0, -1, 300), None);
}

#[test]
fn unit_name_job_type_above_42_does_not_index_troop() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(0, 74, 43), None);
}

#[test]
fn unit_name_wide_job_types_do_not_truncate_to_character_index() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(-1, 73, 256), None);
    assert_eq!(dictionary.unit_name(-1, 73, u32::MAX), None);
}

#[test]
fn unit_name_missing_data_returns_none() {
    let dictionary = unit_name_dictionary();

    assert_eq!(dictionary.unit_name(0, 74, 41), None);
}

#[test]
fn stg_unit_name_uses_special_prefix_rules_in_catalog_order() {
    let dictionary = stg_unit_name_dictionary();

    assert_eq!(
        dictionary.stg_unit_name("-hErOAlpha", 45, 0),
        "First Special"
    );
    assert_eq!(
        dictionary.stg_unit_name("pAlAdInGuard", 6, 13),
        "Paladin Special"
    );
    assert_eq!(dictionary.stg_unit_name("PaladinGuard", 6, 12), "Troop 6");
    assert_eq!(
        dictionary.stg_unit_name("eLfArcherGuard", 19, 7),
        "Elf Special"
    );
    assert_eq!(
        dictionary.stg_unit_name("ElfArcherGuard", 19, 6),
        "Troop 19"
    );
    assert_eq!(dictionary.stg_unit_name("-", 45, 0), "-");
    assert_eq!(dictionary.stg_unit_name("MissingDisplay", 6, 13), "Troop 6");
    assert_eq!(
        dictionary.stg_unit_name("MissingDisplayGuard", 6, 13),
        "Troop 6"
    );
    assert_eq!(
        dictionary.stg_unit_name("--wRaPpEdPrEfIx--Unit", 45, 0),
        "--Wrapped Display--"
    );
    assert_eq!(
        dictionary.translate("WrappedPrefix"),
        Some("Wrapped Display".to_owned())
    );
}

#[test]
fn stg_unit_name_prefers_dark_orc_and_every_character_job() {
    let dictionary = stg_unit_name_dictionary();

    assert_eq!(dictionary.stg_unit_name("DarkOrc", 26, 0), "Character 26");
    assert_eq!(dictionary.stg_unit_name("DarkOrc", 26, 1), "Troop 26");

    for job_type in [32_u8, 33, 34, 35, 36, 37, 38, 43, 44, 46, 47] {
        assert_eq!(
            dictionary.stg_unit_name("CharacterInternal", job_type, 0),
            format!("Character {job_type}")
        );
    }
}

#[test]
fn stg_unit_name_falls_back_through_troop_translation_internal_and_unknown() {
    let dictionary = stg_unit_name_dictionary();

    assert_eq!(
        dictionary.stg_unit_name("StandardInternal", 7, 0),
        "Troop 7"
    );
    assert_eq!(
        dictionary.stg_unit_name("BoundaryInternal", 42, 0),
        "Troop 42"
    );
    assert_eq!(
        dictionary.stg_unit_name("TranslatedInternal", 45, 0),
        "Translated Unit"
    );
    assert_eq!(
        dictionary.stg_unit_name("RawInternal", 45, 0),
        "RawInternal"
    );
    assert_eq!(dictionary.stg_unit_name("", 45, 0), "Unknown");
    assert_eq!(dictionary.stg_unit_name("", u8::MAX, u8::MAX), "Unknown");
}

#[test]
fn stg_unit_name_borrows_catalog_and_internal_names_but_owns_translations() {
    let dictionary = stg_unit_name_dictionary();

    assert!(matches!(
        dictionary.stg_unit_name("StandardInternal", 7, 0),
        Cow::Borrowed("Troop 7")
    ));
    assert!(matches!(
        dictionary.stg_unit_name("TranslatedInternal", 45, 0),
        Cow::Owned(name) if name == "Translated Unit"
    ));
    assert!(matches!(
        dictionary.stg_unit_name("RawInternal", 45, 0),
        Cow::Borrowed("RawInternal")
    ));
}

#[test]
fn stg_unit_name_continues_after_missing_character_catalog_entries() {
    let dictionary = stg_missing_character_dictionary();

    assert_eq!(
        dictionary.stg_unit_name("CharacterInternal", 32, 0),
        "Troop 32"
    );
    assert_eq!(
        dictionary.stg_unit_name("DarkOrcInternal", 26, 0),
        "Troop 26"
    );
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

fn unit_name_dictionary() -> NameDictionary {
    let tree = CatalogTree::new();
    tree.write(
        CatalogRole::TroopNames,
        &indexed_table(&[
            (0, b"Troop Zero"),
            (7, b"Troop Seven"),
            (42, b"Troop Forty Two"),
            (43, b"Troop Forty Three"),
        ]),
    );
    tree.write(
        CatalogRole::CharacterNames,
        &indexed_table(&[
            (0, b"Character Zero"),
            (7, b"Character Seven"),
            (8, b"Character Eight"),
            (200, b"Character Two Hundred"),
            (u32::from(u8::MAX), b"Character 255"),
        ]),
    );
    tree.write(
        CatalogRole::LeaderPools,
        &indexed_table(&[(73, b"LeaderZero LeaderOne"), (u32::MAX, b"WrappedLeader")]),
    );
    load_name_dictionary(&tree.sox).unwrap().dictionary
}

fn stg_unit_name_dictionary() -> NameDictionary {
    let tree = CatalogTree::new();
    let troop_names = [6_u32, 7, 19, 26, 32, 33, 34, 35, 36, 37, 38, 42, 45]
        .map(|job_type| (job_type, format!("Troop {job_type}").into_bytes()));
    let troop_records = troop_names
        .iter()
        .map(|(job_type, name)| (*job_type, name.as_slice()))
        .collect::<Vec<_>>();
    tree.write(CatalogRole::TroopNames, &indexed_table(&troop_records));

    let character_names = [26_u32, 32, 33, 34, 35, 36, 37, 38, 43, 44, 46, 47]
        .map(|job_type| (job_type, format!("Character {job_type}").into_bytes()));
    let character_records = character_names
        .iter()
        .map(|(job_type, name)| (*job_type, name.as_slice()))
        .collect::<Vec<_>>();
    tree.write(
        CatalogRole::CharacterNames,
        &indexed_table(&character_records),
    );
    tree.write(
        CatalogRole::SpecialNameKeys,
        &special_names_table(&[
            (b"-Hero", b"First Special"),
            (b"-HeroAlpha", b"Second Special"),
            (b"Paladin", b"Paladin Special"),
            (b"ElfArcher", b"Elf Special"),
            (b"MissingDisplay", b""),
            (b"MissingDisplayGuard", b"Later Special"),
            (b"TranslatedInternal", b"Translated Unit"),
            (b"--WrappedPrefix--", b"--Wrapped Display--"),
        ]),
    );
    tree.write(
        CatalogRole::SpecialDisplayNames,
        &sequential_table(&[
            b"First Special",
            b"Second Special",
            b"Paladin Special",
            b"Elf Special",
            b"",
            b"Later Special",
            b"Translated Unit",
            b"--Wrapped Display--",
        ]),
    );
    load_name_dictionary(&tree.sox).unwrap().dictionary
}

fn stg_missing_character_dictionary() -> NameDictionary {
    let tree = CatalogTree::new();
    tree.write(
        CatalogRole::TroopNames,
        &indexed_table(&[(26, b"Troop 26"), (32, b"Troop 32")]),
    );
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
