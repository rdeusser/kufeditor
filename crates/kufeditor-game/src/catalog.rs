#![allow(
    dead_code,
    reason = "Task 5 consumes the crate-private raw catalog loader and model"
)]

use std::{
    fmt::{self, Display, Formatter},
    fs,
    path::{Path, PathBuf},
};

use kufeditor_formats::{
    FormatError, SchemaDocument, SoxSchema, SoxStringTableDocument, SoxStringTableLayout,
};

use crate::{CatalogFileError, CatalogIssue, CatalogLoadError};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatalogRole {
    TroopNames,
    CharacterNames,
    LeaderPools,
    SpecialNameKeys,
    SpecialDisplayNames,
    ItemAttributes,
    ItemTypePrefixes,
    WeaponNames,
}

impl CatalogRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TroopNames => "troop names",
            Self::CharacterNames => "character names",
            Self::LeaderPools => "leader pools",
            Self::SpecialNameKeys => "special-name keys and defaults",
            Self::SpecialDisplayNames => "special display names",
            Self::ItemAttributes => "item attributes",
            Self::ItemTypePrefixes => "item-type prefixes",
            Self::WeaponNames => "weapon names",
        }
    }

    pub fn relative_path(self) -> &'static Path {
        Path::new(match self {
            Self::TroopNames => "ENG/TroopInfo_ENG.sox",
            Self::CharacterNames => "ENG/CharInfo_ENG.sox",
            Self::LeaderPools => "ENG/LeaderGeneration_ENG.sox",
            Self::SpecialNameKeys => "SpecialNames.sox",
            Self::SpecialDisplayNames => "ENG/SpecialNames_ENG.sox",
            Self::ItemAttributes => "ENG/ItemAttInfo_ENG.sox",
            Self::ItemTypePrefixes => "ENG/ItemTypeInfo_ENG.sox",
            Self::WeaponNames => "Text/ENG/WeaponNames_ENG.txt",
        })
    }
}

