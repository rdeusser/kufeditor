mod support;

use std::{fs, ops::ControlFlow, path::Path};

use kufeditor_game::Game;
use kufeditor_mods::{
    BackupID, GameRoot, GameRootErrorKind, ModError, ModLimits, ModManifest, ModMetadata,
    ModPackageID, ModProgress, ModProgressPhase, ModProgressReporter, ModStorePaths, ModTimestamp,
    OperationID, RelativeGamePath,
};
use support::TestDirectory;

#[test]
fn production_limits_bound_every_package_and_backup_resource() {
    let limits = ModLimits::default();

    assert_eq!(limits.max_zip_bytes, 16 * 1024 * 1024 * 1024);
    assert_eq!(limits.max_manifest_bytes, 1024 * 1024);
    assert_eq!(limits.max_package_files, 65_536);
    assert_eq!(limits.max_file_bytes, 8 * 1024 * 1024 * 1024);
    assert_eq!(limits.max_uncompressed_bytes, 64 * 1024 * 1024 * 1024);
    assert_eq!(limits.max_backup_files, 262_144);
    assert_eq!(limits.max_backup_bytes, 128 * 1024 * 1024 * 1024);
    assert_eq!(limits.max_relative_path_bytes, 4_096);
    assert_eq!(limits.max_relative_path_components, 128);
}

#[test]
fn relative_paths_preserve_display_text_and_make_portable_keys() -> Result<(), ModError> {
    let limits = ModLimits::default();
    let first = RelativeGamePath::parse("Data/Units/Paladin.sox", &limits)?;
    let second = RelativeGamePath::parse("data/units/PALADIN.SOX", &limits)?;

    assert_eq!(first.as_str(), "Data/Units/Paladin.sox");
    assert_eq!(first.portable_key(), "data/units/paladin.sox");
    assert_eq!(first.portable_key(), second.portable_key());
    assert_eq!(first.component_count(), 3);
    Ok(())
}

#[test]
fn relative_paths_reject_archive_controlled_escape_forms() {
    let limits = ModLimits::default();
    let invalid = [
        "",
        "/data/file.sox",
        "C:/data/file.sox",
        "//server/share/file.sox",
        r"data\file.sox",
        "data\0file.sox",
        "data//file.sox",
        "./file.sox",
        "data/../file.sox",
        "data/file.sox ",
        "data/file.sox.",
        "CON",
        "con.txt",
        "data/PRN.bin",
        "AUX/file.sox",
        "nul.dat",
        "COM1/file.sox",
        "com9.txt",
        "LPT1/file.sox",
        "lpt9.log",
    ];

    for value in invalid {
        assert!(
            RelativeGamePath::parse(value, &limits).is_err(),
            "accepted invalid path {value:?}"
        );
    }
}

#[test]
fn relative_paths_enforce_injected_byte_and_component_limits() {
    let limits = ModLimits {
        max_relative_path_bytes: 8,
        max_relative_path_components: 2,
        ..ModLimits::default()
    };

    assert!(RelativeGamePath::parse("12345678", &limits).is_ok());
    assert!(RelativeGamePath::parse("123456789", &limits).is_err());
    assert!(RelativeGamePath::parse("one/two", &limits).is_ok());
    assert!(RelativeGamePath::parse("one/two/three", &limits).is_err());
}

#[test]
fn store_path_construction_has_no_filesystem_side_effect() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = TestDirectory::new("store-paths")?;
    let application_data = directory.path().join("application-data");

    let paths = ModStorePaths::new(application_data.clone());

    assert!(!application_data.exists());
    assert_eq!(paths.application_data(), application_data);
    assert_eq!(paths.root(), application_data.join("mods"));
    assert_eq!(paths.packages(), application_data.join("mods/packages"));
    assert_eq!(
        paths.installation_registry(),
        application_data.join("mods/installations-v1.json")
    );
    assert_eq!(paths.backups(), application_data.join("mods/backups"));
    assert_eq!(paths.operations(), application_data.join("mods/operations"));
    Ok(())
}

