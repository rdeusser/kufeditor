mod support;

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use kufeditor_game::Game;
use kufeditor_mods::{
    CreateModRequest, GameRoot, ManifestErrorKind, ModError, ModLimits, ModMetadata, ModProgress,
    ModProgressPhase, ModProgressReporter, ModService, ModStorePaths, RelativeGamePath,
    SourceFileErrorKind,
};
use support::TestDirectory;
use zip::{DateTime, ZipArchive};

struct ContinueProgress;

impl ModProgressReporter for ContinueProgress {
    fn report(&mut self, _: &ModProgress) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}

struct CancelCreation;

impl ModProgressReporter for CancelCreation {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        if progress.phase == ModProgressPhase::CreatingPackage {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

struct GrowSource {
    source: PathBuf,
    changed: bool,
    error: Option<std::io::Error>,
}

impl ModProgressReporter for GrowSource {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        if !self.changed && progress.phase == ModProgressPhase::CreatingPackage {
            self.changed = true;
            self.error = OpenOptions::new()
                .append(true)
                .open(&self.source)
                .and_then(|mut file| file.write_all(b"grew while streaming"))
                .err();
        }
        ControlFlow::Continue(())
    }
}

#[cfg(unix)]
struct ReplaceSource {
    source: PathBuf,
    replacement: PathBuf,
    changed: bool,
    error: Option<std::io::Error>,
}

#[cfg(unix)]
impl ModProgressReporter for ReplaceSource {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        if !self.changed && progress.phase == ModProgressPhase::CreatingPackage {
            self.changed = true;
            self.error = fs::rename(&self.replacement, &self.source).err();
        }
        ControlFlow::Continue(())
    }
}

#[test]
fn creation_is_byte_deterministic_and_uses_portable_entry_order()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("deterministic-creation")?;
    let first_root_path = directory.path().join("first-game");
    let second_root_path = directory.path().join("second-game");
    write_game_files(&first_root_path)?;
    write_game_files(&second_root_path)?;
    let first_stores = ModStorePaths::new(directory.path().join("first-application-data"));
    let second_stores = ModStorePaths::new(directory.path().join("second-application-data"));
    let first_root = GameRoot::inspect(Game::Heroes, first_root_path, &first_stores)?;
    let second_root = GameRoot::inspect(Game::Heroes, second_root_path, &second_stores)?;
    let first_output = directory.path().join("first.zip");
    let second_output = directory.path().join("second.zip");
    fs::write(&first_output, b"replace this stale output")?;
    let files = selected_paths()?;
    let service = ModService::new(first_stores.clone());

    let first = service.create_package(
        CreateModRequest::new(metadata()?, &first_root, files.clone(), &first_output)?,
        &mut ContinueProgress,
    )?;
    let second = service.create_package(
        CreateModRequest::new(metadata()?, &second_root, files, &second_output)?,
        &mut ContinueProgress,
    )?;

    assert_eq!(fs::read(&first_output)?, fs::read(&second_output)?);
    assert_eq!(first.package_id(), second.package_id());
    assert_eq!(first.output_path(), first_output);
    assert_eq!(first.manifest(), second.manifest());
    assert_eq!(first.file_count(), 2);
    assert_eq!(first.uncompressed_bytes(), 10);
    assert_eq!(first.compressed_bytes(), fs::metadata(&first_output)?.len());
    assert!(!first_stores.application_data().exists());

    let mut archive = ZipArchive::new(File::open(&first_output)?)?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        names.push(entry.name().to_owned());
        assert_eq!(entry.last_modified(), Some(DateTime::DEFAULT));
        assert_eq!(entry.unix_mode().map(|mode| mode & 0o777), Some(0o644));
    }
    assert_eq!(names, ["mod.json", "Data/alpha.sox", "zeta.sox"]);
    let mut manifest_bytes = Vec::new();
    archive.by_index(0)?.read_to_end(&mut manifest_bytes)?;
    assert_eq!(manifest_bytes, first.manifest().to_json()?);
    Ok(())
}

