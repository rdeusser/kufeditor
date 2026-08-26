#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "the compatibility parser reads one checked-in, structurally asserted C++ initializer"
)]

use std::collections::HashSet;

use kufeditor_formats::stg::catalog::{STGScriptInfo, action, actions, condition, conditions};

const LEGACY_CATALOG: &str = include_str!("../../../src/formats/stg_script_catalog.h");

#[derive(Clone, Debug, Eq, PartialEq)]
struct LegacyEntry {
    id: u32,
    name: String,
    parameter_count: u32,
    parameter_hints: [String; 3],
}

#[test]
fn rust_catalog_matches_every_legacy_condition_and_action_tuple() {
    let legacy_conditions = parse_table(LEGACY_CATALOG, "kConditions[] = {");
    let legacy_actions = parse_table(LEGACY_CATALOG, "kActions[] = {");

    assert_eq!(legacy_conditions.len(), 59);
    assert_eq!(legacy_actions.len(), 167);
    assert_catalog_matches(&legacy_conditions, conditions());
    assert_catalog_matches(&legacy_actions, actions());
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

#[test]
fn compatibility_comparison_rejects_missing_extra_duplicate_and_changed_entries() {
    let legacy = parse_table(LEGACY_CATALOG, "kConditions[] = {");
    let rust = owned_entries(conditions());

    assert!(catalogs_match(&legacy, &rust));

    let mut missing = rust.clone();
    missing.pop();
    assert!(!catalogs_match(&legacy, &missing));

    let mut extra = rust.clone();
    extra.push(LegacyEntry {
        id: 999,
        name: "EXTRA".to_owned(),
        parameter_count: 0,
        parameter_hints: [String::new(), String::new(), String::new()],
    });
    assert!(!catalogs_match(&legacy, &extra));

    let mut duplicate = rust.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(!catalogs_match(&legacy, &duplicate));

    let mut changed = rust;
    changed[0].parameter_hints[0] = "Changed".to_owned();
    assert!(!catalogs_match(&legacy, &changed));
}

fn assert_catalog_matches(legacy: &[LegacyEntry], rust: &[STGScriptInfo]) {
    let rust = owned_entries(rust);
    assert!(
        catalogs_match(legacy, &rust),
        "Rust and legacy STG catalogs differ\nlegacy: {legacy:#?}\nrust: {rust:#?}"
    );
}

fn catalogs_match(left: &[LegacyEntry], right: &[LegacyEntry]) -> bool {
    if left != right {
        return false;
    }

    let left_ids: HashSet<_> = left.iter().map(|entry| entry.id).collect();
    let right_ids: HashSet<_> = right.iter().map(|entry| entry.id).collect();
    left_ids.len() == left.len() && right_ids.len() == right.len()
}

fn owned_entries(entries: &[STGScriptInfo]) -> Vec<LegacyEntry> {
    entries
        .iter()
        .map(|entry| LegacyEntry {
            id: entry.id,
            name: entry.name.to_owned(),
            parameter_count: entry.parameter_count,
            parameter_hints: entry.parameter_hints.map(str::to_owned),
        })
        .collect()
}

fn parse_table(source: &str, marker: &str) -> Vec<LegacyEntry> {
    let (_, remainder) = source.split_once(marker).unwrap();
    let (body, _) = remainder.split_once("};").unwrap();
    let mut parser = Parser::new(body);
    let mut entries = Vec::new();

    while parser.peek().is_some() {
        parser.expect(b'{');
        let id = parser.number();
        parser.expect(b',');
        let name = parser.string();
        parser.expect(b',');
        let parameter_count = parser.number();
        parser.expect(b',');
        parser.expect(b'{');
        let parameter_hints = [
            parser.string(),
            {
                parser.expect(b',');
                parser.string()
            },
            {
                parser.expect(b',');
                parser.string()
            },
        ];
        parser.expect(b'}');
        parser.expect(b'}');
        parser.consume(b',');
        entries.push(LegacyEntry {
            id,
            name,
            parameter_count,
            parameter_hints,
        });
    }

    entries
}

struct Parser<'a> {
    source: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    const fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            offset: 0,
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_whitespace();
        self.source.get(self.offset).copied()
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() != Some(expected) {
            return false;
        }
        self.offset += 1;
        true
    }

    fn expect(&mut self, expected: u8) {
        assert!(
            self.consume(expected),
            "expected {:?} at byte {}",
            char::from(expected),
            self.offset
        );
    }

    fn number(&mut self) -> u32 {
        self.skip_whitespace();
        let start = self.offset;
        while self.source.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        assert_ne!(self.offset, start, "expected number at byte {start}");
        std::str::from_utf8(&self.source[start..self.offset])
            .unwrap()
            .parse()
            .unwrap()
    }

    fn string(&mut self) -> String {
        self.expect(b'"');
        let start = self.offset;
        while self.source.get(self.offset).copied() != Some(b'"') {
            assert!(
                self.source.get(self.offset).is_some(),
                "unterminated string at byte {start}"
            );
            self.offset += 1;
        }
        let value = std::str::from_utf8(&self.source[start..self.offset])
            .unwrap()
            .to_owned();
        self.expect(b'"');
        value
    }

    fn skip_whitespace(&mut self) {
        while self
            .source
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
}
