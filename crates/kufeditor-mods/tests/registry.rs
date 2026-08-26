mod support;

use std::{fs, ops::ControlFlow, path::Path};

use kufeditor_game::Game;
use kufeditor_mods::{
    FileSHA256, GameRoot, InstallationIssue, InstallationIssueKind, InstalledMod,
    InstalledModStatus, ModError, ModPackageID, ModProgress, ModProgressReporter, ModService,
    ModStorePaths, PackageErrorKind,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
fn missing_registry_is_empty_and_valid_installations_are_sorted_with_health()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("registry-health")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    fs::write(root_path.join("clean.sox"), b"clean")?;
    fs::write(root_path.join("modified.sox"), b"modified-now")?;
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let other_root_path = directory.path().join("other-game");
    fs::create_dir_all(&other_root_path)?;
    fs::write(other_root_path.join("other.sox"), b"other")?;
    let other_root = GameRoot::inspect(Game::Heroes, other_root_path, &stores)?;
    let service = ModService::new(stores.clone());

    let missing = service.scan_installations(&root)?;
    assert!(missing.installations().is_empty());
    assert!(missing.issues().is_empty());
    assert!(!stores.application_data().exists());

    write_registry(
        &stores,
        &[
            installation(
                &root,
                '3',
                0xcc,
                "Zulu clean",
                "2",
                "clean.sox",
                &digest_hex(b"clean"),
            ),
            installation(
                &other_root,
                '4',
                0xdd,
                "Other root",
                "1",
                "other.sox",
                &digest_hex(b"other"),
            ),
            installation(
                &root,
                '2',
                0xbb,
                "Beta missing",
                "1",
                "missing.sox",
                &digest_hex(b"missing"),
            ),
            installation(
                &root,
                '1',
                0xaa,
                "Alpha modified",
                "3",
                "modified.sox",
                &digest_hex(b"installed-before"),
            ),
        ],
    )?;

    let scan = service.scan_installations(&root)?;

    assert_eq!(
        scan.installations()
            .iter()
            .map(|installation| (
                installation.metadata().name(),
                installation.metadata().version(),
                installation.status()
            ))
            .collect::<Vec<_>>(),
        [
            ("Alpha modified", "3", Some(InstalledModStatus::Modified)),
            ("Beta missing", "1", Some(InstalledModStatus::Missing)),
            ("Zulu clean", "2", Some(InstalledModStatus::Clean)),
        ]
    );
    assert!(scan.issues().is_empty());
    Ok(())
}

#[test]
fn registry_scan_retains_duplicate_conflict_and_invalid_record_issues()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("registry-conflicts")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    for path in ["a.sox", "b.sox", "c.sox"] {
        fs::write(root_path.join(path), path.as_bytes())?;
    }
    let root = GameRoot::inspect(Game::Crusaders, root_path, &stores)?;
    let invalid_path = installation(
        &root,
        '5',
        0xee,
        "Invalid path",
        "1",
        "../escape.sox",
        &digest_hex(b"a.sox"),
    );
    write_registry(
        &stores,
        &[
            installation(
                &root,
                '1',
                0xaa,
                "Alpha",
                "1",
                "a.sox",
                &digest_hex(b"a.sox"),
            ),
            installation(
                &root,
                '2',
                0xbb,
                "Alpha",
                "2",
                "b.sox",
                &digest_hex(b"b.sox"),
            ),
            installation(
                &root,
                '1',
                0xcc,
                "Gamma",
                "1",
                "c.sox",
                &digest_hex(b"c.sox"),
            ),
            installation(
                &root,
                '4',
                0xdd,
                "Delta",
                "1",
                "A.SOX",
                &digest_hex(b"a.sox"),
            ),
            invalid_path,
        ],
    )?;

    let scan = ModService::new(stores).scan_installations(&root)?;

    let kinds = scan
        .issues()
        .iter()
        .map(InstallationIssue::kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&InstallationIssueKind::DuplicateName));
    assert!(kinds.contains(&InstallationIssueKind::DuplicateInstallationID));
    assert!(kinds.contains(&InstallationIssueKind::PathConflict));
    assert!(kinds.contains(&InstallationIssueKind::InvalidRecord));
    assert_eq!(scan.installations().len(), 4);
    Ok(())
}

