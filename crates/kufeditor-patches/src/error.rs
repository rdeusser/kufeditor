use std::{fmt, io, path::PathBuf};

use kufeditor_game::Game;
use thiserror::Error;

use crate::PatchID;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RollbackStage {
    Write,
    Sync,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackFailure {
    stage: RollbackStage,
    offset: Option<u64>,
    message: String,
}

impl RollbackFailure {
    pub(crate) fn write(offset: u64, error: &io::Error) -> Self {
        Self {
            stage: RollbackStage::Write,
            offset: Some(offset),
            message: error.to_string(),
        }
    }

    pub(crate) fn sync(error: &io::Error) -> Self {
        Self {
            stage: RollbackStage::Sync,
            offset: None,
            message: error.to_string(),
        }
    }

    pub const fn stage(&self) -> RollbackStage {
        self.stage
    }

    pub const fn offset(&self) -> Option<u64> {
        self.offset
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryStatus {
    Restored,
    Failed(RollbackFailure),
}

impl fmt::Display for RecoveryStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Restored => formatter.write_str("the original bytes were restored"),
            Self::Failed(failure) => match failure.offset() {
                Some(offset) => write!(
                    formatter,
                    "rollback {:?} failed at offset {offset:#X}: {}",
                    failure.stage(),
                    failure.message(),
                ),
                None => write!(
                    formatter,
                    "rollback {:?} failed: {}",
                    failure.stage(),
                    failure.message(),
                ),
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("{game} does not have executable patches")]
    UnsupportedGame { game: Game },
    #[error("patch executable does not exist: {path}", path = path.display())]
    ExecutableMissing { path: PathBuf },
    #[error("patch executable is a symbolic link: {path}", path = path.display())]
    ExecutableSymbolicLink { path: PathBuf },
    #[error("patch executable is not a regular file: {path}", path = path.display())]
    ExecutableNotRegular { path: PathBuf },
    #[error(
        "patch executable is too short: {path} has {actual} bytes but needs at least {minimum}",
        path = path.display()
    )]
    ExecutableTooShort {
        path: PathBuf,
        actual: u64,
        minimum: u64,
    },
    #[error(
        "patch executable is too large: {path} has {actual} bytes but the limit is {maximum}",
        path = path.display()
    )]
    ExecutableTooLarge {
        path: PathBuf,
        actual: u64,
        maximum: u64,
    },
    #[error("could not inspect patch executable metadata: {path}", path = path.display())]
    ExecutableMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not read patch executable: {path}", path = path.display())]
    ExecutableRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("patch {id:?} has unrecognized executable bytes")]
    UnrecognizedPatch { id: PatchID },
    #[error("the executable has an unrecognized fire-rate instruction or context")]
    UnrecognizedFireRate,
    #[error("patch executable changed before it could be updated: {path}", path = path.display())]
    ExecutableChanged { path: PathBuf },
    #[error("could not open patch executable for writing: {path}", path = path.display())]
    ExecutableOpen {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "could not verify patch executable at offset {offset:#X}: {path}; {recovery}",
        path = path.display()
    )]
    ExecutableVerify {
        path: PathBuf,
        offset: u64,
        recovery: RecoveryStatus,
        #[source]
        source: io::Error,
    },
    #[error(
        "patch executable changed at offset {offset:#X}: {path}; {recovery}",
        path = path.display()
    )]
    ExecutableChangedDuringWrite {
        path: PathBuf,
        offset: u64,
        recovery: RecoveryStatus,
    },
    #[error(
        "could not write patch executable at offset {offset:#X}: {path}; {recovery}",
        path = path.display()
    )]
    ExecutableWrite {
        path: PathBuf,
        offset: u64,
        recovery: RecoveryStatus,
        #[source]
        source: io::Error,
    },
    #[error("could not synchronize patch executable: {path}; {recovery}", path = path.display())]
    ExecutableSync {
        path: PathBuf,
        recovery: RecoveryStatus,
        #[source]
        source: io::Error,
    },
    #[error("patch backup is a symbolic link: {path}", path = path.display())]
    BackupSymbolicLink { path: PathBuf },
    #[error("patch backup is not a regular file: {path}", path = path.display())]
    BackupNotRegular { path: PathBuf },
    #[error(
        "patch backup has the wrong length: {path} has {actual} bytes but the executable has {expected}",
        path = path.display()
    )]
    BackupLength {
        path: PathBuf,
        actual: u64,
        expected: u64,
    },
    #[error("could not inspect patch backup metadata: {path}", path = path.display())]
    BackupMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not create patch backup: {path}", path = path.display())]
    BackupCreate {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write patch backup: {path}", path = path.display())]
    BackupWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "could not write patch backup and could not remove the incomplete file: {path}; cleanup failed: {cleanup}",
        path = path.display()
    )]
    BackupWriteCleanup {
        path: PathBuf,
        #[source]
        source: io::Error,
        cleanup: io::Error,
    },
    #[error("could not synchronize patch backup: {path}", path = path.display())]
    BackupSync {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "could not synchronize patch backup and could not remove the incomplete file: {path}; cleanup failed: {cleanup}",
        path = path.display()
    )]
    BackupSyncCleanup {
        path: PathBuf,
        #[source]
        source: io::Error,
        cleanup: io::Error,
    },
}
