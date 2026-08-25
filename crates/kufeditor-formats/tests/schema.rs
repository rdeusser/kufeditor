use std::collections::HashSet;

use kufeditor_formats::SoxSchema;

#[test]
fn registry_contains_each_named_schema_once() {
    let schemas: HashSet<_> = SoxSchema::ALL.into_iter().collect();

    assert_eq!(SoxSchema::ALL.len(), 18);
    assert_eq!(schemas.len(), 18);
}

#[test]
fn schemas_expose_canonical_stems_markers_and_display_names() {
    let cases = [
        (SoxSchema::AbilityByJob, "AbilityByJob", 100),
        (SoxSchema::AbilityInfo, "AbilityInfo", 100),
        (SoxSchema::CharInfo, "CharInfo", 100),
        (SoxSchema::CustomRandomTable, "KUF2CustomRandomTable", 100),
        (SoxSchema::ItemAttInfo, "ItemAttInfo", 100),
        (SoxSchema::ItemTypeInfo, "ItemTypeInfo", 2),
        (SoxSchema::JobInfo, "JobInfo", 100),
        (SoxSchema::LeaderGeneration, "LeaderGeneration", 100),
        (SoxSchema::LibraryInfo, "LibraryInfo", 100),
        (SoxSchema::ResistInfo, "ResistInfo", 100),
        (SoxSchema::SkillInfo, "SkillInfo", 100),
        (SoxSchema::SkillPointTable, "SkillPointTable", 100),
        (SoxSchema::SpecialNames, "SpecialNames", 100),
        (SoxSchema::TroopInfo, "TroopInfo", 100),
        (SoxSchema::UnitUvInfo, "UnitUVInfo", 100),
        (SoxSchema::UnitUvid, "UnitUVID", 100),
        (SoxSchema::WorldmapCharInfo, "WorldMap_CharInfo", 100),
        (SoxSchema::WorldmapTroopInfo, "WorldMap_TroopInfo", 100),
    ];

    for (schema, stem, marker) in cases {
        assert_eq!(schema.file_stem(), stem);
        assert_eq!(schema.marker(), marker);
        assert_eq!(schema.to_string(), stem);
    }
}
