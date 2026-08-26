use std::{io, path::PathBuf};

use kufeditor_game::Game;
use thiserror::Error;

use crate::{BackupID, ChangedInstalledFiles, InstallationID, RecoveryReport, RelativeGamePath};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelativeGamePathErrorKind {
    Empty,
    Absolute,
    Backslash,
    NUL,
    Colon,
    EmptyComponent,
    CurrentComponent,
    ParentComponent,
    TerminalSpaceOrPeriod,
    WindowsDeviceName,
    TooLong,
    TooManyComponents,
}

impl std::fmt::Display for RelativeGamePathErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Empty => "the path is empty",
            Self::Absolute => "the path is absolute",
            Self::Backslash => "the path contains a backslash",
            Self::NUL => "the path contains a NUL byte",
            Self::Colon => "the path contains a colon",
            Self::EmptyComponent => "the path contains an empty component",
            Self::CurrentComponent => "the path contains a current-directory component",
            Self::ParentComponent => "the path contains a parent-directory component",
            Self::TerminalSpaceOrPeriod => "a component ends with a space or period",
            Self::WindowsDeviceName => "the path contains a Windows device name",
            Self::TooLong => "the path exceeds its byte limit",
            Self::TooManyComponents => "the path exceeds its component limit",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameRootErrorKind {
    Missing,
    NotDirectory,
    SymbolicLink,
    NonUnicode,
    StoreOverlapsGameRoot,
}

impl std::fmt::Display for GameRootErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Missing => "the game root does not exist",
            Self::NotDirectory => "the game root is not a directory",
            Self::SymbolicLink => "the game root is a symbolic link",
            Self::NonUnicode => "the game root is not Unicode",
            Self::StoreOverlapsGameRoot => "the owned mod store overlaps the game root",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestErrorKind {
    InvalidJSON,
    UnsupportedVersion,
    EmptyName,
    EmptyVersion,
    EmptyAuthor,
    EmptyDescription,
    UnknownGame,
    InvalidTimestamp,
    EmptyFiles,
    TooManyFiles,
    DuplicatePath,
    TooLarge,
    Serialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageErrorKind {
    NotRegularFile,
    NotDirectory,
    SymbolicLink,
    ZIPTooLarge,
    TooManyEntries,
    EntryNameNotUTF8,
    UnsafeEntryPath,
    DuplicateEntry,
    MissingManifest,
    DuplicateManifest,
    NestedManifest,
    EncryptedEntry,
    SymbolicLinkEntry,
    UnsupportedEntryType,
    UnsupportedCompression,
    DirectoryData,
    FileTooLarge,
    TotalDataTooLarge,
    EntrySizeMismatch,
    PayloadMismatch,
    SourceChanged,
    UnexpectedLibraryName,
    DestinationCollision,
    ReferencedPackage,
    MissingLibraryPackage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFileErrorKind {
    Missing,
    ParentNotDirectory,
    NotRegularFile,
    SymbolicLink,
    OutputCollision,
    TooLarge,
    Changed,
}

impl std::fmt::Display for SourceFileErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Missing => "the selected source does not exist",
            Self::ParentNotDirectory => "a selected source parent is not a directory",
            Self::NotRegularFile => "the selected source is not a regular file",
            Self::SymbolicLink => "the selected source contains a symbolic link",
            Self::OutputCollision => "the package output is one of its selected sources",
            Self::TooLarge => "the selected source exceeds a package limit",
            Self::Changed => "the selected source changed during package creation",
        };
        formatter.write_str(message)
    }
}

