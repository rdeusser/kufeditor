use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{
    ModError, ModPackageID, ModPackageInfo, ModProgress, ModProgressPhase, ModProgressReporter,
    ModService, ModStorePaths, PackageErrorKind,
    package::{hash_package_image, inspect_package},
    progress::ContinueProgress,
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedModDisposition {
    Added,
    AlreadyPresent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedMod {
    package: ModPackageInfo,
    disposition: ImportedModDisposition,
}

impl ImportedMod {
    pub const fn package(&self) -> &ModPackageInfo {
        &self.package
    }

    pub const fn disposition(&self) -> ImportedModDisposition {
        self.disposition
    }
}

#[derive(Debug)]
pub struct ModLibraryIssue {
    path: PathBuf,
    error: ModError,
}

impl ModLibraryIssue {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn error(&self) -> &ModError {
        &self.error
    }
}

#[derive(Debug, Default)]
pub struct ModLibraryScan {
    packages: Vec<ModPackageInfo>,
    issues: Vec<ModLibraryIssue>,
}

impl ModLibraryScan {
    pub fn packages(&self) -> &[ModPackageInfo] {
        &self.packages
    }

    pub fn issues(&self) -> &[ModLibraryIssue] {
        &self.issues
    }
}

impl ModService {
    pub fn scan_library(&self) -> Result<ModLibraryScan, ModError> {
        let Some(packages_path) = existing_package_directory(&self.paths)? else {
            return Ok(ModLibraryScan::default());
        };
        let mut paths = fs::read_dir(&packages_path)
            .map_err(|error| ModError::io("scan package library", &packages_path, error))?
            .map(|entry| {
                entry.map(|entry| entry.path()).map_err(|error| {
                    ModError::io("read package library entry", &packages_path, error)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();

        let mut scan = ModLibraryScan::default();
        for path in paths {
            if path.extension().and_then(|extension| extension.to_str()) != Some("zip") {
                continue;
            }
            match inspect_package(&path, &self.limits, &mut ContinueProgress) {
                Ok(package) if has_content_addressed_name(&path, package.package_id()) => {
                    scan.packages.push(package);
                }
                Ok(_) => scan.issues.push(ModLibraryIssue {
                    error: ModError::package(&path, None, PackageErrorKind::UnexpectedLibraryName),
                    path,
                }),
                Err(error) => scan.issues.push(ModLibraryIssue { path, error }),
            }
        }
        scan.packages.sort_by(|left, right| {
            left.manifest()
                .metadata()
                .name()
                .cmp(right.manifest().metadata().name())
                .then_with(|| {
                    left.manifest()
                        .metadata()
                        .version()
                        .cmp(right.manifest().metadata().version())
                })
                .then_with(|| left.package_id().cmp(&right.package_id()))
        });
        scan.issues
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(scan)
    }

    pub fn import_package(
        &self,
        source: &Path,
        progress: &mut impl ModProgressReporter,
    ) -> Result<ImportedMod, ModError> {
        self.import_package_with_hook(source, progress, || Ok(()))
    }

    fn import_package_with_hook(
        &self,
        source: &Path,
        progress: &mut impl ModProgressReporter,
        after_inspection: impl FnOnce() -> Result<(), ModError>,
    ) -> Result<ImportedMod, ModError> {
        let source_package = inspect_package(source, &self.limits, progress)?;
        after_inspection()?;
        let packages_path = prepare_package_directory(&self.paths)?;

        let mut temporary = NamedTempFile::new_in(&packages_path)
            .map_err(|error| ModError::io("create temporary package", &packages_path, error))?;
        let copied_id = copy_package(source, &mut temporary, &self.limits, progress)?;
        if copied_id != source_package.package_id() {
            return Err(ModError::package(
                source,
                None,
                PackageErrorKind::SourceChanged,
            ));
        }
        temporary
            .as_file_mut()
            .flush()
            .map_err(|error| ModError::io("flush temporary package", temporary.path(), error))?;
        temporary.as_file().sync_all().map_err(|error| {
            ModError::io("synchronize temporary package", temporary.path(), error)
        })?;
        let copied_package = inspect_package(temporary.path(), &self.limits, progress)?;
        if !source_package.same_content(&copied_package) {
            return Err(ModError::package(
                source,
                None,
                PackageErrorKind::SourceChanged,
            ));
        }

        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::PublishingPackage,
                completed: 0,
                total: 1,
                path: None,
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "package import",
            });
        }

        let destination = packages_path.join(format!("{copied_id}.zip"));
        if destination.exists() {
            let package =
                validate_existing_destination(&destination, &copied_package, &self.limits)?;
            return Ok(ImportedMod {
                package,
                disposition: ImportedModDisposition::AlreadyPresent,
            });
        }

        match temporary.persist_noclobber(&destination) {
            Ok(file) => {
                drop(file);
                Ok(ImportedMod {
                    package: copied_package.at_path(destination),
                    disposition: ImportedModDisposition::Added,
                })
            }
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                let package =
                    validate_existing_destination(&destination, &copied_package, &self.limits)?;
                Ok(ImportedMod {
                    package,
                    disposition: ImportedModDisposition::AlreadyPresent,
                })
            }
            Err(error) => Err(ModError::io(
                "save imported package",
                destination,
                error.error,
            )),
        }
    }
}

pub(crate) fn existing_package_directory(
    paths: &ModStorePaths,
) -> Result<Option<PathBuf>, ModError> {
    if existing_mod_store_root(paths)?.is_none() {
        return Ok(None);
    }
    let packages = paths.packages();
    validate_existing_directory(&packages).map(|exists| exists.then_some(packages))
}

fn prepare_package_directory(paths: &ModStorePaths) -> Result<PathBuf, ModError> {
    prepare_mod_store_root(paths)?;

    let packages = paths.packages();
    if !validate_existing_directory(&packages)? {
        fs::create_dir(&packages)
            .map_err(|error| ModError::io("create package library", &packages, error))?;
        require_directory(&packages)?;
    }
    Ok(packages)
}

pub(crate) fn existing_mod_store_root(paths: &ModStorePaths) -> Result<Option<PathBuf>, ModError> {
    let root = paths.root();
    validate_existing_directory(root).map(|exists| exists.then(|| root.to_path_buf()))
}

pub(crate) fn prepare_mod_store_root(paths: &ModStorePaths) -> Result<PathBuf, ModError> {
    let root = paths.root();
    if !validate_existing_directory(root)? {
        fs::create_dir_all(root).map_err(|error| ModError::io("create mod store", root, error))?;
        require_directory(root)?;
    }
    Ok(root.to_path_buf())
}

fn validate_existing_directory(path: &Path) -> Result<bool, ModError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_directory_metadata(path, &metadata)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ModError::io("inspect mod-store directory", path, error)),
    }
}

