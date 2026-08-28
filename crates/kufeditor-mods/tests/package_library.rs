mod support;

use std::{fs, io, ops::ControlFlow};

use kufeditor_mods::{
    ImportedModDisposition, ModError, ModLimits, ModPackageID, ModProgress, ModProgressPhase,
    ModProgressReporter, ModService, ModStorePaths, PackageErrorKind,
};
use sha2::{Digest, Sha256};
use support::{
    TestDirectory,
    package::{FixtureCompression, RawZIPEntry, write_raw_zip, write_zip_package},
};

struct RecordingProgress {
    reports: Vec<ModProgress>,
    cancel_after: Option<usize>,
}

impl RecordingProgress {
    const fn continuing() -> Self {
        Self {
            reports: Vec::new(),
            cancel_after: None,
        }
    }

    const fn cancel_after(report_count: usize) -> Self {
        Self {
            reports: Vec::new(),
            cancel_after: Some(report_count),
        }
    }
}

impl ModProgressReporter for RecordingProgress {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        self.reports.push(progress.clone());
        if self
            .cancel_after
            .is_some_and(|count| self.reports.len() >= count)
        {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[test]
fn empty_library_scan_has_no_filesystem_side_effect() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("service-construction")?;
    let application_data = directory.path().join("application-data");

    let service = ModService::new(ModStorePaths::new(application_data.clone()));

    let scan = service.scan_library()?;

    assert!(scan.packages().is_empty());
    assert!(scan.issues().is_empty());
    assert!(!application_data.exists());
    Ok(())
}

#[test]
fn import_publishes_a_content_addressed_package_and_is_neutral_when_repeated()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("import")?;
    let source = directory.path().join("knights.zip");
    write_zip_package(
        &source,
        manifest_json(
            "Knight textures",
            "1.2.0",
            "crusaders",
            &["data/knight.sox"],
        )
        .as_bytes(),
        &[("data/knight.sox", b"knight-bytes")],
        FixtureCompression::Stored,
    )?;
    let source_bytes = fs::read(&source)?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let service = ModService::new(stores.clone());
    let mut progress = RecordingProgress::continuing();

    let imported = service.import_package(&source, &mut progress)?;

    assert_eq!(imported.disposition(), ImportedModDisposition::Added);
    assert_eq!(
        imported.package().manifest().metadata().name(),
        "Knight textures"
    );
    assert_eq!(imported.package().file_count(), 1);
    assert_eq!(imported.package().uncompressed_bytes(), 12);
    assert_eq!(
        imported.package().compressed_bytes(),
        source_bytes.len() as u64
    );
    assert_eq!(
        imported.package().package_id(),
        ModPackageID::from_bytes(Sha256::digest(&source_bytes).into())
    );
    assert_eq!(
        imported.package().library_path(),
        stores
            .packages()
            .join(format!("{}.zip", imported.package().package_id()))
    );
    assert_eq!(fs::read(imported.package().library_path())?, source_bytes);
    assert!(
        progress
            .reports
            .iter()
            .any(|progress| progress.phase == ModProgressPhase::InspectingPackage)
    );
    assert!(
        progress
            .reports
            .iter()
            .any(|progress| progress.phase == ModProgressPhase::CopyingPackage)
    );
    assert!(
        progress
            .reports
            .iter()
            .any(|progress| progress.phase == ModProgressPhase::PublishingPackage)
    );

    let second = service.import_package(&source, &mut RecordingProgress::continuing())?;
    assert_eq!(second.disposition(), ImportedModDisposition::AlreadyPresent);
    assert_eq!(
        second.package().package_id(),
        imported.package().package_id()
    );
    assert_eq!(fs::read_dir(stores.packages())?.count(), 1);
    Ok(())
}

#[test]
fn import_accepts_deflated_packages() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("deflated")?;
    let source = directory.path().join("deflated.zip");
    write_zip_package(
        &source,
        manifest_json("Deflated", "1", "HeRoEs", &["data/file.sox"]).as_bytes(),
        &[("data/file.sox", &[b'x'; 4096])],
        FixtureCompression::Deflated,
    )?;
    let service = ModService::new(ModStorePaths::new(
        directory.path().join("application-data"),
    ));

    let imported = service.import_package(&source, &mut RecordingProgress::continuing())?;

    assert_eq!(imported.package().uncompressed_bytes(), 4096);
    Ok(())
}

#[test]
fn import_does_not_replace_a_content_address_collision() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("destination-collision")?;
    let source = directory.path().join("source.zip");
    let other = directory.path().join("other.zip");
    write_zip_package(
        &source,
        manifest_json("Source", "1", "heroes", &["file.sox"]).as_bytes(),
        &[("file.sox", b"source")],
        FixtureCompression::Stored,
    )?;
    write_zip_package(
        &other,
        manifest_json("Other", "1", "heroes", &["file.sox"]).as_bytes(),
        &[("file.sox", b"other")],
        FixtureCompression::Stored,
    )?;
    let service = ModService::new(ModStorePaths::new(
        directory.path().join("application-data"),
    ));
    let imported = service.import_package(&source, &mut RecordingProgress::continuing())?;
    let destination = imported.package().library_path().to_path_buf();
    fs::copy(&other, &destination)?;
    let collision_bytes = fs::read(&destination)?;

    let result = service.import_package(&source, &mut RecordingProgress::continuing());

    assert!(matches!(
        result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::DestinationCollision,
            ..
        })
    ));
    assert_eq!(fs::read(destination)?, collision_bytes);
    Ok(())
}

