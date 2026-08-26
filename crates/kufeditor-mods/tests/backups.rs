mod support;

use std::{fs, ops::ControlFlow, path::PathBuf};

use kufeditor_game::Game;
use kufeditor_mods::{
    BackupErrorKind, BackupID, CreateBackupRequest, GameRoot, ModError, ModLimits, ModProgress,
    ModProgressPhase, ModProgressReporter, ModService, ModStorePaths, RelativeGamePath,
    RestoreBackupRequest,
};
use support::TestDirectory;

#[derive(Default)]
struct RecordingProgress {
    reports: Vec<ModProgress>,
    cancel_phase: Option<ModProgressPhase>,
    mutate_after_first_copy: Option<PathBuf>,
}

impl ModProgressReporter for RecordingProgress {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        self.reports.push(progress.clone());
        if progress.phase == ModProgressPhase::CopyingBackup
            && progress.completed == 1
            && let Some(path) = self.mutate_after_first_copy.take()
        {
            let _ = fs::write(path, b"changed after copy");
        }
        if self.cancel_phase == Some(progress.phase) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[test]
fn backup_creation_and_scan_cover_empty_and_nested_roots_in_portable_order()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = BackupFixture::new("create-scan")?;

    let empty = fixture.service.create_backup(
        CreateBackupRequest::new(&fixture.root, Some("Empty baseline".to_owned()))?,
        &mut RecordingProgress::default(),
    )?;
    assert_eq!(empty.file_count(), 0);
    assert_eq!(empty.total_bytes(), 0);
    assert!(empty.files().is_empty());

    write_game_file(&fixture.root_path, "Zulu/file.sox", b"zulu")?;
    write_game_file(&fixture.root_path, "alpha.sox", b"alpha")?;
    let mut progress = RecordingProgress::default();
    let nested = fixture.service.create_backup(
        CreateBackupRequest::new(&fixture.root, Some("Playable".to_owned()))?,
        &mut progress,
    )?;

    assert_eq!(nested.label(), Some("Playable"));
    assert_eq!(nested.file_count(), 2);
    assert_eq!(nested.total_bytes(), 9);
    assert_eq!(
        nested
            .files()
            .iter()
            .map(|file| file.path().as_str())
            .collect::<Vec<_>>(),
        ["alpha.sox", "Zulu/file.sox"]
    );
    assert!(nested.directory().join("backup-v1.json").is_file());
    assert_eq!(
        progress
            .reports
            .iter()
            .filter(|report| report.phase == ModProgressPhase::CopyingBackup)
            .filter_map(|report| report.path.as_ref().map(RelativeGamePath::as_str))
            .collect::<Vec<_>>(),
        ["alpha.sox", "Zulu/file.sox"]
    );

    let scan = fixture.service.scan_backups(&fixture.root)?;
    assert_eq!(scan.backups().len(), 2);
    assert!(scan.issues().is_empty());
    assert!(
        scan.backups()
            .iter()
            .any(|backup| backup.backup_id() == empty.backup_id())
    );
    assert!(
        scan.backups()
            .iter()
            .any(|backup| backup.backup_id() == nested.backup_id())
    );
    Ok(())
}

#[test]
fn backup_creation_rejects_limits_links_unsupported_objects_cancellation_and_source_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = BackupFixture::new("creation-errors")?;
    write_game_file(&fixture.root_path, "a.sox", b"aaaa")?;
    write_game_file(&fixture.root_path, "b.sox", b"bbbb")?;

    let limits = ModLimits {
        max_backup_files: 1,
        ..ModLimits::default()
    };
    let limited = ModService::with_limits(fixture.stores.clone(), limits);
    assert_backup_error(
        &limited.create_backup(
            CreateBackupRequest::new(&fixture.root, None)?,
            &mut RecordingProgress::default(),
        ),
        BackupErrorKind::TooManyFiles,
    );

    let limits = ModLimits {
        max_backup_files: 2,
        max_backup_bytes: 7,
        ..ModLimits::default()
    };
    let limited = ModService::with_limits(fixture.stores.clone(), limits);
    assert_backup_error(
        &limited.create_backup(
            CreateBackupRequest::new(&fixture.root, None)?,
            &mut RecordingProgress::default(),
        ),
        BackupErrorKind::TooLarge,
    );

    let mut canceled = RecordingProgress {
        cancel_phase: Some(ModProgressPhase::CopyingBackup),
        ..RecordingProgress::default()
    };
    assert!(matches!(
        fixture.service.create_backup(
            CreateBackupRequest::new(&fixture.root, None)?,
            &mut canceled
        ),
        Err(ModError::Canceled { .. })
    ));

    let mut changed = RecordingProgress {
        mutate_after_first_copy: Some(fixture.root_path.join("a.sox")),
        ..RecordingProgress::default()
    };
    assert_backup_error(
        &fixture
            .service
            .create_backup(CreateBackupRequest::new(&fixture.root, None)?, &mut changed),
        BackupErrorKind::SourceChanged,
    );

    #[cfg(unix)]
    {
        use std::{os::unix::fs::symlink, process::Command};

        let outside = fixture.directory.path().join("outside.sox");
        fs::write(&outside, b"outside")?;
        symlink(&outside, fixture.root_path.join("linked.sox"))?;
        assert_backup_error(
            &fixture.service.create_backup(
                CreateBackupRequest::new(&fixture.root, None)?,
                &mut RecordingProgress::default(),
            ),
            BackupErrorKind::SymbolicLink,
        );
        fs::remove_file(fixture.root_path.join("linked.sox"))?;

        let pipe_path = fixture.root_path.join("pipe");
        let status = Command::new("mkfifo").arg(&pipe_path).status()?;
        if !status.success() {
            return Err("mkfifo failed for the unsupported-object fixture".into());
        }
        assert_backup_error(
            &fixture.service.create_backup(
                CreateBackupRequest::new(&fixture.root, None)?,
                &mut RecordingProgress::default(),
            ),
            BackupErrorKind::UnsupportedObject,
        );
        fs::remove_file(pipe_path)?;
    }

    assert_no_visible_backups(&fixture)?;
    Ok(())
}