#[test]
fn malformed_and_unsupported_registries_remain_byte_identical_after_scan_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("registry-read-errors")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    fs::create_dir_all(stores.root())?;
    let service = ModService::new(stores.clone());

    for bytes in [
        b"{ not JSON".as_slice(),
        br#"{"formatVersion":2,"installations":[]}"#.as_slice(),
    ] {
        fs::write(stores.installation_registry(), bytes)?;
        assert!(service.scan_installations(&root).is_err());
        assert_eq!(fs::read(stores.installation_registry())?, bytes);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn unreadable_health_target_retains_the_installation_and_reports_an_issue()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let directory = TestDirectory::new("registry-unreadable-health")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    let target = root_path.join("unreadable.sox");
    fs::write(&target, b"content")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o000))?;
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    write_registry(
        &stores,
        &[installation(
            &root,
            '1',
            0xaa,
            "Unreadable",
            "1",
            "unreadable.sox",
            &digest_hex(b"content"),
        )],
    )?;

    let scan = ModService::new(stores).scan_installations(&root)?;

    fs::set_permissions(&target, fs::Permissions::from_mode(0o600))?;
    assert_eq!(scan.installations().len(), 1);
    assert_eq!(
        scan.installations().first().and_then(InstalledMod::status),
        None
    );
    assert_eq!(scan.issues().len(), 1);
    assert_eq!(
        scan.issues().first().map(InstallationIssue::kind),
        Some(InstallationIssueKind::Health)
    );
    Ok(())
}

#[test]
fn package_removal_rejects_references_removes_only_the_requested_file_and_preserves_registry()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("registry-package-removal")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    fs::write(root_path.join("file.sox"), b"installed")?;
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let service = ModService::new(stores.clone());
    let referenced = import_package(&service, directory.path(), "referenced", b"one")?;
    let removable = import_package(&service, directory.path(), "removable", b"two")?;
    write_registry(
        &stores,
        &[installation_with_package(
            &root,
            '1',
            referenced,
            "Referenced",
            "1",
            "file.sox",
            &digest_hex(b"installed"),
        )],
    )?;
    let registry_bytes = fs::read(stores.installation_registry())?;

    let referenced_result = service.remove_package(referenced);
    assert!(matches!(
        referenced_result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::ReferencedPackage,
            ..
        })
    ));
    assert!(
        stores
            .packages()
            .join(format!("{referenced}.zip"))
            .is_file()
    );

    service.remove_package(removable)?;
    assert!(!stores.packages().join(format!("{removable}.zip")).exists());
    let missing_result = service.remove_package(removable);
    assert!(matches!(
        missing_result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::MissingLibraryPackage,
            ..
        })
    ));
    assert_eq!(fs::read(stores.installation_registry())?, registry_bytes);
    Ok(())
}

fn write_registry(stores: &ModStorePaths, installations: &[Value]) -> std::io::Result<()> {
    fs::create_dir_all(stores.root())?;
    let mut bytes = serde_json::to_vec_pretty(&json!({
        "formatVersion": 1,
        "installations": installations,
    }))?;
    bytes.push(b'\n');
    fs::write(stores.installation_registry(), bytes)
}

fn installation(
    root: &GameRoot,
    installation_digit: char,
    package_byte: u8,
    name: &str,
    version: &str,
    path: &str,
    installed_sha256: &str,
) -> Value {
    installation_with_package(
        root,
        installation_digit,
        ModPackageID::from_bytes([package_byte; 32]),
        name,
        version,
        path,
        installed_sha256,
    )
}

fn installation_with_package(
    root: &GameRoot,
    installation_digit: char,
    package_id: ModPackageID,
    name: &str,
    version: &str,
    path: &str,
    installed_sha256: &str,
) -> Value {
    json!({
        "installationID": installation_digit.to_string().repeat(64),
        "packageID": package_id.to_string(),
        "name": name,
        "version": version,
        "game": match root.game() {
            Game::Crusaders => "crusaders",
            Game::Heroes => "heroes",
        },
        "configuredRoot": root.configured_path().to_string_lossy(),
        "canonicalRoot": root.canonical_path().to_string_lossy(),
        "rootKey": root.key().to_string(),
        "installedAt": "2026-08-26T12:00:00Z",
        "operationID": "f".repeat(64),
        "files": [{
            "path": path,
            "installedSHA256": installed_sha256,
            "originalExisted": true,
        }],
    })
}

fn import_package(
    service: &ModService,
    directory: &Path,
    name: &str,
    payload: &[u8],
) -> Result<ModPackageID, Box<dyn std::error::Error>> {
    let source = directory.join(format!("{name}.zip"));
    let manifest = format!(
        "{{\"name\":\"{name}\",\"version\":\"1\",\"game\":\"heroes\",\"files\":[\"file.sox\"]}}"
    );
    write_zip_package(
        &source,
        manifest.as_bytes(),
        &[("file.sox", payload)],
        FixtureCompression::Stored,
    )?;
    Ok(service
        .import_package(&source, &mut ContinueProgress)?
        .package()
        .package_id())
}

fn digest_hex(bytes: &[u8]) -> String {
    FileSHA256::from_bytes(Sha256::digest(bytes).into()).to_string()
}