impl Display for CatalogRole {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug)]
pub(crate) struct RawCatalogData {
    pub(crate) troop_names: Vec<RawIndexedRecord>,
    pub(crate) character_names: Vec<RawIndexedRecord>,
    pub(crate) leader_pools: Vec<RawIndexedRecord>,
    pub(crate) special_names: Vec<RawSpecialName>,
    pub(crate) special_display_names: Vec<Vec<u8>>,
    pub(crate) item_attributes: Vec<RawItemAttribute>,
    pub(crate) item_type_prefixes: Vec<RawItemTypePrefixes>,
    pub(crate) weapon_types: Vec<Vec<RawWeaponVariant>>,
    pub(crate) issues: Vec<CatalogIssue>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RawIndexedRecord {
    pub(crate) id: u32,
    pub(crate) value: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RawSpecialName {
    pub(crate) key: Vec<u8>,
    pub(crate) default_value: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RawItemAttribute {
    pub(crate) id: u32,
    pub(crate) name: Vec<u8>,
    pub(crate) description: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RawItemTypePrefixes {
    pub(crate) id: u32,
    pub(crate) prefixes: [Vec<u8>; 3],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RawWeaponVariant {
    pub(crate) id: i32,
    pub(crate) short_name: String,
    pub(crate) long_name: String,
}

pub(crate) fn load_catalog_data(sox_directory: &Path) -> Result<RawCatalogData, CatalogLoadError> {
    if !sox_directory.is_dir() {
        return Err(CatalogLoadError::InvalidSoxDirectory {
            path: sox_directory.to_path_buf(),
        });
    }

    let data_directory = sox_directory.parent().unwrap_or(sox_directory);
    let mut issues = Vec::new();

    let troop_names = load_indexed_role(
        CatalogRole::TroopNames,
        &role_path(sox_directory, data_directory, CatalogRole::TroopNames),
        &mut issues,
    );
    let character_names = load_indexed_role(
        CatalogRole::CharacterNames,
        &role_path(sox_directory, data_directory, CatalogRole::CharacterNames),
        &mut issues,
    );
    let leader_pools = load_indexed_role(
        CatalogRole::LeaderPools,
        &role_path(sox_directory, data_directory, CatalogRole::LeaderPools),
        &mut issues,
    );
    let special_names = load_special_names(
        &role_path(sox_directory, data_directory, CatalogRole::SpecialNameKeys),
        &mut issues,
    );
    let special_display_names = load_sequential_role(
        CatalogRole::SpecialDisplayNames,
        &role_path(
            sox_directory,
            data_directory,
            CatalogRole::SpecialDisplayNames,
        ),
        &mut issues,
    );
    let item_attributes = load_item_attributes(
        &role_path(sox_directory, data_directory, CatalogRole::ItemAttributes),
        &mut issues,
    );
    let item_type_prefixes = load_item_type_prefixes(
        &role_path(sox_directory, data_directory, CatalogRole::ItemTypePrefixes),
        &mut issues,
    );
    let weapon_types = load_weapon_names(
        &role_path(sox_directory, data_directory, CatalogRole::WeaponNames),
        &mut issues,
    );

    if troop_names.is_empty() && character_names.is_empty() && special_names.is_empty() {
        return Err(CatalogLoadError::NoUsableCatalogs { issues });
    }

    Ok(RawCatalogData {
        troop_names,
        character_names,
        leader_pools,
        special_names,
        special_display_names,
        item_attributes,
        item_type_prefixes,
        weapon_types,
        issues,
    })
}

fn role_path(sox_directory: &Path, data_directory: &Path, role: CatalogRole) -> PathBuf {
    match role {
        CatalogRole::WeaponNames => data_directory.join(role.relative_path()),
        _ => sox_directory.join(role.relative_path()),
    }
}

fn load_indexed_role(
    role: CatalogRole,
    path: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> Vec<RawIndexedRecord> {
    load_string_table(role, path, SoxStringTableLayout::Indexed)
        .and_then(|document| project_indexed(role, path, &document))
        .unwrap_or_else(|error| {
            issues.push(error);
            Vec::new()
        })
}

fn load_sequential_role(
    role: CatalogRole,
    path: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> Vec<Vec<u8>> {
    load_string_table(role, path, SoxStringTableLayout::Sequential)
        .and_then(|document| project_sequential(role, path, &document))
        .unwrap_or_else(|error| {
            issues.push(error);
            Vec::new()
        })
}

fn load_item_attributes(path: &Path, issues: &mut Vec<CatalogIssue>) -> Vec<RawItemAttribute> {
    let role = CatalogRole::ItemAttributes;
    load_string_table(role, path, SoxStringTableLayout::IndexedPair)
        .and_then(|document| project_item_attributes(path, &document))
        .unwrap_or_else(|error| {
            issues.push(error);
            Vec::new()
        })
}

fn load_item_type_prefixes(
    path: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> Vec<RawItemTypePrefixes> {
    let role = CatalogRole::ItemTypePrefixes;
    load_string_table(role, path, SoxStringTableLayout::IndexedTriple)
        .and_then(|document| project_item_type_prefixes(path, &document))
        .unwrap_or_else(|error| {
            issues.push(error);
            Vec::new()
        })
}

fn load_string_table(
    role: CatalogRole,
    path: &Path,
    layout: SoxStringTableLayout,
) -> Result<SoxStringTableDocument, CatalogIssue> {
    let bytes = read_catalog(role, path)?;
    SoxStringTableDocument::parse(layout, bytes).map_err(|source| format_issue(role, path, source))
}

fn project_indexed(
    role: CatalogRole,
    path: &Path,
    document: &SoxStringTableDocument,
) -> Result<Vec<RawIndexedRecord>, CatalogIssue> {
    let mut records = Vec::with_capacity(document.record_count());
    for record in 0..document.record_count() {
        let id = document
            .record_id(record)
            .map_err(|source| format_issue(role, path, source))?;
        let value = document
            .field(record, 0)
            .map_err(|source| format_issue(role, path, source))?;
        if let Some(id) = id {
            records.push(RawIndexedRecord {
                id,
                value: value.to_vec(),
            });
        }
    }
    Ok(records)
}

fn project_sequential(
    role: CatalogRole,
    path: &Path,
    document: &SoxStringTableDocument,
) -> Result<Vec<Vec<u8>>, CatalogIssue> {
    let mut records = Vec::with_capacity(document.record_count());
    for record in 0..document.record_count() {
        let value = document
            .field(record, 0)
            .map_err(|source| format_issue(role, path, source))?;
        records.push(value.to_vec());
    }
    Ok(records)
}

fn project_item_attributes(
    path: &Path,
    document: &SoxStringTableDocument,
) -> Result<Vec<RawItemAttribute>, CatalogIssue> {
    let role = CatalogRole::ItemAttributes;
    let mut records = Vec::with_capacity(document.record_count());
    for record in 0..document.record_count() {
        let id = document
            .record_id(record)
            .map_err(|source| format_issue(role, path, source))?;
        let name = document
            .field(record, 0)
            .map_err(|source| format_issue(role, path, source))?;
        let description = document
            .field(record, 1)
            .map_err(|source| format_issue(role, path, source))?;
        if let Some(id) = id {
            records.push(RawItemAttribute {
                id,
                name: name.to_vec(),
                description: description.to_vec(),
            });
        }
    }
    Ok(records)
}

fn project_item_type_prefixes(
    path: &Path,
    document: &SoxStringTableDocument,
) -> Result<Vec<RawItemTypePrefixes>, CatalogIssue> {
    let role = CatalogRole::ItemTypePrefixes;
    let mut records = Vec::with_capacity(document.record_count());
    for record in 0..document.record_count() {
        let id = document
            .record_id(record)
            .map_err(|source| format_issue(role, path, source))?;
        let first = document
            .field(record, 0)
            .map_err(|source| format_issue(role, path, source))?;
        let second = document
            .field(record, 1)
            .map_err(|source| format_issue(role, path, source))?;
        let third = document
            .field(record, 2)
            .map_err(|source| format_issue(role, path, source))?;
        if let Some(id) = id {
            records.push(RawItemTypePrefixes {
                id,
                prefixes: [first.to_vec(), second.to_vec(), third.to_vec()],
            });
        }
    }
    Ok(records)
}

fn load_special_names(path: &Path, issues: &mut Vec<CatalogIssue>) -> Vec<RawSpecialName> {
    let role = CatalogRole::SpecialNameKeys;
    let result = read_catalog(role, path)
        .and_then(|bytes| {
            SchemaDocument::parse(SoxSchema::SpecialNames, bytes)
                .map_err(|source| format_issue(role, path, source))
        })
        .map(|document| {
            let mut records = Vec::with_capacity(document.record_count());
            for record in 0..document.record_count() {
                if let Some(value) = document.special_name(record) {
                    records.push(RawSpecialName {
                        key: value.key.to_vec(),
                        default_value: value.value.to_vec(),
                    });
                }
            }
            records
        });

    result.unwrap_or_else(|error| {
        issues.push(error);
        Vec::new()
    })
}

fn load_weapon_names(path: &Path, issues: &mut Vec<CatalogIssue>) -> Vec<Vec<RawWeaponVariant>> {
    let role = CatalogRole::WeaponNames;
    let result = read_catalog(role, path).and_then(|bytes| {
        let text = std::str::from_utf8(&bytes).map_err(|source| CatalogIssue {
            role,
            path: path.to_path_buf(),
            error: CatalogFileError::InvalidWeaponUtf8 { source },
        })?;
        parse_weapon_names(text, path)
    });

    result.unwrap_or_else(|error| {
        issues.push(error);
        Vec::new()
    })
}

fn parse_weapon_names(text: &str, path: &Path) -> Result<Vec<Vec<RawWeaponVariant>>, CatalogIssue> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut cursor = 0;
    let type_count_line = next_line(&lines, &mut cursor)
        .ok_or_else(|| weapon_syntax(path, 1, "weapon-type count is missing"))?;
    let type_count = parse_count(
        type_count_line,
        path,
        1,
        "invalid weapon-type count",
        "negative weapon-type count",
    )?;
    let remaining = lines.len().saturating_sub(cursor);
    if type_count > remaining {
        return Err(weapon_syntax(
            path,
            lines.len().saturating_add(1),
            "weapon type is missing",
        ));
    }

    let mut weapon_types = Vec::with_capacity(type_count);
    for _ in 0..type_count {
        skip_blank_lines(&lines, &mut cursor);
        let count_line_number = cursor.saturating_add(1);
        let count_line = next_line(&lines, &mut cursor)
            .ok_or_else(|| weapon_syntax(path, count_line_number, "weapon type is missing"))?;
        let variant_count = parse_count(
            count_line,
            path,
            count_line_number,
            "invalid weapon-variant count",
            "negative weapon-variant count",
        )?;
        preflight_variants(&lines, cursor, variant_count, path)?;

        let mut variants = Vec::with_capacity(variant_count);
        for _ in 0..variant_count {
            let id_line_number = cursor.saturating_add(1);
            let id_line = next_line(&lines, &mut cursor)
                .ok_or_else(|| weapon_syntax(path, id_line_number, "weapon ID is missing"))?;
            let id = id_line
                .trim()
                .parse::<i32>()
                .map_err(|_| weapon_syntax(path, id_line_number, "invalid weapon ID"))?;

            let short_line_number = cursor.saturating_add(1);
            let short_name = next_line(&lines, &mut cursor).ok_or_else(|| {
                weapon_syntax(path, short_line_number, "weapon short name is missing")
            })?;
            if short_name.is_empty() {
                return Err(weapon_syntax(
                    path,
                    short_line_number,
                    "weapon short name is empty",
                ));
            }

            let long_line_number = cursor.saturating_add(1);
            let long_name = next_line(&lines, &mut cursor).ok_or_else(|| {
                weapon_syntax(path, long_line_number, "weapon long name is missing")
            })?;
            if long_name.is_empty() {
                return Err(weapon_syntax(
                    path,
                    long_line_number,
                    "weapon long name is empty",
                ));
            }

            variants.push(RawWeaponVariant {
                id,
                short_name: short_name.to_owned(),
                long_name: long_name.to_owned(),
            });
        }
        weapon_types.push(variants);
    }

    Ok(weapon_types)
}

fn preflight_variants(
    lines: &[&str],
    cursor: usize,
    variant_count: usize,
    path: &Path,
) -> Result<(), CatalogIssue> {
    let remaining = lines.len().saturating_sub(cursor);
    let required = variant_count.checked_mul(3);
    if required.is_some_and(|required| required <= remaining) {
        return Ok(());
    }

    let reason = match remaining % 3 {
        0 => "weapon ID is missing",
        1 => "weapon short name is missing",
        _ => "weapon long name is missing",
    };
    Err(weapon_syntax(path, lines.len().saturating_add(1), reason))
}

fn parse_count(
    line: &str,
    path: &Path,
    line_number: usize,
    invalid_reason: &'static str,
    negative_reason: &'static str,
) -> Result<usize, CatalogIssue> {
    let value = count_prefix(line)
        .trim()
        .parse::<i64>()
        .map_err(|_| weapon_syntax(path, line_number, invalid_reason))?;
    if value < 0 {
        return Err(weapon_syntax(path, line_number, negative_reason));
    }
    usize::try_from(value).map_err(|_| weapon_syntax(path, line_number, invalid_reason))
}

fn count_prefix(line: &str) -> &str {
    let tab = line.find('\t');
    let comment = line.find("//");
    let end = match (tab, comment) {
        (Some(tab), Some(comment)) => tab.min(comment),
        (Some(tab), None) => tab,
        (None, Some(comment)) => comment,
        (None, None) => return line,
    };
    line.get(..end).unwrap_or_default()
}

fn skip_blank_lines(lines: &[&str], cursor: &mut usize) {
    while lines
        .get(*cursor)
        .is_some_and(|line| line.trim().is_empty())
    {
        *cursor = cursor.saturating_add(1);
    }
}

fn next_line<'a>(lines: &[&'a str], cursor: &mut usize) -> Option<&'a str> {
    let line = lines.get(*cursor).copied();
    if line.is_some() {
        *cursor = cursor.saturating_add(1);
    }
    line
}

fn read_catalog(role: CatalogRole, path: &Path) -> Result<Vec<u8>, CatalogIssue> {
    fs::read(path).map_err(|source| CatalogIssue {
        role,
        path: path.to_path_buf(),
        error: CatalogFileError::Read { source },
    })
}

fn format_issue(role: CatalogRole, path: &Path, source: FormatError) -> CatalogIssue {
    CatalogIssue {
        role,
        path: path.to_path_buf(),
        error: CatalogFileError::Format { source },
    }
}

fn weapon_syntax(path: &Path, line: usize, reason: &'static str) -> CatalogIssue {
    CatalogIssue {
        role: CatalogRole::WeaponNames,
        path: path.to_path_buf(),
        error: CatalogFileError::WeaponSyntax { line, reason },
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
    };

    use tempfile::TempDir;

    use super::{CatalogRole, load_catalog_data};
    use crate::{CatalogFileError, CatalogLoadError};

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
    fn catalog_roles_have_stable_labels_paths_and_display() {
        let cases = [
            (
                CatalogRole::TroopNames,
                "troop names",
                "ENG/TroopInfo_ENG.sox",
            ),
            (
                CatalogRole::CharacterNames,
                "character names",
                "ENG/CharInfo_ENG.sox",
            ),
            (
                CatalogRole::LeaderPools,
                "leader pools",
                "ENG/LeaderGeneration_ENG.sox",
            ),
            (
                CatalogRole::SpecialNameKeys,
                "special-name keys and defaults",
                "SpecialNames.sox",
            ),
            (
                CatalogRole::SpecialDisplayNames,
                "special display names",
                "ENG/SpecialNames_ENG.sox",
            ),
            (
                CatalogRole::ItemAttributes,
                "item attributes",
                "ENG/ItemAttInfo_ENG.sox",
            ),
            (
                CatalogRole::ItemTypePrefixes,
                "item-type prefixes",
                "ENG/ItemTypeInfo_ENG.sox",
            ),
            (
                CatalogRole::WeaponNames,
                "weapon names",
                "Text/ENG/WeaponNames_ENG.txt",
            ),
        ];

        let mut roles = HashSet::new();
        for (role, label, relative_path) in cases {
            assert!(roles.insert(role));
            assert_eq!(role.label(), label);
            assert_eq!(role.relative_path(), Path::new(relative_path));
            assert_eq!(role.to_string(), label);
        }
    }

    #[test]
    fn catalog_complete_load_preserves_all_literal_sources() {
        let tree = complete_catalog_tree();

        let loaded = load_catalog_data(&tree.sox).unwrap();

        assert!(loaded.issues.is_empty());
        assert_indexed(&loaded.troop_names, &[(2, b"Footman"), (4_096, b"Archer")]);
        assert_indexed(&loaded.character_names, &[(0xab, b"Gerald")]);
        assert_indexed(&loaded.leader_pools, &[(73, b"Alpha  Beta\tGamma\x80")]);
        assert_eq!(
            loaded.special_names,
            [
                super::RawSpecialName {
                    key: b"Hero".to_vec(),
                    default_value: vec![0xb0, 0xa1],
                },
                super::RawSpecialName {
                    key: b"NPC".to_vec(),
                    default_value: Vec::new(),
                },
            ]
        );
        assert_eq!(
            loaded.special_display_names,
            [b"Localized Hero".to_vec(), b"Localized NPC".to_vec()]
        );
        assert_eq!(
            loaded.item_attributes,
            [super::RawItemAttribute {
                id: 91,
                name: b"Flame\xff".to_vec(),
                description: b"Adds fire".to_vec(),
            }]
        );
        assert_eq!(
            loaded.item_type_prefixes,
            [super::RawItemTypePrefixes {
                id: 44,
                prefixes: [b"Fine".to_vec(), b"Rare ".to_vec(), b"Epic".to_vec()],
            }]
        );
        assert_eq!(
            loaded.weapon_types,
            [
                vec![
                    super::RawWeaponVariant {
                        id: 9,
                        short_name: "Sword".to_owned(),
                        long_name: "Long Sword".to_owned(),
                    },
                    super::RawWeaponVariant {
                        id: 27,
                        short_name: "Sabre".to_owned(),
                        long_name: "Long Sabre".to_owned(),
                    },
                ],
                vec![super::RawWeaponVariant {
                    id: 3,
                    short_name: "Axe".to_owned(),
                    long_name: "War Axe".to_owned(),
                }],
            ]
        );
    }

    #[test]
    fn catalog_missing_optional_role_is_an_issue_with_useful_data() {
        let tree = complete_catalog_tree();
        let missing_path = tree.role_path(CatalogRole::LeaderPools);
        fs::remove_file(&missing_path).unwrap();

        let loaded = load_catalog_data(&tree.sox).unwrap();

        assert_eq!(loaded.troop_names.len(), 2);
        assert!(loaded.leader_pools.is_empty());
        assert_eq!(loaded.issues.len(), 1);
        let issue = loaded.issues.first().unwrap();
        assert_eq!(issue.role, CatalogRole::LeaderPools);
        assert_eq!(issue.path, missing_path);
        assert!(matches!(issue.error, CatalogFileError::Read { .. }));
    }

    #[test]
    fn catalog_malformed_role_keeps_exact_path_and_later_role_data() {
        let tree = complete_catalog_tree();
        let malformed_path = tree.role_path(CatalogRole::CharacterNames);
        tree.write(CatalogRole::CharacterNames, b"not a SOX table");

        let loaded = load_catalog_data(&tree.sox).unwrap();

        assert!(loaded.character_names.is_empty());
        assert_eq!(loaded.weapon_types.len(), 2);
        let issue = loaded
            .issues
            .iter()
            .find(|issue| issue.role == CatalogRole::CharacterNames)
            .unwrap();
        assert_eq!(issue.path, malformed_path);
        assert!(matches!(issue.error, CatalogFileError::Format { .. }));
    }

    #[test]
    fn catalog_weapon_utf8_and_syntax_issues_retain_exact_lines() {
        let tree = complete_catalog_tree();
        let weapon_path = tree.role_path(CatalogRole::WeaponNames);
        tree.write(CatalogRole::WeaponNames, &[0xff, 0xfe]);

        let invalid_utf8 = load_catalog_data(&tree.sox).unwrap();
        let utf8_issue = invalid_utf8
            .issues
            .iter()
            .find(|issue| issue.role == CatalogRole::WeaponNames)
            .unwrap();
        assert_eq!(utf8_issue.path, weapon_path);
        assert!(matches!(
            utf8_issue.error,
            CatalogFileError::InvalidWeaponUtf8 { .. }
        ));

        let syntax_cases = [
            (b"x\n".as_slice(), 1, "invalid weapon-type count"),
            (
                b"1\n\n-1 // invalid\n".as_slice(),
                3,
                "negative weapon-variant count",
            ),
            (
                b"1\n1\nwrong\nShort\nLong\n".as_slice(),
                3,
                "invalid weapon ID",
            ),
            (
                b"1\n1\n7\n\nLong\n".as_slice(),
                4,
                "weapon short name is empty",
            ),
            (
                b"1\n1\n7\nShort\n".as_slice(),
                5,
                "weapon long name is missing",
            ),
            (
                b"2\n1\n7\nShort\nLong\n".as_slice(),
                6,
                "weapon type is missing",
            ),
        ];

        for (bytes, line, reason) in syntax_cases {
            tree.write(CatalogRole::WeaponNames, bytes);
            let loaded = load_catalog_data(&tree.sox).unwrap();
            let issue = loaded
                .issues
                .iter()
                .find(|issue| issue.role == CatalogRole::WeaponNames)
                .unwrap();
            assert!(matches!(
                &issue.error,
                CatalogFileError::WeaponSyntax {
                    line: actual_line,
                    reason: actual_reason,
                } if *actual_line == line && *actual_reason == reason
            ));
        }
    }

    #[test]
    fn catalog_no_core_result_retains_every_issue() {
        let tree = CatalogTree::new();

        let error = load_catalog_data(&tree.sox).unwrap_err();

        let CatalogLoadError::NoUsableCatalogs { issues } = error else {
            panic!("expected no usable catalogs");
        };
        assert_eq!(issues.len(), 8);
        let roles = issues
            .iter()
            .map(|issue| issue.role)
            .collect::<HashSet<_>>();
        assert_eq!(roles.len(), 8);
        assert!(roles.contains(&CatalogRole::TroopNames));
        assert!(roles.contains(&CatalogRole::WeaponNames));
    }

    #[test]
    fn catalog_empty_core_sources_are_not_usable() {
        let tree = complete_catalog_tree();
        let empty_indexed = indexed_table(&[]);
        tree.write(CatalogRole::TroopNames, &empty_indexed);
        tree.write(CatalogRole::CharacterNames, &empty_indexed);
        tree.write(CatalogRole::SpecialNameKeys, &special_names_table(&[]));

        let error = load_catalog_data(&tree.sox).unwrap_err();

        assert!(matches!(
            error,
            CatalogLoadError::NoUsableCatalogs { ref issues } if issues.is_empty()
        ));
    }

    #[test]
    fn catalog_selected_sox_path_must_be_a_directory() {
        let temporary = TempDir::new().unwrap();
        let file_path = temporary.path().join("SOX");
        fs::write(&file_path, b"not a directory").unwrap();

        let error = load_catalog_data(&file_path).unwrap_err();

        assert!(matches!(
            error,
            CatalogLoadError::InvalidSoxDirectory { path } if path == file_path
        ));
    }

    fn complete_catalog_tree() -> CatalogTree {
        let tree = CatalogTree::new();
        tree.write(
            CatalogRole::TroopNames,
            &indexed_table(&[(2, b"Footman"), (4_096, b"Archer")]),
        );
        let character_source = mixed_case_ascii_hex(&indexed_table(&[(0xab, b"Gerald")]));
        assert!(
            character_source
                .iter()
                .any(|byte| matches!(byte, b'a'..=b'f'))
        );
        tree.write(CatalogRole::CharacterNames, &character_source);
        tree.write(
            CatalogRole::LeaderPools,
            &indexed_table(&[(73, b"Alpha  Beta\tGamma\x80")]),
        );
        tree.write(
            CatalogRole::SpecialNameKeys,
            &special_names_table(&[(b"Hero", &[0xb0, 0xa1]), (b"NPC", b"")]),
        );
        tree.write(
            CatalogRole::SpecialDisplayNames,
            &sequential_table(&[b"Localized Hero", b"Localized NPC"]),
        );
        tree.write(
            CatalogRole::ItemAttributes,
            &indexed_fields_table(&[(91, &[b"Flame\xff", b"Adds fire"])]),
        );
        tree.write(
            CatalogRole::ItemTypePrefixes,
            &indexed_fields_table(&[(44, &[b"Fine", b"Rare ", b"Epic"])]),
        );
        tree.write(
            CatalogRole::WeaponNames,
            b"2\n\n2\t// swords\n9\nSword\nLong Sword\n27\nSabre\nLong Sabre\n  \n1 // axes\n3\nAxe\nWar Axe\n",
        );
        tree
    }

    fn assert_indexed(records: &[super::RawIndexedRecord], expected: &[(u32, &[u8])]) {
        assert_eq!(records.len(), expected.len());
        for (record, (id, value)) in records.iter().zip(expected) {
            assert_eq!(record.id, *id);
            assert_eq!(record.value, *value);
        }
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

    fn mixed_case_ascii_hex(bytes: &[u8]) -> Vec<u8> {
        const DIGITS: &[u8; 16] = b"0123456789ABCDEF";

        let mut encoded = Vec::with_capacity(bytes.len() * 2);
        for (index, byte) in bytes.iter().copied().enumerate() {
            let high = DIGITS.get(usize::from(byte >> 4)).copied().unwrap();
            let low = DIGITS.get(usize::from(byte & 0x0f)).copied().unwrap();
            encoded.push(if index.is_multiple_of(2) {
                high.to_ascii_lowercase()
            } else {
                high
            });
            encoded.push(low);
        }
        encoded
    }
}