#[test]
fn game_root_inspection_canonicalizes_a_real_directory() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("game-root")?;
    let configured = directory.path().join("game/../game");
    fs::create_dir(directory.path().join("game"))?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));

    let root = GameRoot::inspect(Game::Crusaders, configured.clone(), &stores)?;

    assert_eq!(root.game(), Game::Crusaders);
    assert_eq!(root.configured_path(), configured);
    assert_eq!(
        root.canonical_path(),
        fs::canonicalize(directory.path().join("game"))?
    );
    assert_eq!(root.key().to_string().len(), 64);
    Ok(())
}

#[test]
fn game_root_rejects_missing_files_and_nested_application_data()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("invalid-roots")?;
    let game = directory.path().join("game");
    fs::create_dir(&game)?;
    let file = directory.path().join("file");
    fs::write(&file, b"not a directory")?;

    let outside_stores = ModStorePaths::new(directory.path().join("application-data"));
    assert!(matches!(
        GameRoot::inspect(
            Game::Crusaders,
            directory.path().join("missing"),
            &outside_stores
        ),
        Err(ModError::InvalidGameRoot {
            kind: GameRootErrorKind::Missing,
            ..
        })
    ));
    assert!(matches!(
        GameRoot::inspect(Game::Crusaders, file, &outside_stores),
        Err(ModError::InvalidGameRoot {
            kind: GameRootErrorKind::NotDirectory,
            ..
        })
    ));

    let nested_stores = ModStorePaths::new(game.join("kufeditor-data"));
    assert!(matches!(
        GameRoot::inspect(Game::Crusaders, game, &nested_stores),
        Err(ModError::InvalidGameRoot {
            kind: GameRootErrorKind::StoreOverlapsGameRoot,
            ..
        })
    ));
    Ok(())
}

#[test]
fn game_root_rejects_overlap_with_the_owned_mod_store() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("overlapping-mod-store")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    fs::create_dir_all(stores.root())?;

    for game_root in [stores.root().to_path_buf(), stores.root().join("game")] {
        fs::create_dir_all(&game_root)?;
        assert!(matches!(
            GameRoot::inspect(Game::Crusaders, game_root, &stores),
            Err(ModError::InvalidGameRoot {
                kind: GameRootErrorKind::StoreOverlapsGameRoot,
                ..
            })
        ));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn game_root_rejects_a_symbolic_link() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("symlink-root")?;
    let real = directory.path().join("real");
    let linked = directory.path().join("linked");
    fs::create_dir(&real)?;
    symlink(&real, &linked)?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));

    assert!(matches!(
        GameRoot::inspect(Game::Heroes, linked, &stores),
        Err(ModError::InvalidGameRoot {
            kind: GameRootErrorKind::SymbolicLink,
            ..
        })
    ));
    Ok(())
}

#[test]
fn manifest_parses_case_insensitive_games_and_omits_format_version()
-> Result<(), Box<dyn std::error::Error>> {
    let source = br#"{
  "name": "Knight textures",
  "version": "1.2.0",
  "author": "KUF community",
  "description": "Sharper armor",
  "game": "CRUSADERS",
  "created": "2026-08-26T10:00:00-04:00",
  "files": ["data/Z.sox", "Data/a.sox"]
}"#;

    let manifest = ModManifest::from_json(source, &ModLimits::default())?;

    assert_eq!(manifest.game(), Game::Crusaders);
    assert_eq!(manifest.metadata().name(), "Knight textures");
    assert_eq!(manifest.metadata().version(), "1.2.0");
    assert_eq!(manifest.metadata().author(), Some("KUF community"));
    assert_eq!(manifest.metadata().description(), Some("Sharper armor"));
    assert_eq!(
        manifest.metadata().created().map(ModTimestamp::as_str),
        Some("2026-08-26T14:00:00Z")
    );
    assert_eq!(
        manifest
            .files()
            .iter()
            .map(RelativeGamePath::as_str)
            .collect::<Vec<_>>(),
        ["Data/a.sox", "data/Z.sox"]
    );

    let encoded = String::from_utf8(manifest.to_json()?)?;
    assert_eq!(
        encoded,
        concat!(
            "{\n",
            "  \"name\": \"Knight textures\",\n",
            "  \"version\": \"1.2.0\",\n",
            "  \"author\": \"KUF community\",\n",
            "  \"description\": \"Sharper armor\",\n",
            "  \"game\": \"crusaders\",\n",
            "  \"created\": \"2026-08-26T14:00:00Z\",\n",
            "  \"files\": [\n",
            "    \"Data/a.sox\",\n",
            "    \"data/Z.sox\"\n",
            "  ]\n",
            "}\n"
        )
    );
    Ok(())
}

