mod support;

use std::{fs, ops::ControlFlow};

use kufeditor_game::Game;
use kufeditor_mods::{
    ApplyModRequest, GameRoot, InstallationConflictKind, ModError, ModProgress, ModProgressPhase,
    ModProgressReporter, ModService, ModStorePaths, OperationState, RelativeGamePath,
    TargetPathErrorKind,
};
use serde_json::Value;
use support::{
    TestDirectory,
    package::{FixtureCompression, write_zip_package},
};

#[derive(Default)]
struct RecordingProgress {
    reports: Vec<ModProgress>,
    cancel_after_first_commit: bool,
}

impl ModProgressReporter for RecordingProgress {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        self.reports.push(progress.clone());
        if self.cancel_after_first_commit
            && progress.phase == ModProgressPhase::CommittingFiles
            && progress.completed == 1
        {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[test]
fn apply_commits_existing_and_absent_files_then_records_a_clean_installation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = ApplyFixture::new("success", Game::Heroes)?;
    fs::write(fixture.root_path.join("data/existing.sox"), b"old")?;
    let package = fixture.import(
        "Combined",
        Game::Heroes,
        &[
            ("data/existing.sox", b"new"),
            ("data/nested/added.sox", b"added"),
        ],
    )?;
    let mut progress = RecordingProgress::default();

    let report = fixture
        .service
        .apply(ApplyModRequest::new(&fixture.root, package), &mut progress)?;

    assert_eq!(
        fs::read(fixture.root_path.join("data/existing.sox"))?,
        b"new"
    );
    assert_eq!(
        fs::read(fixture.root_path.join("data/nested/added.sox"))?,
        b"added"
    );
    assert_eq!(
        report
            .committed_paths()
            .iter()
            .map(RelativeGamePath::as_str)
            .collect::<Vec<_>>(),
        ["data/existing.sox", "data/nested/added.sox"]
    );
    assert_eq!(report.installation().metadata().name(), "Combined");
    assert_eq!(report.installation().files().len(), 2);
    assert!(
        report
            .installation()
            .files()
            .first()
            .ok_or("missing existing installed file")?
            .original_existed()
    );
    assert!(
        !report
            .installation()
            .files()
            .get(1)
            .ok_or("missing absent installed file")?
            .original_existed()
    );

    let scan = fixture.service.scan_installations(&fixture.root)?;
    assert_eq!(
        scan.installations(),
        std::slice::from_ref(report.installation())
    );
    assert!(scan.issues().is_empty());

    assert_operation_image(&fixture, &report)?;

    assert_progress_is_bounded_and_monotonic(&progress.reports);
    for phase in [
        ModProgressPhase::StagingFiles,
        ModProgressPhase::CreatingRecovery,
        ModProgressPhase::CommittingFiles,
    ] {
        assert_eq!(
            progress
                .reports
                .iter()
                .filter(|report| report.phase == phase)
                .filter_map(|report| report.path.as_ref().map(RelativeGamePath::as_str))
                .collect::<Vec<_>>(),
            ["data/existing.sox", "data/nested/added.sox"]
        );
    }
    Ok(())
}

#[test]
fn apply_rejects_wrong_game_missing_root_and_unsafe_target_without_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = ApplyFixture::new("validation", Game::Heroes)?;
    let wrong_game = fixture.import("Wrong game", Game::Crusaders, &[("wrong.sox", b"wrong")])?;

    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, wrong_game),
            &mut RecordingProgress::default()
        ),
        Err(ModError::PackageGameMismatch {
            package: Game::Crusaders,
            target: Game::Heroes,
        })
    ));
    assert!(!fixture.root_path.join("wrong.sox").exists());
    assert!(!fixture.stores.installation_registry().exists());

    let missing = fixture.import("Missing root", Game::Heroes, &[("missing.sox", b"missing")])?;
    fs::remove_dir_all(&fixture.root_path)?;
    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, missing),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InvalidGameRoot {
            kind: kufeditor_mods::GameRootErrorKind::Missing,
            ..
        })
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        fs::create_dir_all(&fixture.root_path)?;
        let outside = fixture.directory.path().join("outside");
        fs::create_dir_all(&outside)?;
        symlink(&outside, fixture.root_path.join("linked"))?;
        let linked = fixture.import(
            "Linked target",
            Game::Heroes,
            &[("linked/file.sox", b"escape")],
        )?;
        assert!(matches!(
            fixture.service.apply(
                ApplyModRequest::new(&fixture.root, linked),
                &mut RecordingProgress::default()
            ),
            Err(ModError::InvalidTargetPath {
                kind: TargetPathErrorKind::SymbolicLink,
                ..
            })
        ));
        assert!(!outside.join("file.sox").exists());
    }
    Ok(())
}