#[test]
fn library_scan_sorts_packages_and_retains_malformed_zip_issues()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("scan")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let service = ModService::new(stores.clone());
    for (filename, name, version) in [
        ("z.zip", "Zulu", "1"),
        ("a2.zip", "Alpha", "2"),
        ("a1.zip", "Alpha", "1"),
    ] {
        let source = directory.path().join(filename);
        write_zip_package(
            &source,
            manifest_json(name, version, "heroes", &["file.sox"]).as_bytes(),
            &[("file.sox", name.as_bytes())],
            FixtureCompression::Stored,
        )?;
        service.import_package(&source, &mut RecordingProgress::continuing())?;
    }
    let malformed = stores.packages().join(format!("{}.zip", "f".repeat(64)));
    fs::write(&malformed, b"not a zip")?;
    fs::write(stores.packages().join("notes.txt"), b"ignore me")?;

    let scan = service.scan_library()?;

    assert_eq!(
        scan.packages()
            .iter()
            .map(|package| (
                package.manifest().metadata().name(),
                package.manifest().metadata().version()
            ))
            .collect::<Vec<_>>(),
        [("Alpha", "1"), ("Alpha", "2"), ("Zulu", "1")]
    );
    assert_eq!(scan.issues().len(), 1);
    let issue = scan
        .issues()
        .first()
        .ok_or_else(|| io::Error::other("missing malformed-package issue"))?;
    assert_eq!(issue.path(), malformed);
    assert!(issue.error().to_string().contains("ZIP"));
    Ok(())
}

#[test]
fn canceled_import_leaves_the_library_absent() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("cancel")?;
    let source = directory.path().join("source.zip");
    write_zip_package(
        &source,
        manifest_json("Canceled", "1", "heroes", &["file.sox"]).as_bytes(),
        &[("file.sox", b"data")],
        FixtureCompression::Stored,
    )?;
    let application_data = directory.path().join("application-data");
    let service = ModService::new(ModStorePaths::new(application_data.clone()));

    let result = service.import_package(&source, &mut RecordingProgress::cancel_after(1));

    assert!(matches!(result, Err(ModError::Canceled { .. })));
    assert!(!application_data.exists());
    Ok(())
}

#[test]
fn package_validation_rejects_unsafe_entries_and_payload_mismatches()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("unsafe")?;
    let manifest = manifest_json("Unsafe", "1", "heroes", &["data/file.sox"]);
    let fixtures = [
        (
            "missing-manifest.zip",
            vec![RawZIPEntry::file(b"data/file.sox", b"data")],
        ),
        (
            "duplicate-manifest.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/file.sox", b"data"),
            ],
        ),
        (
            "non-utf8.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/\xff.sox", b"data"),
            ],
        ),
        (
            "encrypted.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/file.sox", b"data").with_flags(1),
            ],
        ),
        (
            "symlink.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/file.sox", b"target").with_unix_mode(0o120_777),
            ],
        ),
        (
            "fifo.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/file.sox", b"data").with_unix_mode(0o010_644),
            ],
        ),
        (
            "unsupported-compression.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/file.sox", b"data").with_compression(12),
            ],
        ),
        (
            "traversal.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"../file.sox", b"data"),
            ],
        ),
        (
            "portable-collision.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"Data/File.sox", b"first"),
                RawZIPEntry::file(b"data/file.SOX", b"second"),
            ],
        ),
        (
            "duplicate-payload.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/file.sox", b"first"),
                RawZIPEntry::file(b"data/file.sox", b"second"),
            ],
        ),
        (
            "payload-mismatch.zip",
            vec![
                RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
                RawZIPEntry::file(b"data/other.sox", b"data"),
            ],
        ),
    ];
    let service = ModService::new(ModStorePaths::new(
        directory.path().join("application-data"),
    ));

    for (filename, entries) in fixtures {
        let source = directory.path().join(filename);
        write_raw_zip(&source, &entries)?;
        assert!(
            service
                .import_package(&source, &mut RecordingProgress::continuing())
                .is_err(),
            "accepted unsafe fixture {filename}"
        );
    }
    Ok(())
}

#[test]
fn traversal_entries_are_reported_as_unsafe_paths() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("unsafe-entry-path")?;
    let source = directory.path().join("traversal.zip");
    write_raw_zip(
        &source,
        &[
            RawZIPEntry::file(
                b"mod.json",
                manifest_json("Unsafe", "1", "heroes", &["file.sox"]).as_bytes(),
            ),
            RawZIPEntry::file(b"../file.sox", b"data"),
        ],
    )?;
    let service = ModService::new(ModStorePaths::new(
        directory.path().join("application-data"),
    ));

    let result = service.import_package(&source, &mut RecordingProgress::continuing());

    assert!(matches!(
        result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::UnsafeEntryPath,
            ..
        })
    ));
    Ok(())
}

