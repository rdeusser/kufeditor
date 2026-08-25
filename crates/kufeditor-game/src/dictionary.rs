use std::{collections::HashMap, path::Path};

use encoding_rs::EUC_KR;

use crate::{
    CatalogFileError, CatalogIssue, CatalogLoadError, CatalogRole,
    catalog::{
        RawCatalogData, RawIndexedRecord, RawItemAttribute, RawItemTypePrefixes, RawSpecialName,
        RawWeaponVariant, load_catalog_data, role_path,
    },
    static_translations::STATIC_TRANSLATIONS,
};

const MAX_STANDARD_JOB_TYPE: u32 = 42;

#[derive(Debug)]
pub struct NameDictionary {
    troop_names: HashMap<u32, String>,
    character_names: HashMap<u32, String>,
    leader_pools: HashMap<u32, Vec<String>>,
    weapon_types: Vec<Vec<RawWeaponVariant>>,
    item_attributes: HashMap<u32, ItemAttribute>,
    item_type_prefixes: HashMap<u32, [Option<String>; 3]>,
    forward_translations: HashMap<String, String>,
    reverse_translations: HashMap<String, String>,
    reverse_phrases: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct CatalogLoad {
    pub dictionary: NameDictionary,
    pub issues: Vec<CatalogIssue>,
}

#[derive(Debug)]
struct ItemAttribute {
    name: Option<String>,
    description: Option<String>,
}

struct TranslationMaps {
    forward: HashMap<String, String>,
    reverse: HashMap<String, String>,
    phrases: Vec<(String, String)>,
}

pub fn load_name_dictionary(sox_directory: &Path) -> Result<CatalogLoad, CatalogLoadError> {
    let RawCatalogData {
        troop_names: raw_troop_names,
        character_names: raw_character_names,
        leader_pools: raw_leader_pools,
        special_names: raw_special_names,
        special_display_names: raw_special_display_names,
        item_attributes: raw_item_attributes,
        item_type_prefixes: raw_item_type_prefixes,
        weapon_types,
        mut issues,
    } = load_catalog_data(sox_directory)?;

    let troop_names = decode_indexed_values(
        raw_troop_names,
        CatalogRole::TroopNames,
        sox_directory,
        &mut issues,
    );
    let character_names = decode_indexed_values(
        raw_character_names,
        CatalogRole::CharacterNames,
        sox_directory,
        &mut issues,
    );
    let leader_pools = decode_leader_pools(raw_leader_pools, sox_directory, &mut issues);
    let special_names = decode_special_names(
        &raw_special_names,
        &raw_special_display_names,
        sox_directory,
        &mut issues,
    );
    let item_attributes = decode_item_attributes(raw_item_attributes, sox_directory, &mut issues);
    let item_type_prefixes =
        decode_item_type_prefixes(raw_item_type_prefixes, sox_directory, &mut issues);

    if troop_names.is_empty() && character_names.is_empty() && special_names.is_empty() {
        return Err(CatalogLoadError::NoUsableCatalogs { issues });
    }

    let TranslationMaps {
        forward: forward_translations,
        reverse: reverse_translations,
        phrases: reverse_phrases,
    } = build_translation_maps(special_names);

    Ok(CatalogLoad {
        dictionary: NameDictionary {
            troop_names,
            character_names,
            leader_pools,
            weapon_types,
            item_attributes,
            item_type_prefixes,
            forward_translations,
            reverse_translations,
            reverse_phrases,
        },
        issues,
    })
}

impl NameDictionary {
    pub fn troop_name(&self, index: u32) -> Option<&str> {
        self.troop_names.get(&index).map(String::as_str)
    }

    pub fn character_name(&self, job_type: u8) -> Option<&str> {
        self.character_names
            .get(&u32::from(job_type))
            .map(String::as_str)
    }

    pub fn leader_name(&self, pool_index: u32, name_index: i32) -> Option<&str> {
        let name_index = usize::try_from(name_index).ok()?;
        self.leader_pools
            .get(&pool_index)?
            .get(name_index)
            .map(String::as_str)
    }

