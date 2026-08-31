const UI_SOURCES: &[(&str, &str)] = &[
    ("frame.rs", include_str!("../src/frame.rs")),
    ("frame/catalog.rs", include_str!("../src/frame/catalog.rs")),
    (
        "frame/discovery.rs",
        include_str!("../src/frame/discovery.rs"),
    ),
    ("frame/mods.rs", include_str!("../src/frame/mods.rs")),
    ("frame/save.rs", include_str!("../src/frame/save.rs")),
    ("frame/stg.rs", include_str!("../src/frame/stg.rs")),
    ("views/files.rs", include_str!("../src/views/files.rs")),
    ("views/mods.rs", include_str!("../src/views/mods.rs")),
    ("views/patches.rs", include_str!("../src/views/patches.rs")),
    ("views/save.rs", include_str!("../src/views/save.rs")),
    (
        "views/settings.rs",
        include_str!("../src/views/settings.rs"),
    ),
    ("views/stg.rs", include_str!("../src/views/stg.rs")),
    (
        "kufeditor-formats/error.rs",
        include_str!("../../kufeditor-formats/src/error.rs"),
    ),
    (
        "kufeditor-formats/stg/mutation.rs",
        include_str!("../../kufeditor-formats/src/stg/mutation.rs"),
    ),
    (
        "kufeditor-game/discovery.rs",
        include_str!("../../kufeditor-game/src/discovery.rs"),
    ),
    (
        "kufeditor-mods/error.rs",
        include_str!("../../kufeditor-mods/src/error.rs"),
    ),
    (
        "kufeditor-patches/definitions.rs",
        include_str!("../../kufeditor-patches/src/definitions.rs"),
    ),
];

const REJECTED_PHRASES: &[&str] = &[
    "tools, reforged",
    "first slice",
    "source-preserving Rust codecs",
    "guarded binary edits",
    "Recovery backup",
    "executable context",
    "Creating recovery",
    "Creating restore recovery",
    "configured game root",
    "The game root",
    "Game root",
    "active game root",
    "another game root",
    "selected game root",
    "invalid game root",
    "safe game-relative path",
    "Full snapshots",
    "source-preserved",
    "source fields",
    "typed source value",
    "source values",
    "preserved exactly",
    "preserved as raw bytes",
    "stable equipment slots",
    "raw value preserved",
    "raw fallback labels",
    "Opaque source data",
    "display safely",
    "structured-view stage",
    "raw values remain available",
    "Raw IDs remain available",
    "TYPED EDITING",
    "STRUCTURED VIEW",
    "SCAN ISSUES",
    "MOD WORKSHOP",
    "game catalogs",
    "Steam discovery",
    "\"Confirm\"",
];

#[test]
fn rendered_copy_avoids_rejected_phrases() {
    let mut matches = Vec::new();

    for (path, source) in UI_SOURCES {
        for phrase in REJECTED_PHRASES {
            if source.contains(phrase) {
                matches.push(format!("{path}: {phrase}"));
            }
        }
    }

    assert!(
        matches.is_empty(),
        "rejected UI copy remains:\n{}",
        matches.join("\n")
    );
}
