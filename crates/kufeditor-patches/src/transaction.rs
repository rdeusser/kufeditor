use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

use kufeditor_game::Game;

use crate::inspection::{
    BASE_DELAY_OFFSET, FACTOR_OFFSET, MULTIPLIER_OFFSET, backup_path, inspect_backup,
    inspect_fire_rate, inspect_patch, source_range, validate_executable_length,
};
use crate::{
    BackupStatus, ExecutableInspection, FireRatePresetID, FireRateStatus, PatchError, PatchID,
    PatchStatus, RecoveryStatus, RollbackFailure, fire_rate_presets, inspect, patch_definitions,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchTarget {
    Applied,
    NotApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchChange {
    Changed,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchOperationResult {
    change: PatchChange,
    backup_status: BackupStatus,
    backup_created: bool,
}

impl PatchOperationResult {
    pub const fn change(self) -> PatchChange {
        self.change
    }

    pub const fn backup_status(self) -> BackupStatus {
        self.backup_status
    }

    pub const fn backup_created(self) -> bool {
        self.backup_created
    }
}

pub fn set_patch(
    game: Game,
    root: &Path,
    id: PatchID,
    target: PatchTarget,
) -> Result<PatchOperationResult, PatchError> {
    let inspection = inspect(game, root)?;
    let status = inspection.patch_status(id);
    if patch_target_matches(status, target) {
        return Ok(unchanged_result(&inspection));
    }
    if status == PatchStatus::Unknown {
        return Err(PatchError::UnrecognizedPatch { id });
    }

    let source = read_source(inspection.path())?;
    let Some(definition) = patch_definitions()
        .iter()
        .find(|definition| definition.id() == id)
    else {
        return Err(PatchError::UnrecognizedPatch { id });
    };
    if inspect_patch(&source, definition) != status {
        return Err(PatchError::ExecutableChanged {
            path: inspection.path().to_path_buf(),
        });
    }

    let changes = definition
        .edits()
        .iter()
        .map(|edit| match target {
            PatchTarget::Applied => RangeChange::new(
                edit.offset(),
                edit.original().to_vec(),
                edit.patched().to_vec(),
            ),
            PatchTarget::NotApplied => RangeChange::new(
                edit.offset(),
                edit.patched().to_vec(),
                edit.original().to_vec(),
            ),
        })
        .collect::<Vec<_>>();
    mutate(&inspection, &source, &changes)
}

pub fn set_fire_rate(
    game: Game,
    root: &Path,
    id: FireRatePresetID,
) -> Result<PatchOperationResult, PatchError> {
    let inspection = inspect(game, root)?;
    let status = inspection.fire_rate();
    if status == FireRateStatus::Preset(id) {
        return Ok(unchanged_result(&inspection));
    }
    if status == FireRateStatus::Unknown {
        return Err(PatchError::UnrecognizedFireRate);
    }

    let source = read_source(inspection.path())?;
    if inspect_fire_rate(&source) != status {
        return Err(PatchError::ExecutableChanged {
            path: inspection.path().to_path_buf(),
        });
    }
    let Some(preset) = fire_rate_presets().iter().find(|preset| preset.id() == id) else {
        return Err(PatchError::UnrecognizedFireRate);
    };
    let values = preset.values();
    let Some(multiplier) = encode_multiplier(values.multiplier()) else {
        return Err(PatchError::UnrecognizedFireRate);
    };
    let changes = [
        fire_rate_change(
            inspection.path(),
            &source,
            BASE_DELAY_OFFSET,
            values.base_delay().to_le_bytes().to_vec(),
        )?,
        fire_rate_change(
            inspection.path(),
            &source,
            MULTIPLIER_OFFSET,
            multiplier.to_vec(),
        )?,
        fire_rate_change(
            inspection.path(),
            &source,
            FACTOR_OFFSET,
            values.distance_factor_bits().to_le_bytes().to_vec(),
        )?,
    ];
    mutate(&inspection, &source, &changes)
}

fn patch_target_matches(status: PatchStatus, target: PatchTarget) -> bool {
    matches!(
        (status, target),
        (PatchStatus::Applied, PatchTarget::Applied)
            | (PatchStatus::NotApplied, PatchTarget::NotApplied)
    )
}

fn unchanged_result(inspection: &ExecutableInspection) -> PatchOperationResult {
    PatchOperationResult {
        change: PatchChange::Unchanged,
        backup_status: inspection.backup_status(),
        backup_created: false,
    }
}

fn read_source(path: &Path) -> Result<Vec<u8>, PatchError> {
    let source = fs::read(path).map_err(|source| PatchError::ExecutableRead {
        path: path.to_path_buf(),
        source,
    })?;
    validate_executable_length(path, source.len() as u64)?;
    Ok(source)
}

fn fire_rate_change(
    path: &Path,
    source: &[u8],
    offset: u64,
    replacement: Vec<u8>,
) -> Result<RangeChange, PatchError> {
    let Some(expected) = source_range(source, offset, replacement.len()) else {
        return Err(PatchError::ExecutableChanged {
            path: path.to_path_buf(),
        });
    };
    Ok(RangeChange::new(offset, expected.to_vec(), replacement))
}

const fn encode_multiplier(multiplier: i32) -> Option<[u8; 3]> {
    match multiplier {
        3 => Some([0x8D, 0x04, 0x40]),
        2 => Some([0x8D, 0x04, 0x00]),
        1 => Some([0x89, 0xC0, 0x90]),
        _ => None,
    }
}

fn mutate(
    inspection: &ExecutableInspection,
    source: &[u8],
    changes: &[RangeChange],
) -> Result<PatchOperationResult, PatchError> {
    let backup_path = backup_path(inspection.path());
    let backup_created = ensure_backup(&backup_path, source)?;
    let mut executable = OpenOptions::new()
        .read(true)
        .write(true)
        .open(inspection.path())
        .map_err(|source| PatchError::ExecutableOpen {
            path: inspection.path().to_path_buf(),
            source,
        })?;
    execute_transaction(&mut executable, inspection.path(), source, changes)?;
    Ok(PatchOperationResult {
        change: PatchChange::Changed,
        backup_status: BackupStatus::Present,
        backup_created,
    })
}

fn ensure_backup(path: &Path, source: &[u8]) -> Result<bool, PatchError> {
    if inspect_backup(path, source.len() as u64)? == BackupStatus::Present {
        return Ok(false);
    }

    let mut backup = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| PatchError::BackupCreate {
            path: path.to_path_buf(),
            source,
        })?;
    if let Err(source) = backup.write_all(source) {
        drop(backup);
        return Err(backup_write_error(path, source));
    }
    if let Err(source) = backup.sync_all() {
        drop(backup);
        return Err(backup_sync_error(path, source));
    }
    Ok(true)
}

fn backup_write_error(path: &Path, source: io::Error) -> PatchError {
    match remove_incomplete_backup(path) {
        Ok(()) => PatchError::BackupWrite {
            path: path.to_path_buf(),
            source,
        },
        Err(cleanup) => PatchError::BackupWriteCleanup {
            path: path.to_path_buf(),
            source,
            cleanup,
        },
    }
}

fn backup_sync_error(path: &Path, source: io::Error) -> PatchError {
    match remove_incomplete_backup(path) {
        Ok(()) => PatchError::BackupSync {
            path: path.to_path_buf(),
            source,
        },
        Err(cleanup) => PatchError::BackupSyncCleanup {
            path: path.to_path_buf(),
            source,
            cleanup,
        },
    }
}

fn remove_incomplete_backup(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RangeChange {
    offset: u64,
    expected: Vec<u8>,
    replacement: Vec<u8>,
}

impl RangeChange {
    fn new(offset: u64, expected: Vec<u8>, replacement: Vec<u8>) -> Self {
        Self {
            offset,
            expected,
            replacement,
        }
    }
}

trait MutationIO {
    fn snapshot(&mut self) -> io::Result<Vec<u8>>;
    fn read_at(&mut self, offset: u64, length: usize) -> io::Result<Vec<u8>>;
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
}

impl MutationIO for File {
    fn snapshot(&mut self) -> io::Result<Vec<u8>> {
        self.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        self.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn read_at(&mut self, offset: u64, length: usize) -> io::Result<Vec<u8>> {
        self.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; length];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        self.seek(SeekFrom::Start(offset))?;
        self.write_all(bytes)
    }

    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

fn execute_transaction(
    executable: &mut impl MutationIO,
    path: &Path,
    source: &[u8],
    changes: &[RangeChange],
) -> Result<(), PatchError> {
    let snapshot = executable
        .snapshot()
        .map_err(|source| PatchError::ExecutableVerify {
            path: path.to_path_buf(),
            offset: 0,
            recovery: RecoveryStatus::Restored,
            source,
        })?;
    if snapshot != source {
        return Err(PatchError::ExecutableChanged {
            path: path.to_path_buf(),
        });
    }

    let mut applied = Vec::with_capacity(changes.len());
    for (index, change) in changes.iter().enumerate() {
        let actual = match executable.read_at(change.offset, change.expected.len()) {
            Ok(actual) => actual,
            Err(source) => {
                let recovery = rollback(executable, changes, &applied);
                return Err(PatchError::ExecutableVerify {
                    path: path.to_path_buf(),
                    offset: change.offset,
                    recovery,
                    source,
                });
            }
        };
        if actual != change.expected {
            let recovery = rollback(executable, changes, &applied);
            return Err(PatchError::ExecutableChangedDuringWrite {
                path: path.to_path_buf(),
                offset: change.offset,
                recovery,
            });
        }

        applied.push(index);
        if let Err(source) = executable.write_at(change.offset, &change.replacement) {
            let recovery = rollback(executable, changes, &applied);
            return Err(PatchError::ExecutableWrite {
                path: path.to_path_buf(),
                offset: change.offset,
                recovery,
                source,
            });
        }
    }

    if let Err(source) = executable.sync() {
        let recovery = rollback(executable, changes, &applied);
        return Err(PatchError::ExecutableSync {
            path: path.to_path_buf(),
            recovery,
            source,
        });
    }
    Ok(())
}

fn rollback(
    executable: &mut impl MutationIO,
    changes: &[RangeChange],
    applied: &[usize],
) -> RecoveryStatus {
    let mut failure = None;
    for index in applied.iter().rev() {
        let Some(change) = changes.get(*index) else {
            continue;
        };
        if let Err(error) = executable.write_at(change.offset, &change.expected)
            && failure.is_none()
        {
            failure = Some(RollbackFailure::write(change.offset, &error));
        }
    }
    if let Err(error) = executable.sync()
        && failure.is_none()
    {
        failure = Some(RollbackFailure::sync(&error));
    }
    failure.map_or(RecoveryStatus::Restored, RecoveryStatus::Failed)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, io, path::Path};

    use tempfile::TempDir;

    use super::{
        MutationIO, PatchError, RangeChange, RecoveryStatus, backup_write_error,
        execute_transaction,
    };
    use crate::RollbackStage;

    #[test]
    fn incomplete_backup_cleanup_failure_is_typed() {
        let directory = TempDir::new().expect("temporary directory");
        let backup = directory.path().join("Kuf2Main.exe.bak");
        fs::create_dir(&backup).expect("backup directory");

        let error = backup_write_error(&backup, io::Error::other("write failed"));

        assert!(matches!(error, PatchError::BackupWriteCleanup { .. }));
    }

    #[test]
    fn partial_second_write_is_restored_in_reverse_order() {
        let source = b"abcdefghijklmnop".to_vec();
        let changes = changes();
        let mut executable = FakeMutationIO::new(source.clone()).with_partial_write_failure(2);

        let error = execute_transaction(
            &mut executable,
            Path::new("Kuf2Main.exe"),
            &source,
            &changes,
        )
        .expect_err("second write must fail");

        assert!(matches!(
            error,
            PatchError::ExecutableWrite {
                offset: 7,
                recovery: RecoveryStatus::Restored,
                ..
            }
        ));
        assert_eq!(executable.bytes, source);
        assert_eq!(executable.write_offsets, [2, 7, 7, 2]);
    }

    #[test]
    fn final_sync_failure_restores_every_range_and_syncs_the_rollback() {
        let source = b"abcdefghijklmnop".to_vec();
        let changes = changes();
        let mut executable = FakeMutationIO::new(source.clone()).with_sync_failure(1);

        let error = execute_transaction(
            &mut executable,
            Path::new("Kuf2Main.exe"),
            &source,
            &changes,
        )
        .expect_err("first sync must fail");

        assert!(matches!(
            error,
            PatchError::ExecutableSync {
                recovery: RecoveryStatus::Restored,
                ..
            }
        ));
        assert_eq!(executable.bytes, source);
        assert_eq!(executable.write_offsets, [2, 7, 7, 2]);
        assert_eq!(executable.sync_calls, 2);
    }

    #[test]
    fn rollback_failure_is_typed_and_does_not_stop_later_restoration() {
        let source = b"abcdefghijklmnop".to_vec();
        let changes = vec![
            RangeChange::new(2, b"cde".to_vec(), b"CDE".to_vec()),
            RangeChange::new(7, b"hij".to_vec(), b"HIJ".to_vec()),
            RangeChange::new(12, b"mno".to_vec(), b"MNO".to_vec()),
        ];
        let mut executable = FakeMutationIO::new(source.clone())
            .with_partial_write_failure(3)
            .with_write_failure(5);

        let error = execute_transaction(
            &mut executable,
            Path::new("Kuf2Main.exe"),
            &source,
            &changes,
        )
        .expect_err("third write must fail");

        let PatchError::ExecutableWrite {
            recovery: RecoveryStatus::Failed(failure),
            ..
        } = error
        else {
            panic!("expected a failed rollback");
        };
        assert_eq!(failure.stage(), RollbackStage::Write);
        assert_eq!(failure.offset(), Some(7));
        assert_eq!(executable.write_offsets, [2, 7, 12, 12, 7, 2]);
        assert_eq!(executable.bytes.get(2..5), Some(&b"cde"[..]));
        assert_eq!(executable.bytes.get(7..10), Some(&b"HIJ"[..]));
        assert_eq!(executable.bytes.get(12..15), Some(&b"mno"[..]));
        assert_eq!(executable.sync_calls, 1);
    }

    #[test]
    fn changed_snapshot_stops_before_the_first_write() {
        let source = b"abcdefghijklmnop".to_vec();
        let mut changed = source.clone();
        let Some(first) = changed.first_mut() else {
            panic!("test source must not be empty");
        };
        *first = b'!';
        let mut executable = FakeMutationIO::new(changed);

        let error = execute_transaction(
            &mut executable,
            Path::new("Kuf2Main.exe"),
            &source,
            &changes(),
        )
        .expect_err("changed source must fail");

        assert!(matches!(error, PatchError::ExecutableChanged { .. }));
        assert!(executable.write_offsets.is_empty());
        assert_eq!(executable.sync_calls, 0);
    }

    fn changes() -> Vec<RangeChange> {
        vec![
            RangeChange::new(2, b"cde".to_vec(), b"CDE".to_vec()),
            RangeChange::new(7, b"hij".to_vec(), b"HIJ".to_vec()),
        ]
    }

    struct FakeMutationIO {
        bytes: Vec<u8>,
        write_calls: usize,
        write_offsets: Vec<u64>,
        write_failures: BTreeSet<usize>,
        partial_write_failures: BTreeSet<usize>,
        sync_calls: usize,
        sync_failures: BTreeSet<usize>,
    }

    impl FakeMutationIO {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                bytes,
                write_calls: 0,
                write_offsets: Vec::new(),
                write_failures: BTreeSet::new(),
                partial_write_failures: BTreeSet::new(),
                sync_calls: 0,
                sync_failures: BTreeSet::new(),
            }
        }

        fn with_write_failure(mut self, call: usize) -> Self {
            self.write_failures.insert(call);
            self
        }

        fn with_partial_write_failure(mut self, call: usize) -> Self {
            self.partial_write_failures.insert(call);
            self
        }

        fn with_sync_failure(mut self, call: usize) -> Self {
            self.sync_failures.insert(call);
            self
        }
    }

    impl MutationIO for FakeMutationIO {
        fn snapshot(&mut self) -> io::Result<Vec<u8>> {
            Ok(self.bytes.clone())
        }

        fn read_at(&mut self, offset: u64, length: usize) -> io::Result<Vec<u8>> {
            let start = usize::try_from(offset)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
            let end = start
                .checked_add(length)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
            self.bytes
                .get(start..end)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short source"))
        }

        fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
            self.write_calls += 1;
            self.write_offsets.push(offset);
            if self.write_failures.contains(&self.write_calls) {
                return Err(io::Error::other("injected write failure"));
            }

            let start = usize::try_from(offset)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
            let end = start
                .checked_add(bytes.len())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
            let destination = self
                .bytes
                .get_mut(start..end)
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short source"))?;
            if self.partial_write_failures.contains(&self.write_calls) {
                if let (Some(destination), Some(source)) =
                    (destination.first_mut(), bytes.first().copied())
                {
                    *destination = source;
                }
                return Err(io::Error::other("injected partial write failure"));
            }
            destination.copy_from_slice(bytes);
            Ok(())
        }

        fn sync(&mut self) -> io::Result<()> {
            self.sync_calls += 1;
            if self.sync_failures.contains(&self.sync_calls) {
                Err(io::Error::other("injected sync failure"))
            } else {
                Ok(())
            }
        }
    }
}