    pub fn unit_name(
        &self,
        leader_name_index: i32,
        troop_info_index: i32,
        job_type: u32,
    ) -> Option<&str> {
        if leader_name_index < 0 {
            if let Ok(character_index) = u8::try_from(job_type)
                && let Some(name) = self.character_name(character_index)
            {
                return Some(name);
            }
        } else if let Ok(pool_index) = u32::try_from(troop_info_index)
            && let Some(name) = self.leader_name(pool_index, leader_name_index)
        {
            return Some(name);
        }

        if job_type <= MAX_STANDARD_JOB_TYPE
            && let Some(name) = self.troop_name(job_type)
        {
            return Some(name);
        }

        let character_index = u8::try_from(job_type).ok()?;
        self.character_name(character_index)
    }

    pub fn weapon_name(
        &self,
        item_type: i32,
        variant: u16,
        enhancement_tier: i16,
    ) -> Option<String> {
        let type_index = usize::try_from(item_type).ok()?;
        let variants = self.weapon_types.get(type_index)?;
        let stored_id = i32::from(variant).checked_add(1)?;
        let weapon = variants
            .get(usize::from(variant))
            .or_else(|| variants.iter().find(|weapon| weapon.id == stored_id))
            .or_else(|| variants.first())?;

        if enhancement_tier < 0 {
            return Some(weapon.long_name.clone());
        }

        let mut name = weapon.short_name.clone();
        let Ok(prefix_index) = usize::try_from(enhancement_tier) else {
            return Some(name);
        };
        if prefix_index > 2 {
            return Some(name);
        }

        let item_type_id = u32::try_from(item_type).ok()?;
        let Some(prefix) = self
            .item_type_prefixes
            .get(&item_type_id)
            .and_then(|prefixes| prefixes.get(prefix_index))
            .and_then(Option::as_deref)
            .filter(|prefix| !prefix.is_empty())
        else {
            return Some(name);
        };

        if prefix.chars().last().is_some_and(char::is_whitespace) {
            name.insert_str(0, prefix);
        } else {
            name.insert(0, ' ');
            name.insert_str(0, prefix);
        }
        Some(name)
    }

    pub fn item_type_base_name(&self, item_type: i32) -> Option<&str> {
        let type_index = usize::try_from(item_type).ok()?;
        self.weapon_types
            .get(type_index)?
            .first()
            .map(|weapon| weapon.short_name.as_str())
    }

    pub fn item_attribute_name(&self, index: i32) -> Option<&str> {
        let index = u32::try_from(index).ok()?;
        self.item_attributes.get(&index)?.name.as_deref()
    }

    pub fn item_attribute_description(&self, index: i32) -> Option<&str> {
        let index = u32::try_from(index).ok()?;
        self.item_attributes.get(&index)?.description.as_deref()
    }

    pub fn translate(&self, korean: &str) -> Option<String> {
        if korean.is_empty() {
            return None;
        }
        let normalized = strip_delimiters(korean);
        if normalized.is_empty() {
            return None;
        }

        if let Some(translation) = self.forward_translations.get(normalized) {
            return Some(translation.clone());
        }

        let without_digits =
            normalized.trim_end_matches(|character: char| character.is_ascii_digit());
        if !without_digits.is_empty()
            && without_digits != normalized
            && let Some(translation) = self.forward_translations.get(without_digits)
        {
            return Some(translation.clone());
        }

        translate_hangul_segments(&self.forward_translations, normalized)
    }