#[test]
fn apply_rejects_duplicate_names_and_portable_path_conflicts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = ApplyFixture::new("conflicts", Game::Heroes)?;
    let first = fixture.import("Owned", Game::Heroes, &[("Data/owned.sox", b"one")])?;
    fixture.service.apply(
        ApplyModRequest::new(&fixture.root, first),
        &mut RecordingProgress::default(),
    )?;
    let registry_before = fs::read(fixture.stores.installation_registry())?;

    let duplicate_name = fixture.import("owned", Game::Heroes, &[("other.sox", b"two")])?;
    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, duplicate_name),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InstallationConflict {
            kind: InstallationConflictKind::DuplicateName,
            path: None,
            ..
        })
    ));

    let overlapping_path =
        fixture.import("Different", Game::Heroes, &[("data/OWNED.SOX", b"three")])?;
    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, overlapping_path),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InstallationConflict {
            kind: InstallationConflictKind::PathOverlap,
            path: Some(_),
            ..
        })
    ));
    assert_eq!(
        fs::read(fixture.stores.installation_registry())?,
        registry_before
    );
    assert_eq!(fs::read(fixture.root_path.join("Data/owned.sox"))?, b"one");
    Ok(())
}

#[test]
fn apply_rejects_missing_and_changed_library_packages_without_game_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = ApplyFixture::new("package-image", Game::Heroes)?;
    let missing = kufeditor_mods::ModPackageID::from_bytes([0xee; 32]);
    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, missing),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InvalidPackage {
            kind: kufeditor_mods::PackageErrorKind::MissingLibraryPackage,
            ..
        })
    ));

    let changed = fixture.import("Changed", Game::Heroes, &[("changed.sox", b"expected")])?;
    let library_path = fixture.stores.packages().join(format!("{changed}.zip"));
    write_zip_package(
        &library_path,
        br#"{"name":"Replacement","version":"1","game":"heroes","files":["changed.sox"]}"#,
        &[("changed.sox", b"replacement")],
        FixtureCompression::Stored,
    )?;

    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, changed),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InvalidPackage {
            kind: kufeditor_mods::PackageErrorKind::DestinationCollision,
            ..
        })
    ));
    assert!(!fixture.root_path.join("changed.sox").exists());
    assert!(!fixture.stores.installation_registry().exists());
    Ok(())
}

#[test]
fn apply_rejects_unsupported_target_objects_and_nested_application_data()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = ApplyFixture::new("unsupported-target", Game::Crusaders)?;
    fs::write(fixture.root_path.join("blocked"), b"not a directory")?;
    let blocked = fixture.import(
        "Blocked parent",
        Game::Crusaders,
        &[("blocked/file.sox", b"data")],
    )?;
    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, blocked),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InvalidTargetPath {
            kind: TargetPathErrorKind::ParentNotDirectory,
            ..
        })
    ));

    fs::create_dir(fixture.root_path.join("directory.sox"))?;
    let directory = fixture.import(
        "Directory target",
        Game::Crusaders,
        &[("directory.sox", b"data")],
    )?;
    assert!(matches!(
        fixture.service.apply(
            ApplyModRequest::new(&fixture.root, directory),
            &mut RecordingProgress::default()
        ),
        Err(ModError::InvalidTargetPath {
            kind: TargetPathErrorKind::NotRegularFile,
            ..
        })
    ));

    let nested_stores = ModStorePaths::new(fixture.root_path.join("application-data"));
    assert!(matches!(
        GameRoot::inspect(Game::Crusaders, fixture.root_path.clone(), &nested_stores),
        Err(ModError::InvalidGameRoot {
            kind: kufeditor_mods::GameRootErrorKind::StoreOverlapsGameRoot,
            ..
        })
    ));
    Ok(())
}

