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
}
