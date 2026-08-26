//! Mod packages, backups, installation, and restoration.

mod error;
mod library;
mod manifest;
mod package;
mod path;
mod progress;

pub use error::{
    GameRootErrorKind, ManifestErrorKind, ModError, PackageErrorKind, RelativeGamePathErrorKind,
};
pub use library::{ImportedMod, ImportedModDisposition, ModLibraryIssue, ModLibraryScan};
pub use manifest::{ModManifest, ModMetadata, ModTimestamp};
pub use package::ModPackageInfo;
pub use path::{
    BackupID, GameRoot, GameRootKey, ModPackageID, ModStorePaths, OperationID, RelativeGamePath,
};
pub use progress::{ModProgress, ModProgressPhase, ModProgressReporter};

/// Resource ceilings for package, path, and backup operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModLimits {
    pub max_zip_bytes: u64,
    pub max_manifest_bytes: u64,
    pub max_package_files: u64,
    pub max_file_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_backup_files: u64,
    pub max_backup_bytes: u64,
    pub max_relative_path_bytes: usize,
    pub max_relative_path_components: usize,
}

impl Default for ModLimits {
    fn default() -> Self {
        Self {
            max_zip_bytes: 16 * 1024 * 1024 * 1024,
            max_manifest_bytes: 1024 * 1024,
            max_package_files: 65_536,
            max_file_bytes: 8 * 1024 * 1024 * 1024,
            max_uncompressed_bytes: 64 * 1024 * 1024 * 1024,
            max_backup_files: 262_144,
            max_backup_bytes: 128 * 1024 * 1024 * 1024,
            max_relative_path_bytes: 4_096,
            max_relative_path_components: 128,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModService {
    paths: ModStorePaths,
    limits: ModLimits,
}

impl ModService {
    pub fn new(paths: ModStorePaths) -> Self {
        Self::with_limits(paths, ModLimits::default())
    }

    pub const fn with_limits(paths: ModStorePaths, limits: ModLimits) -> Self {
        Self { paths, limits }
    }
}
