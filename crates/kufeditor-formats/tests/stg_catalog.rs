use std::collections::HashSet;

use kufeditor_formats::stg::catalog::{STGScriptInfo, action, actions, condition, conditions};

const FNV64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV64_PRIME: u64 = 0x0000_0100_0000_01b3;

#[test]
fn script_catalog_retains_the_migrated_compatibility_snapshot() {
    assert_eq!(conditions().len(), 59);
    assert_eq!(actions().len(), 167);
    assert_eq!(catalog_identity(), 0x0df0_6072_6802_eddf);
}

#[test]
fn script_catalog_ids_are_unique() {
    assert_unique_ids("condition", conditions());
    assert_unique_ids("action", actions());
}

#[test]
fn lookups_preserve_known_gaps_and_raw_ids() {
    assert_eq!(
        condition(0).map(|entry| entry.name),
        Some("CON_TIME_ELAPSED")
    );
    assert_eq!(
        condition(60).map(|entry| entry.name),
        Some("CON_SELECTED_TROOP")
    );
    assert!(condition(16).is_none());
    assert!(condition(21).is_none());
    assert!(condition(u32::MAX).is_none());

    assert_eq!(
        action(0).map(|entry| entry.name),
        Some("ACT_TRIGGER_ACTIVATE")
    );
    assert_eq!(action(182).map(|entry| entry.name), Some("ACT_SKIP_TEXT"));
    for gap in [
        25, 30, 31, 36, 37, 40, 41, 42, 43, 44, 45, 46, 48, 69, 134, 156,
    ] {
        assert!(action(gap).is_none(), "action gap {gap} was populated");
    }
    assert!(action(u32::MAX).is_none());
}

fn catalog_identity() -> u64 {
    let mut hash = FNV64_OFFSET_BASIS;
    update_hash(&mut hash, b"conditions\0");
    update_catalog_hash(&mut hash, conditions());
    update_hash(&mut hash, b"actions\0");
    update_catalog_hash(&mut hash, actions());
    hash
}

fn update_catalog_hash(hash: &mut u64, entries: &[STGScriptInfo]) {
    for entry in entries {
        update_hash(hash, &entry.id.to_le_bytes());
        update_hash(hash, entry.name.as_bytes());
        update_hash(hash, &[0]);
        update_hash(hash, &entry.parameter_count.to_le_bytes());
        for hint in entry.parameter_hints {
            update_hash(hash, hint.as_bytes());
            update_hash(hash, &[0]);
        }
    }
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV64_PRIME);
    }
}

fn assert_unique_ids(label: &str, entries: &[STGScriptInfo]) {
    let mut ids = HashSet::with_capacity(entries.len());
    for entry in entries {
        assert!(
            ids.insert(entry.id),
            "{label} ID {} is duplicated",
            entry.id
        );
    }
}