impl std::fmt::Display for PackageErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NotRegularFile => "the package is not a regular file",
            Self::NotDirectory => "the mod-store path is not a directory",
            Self::SymbolicLink => "the package is a symbolic link",
            Self::ZIPTooLarge => "the ZIP exceeds its byte limit",
            Self::TooManyEntries => "the ZIP exceeds its entry limit",
            Self::EntryNameNotUTF8 => "a ZIP entry name is not UTF8",
            Self::UnsafeEntryPath => "a ZIP entry has an unsafe game-relative path",
            Self::DuplicateEntry => "the ZIP contains duplicate portable entry names",
            Self::MissingManifest => "the ZIP has no root mod.json",
            Self::DuplicateManifest => "the ZIP has more than one root mod.json",
            Self::NestedManifest => "the ZIP contains a nested mod.json",
            Self::EncryptedEntry => "the ZIP contains an encrypted entry",
            Self::SymbolicLinkEntry => "the ZIP contains a symbolic-link entry",
            Self::UnsupportedEntryType => "the ZIP contains an unsupported entry type",
            Self::UnsupportedCompression => "the ZIP uses unsupported compression",
            Self::DirectoryData => "a ZIP directory entry contains data",
            Self::FileTooLarge => "a ZIP entry exceeds its byte limit",
            Self::TotalDataTooLarge => "the ZIP payload exceeds its aggregate byte limit",
            Self::EntrySizeMismatch => "a ZIP entry produced a different size than declared",
            Self::PayloadMismatch => "the ZIP payload does not match its manifest",
            Self::SourceChanged => "the package changed while it was inspected",
            Self::UnexpectedLibraryName => "the library filename is not its package identity",
            Self::DestinationCollision => "the package identity collides with different content",
            Self::ReferencedPackage => "an installation still references the package",
            Self::MissingLibraryPackage => "the requested library package does not exist",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryErrorKind {
    TooLarge,
    TooManyRecords,
    InvalidJSON,
    UnsupportedVersion,
    SymbolicLink,
    NotRegularFile,
    Changed,
    InvalidRecord,
}

impl std::fmt::Display for RegistryErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::TooLarge => "the installation registry exceeds its byte limit",
            Self::TooManyRecords => "the installation registry exceeds its record limit",
            Self::InvalidJSON => "the installation registry is not valid JSON",
            Self::UnsupportedVersion => "the installation registry version is unsupported",
            Self::SymbolicLink => "the installation registry is a symbolic link",
            Self::NotRegularFile => "the installation registry is not a regular file",
            Self::Changed => "the installation registry changed while it was read",
            Self::InvalidRecord => "the installation registry contains invalid records",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstalledFileErrorKind {
    SymbolicLink,
    NotRegularFile,
    TooLarge,
    Changed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationConflictKind {
    DuplicateName,
    PathOverlap,
}

impl std::fmt::Display for InstallationConflictKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::DuplicateName => "another installed mod has the same name",
            Self::PathOverlap => "another installed mod owns the same path",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetPathErrorKind {
    GameRootChanged,
    SymbolicLink,
    ParentNotDirectory,
    NotRegularFile,
    Changed,
    TooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UninstallErrorKind {
    MissingInstallation,
    WrongRoot,
    MissingRecoveryImage,
    InvalidRecoveryImage,
    UnsupportedOperationVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupErrorKind {
    InvalidLabel,
    TooManyFiles,
    TooLarge,
    SymbolicLink,
    NotDirectory,
    UnsupportedObject,
    UnsafePath,
    SourceChanged,
    Missing,
    InvalidMetadata,
    UnsupportedVersion,
    WrongRoot,
    IDMismatch,
    PayloadMismatch,
    DestinationCollision,
}

impl std::fmt::Display for BackupErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLabel => "the backup label is invalid",
            Self::TooManyFiles => "the backup exceeds its file-count limit",
            Self::TooLarge => "the backup exceeds its byte limit",
            Self::SymbolicLink => "the backup path contains a symbolic link",
            Self::NotDirectory => "the backup path is not a directory",
            Self::UnsupportedObject => "the backup source contains an unsupported object",
            Self::UnsafePath => "the backup contains an unsafe relative path",
            Self::SourceChanged => "the backup source changed while it was copied",
            Self::Missing => "the backup does not exist",
            Self::InvalidMetadata => "the backup metadata is invalid",
            Self::UnsupportedVersion => "the backup metadata version is unsupported",
            Self::WrongRoot => "the backup belongs to another game root",
            Self::IDMismatch => "the backup ID does not match its content",
            Self::PayloadMismatch => "the backup payload does not match its metadata",
            Self::DestinationCollision => "the backup ID collides with another directory",
        })
    }
}