fn require_directory(path: &Path) -> Result<(), ModError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("inspect created mod-store directory", path, error))?;
    validate_directory_metadata(path, &metadata)
}

fn validate_directory_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), ModError> {
    if metadata.file_type().is_symlink() {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::SymbolicLink,
        ));
    }
    if !metadata.is_dir() {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::NotDirectory,
        ));
    }
    Ok(())
}

fn has_content_addressed_name(path: &Path, package_id: ModPackageID) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(&format!("{package_id}.zip"))
}

fn copy_package(
    source: &Path,
    destination: &mut NamedTempFile,
    limits: &crate::ModLimits,
    progress: &mut impl ModProgressReporter,
) -> Result<ModPackageID, ModError> {
    let (expected_id, total) = hash_package_image(source, limits)?;
    let mut input = File::open(source)
        .map_err(|error| ModError::io("open package for import", source, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES].into_boxed_slice();
    let mut completed = 0u64;
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| ModError::io("read package for import", source, error))?;
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).ok_or_else(|| {
            ModError::io(
                "validate package-copy read length",
                source,
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reader returned more bytes than the supplied buffer",
                ),
            )
        })?;
        destination
            .write_all(bytes)
            .map_err(|error| ModError::io("write temporary package", destination.path(), error))?;
        hasher.update(bytes);
        completed = completed
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| ModError::package(source, None, PackageErrorKind::ZIPTooLarge))?;
        if completed > limits.max_zip_bytes {
            return Err(ModError::package(
                source,
                None,
                PackageErrorKind::ZIPTooLarge,
            ));
        }
        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::CopyingPackage,
                completed,
                total,
                path: None,
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "package import",
            });
        }
    }
    let copied_id = ModPackageID::from_bytes(hasher.finalize().into());
    if completed != total || copied_id != expected_id {
        return Err(ModError::package(
            source,
            None,
            PackageErrorKind::SourceChanged,
        ));
    }
    Ok(copied_id)
}

fn validate_existing_destination(
    destination: &Path,
    expected: &ModPackageInfo,
    limits: &crate::ModLimits,
) -> Result<ModPackageInfo, ModError> {
    let existing = inspect_package(destination, limits, &mut ContinueProgress)?;
    if existing.same_content(expected) {
        Ok(existing)
    } else {
        Err(ModError::package(
            destination,
            None,
            PackageErrorKind::DestinationCollision,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::Path};

    use tempfile::tempdir;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::{ContinueProgress, ModError, ModService, PackageErrorKind};
    use crate::ModStorePaths;

    #[test]
    fn source_change_after_inspection_is_rejected_before_publication()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let source = directory.path().join("source.zip");
        let replacement = directory.path().join("replacement.zip");
        write_package(&source, "Original", b"original")?;
        write_package(&replacement, "Replacement", b"replacement")?;
        let stores = ModStorePaths::new(directory.path().join("application-data"));
        let service = ModService::new(stores.clone());

        let result = service.import_package_with_hook(&source, &mut ContinueProgress, || {
            fs::copy(&replacement, &source)
                .map(|_| ())
                .map_err(|error| ModError::io("replace test package", &source, error))
        });

        assert!(matches!(
            result,
            Err(ModError::InvalidPackage {
                kind: PackageErrorKind::SourceChanged,
                ..
            })
        ));
        assert!(!stores.packages().join("replacement.zip").exists());
        if stores.packages().exists() {
            assert_eq!(fs::read_dir(stores.packages())?.count(), 0);
        }
        Ok(())
    }

    fn write_package(
        path: &Path,
        name: &str,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = fs::File::create(path)?;
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        writer.start_file("mod.json", options)?;
        write!(
            writer,
            "{{\"name\":\"{name}\",\"version\":\"1\",\"game\":\"heroes\",\"files\":[\"file.sox\"]}}"
        )?;
        writer.start_file("file.sox", options)?;
        writer.write_all(payload)?;
        writer.finish()?.sync_all()?;
        Ok(())
    }
}