#[test]
fn request_rejects_missing_directory_and_duplicate_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("invalid-creation-sources")?;
    let root_path = directory.path().join("game");
    fs::create_dir_all(root_path.join("directory"))?;
    fs::write(root_path.join("file.sox"), b"data")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root = GameRoot::inspect(Game::Crusaders, root_path, &stores)?;
    let output = directory.path().join("output.zip");

    for path in ["missing.sox", "directory"] {
        assert!(
            CreateModRequest::new(metadata()?, &root, vec![relative(path)?], &output,).is_err(),
            "accepted invalid source {path}"
        );
    }
    let duplicate = CreateModRequest::new(
        metadata()?,
        &root,
        vec![relative("file.sox")?, relative("FILE.SOX")?],
        &output,
    );
    assert!(matches!(
        duplicate,
        Err(ModError::InvalidManifest {
            kind: ManifestErrorKind::DuplicatePath
        })
    ));

    let source_output = root.canonical_path().join("file.sox");
    let output_collision = CreateModRequest::new(
        metadata()?,
        &root,
        vec![relative("file.sox")?],
        &source_output,
    );
    assert!(matches!(
        output_collision,
        Err(ModError::InvalidSourceFile {
            kind: SourceFileErrorKind::OutputCollision,
            ..
        })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn request_rejects_a_symbolic_link_source() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("creation-source-symlink")?;
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    fs::write(root_path.join("target.sox"), b"data")?;
    symlink("target.sox", root_path.join("link.sox"))?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let output = directory.path().join("output.zip");

    let result = CreateModRequest::new(metadata()?, &root, vec![relative("link.sox")?], &output);

    assert!(matches!(
        result,
        Err(ModError::InvalidSourceFile {
            kind: SourceFileErrorKind::SymbolicLink,
            ..
        })
    ));
    Ok(())
}

#[test]
fn cancellation_and_parent_failure_preserve_the_requested_output()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("creation-output-preservation")?;
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    fs::write(root_path.join("file.sox"), vec![b'x'; 128 * 1024])?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let service = ModService::new(stores);
    let output = directory.path().join("existing.zip");
    fs::write(&output, b"keep this output")?;

    let canceled = service.create_package(
        CreateModRequest::new(metadata()?, &root, vec![relative("file.sox")?], &output)?,
        &mut CancelCreation,
    );

    assert!(matches!(canceled, Err(ModError::Canceled { .. })));
    assert_eq!(fs::read(&output)?, b"keep this output");

    let missing_parent = directory.path().join("missing").join("output.zip");
    let parent_failure = service.create_package(
        CreateModRequest::new(
            metadata()?,
            &root,
            vec![relative("file.sox")?],
            &missing_parent,
        )?,
        &mut ContinueProgress,
    );
    assert!(parent_failure.is_err());
    assert!(!directory.path().join("missing").exists());
    Ok(())
}

#[test]
fn a_changed_source_component_is_rejected_before_output_creation()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("changed-source-component")?;
    let root_path = directory.path().join("game");
    let source_directory = root_path.join("data");
    fs::create_dir_all(&source_directory)?;
    fs::write(source_directory.join("file.sox"), b"original")?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let output = directory.path().join("output.zip");
    let request = CreateModRequest::new(
        metadata()?,
        &root,
        vec![relative("data/file.sox")?],
        &output,
    )?;
    fs::rename(&source_directory, root.canonical_path().join("old-data"))?;
    fs::create_dir(&source_directory)?;
    fs::write(source_directory.join("file.sox"), b"original")?;

    let result = ModService::new(stores).create_package(request, &mut ContinueProgress);

    assert!(matches!(
        result,
        Err(ModError::InvalidSourceFile {
            kind: SourceFileErrorKind::Changed,
            ..
        })
    ));
    assert!(!output.exists());
    Ok(())
}

#[test]
fn source_growth_during_streaming_is_rejected_without_replacing_output()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("source-growth")?;
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    let source = root_path.join("file.sox");
    fs::write(&source, vec![b'x'; 128 * 1024])?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let output = directory.path().join("output.zip");
    fs::write(&output, b"original output")?;
    let request = CreateModRequest::new(metadata()?, &root, vec![relative("file.sox")?], &output)?;
    let mut progress = GrowSource {
        source,
        changed: false,
        error: None,
    };

    let result = ModService::new(stores).create_package(request, &mut progress);

    assert!(progress.changed);
    assert!(progress.error.is_none());
    assert!(matches!(
        result,
        Err(ModError::InvalidSourceFile {
            kind: SourceFileErrorKind::Changed,
            ..
        })
    ));
    assert_eq!(fs::read(output)?, b"original output");
    Ok(())
}

#[cfg(unix)]
#[test]
fn source_replacement_during_streaming_is_rejected_without_publishing()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("source-replacement")?;
    let root_path = directory.path().join("game");
    fs::create_dir_all(&root_path)?;
    let source = root_path.join("file.sox");
    let replacement = root_path.join("replacement.sox");
    fs::write(&source, vec![b'x'; 128 * 1024])?;
    fs::write(&replacement, vec![b'y'; 128 * 1024])?;
    let stores = ModStorePaths::new(directory.path().join("application-data"));
    let root = GameRoot::inspect(Game::Heroes, root_path, &stores)?;
    let output = directory.path().join("output.zip");
    let request = CreateModRequest::new(metadata()?, &root, vec![relative("file.sox")?], &output)?;
    let mut progress = ReplaceSource {
        source,
        replacement,
        changed: false,
        error: None,
    };

    let result = ModService::new(stores).create_package(request, &mut progress);

    assert!(progress.changed);
    assert!(progress.error.is_none());
    assert!(matches!(
        result,
        Err(ModError::InvalidSourceFile {
            kind: SourceFileErrorKind::Changed,
            ..
        })
    ));
    assert!(!output.exists());
    Ok(())
}

fn write_game_files(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("Data"))?;
    fs::write(root.join("Data/alpha.sox"), b"alpha")?;
    fs::write(root.join("zeta.sox"), b"zebra")
}

fn selected_paths() -> Result<Vec<RelativeGamePath>, ModError> {
    Ok(vec![relative("zeta.sox")?, relative("Data/alpha.sox")?])
}

fn relative(value: &str) -> Result<RelativeGamePath, ModError> {
    RelativeGamePath::parse(value, &ModLimits::default())
}

fn metadata() -> Result<ModMetadata, ModError> {
    ModMetadata::new(
        "Deterministic",
        "1.0.0",
        Some("KUF Editor".to_owned()),
        Some("A deterministic package".to_owned()),
        None,
    )
}