impl std::fmt::Display for UninstallErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MissingInstallation => "the installation does not exist",
            Self::WrongRoot => "the installation belongs to another game root",
            Self::MissingRecoveryImage => "the installation recovery image is missing",
            Self::InvalidRecoveryImage => "the installation recovery image is invalid",
            Self::UnsupportedOperationVersion => {
                "the installation recovery image version is unsupported"
            }
        })
    }
}

impl std::fmt::Display for TargetPathErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::GameRootChanged => "the selected game root changed",
            Self::SymbolicLink => "a target component is a symbolic link",
            Self::ParentNotDirectory => "a target parent is not a directory",
            Self::NotRegularFile => "the target is not a regular file",
            Self::Changed => "the target changed during the operation",
            Self::TooLarge => "the target exceeds the file-size limit",
        })
    }
}

impl std::fmt::Display for InstalledFileErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::SymbolicLink => "the installed path contains a symbolic link",
            Self::NotRegularFile => "the installed path is not a regular file",
            Self::TooLarge => "the installed file exceeds its byte limit",
            Self::Changed => "the installed file changed while its health was checked",
        };
        formatter.write_str(message)
    }
}

impl std::fmt::Display for ManifestErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidJSON => "the manifest is not valid JSON",
            Self::UnsupportedVersion => "the manifest version is unsupported",
            Self::EmptyName => "the mod name is empty",
            Self::EmptyVersion => "the mod version is empty",
            Self::EmptyAuthor => "the mod author is empty",
            Self::EmptyDescription => "the mod description is empty",
            Self::UnknownGame => "the manifest game is unsupported",
            Self::InvalidTimestamp => "the manifest timestamp is invalid",
            Self::EmptyFiles => "the manifest file list is empty",
            Self::TooManyFiles => "the manifest file list exceeds its limit",
            Self::DuplicatePath => "the manifest contains a portable path collision",
            Self::TooLarge => "the manifest exceeds its byte limit",
            Self::Serialization => "the manifest could not be serialized",
        };
        formatter.write_str(message)
    }
}

