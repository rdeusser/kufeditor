mod support;

use std::{fs, ops::ControlFlow, path::Path};

use kufeditor_game::Game;
use kufeditor_mods::{
    ApplyModRequest, GameRoot, InstallationID, InstalledFileChangeKind, ModError, ModProgress,
    ModProgressReporter, ModService, ModStorePaths, UninstallErrorKind, UninstallModRequest,
};
use support::{
    TestDirectory,
    package::{FixtureCompression, write_zip_package},
};

struct ContinueProgress;

impl ModProgressReporter for ContinueProgress {
    fn report(&mut self, _: &ModProgress) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

#[test]
fn uninstall_restores_before_images_removes_added_files_and_cleans_only_owned_empty_directories()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InstalledFixture::new(
        "success",
        &[
            ("existing.sox", b"old"),
            ("shared/unrelated.txt", b"keep-shared"),
            ("unrelated.txt", b"keep-root"),
        ],
        &[
            ("existing.sox", b"new"),
            ("created/only.sox", b"created"),
            ("shared/mod.sox", b"mod"),
        ],
    )?;
    let installation_id = fixture.installation_id;
    fs::remove_file(
        fixture
            .stores
            .packages()
            .join(format!("{}.zip", fixture.package_id)),
    )?;

    let report = fixture.service.uninstall(
        UninstallModRequest::new(&fixture.root, installation_id),
        &mut ContinueProgress,
    )?;

    assert_eq!(report.installation_id(), installation_id);
    assert_eq!(path_strings(report.restored_paths()), ["existing.sox"]);
    assert_eq!(
        path_strings(report.removed_paths()),
        ["created/only.sox", "shared/mod.sox"]
    );
    assert_eq!(fs::read(fixture.root_path.join("existing.sox"))?, b"old");
    assert!(!fixture.root_path.join("created").exists());
    assert!(!fixture.root_path.join("shared/mod.sox").exists());
    assert_eq!(
        fs::read(fixture.root_path.join("shared/unrelated.txt"))?,
        b"keep-shared"
    );
    assert_eq!(
        fs::read(fixture.root_path.join("unrelated.txt"))?,
        b"keep-root"
    );
    assert!(
        fixture
            .service
            .scan_installations(&fixture.root)?
            .installations()
            .is_empty()
    );
    assert!(!fixture.operation_directory().exists());
    Ok(())
}

#[test]
fn uninstall_reports_every_modified_or_missing_target_and_performs_no_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InstalledFixture::new(
        "changed",
        &[],
        &[
            ("a.sox", b"new-a"),
            ("b.sox", b"new-b"),
            ("c.sox", b"new-c"),
        ],
    )?;
    fs::write(fixture.root_path.join("a.sox"), b"user-change")?;
    fs::remove_file(fixture.root_path.join("b.sox"))?;
    let registry_before = fs::read(fixture.stores.installation_registry())?;

    let error = fixture
        .service
        .uninstall(
            UninstallModRequest::new(&fixture.root, fixture.installation_id),
            &mut ContinueProgress,
        )
        .expect_err("changed installed files must stop uninstall");
    let changed = error
        .changed_installed_files()
        .ok_or("missing complete changed-file report")?;

    assert_eq!(
        changed
            .files()
            .iter()
            .map(|file| (file.path().as_str(), file.kind()))
            .collect::<Vec<_>>(),
        [
            ("a.sox", InstalledFileChangeKind::Modified),
            ("b.sox", InstalledFileChangeKind::Missing),
        ]
    );
    assert_eq!(fs::read(fixture.root_path.join("a.sox"))?, b"user-change");
    assert!(!fixture.root_path.join("b.sox").exists());
    assert_eq!(fs::read(fixture.root_path.join("c.sox"))?, b"new-c");
    assert_eq!(
        fs::read(fixture.stores.installation_registry())?,
        registry_before
    );
    assert!(fixture.operation_directory().is_dir());
    assert_no_uninstall_staging(&fixture.operation_directory())?;
    Ok(())
}

#[test]
fn uninstall_rejects_missing_installations_and_the_wrong_root()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InstalledFixture::new("identity", &[], &[("file.sox", b"installed")])?;
    let other_path = fixture.directory.path().join("other-game");
    fs::create_dir(&other_path)?;
    let other_root = GameRoot::inspect(Game::Heroes, other_path, &fixture.stores)?;

    assert!(matches!(
        fixture.service.uninstall(
            UninstallModRequest::new(&fixture.root, InstallationID::from_bytes([0xee; 32])),
            &mut ContinueProgress
        ),
        Err(ModError::InvalidUninstall {
            kind: UninstallErrorKind::MissingInstallation,
            ..
        })
    ));
    assert!(matches!(
        fixture.service.uninstall(
            UninstallModRequest::new(&other_root, fixture.installation_id),
            &mut ContinueProgress
        ),
        Err(ModError::InvalidUninstall {
            kind: UninstallErrorKind::WrongRoot,
            ..
        })
    ));
    assert_eq!(fs::read(fixture.root_path.join("file.sox"))?, b"installed");
    Ok(())
}