#[test]
fn cancellation_after_a_commit_rolls_back_and_reports_every_path()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = ApplyFixture::new("cancel-rollback", Game::Crusaders)?;
    fs::write(fixture.root_path.join("a.sox"), b"old-a")?;
    let package = fixture.import(
        "Canceled",
        Game::Crusaders,
        &[
            ("a.sox", b"new-a"),
            ("b.sox", b"new-b"),
            ("c.sox", b"new-c"),
        ],
    )?;
    let mut progress = RecordingProgress {
        reports: Vec::new(),
        cancel_after_first_commit: true,
    };

    let error = fixture
        .service
        .apply(ApplyModRequest::new(&fixture.root, package), &mut progress)
        .expect_err("apply must honor cancellation after its first committed path");
    let recovery = error
        .recovery_report()
        .ok_or("post-write cancellation must include recovery evidence")?;

    assert_eq!(fs::read(fixture.root_path.join("a.sox"))?, b"old-a");
    assert!(!fixture.root_path.join("b.sox").exists());
    assert!(!fixture.root_path.join("c.sox").exists());
    assert_eq!(path_strings(recovery.committed()), ["a.sox"]);
    assert_eq!(path_strings(recovery.rolled_back()), ["a.sox"]);
    assert!(recovery.rollback_failed().is_empty());
    assert_eq!(path_strings(recovery.unchanged()), ["b.sox", "c.sox"]);
    assert!(!fixture.stores.installation_registry().exists());
    assert_eq!(fs::read_dir(fixture.stores.operations())?.count(), 1);
    Ok(())
}

fn assert_progress_is_bounded_and_monotonic(reports: &[ModProgress]) {
    let mut phase_progress = Vec::<(ModProgressPhase, u64)>::new();
    for report in reports {
        assert!(report.completed <= report.total);
        if let Some((_, previous)) = phase_progress
            .iter_mut()
            .find(|(phase, _)| *phase == report.phase)
        {
            assert!(*previous <= report.completed);
            *previous = report.completed;
        } else {
            phase_progress.push((report.phase, report.completed));
        }
    }
}

fn path_strings(paths: &[RelativeGamePath]) -> Vec<&str> {
    paths.iter().map(RelativeGamePath::as_str).collect()
}

fn assert_operation_image(
    fixture: &ApplyFixture,
    report: &kufeditor_mods::ApplyModReport,
) -> Result<(), Box<dyn std::error::Error>> {
    let operation_path = fixture
        .stores
        .operations()
        .join(report.installation().operation_id().to_string())
        .join("operation-v1.json");
    let operation: Value = serde_json::from_slice(&fs::read(operation_path)?)?;
    assert_eq!(
        operation.get("state"),
        Some(&serde_json::json!(OperationState::Committed))
    );
    assert_eq!(
        operation.get("createdDirectories"),
        Some(&serde_json::json!(["data/nested"]))
    );
    let operation_files = operation
        .get("files")
        .and_then(Value::as_array)
        .ok_or("operation image has no file plan")?;
    assert_eq!(
        operation_files
            .first()
            .and_then(|file| file.get("originalExisted")),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        operation_files
            .get(1)
            .and_then(|file| file.get("originalExisted")),
        Some(&Value::Bool(false))
    );
    assert!(operation_files.iter().all(|file| {
        file.get("installedSHA256")
            .and_then(Value::as_str)
            .is_some_and(|digest| digest.len() == 64)
    }));
    Ok(())
}

struct ApplyFixture {
    directory: TestDirectory,
    stores: ModStorePaths,
    service: ModService,
    root_path: std::path::PathBuf,
    root: GameRoot,
}

impl ApplyFixture {
    fn new(label: &str, game: Game) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = TestDirectory::new(label)?;
        let stores = ModStorePaths::new(directory.path().join("application-data"));
        let root_path = directory.path().join("game");
        fs::create_dir_all(root_path.join("data"))?;
        let root = GameRoot::inspect(game, root_path.clone(), &stores)?;
        let service = ModService::new(stores.clone());
        Ok(Self {
            directory,
            stores,
            service,
            root_path,
            root,
        })
    }

    fn import(
        &self,
        name: &str,
        game: Game,
        payloads: &[(&str, &[u8])],
    ) -> Result<kufeditor_mods::ModPackageID, Box<dyn std::error::Error>> {
        let source = self.directory.path().join(format!(
            "{}-{}.zip",
            name.replace(' ', "-"),
            payloads.len()
        ));
        let files = payloads
            .iter()
            .map(|(path, _)| format!("\"{path}\""))
            .collect::<Vec<_>>()
            .join(",");
        let game = match game {
            Game::Crusaders => "crusaders",
            Game::Heroes => "heroes",
        };
        let manifest = format!(
            "{{\"formatVersion\":1,\"name\":{name:?},\"version\":\"1\",\"game\":\"{game}\",\"files\":[{files}]}}"
        );
        write_zip_package(
            &source,
            manifest.as_bytes(),
            payloads,
            FixtureCompression::Stored,
        )?;
        Ok(self
            .service
            .import_package(&source, &mut RecordingProgress::default())?
            .package()
            .package_id())
    }
}