#[derive(Debug, Error)]
pub enum ModError {
    #[error("invalid relative game path {value:?}: {kind}")]
    InvalidRelativeGamePath {
        value: String,
        kind: RelativeGamePathErrorKind,
    },
    #[error("invalid game root for {game} at {path:?}: {kind}")]
    InvalidGameRoot {
        game: Game,
        path: PathBuf,
        kind: GameRootErrorKind,
    },
    #[error("invalid mod manifest: {kind}")]
    InvalidManifest { kind: ManifestErrorKind },
    #[error("invalid package {path:?}: {kind}{entry_suffix}", entry_suffix = entry.as_ref().map(|entry| format!(" ({entry:?})")).unwrap_or_default())]
    InvalidPackage {
        path: PathBuf,
        entry: Option<String>,
        kind: PackageErrorKind,
    },
    #[error("invalid package source {path:?}: {kind}")]
    InvalidSourceFile {
        path: PathBuf,
        kind: SourceFileErrorKind,
    },
    #[error("invalid installation registry {path:?}: {kind}")]
    InvalidRegistry {
        path: PathBuf,
        kind: RegistryErrorKind,
    },
    #[error("invalid installed file {path:?}: {kind}")]
    InvalidInstalledFile {
        path: PathBuf,
        kind: InstalledFileErrorKind,
    },
    #[error("the package is for {package}, but the selected root is for {target}")]
    PackageGameMismatch { package: Game, target: Game },
    #[error("installation conflicts with {installation}: {kind}")]
    InstallationConflict {
        kind: InstallationConflictKind,
        installation: InstallationID,
        path: Option<RelativeGamePath>,
    },
    #[error("invalid game target {path:?}: {kind}")]
    InvalidTargetPath {
        path: PathBuf,
        kind: TargetPathErrorKind,
    },
    #[error("cannot uninstall {installation}: {kind}")]
    InvalidUninstall {
        installation: InstallationID,
        path: Option<PathBuf>,
        kind: UninstallErrorKind,
    },
    #[error("cannot uninstall {installation}: installed files changed")]
    ChangedInstalledFiles {
        installation: InstallationID,
        changes: Box<ChangedInstalledFiles>,
    },
    #[error("invalid backup at {path:?}: {kind}")]
    InvalidBackup {
        path: PathBuf,
        backup: Option<BackupID>,
        kind: BackupErrorKind,
    },
    #[error("could not read ZIP package {path:?}: {source}")]
    ZIP {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
    #[error("{operation} was canceled")]
    Canceled { operation: &'static str },
    #[error("invalid {kind} ID {value:?}")]
    InvalidID { kind: &'static str, value: String },
    #[error("could not {operation} {path:?}: {source}")]
    IO {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{operation} failed: {source}")]
    Transaction {
        operation: &'static str,
        #[source]
        source: Box<Self>,
        recovery: Box<RecoveryReport>,
    },
}

impl ModError {
    pub fn recovery_report(&self) -> Option<&RecoveryReport> {
        match self {
            Self::Transaction { recovery, .. } => Some(recovery.as_ref()),
            _ => None,
        }
    }

    pub fn changed_installed_files(&self) -> Option<&ChangedInstalledFiles> {
        match self {
            Self::ChangedInstalledFiles { changes, .. } => Some(changes.as_ref()),
            Self::Transaction { source, .. } => source.changed_installed_files(),
            _ => None,
        }
    }

    pub(crate) const fn manifest(kind: ManifestErrorKind) -> Self {
        Self::InvalidManifest { kind }
    }

    pub(crate) fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::IO {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn package(
        path: impl Into<PathBuf>,
        entry: Option<String>,
        kind: PackageErrorKind,
    ) -> Self {
        Self::InvalidPackage {
            path: path.into(),
            entry,
            kind,
        }
    }

    pub(crate) fn zip(path: impl Into<PathBuf>, source: zip::result::ZipError) -> Self {
        Self::ZIP {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn source(path: impl Into<PathBuf>, kind: SourceFileErrorKind) -> Self {
        Self::InvalidSourceFile {
            path: path.into(),
            kind,
        }
    }

    pub(crate) fn registry(path: impl Into<PathBuf>, kind: RegistryErrorKind) -> Self {
        Self::InvalidRegistry {
            path: path.into(),
            kind,
        }
    }

    pub(crate) fn installed_file(path: impl Into<PathBuf>, kind: InstalledFileErrorKind) -> Self {
        Self::InvalidInstalledFile {
            path: path.into(),
            kind,
        }
    }

    pub(crate) fn target(path: impl Into<PathBuf>, kind: TargetPathErrorKind) -> Self {
        Self::InvalidTargetPath {
            path: path.into(),
            kind,
        }
    }

    pub(crate) fn transaction(
        operation: &'static str,
        source: Self,
        recovery: RecoveryReport,
    ) -> Self {
        Self::Transaction {
            operation,
            source: Box::new(source),
            recovery: Box::new(recovery),
        }
    }

    pub(crate) fn uninstall(
        installation: InstallationID,
        path: Option<PathBuf>,
        kind: UninstallErrorKind,
    ) -> Self {
        Self::InvalidUninstall {
            installation,
            path,
            kind,
        }
    }

    pub(crate) fn backup(
        path: impl Into<PathBuf>,
        backup: Option<BackupID>,
        kind: BackupErrorKind,
    ) -> Self {
        Self::InvalidBackup {
            path: path.into(),
            backup,
            kind,
        }
    }
}