#[test]
fn backup_scan_retains_malformed_unsupported_and_wrong_root_issues()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = BackupFixture::new("scan-issues")?;
    write_game_file(&fixture.root_path, "file.sox", b"data")?;
    let valid = fixture.service.create_backup(
        CreateBackupRequest::new(&fixture.root, None)?,
        &mut RecordingProgress::default(),
    )?;
    let root_store = fixture
        .stores
        .backups()
        .join(fixture.root.key().to_string());

    let malformed = root_store.join("a".repeat(64));
    fs::create_dir(&malformed)?;
    fs::write(malformed.join("backup-v1.json"), b"not JSON")?;

    let unsupported = root_store.join("b".repeat(64));
    copy_directory(valid.directory(), &unsupported)?;
    set_metadata_version(&unsupported.join("backup-v1.json"), 2)?;

    let other_path = fixture.directory.path().join("other-game");
    fs::create_dir(&other_path)?;
    write_game_file(&other_path, "other.sox", b"other")?;
    let other_root = GameRoot::inspect(Game::Heroes, other_path, &fixture.stores)?;
    let other = fixture.service.create_backup(
        CreateBackupRequest::new(&other_root, None)?,
        &mut RecordingProgress::default(),
    )?;
    let wrong_root = root_store.join("c".repeat(64));
    copy_directory(other.directory(), &wrong_root)?;

    let scan = fixture.service.scan_backups(&fixture.root)?;

    assert_eq!(scan.backups().len(), 1);
    assert_eq!(
        scan.backups()
            .first()
            .map(kufeditor_mods::BackupInfo::backup_id),
        Some(valid.backup_id())
    );
    assert_eq!(scan.issues().len(), 3);
    assert!(scan.issues().iter().any(|issue| {
        matches!(
            issue.error(),
            ModError::InvalidBackup {
                kind: BackupErrorKind::InvalidMetadata,
                ..
            }
        )
    }));
    assert!(scan.issues().iter().any(|issue| {
        matches!(
            issue.error(),
            ModError::InvalidBackup {
                kind: BackupErrorKind::UnsupportedVersion,
                ..
            }
        )
    }));
    assert!(scan.issues().iter().any(|issue| {
        matches!(
            issue.error(),
            ModError::InvalidBackup {
                kind: BackupErrorKind::WrongRoot,
                ..
            }
        )
    }));
    Ok(())
}

#[test]
fn restore_overlays_backup_files_keeps_new_files_and_does_not_rewrite_installations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = BackupFixture::new("restore")?;
    write_game_file(&fixture.root_path, "existing.sox", b"backup-existing")?;
    write_game_file(&fixture.root_path, "nested/missing.sox", b"backup-missing")?;
    let backup = fixture.service.create_backup(
        CreateBackupRequest::new(&fixture.root, None)?,
        &mut RecordingProgress::default(),
    )?;
    fs::write(fixture.root_path.join("existing.sox"), b"current")?;
    fs::remove_file(fixture.root_path.join("nested/missing.sox"))?;
    write_game_file(&fixture.root_path, "after-backup.sox", b"keep")?;
    let registry_before = fixture
        .stores
        .installation_registry()
        .exists()
        .then(|| fs::read(fixture.stores.installation_registry()))
        .transpose()?;

    let report = fixture.service.restore_backup(
        RestoreBackupRequest::new(&fixture.root, backup.backup_id()),
        &mut RecordingProgress::default(),
    )?;

    assert_eq!(
        path_strings(report.committed_paths()),
        ["existing.sox", "nested/missing.sox"]
    );
    assert_eq!(
        fs::read(fixture.root_path.join("existing.sox"))?,
        b"backup-existing"
    );
    assert_eq!(
        fs::read(fixture.root_path.join("nested/missing.sox"))?,
        b"backup-missing"
    );
    assert_eq!(
        fs::read(fixture.root_path.join("after-backup.sox"))?,
        b"keep"
    );
    assert_eq!(
        fixture
            .stores
            .installation_registry()
            .exists()
            .then(|| fs::read(fixture.stores.installation_registry()))
            .transpose()?,
        registry_before
    );
    Ok(())
}