    pub fn reverse_translate(&self, english: &str) -> Option<String> {
        if english.is_empty() {
            return None;
        }
        if let Some(translation) = self.reverse_translations.get(english) {
            return Some(translation.clone());
        }

        let mut translated = String::with_capacity(english.len());
        let mut cursor = 0;
        let mut matched = false;
        while cursor < english.len() {
            let remaining = english.get(cursor..)?;
            if let Some((phrase, replacement)) = self
                .reverse_phrases
                .iter()
                .find(|(phrase, _)| remaining.starts_with(phrase))
            {
                translated.push_str(replacement);
                cursor = cursor.checked_add(phrase.len())?;
                matched = true;
                continue;
            }

            let character = remaining.chars().next()?;
            translated.push(character);
            cursor = cursor.checked_add(character.len_utf8())?;
        }

        matched.then_some(translated)
    }
}

fn decode_indexed_values(
    records: Vec<RawIndexedRecord>,
    role: CatalogRole,
    sox_directory: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> HashMap<u32, String> {
    let path = role_path(sox_directory, role);
    records
        .into_iter()
        .enumerate()
        .filter_map(|(record, value)| {
            let decoded = decode_utf8(&value.value, role, &path, record, 0, issues)?;
            (!decoded.is_empty()).then_some((value.id, decoded))
        })
        .collect()
}

fn decode_leader_pools(
    records: Vec<RawIndexedRecord>,
    sox_directory: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> HashMap<u32, Vec<String>> {
    let role = CatalogRole::LeaderPools;
    let path = role_path(sox_directory, role);
    records
        .into_iter()
        .enumerate()
        .filter_map(|(record, value)| {
            let decoded = decode_utf8(&value.value, role, &path, record, 0, issues)?;
            let names = decoded
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            (!names.is_empty()).then_some((value.id, names))
        })
        .collect()
}

fn decode_special_names(
    raw_names: &[RawSpecialName],
    raw_displays: &[Vec<u8>],
    sox_directory: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> Vec<(String, String)> {
    let key_role = CatalogRole::SpecialNameKeys;
    let display_role = CatalogRole::SpecialDisplayNames;
    let key_path = role_path(sox_directory, key_role);
    let display_path = role_path(sox_directory, display_role);
    let mut pairs = Vec::new();
    let decoded_displays = raw_displays
        .iter()
        .enumerate()
        .map(|(record, value)| decode_utf8(value, display_role, &display_path, record, 0, issues))
        .collect::<Vec<_>>();

    for (record, raw_name) in raw_names.iter().enumerate() {
        let decoded_key = decode_cp949(&raw_name.key, key_role, &key_path, record, 0, issues);
        let key = decoded_key.as_deref().and_then(normalize_dynamic_value);
        let default_value = decode_cp949(
            &raw_name.default_value,
            key_role,
            &key_path,
            record,
            1,
            issues,
        );
        let localized = decoded_displays.get(record).and_then(Option::as_deref);
        let display = localized
            .filter(|value| !value.is_empty())
            .or_else(|| default_value.as_deref().filter(|value| !value.is_empty()))
            .and_then(normalize_dynamic_value);

        if let (Some(key), Some(display)) = (key, display) {
            pairs.push((key, display));
        }
    }

    pairs
}

fn decode_item_attributes(
    records: Vec<RawItemAttribute>,
    sox_directory: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> HashMap<u32, ItemAttribute> {
    let role = CatalogRole::ItemAttributes;
    let path = role_path(sox_directory, role);
    records
        .into_iter()
        .enumerate()
        .map(|(record, value)| {
            let name = decode_utf8(&value.name, role, &path, record, 0, issues)
                .filter(|name| !name.is_empty());
            let description = decode_utf8(&value.description, role, &path, record, 1, issues)
                .filter(|description| !description.is_empty());
            (value.id, ItemAttribute { name, description })
        })
        .collect()
}

fn decode_item_type_prefixes(
    records: Vec<RawItemTypePrefixes>,
    sox_directory: &Path,
    issues: &mut Vec<CatalogIssue>,
) -> HashMap<u32, [Option<String>; 3]> {
    let role = CatalogRole::ItemTypePrefixes;
    let path = role_path(sox_directory, role);
    records
        .into_iter()
        .enumerate()
        .map(|(record, value)| {
            let [first, second, third] = value.prefixes;
            (
                value.id,
                [
                    decode_utf8(&first, role, &path, record, 0, issues),
                    decode_utf8(&second, role, &path, record, 1, issues),
                    decode_utf8(&third, role, &path, record, 2, issues),
                ],
            )
        })
        .collect()
}

fn decode_utf8(
    bytes: &[u8],
    role: CatalogRole,
    path: &Path,
    record: usize,
    field: usize,
    issues: &mut Vec<CatalogIssue>,
) -> Option<String> {
    if let Ok(value) = std::str::from_utf8(bytes) {
        Some(value.to_owned())
    } else {
        push_encoding_issue(issues, role, path, record, field);
        None
    }
}

fn decode_cp949(
    bytes: &[u8],
    role: CatalogRole,
    path: &Path,
    record: usize,
    field: usize,
    issues: &mut Vec<CatalogIssue>,
) -> Option<String> {
    let Some(value) = EUC_KR.decode_without_bom_handling_and_without_replacement(bytes) else {
        push_encoding_issue(issues, role, path, record, field);
        return None;
    };
    Some(value.into_owned())
}

fn push_encoding_issue(
    issues: &mut Vec<CatalogIssue>,
    role: CatalogRole,
    path: &Path,
    record: usize,
    field: usize,
) {
    issues.push(CatalogIssue {
        role,
        path: path.to_path_buf(),
        error: CatalogFileError::InvalidFieldEncoding {
            role,
            record,
            field,
        },
    });
}

fn normalize_dynamic_value(value: &str) -> Option<String> {
    let normalized = strip_delimiters(value);
    (!normalized.is_empty()).then(|| normalized.to_owned())
}

fn strip_delimiters(mut value: &str) -> &str {
    while let Some(stripped) = value.strip_prefix("--") {
        value = stripped;
    }
    while let Some(stripped) = value.strip_suffix("--") {
        value = stripped;
    }
    value
}

fn build_translation_maps(dynamic_pairs: Vec<(String, String)>) -> TranslationMaps {
    let mut forward = HashMap::new();
    for (key, display) in &dynamic_pairs {
        forward.insert(key.clone(), display.clone());
    }
    for &(key, display) in STATIC_TRANSLATIONS {
        forward.insert(key.to_owned(), display.to_owned());
    }

    let mut reverse = HashMap::new();
    for &(key, display) in STATIC_TRANSLATIONS {
        reverse
            .entry(display.to_owned())
            .or_insert_with(|| key.to_owned());
    }
    for (key, display) in dynamic_pairs {
        reverse.entry(display).or_insert(key);
    }

    let mut phrases = reverse
        .iter()
        .filter(|(phrase, _)| !phrase.is_empty())
        .map(|(phrase, replacement)| (phrase.clone(), replacement.clone()))
        .collect::<Vec<_>>();
    phrases.sort_by(|(left, _), (right, _)| {
        right.len().cmp(&left.len()).then_with(|| left.cmp(right))
    });

    TranslationMaps {
        forward,
        reverse,
        phrases,
    }
}

fn translate_hangul_segments(
    translations: &HashMap<String, String>,
    input: &str,
) -> Option<String> {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut matched = false;

    while cursor < input.len() {
        let remaining = input.get(cursor..)?;
        let character = remaining.chars().next()?;
        if !is_hangul(character) {
            output.push(character);
            cursor = cursor.checked_add(character.len_utf8())?;
            continue;
        }

        let segment_start = cursor;
        cursor = cursor.checked_add(character.len_utf8())?;
        while cursor < input.len() {
            let segment_remaining = input.get(cursor..)?;
            let next = segment_remaining.chars().next()?;
            if is_hangul(next) {
                cursor = cursor.checked_add(next.len_utf8())?;
                continue;
            }
            if next == ' ' {
                let after_space = cursor.checked_add(1)?;
                let following = input.get(after_space..)?.chars().next();
                if following.is_some_and(is_hangul) {
                    cursor = after_space;
                    continue;
                }
            }
            break;
        }

        let segment = input.get(segment_start..cursor)?;
        if let Some(translation) = translations.get(segment) {
            output.push_str(translation);
            matched = true;
        } else {
            output.push_str(segment);
        }
    }

    matched.then_some(output)
}

const fn is_hangul(character: char) -> bool {
    matches!(
        character as u32,
        0x1100..=0x11ff | 0x3130..=0x318f | 0xac00..=0xd7a3
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::translate_hangul_segments;

    #[test]
    fn mixed_translation_recognizes_every_hangul_range() {
        let translations = HashMap::from([
            ("가".to_owned(), "syllable".to_owned()),
            ("ㄱ".to_owned(), "compatibility".to_owned()),
            ("ᄀ".to_owned(), "jamo".to_owned()),
        ]);

        assert_eq!(
            translate_hangul_segments(&translations, "x가y"),
            Some("xsyllabley".to_owned())
        );
        assert_eq!(
            translate_hangul_segments(&translations, "xㄱy"),
            Some("xcompatibilityy".to_owned())
        );
        assert_eq!(
            translate_hangul_segments(&translations, "xᄀy"),
            Some("xjamoy".to_owned())
        );
    }
}