#[test]
fn nested_manifests_are_rejected_even_when_listed_as_payloads()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("nested-manifest")?;
    let source = directory.path().join("nested-manifest.zip");
    write_raw_zip(
        &source,
        &[
            RawZIPEntry::file(
                b"mod.json",
                manifest_json("Nested", "1", "heroes", &["nested/mod.json"]).as_bytes(),
            ),
            RawZIPEntry::file(b"nested/mod.json", b"{}"),
        ],
    )?;
    let service = ModService::new(ModStorePaths::new(
        directory.path().join("application-data"),
    ));

    let result = service.import_package(&source, &mut RecordingProgress::continuing());

    assert!(matches!(
        result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::NestedManifest,
            ..
        })
    ));
    Ok(())
}

#[test]
fn package_validation_enforces_declared_and_actual_resource_limits()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("limits")?;
    let manifest = manifest_json("Limits", "1", "heroes", &["one", "two"]);
    let source = directory.path().join("limits.zip");
    write_raw_zip(
        &source,
        &[
            RawZIPEntry::file(b"mod.json", manifest.as_bytes()),
            RawZIPEntry::file(b"one", b"1234"),
            RawZIPEntry::file(b"two", b"5678"),
        ],
    )?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));

    for limits in [
        ModLimits {
            max_zip_bytes: fs::metadata(&source)?.len() - 1,
            ..ModLimits::default()
        },
        ModLimits {
            max_package_files: 1,
            ..ModLimits::default()
        },
        ModLimits {
            max_file_bytes: 3,
            ..ModLimits::default()
        },
        ModLimits {
            max_uncompressed_bytes: 7,
            ..ModLimits::default()
        },
    ] {
        let service = ModService::with_limits(stores.clone(), limits);
        assert!(
            service
                .import_package(&source, &mut RecordingProgress::continuing())
                .is_err()
        );
    }

    let actual_over_declared = directory.path().join("actual-over-declared.zip");
    let one_file_manifest = manifest_json("Actual", "1", "heroes", &["one"]);
    write_raw_zip(
        &actual_over_declared,
        &[
            RawZIPEntry::file(b"mod.json", one_file_manifest.as_bytes()),
            RawZIPEntry::file(b"one", b"1234").with_declared_uncompressed_bytes(1),
        ],
    )?;
    let service = ModService::with_limits(
        stores,
        ModLimits {
            max_file_bytes: 2,
            ..ModLimits::default()
        },
    );
    assert!(
        service
            .import_package(&actual_over_declared, &mut RecordingProgress::continuing())
            .is_err()
    );
    Ok(())
}

#[test]
fn a_valid_zip_with_a_non_content_addressed_library_name_is_an_issue()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("library-name")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    fs::create_dir_all(stores.packages())?;
    let source = stores.packages().join("friendly-name.zip");
    write_zip_package(
        &source,
        manifest_json("Friendly", "1", "heroes", &["file.sox"]).as_bytes(),
        &[("file.sox", b"data")],
        FixtureCompression::Stored,
    )?;
    let service = ModService::new(stores);

    let scan = service.scan_library()?;

    assert!(scan.packages().is_empty());
    assert_eq!(scan.issues().len(), 1);
    let issue = scan
        .issues()
        .first()
        .ok_or_else(|| io::Error::other("missing content-address issue"))?;
    assert!(matches!(
        issue.error(),
        ModError::InvalidPackage {
            kind: PackageErrorKind::UnexpectedLibraryName,
            ..
        }
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn library_scan_rejects_a_broken_package_directory_symlink()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("broken-library-symlink")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let mods = stores.root();
    fs::create_dir_all(mods)?;
    symlink(directory.path().join("missing"), stores.packages())?;
    let service = ModService::new(stores);

    let result = service.scan_library();

    assert!(matches!(
        result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::SymbolicLink,
            ..
        })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn import_rejects_a_symlinked_mod_store_before_writing_outside_application_data()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("symlinked-mod-store")?;
    let source = directory.path().join("source.zip");
    write_zip_package(
        &source,
        manifest_json("Redirected", "1", "heroes", &["file.sox"]).as_bytes(),
        &[("file.sox", b"data")],
        FixtureCompression::Stored,
    )?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let outside = directory.path().join("outside");
    fs::create_dir_all(stores.application_data())?;
    fs::create_dir_all(&outside)?;
    symlink(&outside, stores.root())?;
    let service = ModService::new(stores);

    let result = service.import_package(&source, &mut RecordingProgress::continuing());

    assert!(matches!(
        result,
        Err(ModError::InvalidPackage {
            kind: PackageErrorKind::SymbolicLink,
            ..
        })
    ));
    assert!(!outside.join("packages").exists());
    Ok(())
}

fn manifest_json(name: &str, version: &str, game: &str, files: &[&str]) -> String {
    let files = files
        .iter()
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"name\":\"{name}\",\"version\":\"{version}\",\"game\":\"{game}\",\"files\":[{files}]}}"
    )
}