#[test]
fn delete_is_confined_to_one_valid_backup_id() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = BackupFixture::new("delete")?;
    write_game_file(&fixture.root_path, "file.sox", b"data")?;
    let first = fixture.service.create_backup(
        CreateBackupRequest::new(&fixture.root, Some("Delete".to_owned()))?,
        &mut RecordingProgress::default(),
    )?;
    let second = fixture.service.create_backup(
        CreateBackupRequest::new(&fixture.root, Some("Keep".to_owned()))?,
        &mut RecordingProgress::default(),
    )?;
    let root_store = fixture
        .stores
        .backups()
        .join(fixture.root.key().to_string());
    let incomplete = root_store.join(".backup-incomplete");
    fs::create_dir(&incomplete)?;
    fs::write(incomplete.join("sentinel"), b"keep")?;

    let other_root_path = fixture.directory.path().join("other-game");
    fs::create_dir(&other_root_path)?;
    write_game_file(&other_root_path, "other.sox", b"other")?;
    let other_root = GameRoot::inspect(Game::Heroes, other_root_path, &fixture.stores)?;
    let other = fixture.service.create_backup(
        CreateBackupRequest::new(&other_root, Some("Other root".to_owned()))?,
        &mut RecordingProgress::default(),
    )?;
    assert_backup_error(
        &fixture
            .service
            .delete_backup(&fixture.root, other.backup_id()),
        BackupErrorKind::Missing,
    );
    assert!(other.directory().is_dir());

    fixture
        .service
        .delete_backup(&fixture.root, first.backup_id())?;
    assert!(!first.directory().exists());
    assert!(second.directory().is_dir());
    assert_eq!(fs::read(incomplete.join("sentinel"))?, b"keep");
    assert_backup_error(
        &fixture
            .service
            .delete_backup(&fixture.root, first.backup_id()),
        BackupErrorKind::Missing,
    );
    assert!(BackupID::parse(&format!("{}00", second.backup_id())).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let outside = fixture.directory.path().join("outside-backup");
        fs::create_dir(&outside)?;
        fs::write(outside.join("sentinel"), b"outside")?;
        fs::remove_dir_all(second.directory())?;
        symlink(&outside, second.directory())?;
        assert_backup_error(
            &fixture
                .service
                .delete_backup(&fixture.root, second.backup_id()),
            BackupErrorKind::SymbolicLink,
        );
        assert_eq!(fs::read(outside.join("sentinel"))?, b"outside");
    }
    Ok(())
}

fn assert_backup_error<T>(result: &Result<T, ModError>, expected: BackupErrorKind) {
    assert!(matches!(
        result,
        Err(ModError::InvalidBackup { kind, .. }) if *kind == expected
    ));
}

fn assert_no_visible_backups(fixture: &BackupFixture) -> Result<(), Box<dyn std::error::Error>> {
    let root = fixture
        .stores
        .backups()
        .join(fixture.root.key().to_string());
    if root.exists() {
        assert_eq!(
            fs::read_dir(root)?
                .filter_map(Result::ok)
                .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                .count(),
            0
        );
    }
    Ok(())
}

fn path_strings(paths: &[RelativeGamePath]) -> Vec<&str> {
    paths.iter().map(RelativeGamePath::as_str).collect()
}

struct BackupFixture {
    directory: TestDirectory,
    stores: ModStorePaths,
    service: ModService,
    root_path: PathBuf,
    root: GameRoot,
}

impl BackupFixture {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = TestDirectory::new(label)?;
        let stores = ModStorePaths::new(directory.path().join("application-data"));
        let root_path = directory.path().join("game");
        fs::create_dir(&root_path)?;
        let root = GameRoot::inspect(Game::Heroes, root_path.clone(), &stores)?;
        let service = ModService::new(stores.clone());
        Ok(Self {
            directory,
            stores,
            service,
            root_path,
            root,
        })
    }
}

fn write_game_file(root: &std::path::Path, relative: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

fn copy_directory(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn set_metadata_version(path: &std::path::Path, version: u64) -> std::io::Result<()> {
    let mut metadata: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let object = metadata
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("backup metadata is not an object"))?;
    object.insert("formatVersion".to_owned(), serde_json::json!(version));
    fs::write(path, serde_json::to_vec_pretty(&metadata)?)
}