#[test]
fn manifest_rejects_unknown_games_and_duplicate_paths() {
    let invalid = [
        br#"{"name":"A","version":"1","game":"unknown","files":["a"]}"#.as_slice(),
        br#"{"name":"A","version":"1","game":"heroes","files":["Data/a","data/A"]}"#,
    ];

    for source in invalid {
        assert!(ModManifest::from_json(source, &ModLimits::default()).is_err());
    }
}

#[test]
fn manifest_construction_rejects_blank_required_fields_and_empty_files() -> Result<(), ModError> {
    let timestamp = ModTimestamp::parse("2026-08-26T14:00:00Z")?;
    let path = RelativeGamePath::parse("data/file.sox", &ModLimits::default())?;

    assert!(ModMetadata::new("", "1", None, None, None).is_err());
    assert!(ModMetadata::new("Name", "  ", None, None, None).is_err());
    assert!(ModMetadata::new("Name", "1", Some(" ".to_owned()), None, None).is_err());
    let metadata = ModMetadata::new("Name", "1", None, None, Some(timestamp))?;
    assert!(ModManifest::new(metadata.clone(), Game::Heroes, Vec::new()).is_err());
    assert!(ModManifest::new(metadata, Game::Heroes, vec![path]).is_ok());
    Ok(())
}

#[test]
fn manifest_parser_enforces_the_injected_manifest_limit() {
    let source = br#"{"name":"A","version":"1","game":"heroes","files":["a"]}"#;
    let limits = ModLimits {
        max_manifest_bytes: source.len() as u64 - 1,
        ..ModLimits::default()
    };

    assert!(ModManifest::from_json(source, &limits).is_err());
}

#[test]
fn digest_ids_require_exact_lowercase_hex() -> Result<(), ModError> {
    let bytes = [0xabu8; 32];
    let text = "abababababababababababababababababababababababababababababababab";

    assert_eq!(ModPackageID::from_bytes(bytes).to_string(), text);
    assert_eq!(ModPackageID::parse(text)?, ModPackageID::from_bytes(bytes));
    assert_eq!(BackupID::parse(text)?.to_string(), text);
    assert_eq!(OperationID::parse(text)?.to_string(), text);
    assert!(ModPackageID::parse(&text.to_uppercase()).is_err());
    assert!(BackupID::parse(&text[..63]).is_err());
    assert!(OperationID::parse(&format!("{text}00")).is_err());
    Ok(())
}

#[test]
fn progress_reporter_can_request_cancellation() {
    struct CancelAfterFirst {
        reports: Vec<ModProgress>,
    }

    impl ModProgressReporter for CancelAfterFirst {
        fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
            self.reports.push(progress.clone());
            ControlFlow::Break(())
        }
    }

    let progress = ModProgress {
        phase: ModProgressPhase::InspectingPackage,
        completed: 1,
        total: 3,
        path: None,
    };
    let mut reporter = CancelAfterFirst {
        reports: Vec::new(),
    };

    assert_eq!(reporter.report(&progress), ControlFlow::Break(()));
    assert_eq!(reporter.reports, [progress]);
}

#[test]
fn store_getters_return_owned_paths_without_requiring_their_targets() {
    let stores = ModStorePaths::new(Path::new("relative/application-data").to_path_buf());

    assert_eq!(stores.root(), Path::new("relative/application-data/mods"));
    assert!(!stores.root().is_absolute());
}