#[test]
fn uninstall_rejects_missing_corrupt_and_unsupported_operation_images_without_writes()
-> Result<(), Box<dyn std::error::Error>> {
    for (label, mutate, expected) in [
        (
            "missing-operation",
            remove_operation_image as fn(&Path) -> std::io::Result<()>,
            UninstallErrorKind::MissingRecoveryImage,
        ),
        (
            "corrupt-operation",
            corrupt_operation_image,
            UninstallErrorKind::InvalidRecoveryImage,
        ),
        (
            "version-operation",
            change_operation_version,
            UninstallErrorKind::UnsupportedOperationVersion,
        ),
    ] {
        let fixture = InstalledFixture::new(label, &[], &[("file.sox", b"installed")])?;
        let operation_image = fixture.operation_directory().join("operation-v1.json");
        mutate(&operation_image)?;
        let registry_before = fs::read(fixture.stores.installation_registry())?;

        assert!(matches!(
            fixture.service.uninstall(
                UninstallModRequest::new(&fixture.root, fixture.installation_id),
                &mut ContinueProgress
            ),
            Err(ModError::InvalidUninstall { kind, .. }) if kind == expected
        ));
        assert_eq!(fs::read(fixture.root_path.join("file.sox"))?, b"installed");
        assert_eq!(
            fs::read(fixture.stores.installation_registry())?,
            registry_before
        );
    }
    Ok(())
}

#[test]
fn uninstall_validates_every_required_before_image_before_writing_the_game()
-> Result<(), Box<dyn std::error::Error>> {
    for (label, mutate, expected) in [
        (
            "missing-before",
            remove_operation_image as fn(&Path) -> std::io::Result<()>,
            UninstallErrorKind::MissingRecoveryImage,
        ),
        (
            "corrupt-before",
            corrupt_before_image,
            UninstallErrorKind::InvalidRecoveryImage,
        ),
    ] {
        let fixture = InstalledFixture::new(
            label,
            &[("file.sox", b"original")],
            &[("file.sox", b"installed")],
        )?;
        mutate(&fixture.operation_directory().join("before/file.sox"))?;
        let registry_before = fs::read(fixture.stores.installation_registry())?;

        assert!(matches!(
            fixture.service.uninstall(
                UninstallModRequest::new(&fixture.root, fixture.installation_id),
                &mut ContinueProgress
            ),
            Err(ModError::InvalidUninstall { kind, .. }) if kind == expected
        ));
        assert_eq!(fs::read(fixture.root_path.join("file.sox"))?, b"installed");
        assert_eq!(
            fs::read(fixture.stores.installation_registry())?,
            registry_before
        );
    }
    Ok(())
}

fn path_strings(paths: &[kufeditor_mods::RelativeGamePath]) -> Vec<&str> {
    paths
        .iter()
        .map(kufeditor_mods::RelativeGamePath::as_str)
        .collect()
}

fn assert_no_uninstall_staging(operation: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let has_staging = fs::read_dir(operation)?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("uninstall-")
        });
    assert!(!has_staging);
    Ok(())
}

fn remove_operation_image(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

fn corrupt_operation_image(path: &Path) -> std::io::Result<()> {
    fs::write(path, b"{ not JSON")
}

fn corrupt_before_image(path: &Path) -> std::io::Result<()> {
    fs::write(path, b"not the original bytes")
}

fn change_operation_version(path: &Path) -> std::io::Result<()> {
    let mut image: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let object = image
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("operation fixture is not an object"))?;
    object.insert("formatVersion".to_owned(), serde_json::json!(2));
    fs::write(path, serde_json::to_vec_pretty(&image)?)
}

struct InstalledFixture {
    directory: TestDirectory,
    stores: ModStorePaths,
    service: ModService,
    root_path: std::path::PathBuf,
    root: GameRoot,
    installation_id: InstallationID,
    operation_id: kufeditor_mods::OperationID,
    package_id: kufeditor_mods::ModPackageID,
}

impl InstalledFixture {
    fn new(
        label: &str,
        original_files: &[(&str, &[u8])],
        package_files: &[(&str, &[u8])],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = TestDirectory::new(label)?;
        let stores = ModStorePaths::new(directory.path().join("application-data"));
        let root_path = directory.path().join("game");
        fs::create_dir(&root_path)?;
        for (path, bytes) in original_files {
            write_game_file(&root_path, path, bytes)?;
        }
        let root = GameRoot::inspect(Game::Heroes, root_path.clone(), &stores)?;
        let service = ModService::new(stores.clone());
        let source = directory.path().join("fixture.zip");
        let files = package_files
            .iter()
            .map(|(path, _)| format!("\"{path}\""))
            .collect::<Vec<_>>()
            .join(",");
        let manifest = format!(
            "{{\"name\":\"Fixture\",\"version\":\"1\",\"game\":\"heroes\",\"files\":[{files}]}}"
        );
        write_zip_package(
            &source,
            manifest.as_bytes(),
            package_files,
            FixtureCompression::Stored,
        )?;
        let package = service
            .import_package(&source, &mut ContinueProgress)?
            .package()
            .package_id();
        let applied = service.apply(ApplyModRequest::new(&root, package), &mut ContinueProgress)?;
        Ok(Self {
            directory,
            stores,
            service,
            root_path,
            root,
            installation_id: applied.installation().installation_id(),
            operation_id: applied.installation().operation_id(),
            package_id: package,
        })
    }

    fn operation_directory(&self) -> std::path::PathBuf {
        self.stores.operations().join(self.operation_id.to_string())
    }
}

fn write_game_file(root: &Path, relative: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}
