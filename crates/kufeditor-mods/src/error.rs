use std::{io, path::PathBuf};

use kufeditor_game::Game;
use thiserror::Error;

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
    StoreInsideGameRoot,
}

impl std::fmt::Display for GameRootErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Missing => "the game root does not exist",
            Self::NotDirectory => "the game root is not a directory",
            Self::SymbolicLink => "the game root is a symbolic link",
            Self::NonUnicode => "the game root is not Unicode",
            Self::StoreInsideGameRoot => "the application-data root is inside the game root",
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
}

impl ModError {
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
}
