use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{Builder as TempBuilder, NamedTempFile};
use zip::ZipArchive;

use crate::{
    BackupErrorKind, BackupFileInfo, BackupID, ChangedInstalledFile, ChangedInstalledFiles,
    FileSHA256, GameRoot, GameRootErrorKind, GameRootKey, InstallationID, InstalledFile,
    InstalledFileChangeKind, ModError, ModLimits, ModPackageID, ModPackageInfo, ModProgress,
    ModProgressPhase, ModProgressReporter, ModStorePaths, ModTimestamp, OperationID,
    PackageErrorKind, RelativeGamePath, TargetPathErrorKind, UninstallErrorKind,
    library::prepare_mod_store_root,
    manifest::{game_name, parse_game},
    package::inspect_package,
    progress::ContinueProgress,
    registry::InstallationRecord,
};

const OPERATION_VERSION: u64 = 1;
const MAX_OPERATION_BYTES: usize = 16 * 1024 * 1024;
const COPY_BUFFER_BYTES: usize = 64 * 1024;
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Planned,
    Staged,
    Recoverable,
    Committing,
    Committed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecoveryReport {
    committed: Vec<RelativeGamePath>,
    rolled_back: Vec<RelativeGamePath>,
    rollback_failed: Vec<RelativeGamePath>,
    unchanged: Vec<RelativeGamePath>,
}

impl RecoveryReport {
    pub fn committed(&self) -> &[RelativeGamePath] {
        &self.committed
    }

    pub fn rolled_back(&self) -> &[RelativeGamePath] {
        &self.rolled_back
    }

    pub fn rollback_failed(&self) -> &[RelativeGamePath] {
        &self.rollback_failed
    }

    pub fn unchanged(&self) -> &[RelativeGamePath] {
        &self.unchanged
    }
}

#[derive(Debug, Default)]
pub(crate) struct TransactionFailpoint {
    after_state: Option<OperationState>,
    after_committed_paths: Option<usize>,
    registry_publication: bool,
    rollback_attempt: Option<usize>,
}

impl TransactionFailpoint {
    fn check_state(&self, state: OperationState, path: &Path) -> Result<(), ModError> {
        if self.after_state == Some(state) {
            Err(injected_error("advance transaction state", path))
        } else {
            Ok(())
        }
    }

    fn check_committed_paths(&self, count: usize, path: &Path) -> Result<(), ModError> {
        if self.after_committed_paths == Some(count) {
            Err(injected_error("commit game file", path))
        } else {
            Ok(())
        }
    }

    pub(crate) fn check_registry_publication(&self, path: &Path) -> Result<(), ModError> {
        if self.registry_publication {
            Err(injected_error("publish installation registry", path))
        } else {
            Ok(())
        }
    }

    fn fails_rollback_attempt(&self, attempt: usize) -> bool {
        self.rollback_attempt == Some(attempt)
    }

    #[cfg(test)]
    pub(crate) const fn after_state(state: OperationState) -> Self {
        Self {
            after_state: Some(state),
            after_committed_paths: None,
            registry_publication: false,
            rollback_attempt: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn after_committed_paths(count: usize) -> Self {
        Self {
            after_state: None,
            after_committed_paths: Some(count),
            registry_publication: false,
            rollback_attempt: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn at_registry_publication() -> Self {
        Self {
            after_state: None,
            after_committed_paths: None,
            registry_publication: true,
            rollback_attempt: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn with_rollback_attempt(mut self, attempt: usize) -> Self {
        self.rollback_attempt = Some(attempt);
        self
    }
}

#[derive(Debug)]
pub(crate) struct FileTransaction {
    root: GameRoot,
    directory: PathBuf,
    operation_id: OperationID,
    installation_id: InstallationID,
    package_id: ModPackageID,
    installed_at: ModTimestamp,
    state: OperationState,
    files: Vec<TransactionFile>,
    created_directories: Vec<RelativeGamePath>,
    committed_indices: Vec<usize>,
}

#[derive(Debug)]
struct TransactionFile {
    path: RelativeGamePath,
    installed_sha256: Option<FileSHA256>,
    original_existed: Option<bool>,
    original_sha256: Option<FileSHA256>,
    committed: bool,
}

impl FileTransaction {
    pub(crate) fn begin_apply(
        stores: &ModStorePaths,
        root: &GameRoot,
        package_id: ModPackageID,
        installed_at: ModTimestamp,
        files: &[RelativeGamePath],
    ) -> Result<Self, ModError> {
        let operations = prepare_operations_directory(stores)?;
        let (operation_id, directory) =
            create_operation_directory(&operations, root.key(), package_id.as_bytes())?;
        let installation_id =
            InstallationID::for_installation(root.key(), package_id, operation_id);
        let transaction_files = files
            .iter()
            .cloned()
            .map(|path| TransactionFile {
                path,
                installed_sha256: None,
                original_existed: None,
                original_sha256: None,
                committed: false,
            })
            .collect();
        let transaction = Self {
            root: root.clone(),
            directory,
            operation_id,
            installation_id,
            package_id,
            installed_at,
            state: OperationState::Planned,
            files: transaction_files,
            created_directories: Vec::new(),
            committed_indices: Vec::new(),
        };
        if let Err(error) = transaction.write_image() {
            let _ = fs::remove_dir_all(&transaction.directory);
            return Err(error);
        }
        Ok(transaction)
    }

    pub(crate) const fn operation_id(&self) -> OperationID {
        self.operation_id
    }

    pub(crate) const fn installation_id(&self) -> InstallationID {
        self.installation_id
    }

    pub(crate) const fn installed_at(&self) -> &ModTimestamp {
        &self.installed_at
    }

    pub(crate) fn committed_paths(&self) -> Vec<RelativeGamePath> {
        self.committed_indices
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.clone())
            .collect()
    }

    pub(crate) fn installed_files(&self) -> Result<Vec<InstalledFile>, ModError> {
        self.files
            .iter()
            .map(|file| {
                let installed_sha256 = file.installed_sha256.ok_or_else(|| {
                    ModError::io(
                        "read staged file digest",
                        self.staged_path(&file.path),
                        io::Error::new(io::ErrorKind::InvalidData, "missing staged file digest"),
                    )
                })?;
                let original_existed = file.original_existed.ok_or_else(|| {
                    ModError::io(
                        "read recovery record",
                        self.before_path(&file.path),
                        io::Error::new(io::ErrorKind::InvalidData, "missing recovery record"),
                    )
                })?;
                Ok(InstalledFile::new(
                    file.path.clone(),
                    installed_sha256,
                    original_existed,
                ))
            })
            .collect()
    }

    pub(crate) fn stage_package(
        &mut self,
        package: &ModPackageInfo,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        failpoint.check_state(OperationState::Planned, &self.directory)?;
        let staged_root = self.directory.join("staged");
        create_owned_directory(&staged_root, "create staged-file directory")?;
        let file = File::open(package.library_path()).map_err(|error| {
            ModError::io(
                "open package for apply staging",
                package.library_path(),
                error,
            )
        })?;
        let mut archive =
            ZipArchive::new(file).map_err(|error| ModError::zip(package.library_path(), error))?;
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        for index in 0..self.files.len() {
            let relative_path = self
                .files
                .get(index)
                .map(|file| file.path.clone())
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            let mut entry = archive.by_name(relative_path.as_str()).map_err(|_| {
                ModError::package(
                    package.library_path(),
                    Some(relative_path.as_str().to_owned()),
                    PackageErrorKind::PayloadMismatch,
                )
            })?;
            let staged_path = self.staged_path(&relative_path);
            let digest = write_staged_file(
                package.library_path(),
                &relative_path,
                &mut entry,
                &staged_path,
                limits,
            )?;
            let Some(transaction_file) = self.files.get_mut(index) else {
                return Err(invalid_transaction_index(&self.directory));
            };
            transaction_file.installed_sha256 = Some(digest);
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::StagingFiles,
                    completed,
                    total,
                    path: Some(relative_path),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "mod apply",
                });
            }
        }
        drop(archive);

        let verified = inspect_package(package.library_path(), limits, &mut ContinueProgress)?;
        if !verified.same_content(package) {
            return Err(ModError::package(
                package.library_path(),
                None,
                PackageErrorKind::SourceChanged,
            ));
        }
        self.state = OperationState::Staged;
        self.write_image()?;
        failpoint.check_state(OperationState::Staged, &self.directory)
    }

    pub(crate) fn create_recovery(
        &mut self,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        validate_game_root(&self.root)?;
        let before_root = self.directory.join("before");
        create_owned_directory(&before_root, "create before-image directory")?;
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        let mut missing_directories = HashSet::new();
        for index in 0..self.files.len() {
            let relative_path = self
                .files
                .get(index)
                .map(|file| file.path.clone())
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            let inspection = inspect_target(&self.root, &relative_path, limits)?;
            for directory in inspection.missing_directories {
                missing_directories.insert(directory);
            }
            let (original_existed, original_sha256) = if inspection.exists {
                let digest = copy_stable_file(
                    &inspection.target,
                    &self.before_path(&relative_path),
                    limits.max_file_bytes,
                    "copy game file into recovery image",
                )?;
                (true, Some(digest))
            } else {
                (false, None)
            };
            let Some(transaction_file) = self.files.get_mut(index) else {
                return Err(invalid_transaction_index(&self.directory));
            };
            transaction_file.original_existed = Some(original_existed);
            transaction_file.original_sha256 = original_sha256;
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::CreatingRecovery,
                    completed,
                    total,
                    path: Some(relative_path),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "mod apply",
                });
            }
        }
        self.created_directories = missing_directories.into_iter().collect();
        self.created_directories.sort_by(|left, right| {
            left.component_count()
                .cmp(&right.component_count())
                .then_with(|| left.portable_key().cmp(right.portable_key()))
        });
        self.state = OperationState::Recoverable;
        self.write_image()?;
        failpoint.check_state(OperationState::Recoverable, &self.directory)
    }

    pub(crate) fn commit_files(
        &mut self,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::CommittingFiles,
                completed: 0,
                total,
                path: None,
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "mod apply",
            });
        }
        self.state = OperationState::Committing;
        self.write_image()?;
        for index in 0..self.files.len() {
            let relative_path = self
                .files
                .get(index)
                .map(|file| file.path.clone())
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            self.verify_original_target(index, limits)?;
            self.prepare_target_parent(&relative_path)?;
            let staged = self.staged_path(&relative_path);
            let target = self.root.canonical_path().join(relative_path.as_ref());
            let expected_digest = self
                .files
                .get(index)
                .and_then(|file| file.installed_sha256)
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            publish_file(&staged, &target, expected_digest, limits.max_file_bytes)?;
            self.committed_indices.push(index);
            let Some(transaction_file) = self.files.get_mut(index) else {
                return Err(invalid_transaction_index(&self.directory));
            };
            transaction_file.committed = true;
            self.write_image()?;
            failpoint.check_committed_paths(self.committed_indices.len(), &target)?;

            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::CommittingFiles,
                    completed,
                    total,
                    path: Some(relative_path),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "mod apply",
                });
            }
        }
        self.state = OperationState::Committed;
        self.write_image()
    }

    pub(crate) fn recover_error(
        &mut self,
        error: ModError,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> ModError {
        if self.committed_indices.is_empty() {
            self.remove_created_directories();
            if let Err(cleanup_error) = fs::remove_dir_all(&self.directory) {
                return ModError::io(
                    "remove failed apply operation",
                    &self.directory,
                    cleanup_error,
                );
            }
            return error;
        }

        let committed = self.committed_paths();
        let committed_set = self
            .committed_indices
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let unchanged = self
            .files
            .iter()
            .enumerate()
            .filter(|(index, _)| !committed_set.contains(index))
            .map(|(_, file)| file.path.clone())
            .collect::<Vec<_>>();
        let mut rolled_back = Vec::new();
        let mut rollback_failed = Vec::new();
        let total = u64::try_from(self.committed_indices.len()).unwrap_or(u64::MAX);
        let rollback_indices = self
            .committed_indices
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        for (attempt_index, index) in rollback_indices.into_iter().enumerate() {
            let Some(file) = self.files.get(index) else {
                continue;
            };
            let path = file.path.clone();
            let rollback_result =
                if failpoint.fails_rollback_attempt(attempt_index.saturating_add(1)) {
                    Err(injected_error(
                        "roll back game file",
                        &self.root.canonical_path().join(path.as_ref()),
                    ))
                } else {
                    self.restore_original(index)
                };
            match rollback_result {
                Ok(()) => {
                    rolled_back.push(path.clone());
                    if let Some(file) = self.files.get_mut(index) {
                        file.committed = false;
                    }
                }
                Err(_) => rollback_failed.push(path.clone()),
            }
            let completed = u64::try_from(attempt_index.saturating_add(1)).unwrap_or(u64::MAX);
            let _ = progress.report(&ModProgress {
                phase: ModProgressPhase::RollingBack,
                completed,
                total,
                path: Some(path),
            });
        }
        self.remove_created_directories();
        self.state = OperationState::Recoverable;
        let _ = self.write_image();
        ModError::transaction(
            "mod apply",
            error,
            RecoveryReport {
                committed,
                rolled_back,
                rollback_failed,
                unchanged,
            },
        )
    }

    fn verify_original_target(&self, index: usize, limits: &ModLimits) -> Result<(), ModError> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid_transaction_index(&self.directory))?;
        let inspection = inspect_target(&self.root, &file.path, limits)?;
        match (file.original_existed, inspection.exists) {
            (Some(false), false) => Ok(()),
            (Some(true), true) => {
                let expected = file
                    .original_sha256
                    .ok_or_else(|| invalid_transaction_index(&self.directory))?;
                let actual = hash_stable_file(&inspection.target, limits.max_file_bytes)?;
                if actual == expected {
                    Ok(())
                } else {
                    Err(ModError::target(
                        inspection.target,
                        TargetPathErrorKind::Changed,
                    ))
                }
            }
            _ => Err(ModError::target(
                inspection.target,
                TargetPathErrorKind::Changed,
            )),
        }
    }

    fn prepare_target_parent(&self, path: &RelativeGamePath) -> Result<(), ModError> {
        let parent = path.as_ref().parent().unwrap_or_else(|| Path::new(""));
        let mut current = self.root.canonical_path().to_path_buf();
        let mut relative = PathBuf::new();
        for component in parent.components() {
            relative.push(component.as_os_str());
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(ModError::target(current, TargetPathErrorKind::SymbolicLink));
                    }
                    if !metadata.is_dir() {
                        return Err(ModError::target(
                            current,
                            TargetPathErrorKind::ParentNotDirectory,
                        ));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let relative_text = relative.to_string_lossy().replace('\\', "/");
                    let planned = self
                        .created_directories
                        .iter()
                        .any(|directory| directory.as_str() == relative_text);
                    if !planned {
                        return Err(ModError::target(current, TargetPathErrorKind::Changed));
                    }
                    fs::create_dir(&current).map_err(|error| {
                        ModError::io("create game target directory", &current, error)
                    })?;
                }
                Err(error) => {
                    return Err(ModError::io(
                        "inspect game target directory",
                        current,
                        error,
                    ));
                }
            }
        }
        Ok(())
    }

    fn restore_original(&self, index: usize) -> Result<(), ModError> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid_transaction_index(&self.directory))?;
        let target = self.root.canonical_path().join(file.path.as_ref());
        validate_rollback_target(&target)?;
        if file.original_existed == Some(true) {
            let before = self.before_path(&file.path);
            let expected = file
                .original_sha256
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            publish_file(&before, &target, expected, u64::MAX)
        } else {
            match fs::remove_file(&target) {
                Ok(()) => sync_parent_directory(&target),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(ModError::io("remove applied game file", target, error)),
            }
        }
    }

    fn remove_created_directories(&self) {
        for relative in self.created_directories.iter().rev() {
            let directory = self.root.canonical_path().join(relative.as_ref());
            let _ = fs::remove_dir(&directory);
        }
    }

    fn staged_path(&self, relative_path: &RelativeGamePath) -> PathBuf {
        self.directory.join("staged").join(relative_path.as_ref())
    }

    fn before_path(&self, relative_path: &RelativeGamePath) -> PathBuf {
        self.directory.join("before").join(relative_path.as_ref())
    }

    fn write_image(&self) -> Result<(), ModError> {
        let image = OperationImage {
            format_version: OPERATION_VERSION,
            operation_id: self.operation_id.to_string(),
            installation_id: self.installation_id.to_string(),
            package_id: self.package_id.to_string(),
            kind: OperationKind::Apply,
            state: self.state,
            game: game_name(self.root.game()).to_owned(),
            configured_root: self.root.configured_path().to_string_lossy().into_owned(),
            canonical_root: self.root.canonical_path().to_string_lossy().into_owned(),
            root_key: self.root.key().to_string(),
            installed_at: self.installed_at.as_str().to_owned(),
            files: self
                .files
                .iter()
                .map(|file| OperationFileImage {
                    path: file.path.as_str().to_owned(),
                    installed_sha256: file.installed_sha256.map(|digest| digest.to_string()),
                    original_existed: file.original_existed,
                    original_sha256: file.original_sha256.map(|digest| digest.to_string()),
                    committed: file.committed,
                })
                .collect(),
            created_directories: self
                .created_directories
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
        };
        let mut bytes = serde_json::to_vec_pretty(&image).map_err(|error| {
            ModError::io(
                "serialize operation image",
                self.directory.join("operation-v1.json"),
                io::Error::new(io::ErrorKind::InvalidData, error),
            )
        })?;
        bytes.push(b'\n');
        publish_control_file(
            &self.directory,
            &self.directory.join("operation-v1.json"),
            &bytes,
        )
    }
}

#[derive(Debug)]
pub(crate) struct OverlayTransaction {
    root: GameRoot,
    directory: PathBuf,
    operation_id: OperationID,
    backup_id: BackupID,
    files: Vec<OverlayFile>,
    created_directories: Vec<RelativeGamePath>,
    committed_indices: Vec<usize>,
    state: OperationState,
}

#[derive(Debug)]
struct OverlayFile {
    path: RelativeGamePath,
    staged_sha256: FileSHA256,
    original_existed: Option<bool>,
    original_sha256: Option<FileSHA256>,
    committed: bool,
}

impl OverlayTransaction {
    pub(crate) fn begin_backup_restore(
        stores: &ModStorePaths,
        root: &GameRoot,
        backup_id: BackupID,
        files: &[BackupFileInfo],
    ) -> Result<Self, ModError> {
        let operations = prepare_operations_directory(stores)?;
        let (operation_id, directory) =
            create_operation_directory(&operations, root.key(), backup_id.as_bytes())?;
        let transaction = Self {
            root: root.clone(),
            directory,
            operation_id,
            backup_id,
            files: files
                .iter()
                .map(|file| OverlayFile {
                    path: file.path().clone(),
                    staged_sha256: file.sha256(),
                    original_existed: None,
                    original_sha256: None,
                    committed: false,
                })
                .collect(),
            created_directories: Vec::new(),
            committed_indices: Vec::new(),
            state: OperationState::Planned,
        };
        if let Err(error) = transaction.write_image() {
            let _ = fs::remove_dir_all(&transaction.directory);
            return Err(error);
        }
        Ok(transaction)
    }

    pub(crate) fn stage_backup(
        &mut self,
        backup_directory: &Path,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        failpoint.check_state(OperationState::Planned, &self.directory)?;
        let staged_root = self.directory.join("staged");
        create_owned_directory(&staged_root, "create backup-restore staging")?;
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        for (index, file) in self.files.iter().enumerate() {
            let source = backup_directory.join("files").join(file.path.as_ref());
            let destination = staged_root.join(file.path.as_ref());
            let digest = copy_stable_file(
                &source,
                &destination,
                limits.max_file_bytes,
                "stage backup file for restore",
            )?;
            if digest != file.staged_sha256 {
                return Err(ModError::backup(
                    source,
                    Some(self.backup_id),
                    BackupErrorKind::SourceChanged,
                ));
            }
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::StagingBackupRestore,
                    completed,
                    total,
                    path: Some(file.path.clone()),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "backup restore",
                });
            }
        }
        self.state = OperationState::Staged;
        self.write_image()?;
        failpoint.check_state(OperationState::Staged, &self.directory)
    }

    pub(crate) fn create_recovery(
        &mut self,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        validate_game_root(&self.root)?;
        let before_root = self.directory.join("before");
        create_owned_directory(&before_root, "create backup-restore recovery")?;
        let mut missing_directories = HashSet::new();
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        for index in 0..self.files.len() {
            let path = self
                .files
                .get(index)
                .map(|file| file.path.clone())
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            let inspection = inspect_target(&self.root, &path, limits)?;
            for directory in inspection.missing_directories {
                missing_directories.insert(directory);
            }
            let (original_existed, original_sha256) = if inspection.exists {
                let digest = copy_stable_file(
                    &inspection.target,
                    &self.before_path(&path),
                    limits.max_file_bytes,
                    "copy game file into restore recovery",
                )?;
                (true, Some(digest))
            } else {
                (false, None)
            };
            let Some(file) = self.files.get_mut(index) else {
                return Err(invalid_transaction_index(&self.directory));
            };
            file.original_existed = Some(original_existed);
            file.original_sha256 = original_sha256;
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::CreatingRestoreRecovery,
                    completed,
                    total,
                    path: Some(path),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "backup restore",
                });
            }
        }
        self.created_directories = missing_directories.into_iter().collect();
        self.created_directories.sort_by(|left, right| {
            left.component_count()
                .cmp(&right.component_count())
                .then_with(|| left.portable_key().cmp(right.portable_key()))
        });
        self.state = OperationState::Recoverable;
        self.write_image()?;
        failpoint.check_state(OperationState::Recoverable, &self.directory)
    }

    pub(crate) fn commit(
        &mut self,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::RestoringBackup,
                completed: 0,
                total,
                path: None,
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "backup restore",
            });
        }
        self.state = OperationState::Committing;
        self.write_image()?;
        for index in 0..self.files.len() {
            let path = self
                .files
                .get(index)
                .map(|file| file.path.clone())
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            self.verify_original(index, limits)?;
            self.prepare_target_parent(&path)?;
            let target = self.root.canonical_path().join(path.as_ref());
            let expected = self
                .files
                .get(index)
                .map(|file| file.staged_sha256)
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            publish_file(
                &self.staged_path(&path),
                &target,
                expected,
                limits.max_file_bytes,
            )?;
            self.committed_indices.push(index);
            let Some(file) = self.files.get_mut(index) else {
                return Err(invalid_transaction_index(&self.directory));
            };
            file.committed = true;
            self.write_image()?;
            failpoint.check_committed_paths(self.committed_indices.len(), &target)?;
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::RestoringBackup,
                    completed,
                    total,
                    path: Some(path),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "backup restore",
                });
            }
        }
        self.state = OperationState::Committed;
        self.write_image()
    }

    pub(crate) fn recover_error(
        &mut self,
        error: ModError,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> ModError {
        if self.committed_indices.is_empty() {
            self.remove_created_directories();
            return match fs::remove_dir_all(&self.directory) {
                Ok(()) => error,
                Err(cleanup_error) => ModError::io(
                    "remove incomplete backup restore",
                    &self.directory,
                    cleanup_error,
                ),
            };
        }
        let committed = self.committed_paths();
        let committed_set = self
            .committed_indices
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let unchanged = self
            .files
            .iter()
            .enumerate()
            .filter(|(index, _)| !committed_set.contains(index))
            .map(|(_, file)| file.path.clone())
            .collect::<Vec<_>>();
        let rollback_indices = self
            .committed_indices
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        let total = u64::try_from(rollback_indices.len()).unwrap_or(u64::MAX);
        let mut rolled_back = Vec::new();
        let mut rollback_failed = Vec::new();
        for (attempt_index, index) in rollback_indices.into_iter().enumerate() {
            let path = match self.files.get(index) {
                Some(file) => file.path.clone(),
                None => continue,
            };
            let result = if failpoint.fails_rollback_attempt(attempt_index.saturating_add(1)) {
                Err(injected_error(
                    "roll back backup restore",
                    &self.root.canonical_path().join(path.as_ref()),
                ))
            } else {
                self.restore_original(index)
            };
            match result {
                Ok(()) => rolled_back.push(path.clone()),
                Err(_) => rollback_failed.push(path.clone()),
            }
            let completed = u64::try_from(attempt_index.saturating_add(1)).unwrap_or(u64::MAX);
            let _ = progress.report(&ModProgress {
                phase: ModProgressPhase::RollingBack,
                completed,
                total,
                path: Some(path),
            });
        }
        self.remove_created_directories();
        self.state = OperationState::Recoverable;
        let _ = self.write_image();
        ModError::transaction(
            "backup restore",
            error,
            RecoveryReport {
                committed,
                rolled_back,
                rollback_failed,
                unchanged,
            },
        )
    }

    pub(crate) fn committed_paths(&self) -> Vec<RelativeGamePath> {
        self.committed_indices
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.clone())
            .collect()
    }

    pub(crate) fn finish_success(self) -> Result<(), ModError> {
        let operations = self.directory.parent().map(Path::to_path_buf);
        fs::remove_dir_all(&self.directory).map_err(|error| {
            ModError::io("remove completed backup restore", &self.directory, error)
        })?;
        if let Some(operations) = operations {
            sync_directory(&operations)?;
        }
        Ok(())
    }

    fn verify_original(&self, index: usize, limits: &ModLimits) -> Result<(), ModError> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid_transaction_index(&self.directory))?;
        let inspection = inspect_target(&self.root, &file.path, limits)?;
        match (file.original_existed, inspection.exists) {
            (Some(false), false) => Ok(()),
            (Some(true), true) => {
                let expected = file
                    .original_sha256
                    .ok_or_else(|| invalid_transaction_index(&self.directory))?;
                let actual = hash_stable_file(&inspection.target, limits.max_file_bytes)?;
                if actual == expected {
                    Ok(())
                } else {
                    Err(ModError::target(
                        inspection.target,
                        TargetPathErrorKind::Changed,
                    ))
                }
            }
            _ => Err(ModError::target(
                inspection.target,
                TargetPathErrorKind::Changed,
            )),
        }
    }

    fn prepare_target_parent(&self, path: &RelativeGamePath) -> Result<(), ModError> {
        let parent = path.as_ref().parent().unwrap_or_else(|| Path::new(""));
        let mut current = self.root.canonical_path().to_path_buf();
        let mut relative = PathBuf::new();
        for component in parent.components() {
            current.push(component.as_os_str());
            relative.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(ModError::target(current, TargetPathErrorKind::SymbolicLink));
                    }
                    if !metadata.is_dir() {
                        return Err(ModError::target(
                            current,
                            TargetPathErrorKind::ParentNotDirectory,
                        ));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let relative_text = relative.to_string_lossy().replace('\\', "/");
                    if !self
                        .created_directories
                        .iter()
                        .any(|directory| directory.as_str() == relative_text)
                    {
                        return Err(ModError::target(current, TargetPathErrorKind::Changed));
                    }
                    fs::create_dir(&current).map_err(|error| {
                        ModError::io("create backup-restore target directory", &current, error)
                    })?;
                }
                Err(error) => {
                    return Err(ModError::io(
                        "inspect backup-restore target directory",
                        current,
                        error,
                    ));
                }
            }
        }
        Ok(())
    }

    fn restore_original(&self, index: usize) -> Result<(), ModError> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid_transaction_index(&self.directory))?;
        let target = self.root.canonical_path().join(file.path.as_ref());
        validate_rollback_target(&target)?;
        if file.original_existed == Some(true) {
            let expected = file
                .original_sha256
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            publish_file(&self.before_path(&file.path), &target, expected, u64::MAX)
        } else {
            match fs::remove_file(&target) {
                Ok(()) => sync_parent_directory(&target),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(ModError::io("remove restored backup file", target, error)),
            }
        }
    }

    fn remove_created_directories(&self) {
        for path in self.created_directories.iter().rev() {
            let _ = fs::remove_dir(self.root.canonical_path().join(path.as_ref()));
        }
    }

    fn staged_path(&self, path: &RelativeGamePath) -> PathBuf {
        self.directory.join("staged").join(path.as_ref())
    }

    fn before_path(&self, path: &RelativeGamePath) -> PathBuf {
        self.directory.join("before").join(path.as_ref())
    }

    fn write_image(&self) -> Result<(), ModError> {
        let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "formatVersion": OPERATION_VERSION,
            "operationID": self.operation_id.to_string(),
            "kind": "restoreBackup",
            "state": self.state,
            "backupID": self.backup_id.to_string(),
            "game": game_name(self.root.game()),
            "configuredRoot": self.root.configured_path().to_string_lossy(),
            "canonicalRoot": self.root.canonical_path().to_string_lossy(),
            "rootKey": self.root.key().to_string(),
            "files": self.files.iter().map(|file| serde_json::json!({
                "path": file.path.as_str(),
                "stagedSHA256": file.staged_sha256.to_string(),
                "originalExisted": file.original_existed,
                "originalSHA256": file.original_sha256.map(|digest| digest.to_string()),
                "committed": file.committed,
            })).collect::<Vec<_>>(),
            "createdDirectories": self.created_directories.iter()
                .map(RelativeGamePath::as_str).collect::<Vec<_>>(),
        }))
        .map_err(|error| {
            ModError::io(
                "serialize backup-restore operation",
                &self.directory,
                io::Error::new(io::ErrorKind::InvalidData, error),
            )
        })?;
        bytes.push(b'\n');
        publish_control_file(
            &self.directory,
            &self.directory.join("restore-v1.json"),
            &bytes,
        )
    }
}

#[derive(Debug)]
pub(crate) struct UninstallTransaction {
    root: GameRoot,
    directory: PathBuf,
    installation_id: InstallationID,
    files: Vec<UninstallFile>,
    created_directories: Vec<RelativeGamePath>,
    staging_directory: Option<PathBuf>,
    restored_indices: Vec<usize>,
    restored_paths: Vec<RelativeGamePath>,
    removed_paths: Vec<RelativeGamePath>,
}

#[derive(Debug)]
struct UninstallFile {
    path: RelativeGamePath,
    installed_sha256: FileSHA256,
    original_existed: bool,
    original_sha256: Option<FileSHA256>,
}

impl UninstallTransaction {
    pub(crate) fn load(
        stores: &ModStorePaths,
        root: &GameRoot,
        record: &InstallationRecord,
        limits: &ModLimits,
    ) -> Result<Self, ModError> {
        let installation_id = record.installation_id;
        let operations = stores.operations();
        require_uninstall_directory(&operations, installation_id)?;
        let directory = operations.join(record.operation_id.to_string());
        require_uninstall_directory(&directory, installation_id)?;
        let image_path = directory.join("operation-v1.json");
        let image = read_operation_image(&image_path, installation_id)?;
        let (files, mut created_directories) =
            validate_operation_image(&directory, &image, root, record, limits)?;
        created_directories.sort_by(|left, right| {
            left.component_count()
                .cmp(&right.component_count())
                .then_with(|| left.portable_key().cmp(right.portable_key()))
        });
        Ok(Self {
            root: root.clone(),
            directory,
            installation_id,
            files,
            created_directories,
            staging_directory: None,
            restored_indices: Vec::new(),
            restored_paths: Vec::new(),
            removed_paths: Vec::new(),
        })
    }

    pub(crate) fn stage_installed(
        &mut self,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
    ) -> Result<(), ModError> {
        let temporary = TempBuilder::new()
            .prefix("uninstall-")
            .tempdir_in(&self.directory)
            .map_err(|error| {
                ModError::io("create uninstall staging directory", &self.directory, error)
            })?;
        let files_root = temporary.path().join("files");
        create_owned_directory(&files_root, "create uninstall file staging")?;
        let mut changes = Vec::new();
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        for (index, file) in self.files.iter().enumerate() {
            let inspection = inspect_target(&self.root, &file.path, limits)?;
            if inspection.exists {
                let digest = copy_stable_file(
                    &inspection.target,
                    &files_root.join(file.path.as_ref()),
                    limits.max_file_bytes,
                    "stage installed file for uninstall rollback",
                )?;
                if digest != file.installed_sha256 {
                    changes.push(ChangedInstalledFile::new(
                        file.path.clone(),
                        InstalledFileChangeKind::Modified,
                    ));
                }
            } else {
                changes.push(ChangedInstalledFile::new(
                    file.path.clone(),
                    InstalledFileChangeKind::Missing,
                ));
            }
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::StagingUninstall,
                    completed,
                    total,
                    path: Some(file.path.clone()),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "mod uninstall",
                });
            }
        }
        if !changes.is_empty() {
            return Err(changed_installed_files(self.installation_id, changes));
        }

        write_uninstall_staging_image(temporary.path(), self.installation_id, &self.files)?;
        let staging_directory = temporary.keep();
        self.staging_directory = Some(staging_directory);
        let changes = self.current_changes(limits)?;
        if !changes.is_empty() {
            self.remove_staging()?;
            return Err(changed_installed_files(self.installation_id, changes));
        }
        Ok(())
    }

    pub(crate) fn restore_originals(
        &mut self,
        limits: &ModLimits,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<(), ModError> {
        let total = u64::try_from(self.files.len()).unwrap_or(u64::MAX);
        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::RestoringFiles,
                completed: 0,
                total,
                path: None,
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "mod uninstall",
            });
        }
        let changes = self.current_changes(limits)?;
        if !changes.is_empty() {
            return Err(changed_installed_files(self.installation_id, changes));
        }

        for index in 0..self.files.len() {
            self.verify_installed_file(index, limits)?;
            let file = self
                .files
                .get(index)
                .ok_or_else(|| invalid_transaction_index(&self.directory))?;
            let path = file.path.clone();
            let target = self.root.canonical_path().join(path.as_ref());
            if file.original_existed {
                let expected = file
                    .original_sha256
                    .ok_or_else(|| invalid_transaction_index(&self.directory))?;
                publish_file(
                    &self.before_path(&path),
                    &target,
                    expected,
                    limits.max_file_bytes,
                )?;
                self.restored_paths.push(path.clone());
            } else {
                validate_rollback_target(&target)?;
                fs::remove_file(&target)
                    .map_err(|error| ModError::io("remove installed mod file", &target, error))?;
                sync_parent_directory(&target)?;
                self.removed_paths.push(path.clone());
            }
            self.restored_indices.push(index);
            failpoint.check_committed_paths(self.restored_indices.len(), &target)?;
            let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::RestoringFiles,
                    completed,
                    total,
                    path: Some(path),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "mod uninstall",
                });
            }
        }
        self.remove_owned_empty_directories()
    }

    pub(crate) fn recover_error(
        &mut self,
        error: ModError,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> ModError {
        if self.restored_indices.is_empty() {
            return match self.remove_staging() {
                Ok(()) => error,
                Err(cleanup_error) => cleanup_error,
            };
        }

        let committed = self.restored_paths_in_order();
        let restored_set = self
            .restored_indices
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let unchanged = self
            .files
            .iter()
            .enumerate()
            .filter(|(index, _)| !restored_set.contains(index))
            .map(|(_, file)| file.path.clone())
            .collect::<Vec<_>>();
        let rollback_indices = self
            .restored_indices
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        let total = u64::try_from(rollback_indices.len()).unwrap_or(u64::MAX);
        let mut rolled_back = Vec::new();
        let mut rollback_failed = Vec::new();
        for (attempt_index, index) in rollback_indices.into_iter().enumerate() {
            let path = match self.files.get(index) {
                Some(file) => file.path.clone(),
                None => continue,
            };
            let rollback_result =
                if failpoint.fails_rollback_attempt(attempt_index.saturating_add(1)) {
                    Err(injected_error(
                        "roll back uninstalled file",
                        &self.root.canonical_path().join(path.as_ref()),
                    ))
                } else {
                    self.restore_installed(index)
                };
            match rollback_result {
                Ok(()) => rolled_back.push(path.clone()),
                Err(_) => rollback_failed.push(path.clone()),
            }
            let completed = u64::try_from(attempt_index.saturating_add(1)).unwrap_or(u64::MAX);
            let _ = progress.report(&ModProgress {
                phase: ModProgressPhase::RollingBack,
                completed,
                total,
                path: Some(path),
            });
        }
        ModError::transaction(
            "mod uninstall",
            error,
            RecoveryReport {
                committed,
                rolled_back,
                rollback_failed,
                unchanged,
            },
        )
    }

    pub(crate) fn restored_paths(&self) -> &[RelativeGamePath] {
        &self.restored_paths
    }

    pub(crate) fn removed_paths(&self) -> &[RelativeGamePath] {
        &self.removed_paths
    }

    pub(crate) fn finish_success(self) -> Result<(), ModError> {
        let operations = self.directory.parent().map(Path::to_path_buf);
        fs::remove_dir_all(&self.directory).map_err(|error| {
            ModError::io(
                "remove completed installation recovery",
                &self.directory,
                error,
            )
        })?;
        if let Some(operations) = operations {
            sync_directory(&operations)?;
        }
        Ok(())
    }

    fn current_changes(&self, limits: &ModLimits) -> Result<Vec<ChangedInstalledFile>, ModError> {
        let mut changes = Vec::new();
        for file in &self.files {
            let inspection = inspect_target(&self.root, &file.path, limits)?;
            if !inspection.exists {
                changes.push(ChangedInstalledFile::new(
                    file.path.clone(),
                    InstalledFileChangeKind::Missing,
                ));
                continue;
            }
            let digest = hash_stable_file(&inspection.target, limits.max_file_bytes)?;
            if digest != file.installed_sha256 {
                changes.push(ChangedInstalledFile::new(
                    file.path.clone(),
                    InstalledFileChangeKind::Modified,
                ));
            }
        }
        Ok(changes)
    }

    fn verify_installed_file(&self, index: usize, limits: &ModLimits) -> Result<(), ModError> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid_transaction_index(&self.directory))?;
        let inspection = inspect_target(&self.root, &file.path, limits)?;
        let kind = if inspection.exists {
            let digest = hash_stable_file(&inspection.target, limits.max_file_bytes)?;
            (digest != file.installed_sha256).then_some(InstalledFileChangeKind::Modified)
        } else {
            Some(InstalledFileChangeKind::Missing)
        };
        if let Some(kind) = kind {
            Err(changed_installed_files(
                self.installation_id,
                vec![ChangedInstalledFile::new(file.path.clone(), kind)],
            ))
        } else {
            Ok(())
        }
    }

    fn restore_installed(&self, index: usize) -> Result<(), ModError> {
        let file = self
            .files
            .get(index)
            .ok_or_else(|| invalid_transaction_index(&self.directory))?;
        self.prepare_rollback_parent(&file.path)?;
        let target = self.root.canonical_path().join(file.path.as_ref());
        validate_rollback_target(&target)?;
        let staged = self.staging_path(&file.path)?;
        publish_file(&staged, &target, file.installed_sha256, u64::MAX)
    }

    fn prepare_rollback_parent(&self, path: &RelativeGamePath) -> Result<(), ModError> {
        let parent = path.as_ref().parent().unwrap_or_else(|| Path::new(""));
        let mut current = self.root.canonical_path().to_path_buf();
        let mut relative = PathBuf::new();
        for component in parent.components() {
            current.push(component.as_os_str());
            relative.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(ModError::target(current, TargetPathErrorKind::SymbolicLink));
                    }
                    if !metadata.is_dir() {
                        return Err(ModError::target(
                            current,
                            TargetPathErrorKind::ParentNotDirectory,
                        ));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let relative_text = relative.to_string_lossy().replace('\\', "/");
                    if !self
                        .created_directories
                        .iter()
                        .any(|directory| directory.as_str() == relative_text)
                    {
                        return Err(ModError::target(current, TargetPathErrorKind::Changed));
                    }
                    fs::create_dir(&current).map_err(|error| {
                        ModError::io("recreate installed-file directory", &current, error)
                    })?;
                }
                Err(error) => {
                    return Err(ModError::io(
                        "inspect installed-file directory for rollback",
                        current,
                        error,
                    ));
                }
            }
        }
        Ok(())
    }

    fn remove_owned_empty_directories(&self) -> Result<(), ModError> {
        for relative in self.created_directories.iter().rev() {
            let directory = self.root.canonical_path().join(relative.as_ref());
            match fs::remove_dir(&directory) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => {
                    return Err(ModError::io(
                        "remove empty installed-file directory",
                        directory,
                        error,
                    ));
                }
            }
        }
        Ok(())
    }

    fn remove_staging(&mut self) -> Result<(), ModError> {
        let Some(staging) = self.staging_directory.take() else {
            return Ok(());
        };
        fs::remove_dir_all(&staging)
            .map_err(|error| ModError::io("remove uninstall staging", staging, error))
    }

    fn restored_paths_in_order(&self) -> Vec<RelativeGamePath> {
        self.restored_indices
            .iter()
            .filter_map(|index| self.files.get(*index))
            .map(|file| file.path.clone())
            .collect()
    }

    fn before_path(&self, path: &RelativeGamePath) -> PathBuf {
        self.directory.join("before").join(path.as_ref())
    }

    fn staging_path(&self, path: &RelativeGamePath) -> Result<PathBuf, ModError> {
        self.staging_directory
            .as_ref()
            .map(|directory| directory.join("files").join(path.as_ref()))
            .ok_or_else(|| {
                ModError::io(
                    "resolve uninstall staging file",
                    &self.directory,
                    io::Error::new(io::ErrorKind::NotFound, "uninstall staging is missing"),
                )
            })
    }
}

fn read_operation_image(
    path: &Path,
    installation_id: InstallationID,
) -> Result<OperationImage, ModError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ModError::uninstall(
                installation_id,
                Some(path.to_path_buf()),
                UninstallErrorKind::MissingRecoveryImage,
            ));
        }
        Err(error) => return Err(ModError::io("inspect operation image", path, error)),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_OPERATION_BYTES as u64
    {
        return Err(ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        ));
    }
    let before_stamp = file_stamp(&metadata);
    let file =
        File::open(path).map_err(|error| ModError::io("open operation image", path, error))?;
    let opened = file
        .metadata()
        .map_err(|error| ModError::io("inspect open operation image", path, error))?;
    if file_stamp(&opened) != before_stamp {
        return Err(ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        ));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(MAX_OPERATION_BYTES)
            .min(MAX_OPERATION_BYTES),
    );
    file.take((MAX_OPERATION_BYTES as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ModError::io("read operation image", path, error))?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("reinspect operation image", path, error))?;
    if bytes.len() > MAX_OPERATION_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len()
        || after.file_type().is_symlink()
        || !after.is_file()
        || file_stamp(&after) != before_stamp
    {
        return Err(ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        )
    })?;
    let Some(version) = value.get("formatVersion").and_then(Value::as_u64) else {
        return Err(ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        ));
    };
    if version != OPERATION_VERSION {
        return Err(ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::UnsupportedOperationVersion,
        ));
    }
    serde_json::from_value(value).map_err(|_| {
        ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        )
    })
}

fn validate_operation_image(
    directory: &Path,
    image: &OperationImage,
    root: &GameRoot,
    record: &InstallationRecord,
    limits: &ModLimits,
) -> Result<(Vec<UninstallFile>, Vec<RelativeGamePath>), ModError> {
    let invalid = || {
        ModError::uninstall(
            record.installation_id,
            Some(directory.join("operation-v1.json")),
            UninstallErrorKind::InvalidRecoveryImage,
        )
    };
    if OperationID::parse(&image.operation_id).ok() != Some(record.operation_id)
        || InstallationID::parse(&image.installation_id).ok() != Some(record.installation_id)
        || ModPackageID::parse(&image.package_id).ok() != Some(record.package_id)
        || image.kind != OperationKind::Apply
        || image.state != OperationState::Committed
        || parse_game(&image.game).ok() != Some(record.game)
        || image.configured_root != record.configured_root.to_string_lossy()
        || image.canonical_root != record.canonical_root.to_string_lossy()
        || GameRootKey::parse(&image.root_key).ok() != Some(record.root_key)
        || image.installed_at != record.installed_at.as_str()
        || record.root_key != root.key()
        || image.files.len() != record.files.len()
    {
        return Err(invalid());
    }

    let mut files = Vec::with_capacity(image.files.len());
    for (file_image, installed) in image.files.iter().zip(&record.files) {
        let path = RelativeGamePath::parse(&file_image.path, limits).map_err(|_| invalid())?;
        let installed_sha256 = file_image
            .installed_sha256
            .as_deref()
            .and_then(|value| FileSHA256::parse(value).ok())
            .ok_or_else(&invalid)?;
        let original_existed = file_image.original_existed.ok_or_else(&invalid)?;
        let original_sha256 = file_image
            .original_sha256
            .as_deref()
            .map(FileSHA256::parse)
            .transpose()
            .map_err(|_| invalid())?;
        if path != *installed.path()
            || installed_sha256 != installed.installed_sha256()
            || original_existed != installed.original_existed()
            || original_existed != original_sha256.is_some()
            || !file_image.committed
        {
            return Err(invalid());
        }
        if let Some(expected) = original_sha256 {
            let before = require_owned_recovery_file(
                &directory.join("before"),
                &path,
                record.installation_id,
            )?;
            let actual = hash_stable_file(&before, limits.max_file_bytes).map_err(|_| invalid())?;
            if actual != expected {
                return Err(invalid());
            }
        }
        files.push(UninstallFile {
            path,
            installed_sha256,
            original_existed,
            original_sha256,
        });
    }

    let mut portable_directories = HashSet::with_capacity(image.created_directories.len());
    let mut created_directories = Vec::with_capacity(image.created_directories.len());
    for value in &image.created_directories {
        let path = RelativeGamePath::parse(value, limits).map_err(|_| invalid())?;
        let prefix = format!("{}/", path.as_str());
        if !portable_directories.insert(path.portable_key().to_owned())
            || !files
                .iter()
                .any(|file| file.path.as_str().starts_with(&prefix))
        {
            return Err(invalid());
        }
        created_directories.push(path);
    }
    Ok((files, created_directories))
}

fn require_uninstall_directory(
    path: &Path,
    installation_id: InstallationID,
) -> Result<(), ModError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ModError::uninstall(
                installation_id,
                Some(path.to_path_buf()),
                UninstallErrorKind::MissingRecoveryImage,
            ));
        }
        Err(error) => return Err(ModError::io("inspect recovery directory", path, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ModError::uninstall(
            installation_id,
            Some(path.to_path_buf()),
            UninstallErrorKind::InvalidRecoveryImage,
        ));
    }
    Ok(())
}

fn require_owned_recovery_file(
    root: &Path,
    path: &RelativeGamePath,
    installation_id: InstallationID,
) -> Result<PathBuf, ModError> {
    require_uninstall_directory(root, installation_id)?;
    let mut current = root.to_path_buf();
    for (index, component) in path.as_str().split('/').enumerate() {
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ModError::uninstall(
                    installation_id,
                    Some(current),
                    UninstallErrorKind::MissingRecoveryImage,
                ));
            }
            Err(error) => {
                return Err(ModError::io("inspect recovery file", current, error));
            }
        };
        let is_file = index.saturating_add(1) == path.component_count();
        let valid = !metadata.file_type().is_symlink()
            && ((is_file && metadata.is_file()) || (!is_file && metadata.is_dir()));
        if !valid {
            return Err(ModError::uninstall(
                installation_id,
                Some(current),
                UninstallErrorKind::InvalidRecoveryImage,
            ));
        }
    }
    Ok(current)
}

fn write_uninstall_staging_image(
    directory: &Path,
    installation_id: InstallationID,
    files: &[UninstallFile],
) -> Result<(), ModError> {
    let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "formatVersion": 1,
        "installationID": installation_id.to_string(),
        "files": files.iter().map(|file| serde_json::json!({
            "path": file.path.as_str(),
            "installedSHA256": file.installed_sha256.to_string(),
        })).collect::<Vec<_>>(),
    }))
    .map_err(|error| {
        ModError::io(
            "serialize uninstall staging image",
            directory,
            io::Error::new(io::ErrorKind::InvalidData, error),
        )
    })?;
    bytes.push(b'\n');
    publish_control_file(directory, &directory.join("uninstall-v1.json"), &bytes)
}

fn changed_installed_files(
    installation: InstallationID,
    files: Vec<ChangedInstalledFile>,
) -> ModError {
    ModError::ChangedInstalledFiles {
        installation,
        changes: Box::new(ChangedInstalledFiles::new(files)),
    }
}

pub(crate) fn validate_game_root(root: &GameRoot) -> Result<(), ModError> {
    let metadata = match fs::symlink_metadata(root.canonical_path()) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ModError::InvalidGameRoot {
                game: root.game(),
                path: root.configured_path().to_path_buf(),
                kind: GameRootErrorKind::Missing,
            });
        }
        Err(error) => {
            return Err(ModError::io(
                "inspect game root before mod operation",
                root.canonical_path(),
                error,
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(ModError::InvalidGameRoot {
            game: root.game(),
            path: root.configured_path().to_path_buf(),
            kind: GameRootErrorKind::SymbolicLink,
        });
    }
    if !metadata.is_dir() {
        return Err(ModError::InvalidGameRoot {
            game: root.game(),
            path: root.configured_path().to_path_buf(),
            kind: GameRootErrorKind::NotDirectory,
        });
    }
    let canonical = fs::canonicalize(root.canonical_path()).map_err(|error| {
        ModError::io(
            "canonicalize game root before mod operation",
            root.canonical_path(),
            error,
        )
    })?;
    if canonical != root.canonical_path() {
        return Err(ModError::target(
            root.canonical_path(),
            TargetPathErrorKind::GameRootChanged,
        ));
    }
    Ok(())
}

fn prepare_operations_directory(stores: &ModStorePaths) -> Result<PathBuf, ModError> {
    prepare_mod_store_root(stores)?;
    let operations = stores.operations();
    match fs::symlink_metadata(&operations) {
        Ok(metadata) => validate_owned_directory(&operations, &metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&operations)
                .map_err(|error| ModError::io("create operation store", &operations, error))?;
            let metadata = fs::symlink_metadata(&operations).map_err(|error| {
                ModError::io("inspect created operation store", &operations, error)
            })?;
            validate_owned_directory(&operations, &metadata)?;
        }
        Err(error) => {
            return Err(ModError::io("inspect operation store", operations, error));
        }
    }
    Ok(operations)
}

fn create_operation_directory(
    operations: &Path,
    root_key: GameRootKey,
    seed: &[u8],
) -> Result<(OperationID, PathBuf), ModError> {
    for _ in 0..128 {
        let operation_id = next_operation_id(root_key, seed);
        let directory = operations.join(operation_id.to_string());
        match fs::create_dir(&directory) {
            Ok(()) => return Ok((operation_id, directory)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(ModError::io("create operation directory", directory, error));
            }
        }
    }
    Err(ModError::io(
        "create unique apply operation",
        operations,
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "operation ID collision limit reached",
        ),
    ))
}

fn next_operation_id(root_key: GameRootKey, seed: &[u8]) -> OperationID {
    let sequence = NEXT_OPERATION.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut hasher = Sha256::new();
    hasher.update(b"kufeditor-operation-v1\0");
    hasher.update(root_key.as_bytes());
    hasher.update(seed);
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(sequence.to_le_bytes());
    hasher.update(now.to_le_bytes());
    OperationID::from_bytes(hasher.finalize().into())
}

fn create_owned_directory(path: &Path, operation: &'static str) -> Result<(), ModError> {
    fs::create_dir(path).map_err(|error| ModError::io(operation, path, error))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("inspect created operation directory", path, error))?;
    validate_owned_directory(path, &metadata)
}

fn validate_owned_directory(path: &Path, metadata: &fs::Metadata) -> Result<(), ModError> {
    if metadata.file_type().is_symlink() {
        return Err(ModError::target(path, TargetPathErrorKind::SymbolicLink));
    }
    if !metadata.is_dir() {
        return Err(ModError::target(
            path,
            TargetPathErrorKind::ParentNotDirectory,
        ));
    }
    Ok(())
}

fn write_staged_file(
    package_path: &Path,
    relative_path: &RelativeGamePath,
    entry: &mut impl Read,
    destination: &Path,
    limits: &ModLimits,
) -> Result<FileSHA256, ModError> {
    let parent = destination.parent().ok_or_else(|| {
        ModError::io(
            "resolve staged-file parent",
            destination,
            io::Error::new(io::ErrorKind::InvalidInput, "staged file has no parent"),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| ModError::io("create staged-file parent", parent, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| ModError::io("create temporary staged file", parent, error))?;
    let (digest, bytes) = copy_and_hash(
        entry,
        temporary.as_file_mut(),
        limits.max_file_bytes,
        package_path,
        "extract package payload",
        TargetPathErrorKind::TooLarge,
    )?;
    if bytes > limits.max_file_bytes {
        return Err(ModError::package(
            package_path,
            Some(relative_path.as_str().to_owned()),
            PackageErrorKind::FileTooLarge,
        ));
    }
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| ModError::io("flush staged file", temporary.path(), error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| ModError::io("synchronize staged file", temporary.path(), error))?;
    temporary
        .persist_noclobber(destination)
        .map_err(|error| ModError::io("publish staged file", destination, error.error))?;
    sync_directory(parent)?;
    Ok(digest)
}

struct TargetInspection {
    target: PathBuf,
    exists: bool,
    missing_directories: Vec<RelativeGamePath>,
}

fn inspect_target(
    root: &GameRoot,
    path: &RelativeGamePath,
    limits: &ModLimits,
) -> Result<TargetInspection, ModError> {
    validate_game_root(root)?;
    let components = path.as_str().split('/').collect::<Vec<_>>();
    let mut current = root.canonical_path().to_path_buf();
    let mut relative_parts = Vec::new();
    let mut missing_directories = Vec::new();
    let mut missing = false;
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        relative_parts.push(*component);
        let is_leaf = index.saturating_add(1) == components.len();
        if missing {
            if !is_leaf {
                missing_directories
                    .push(RelativeGamePath::parse(&relative_parts.join("/"), limits)?);
            }
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(ModError::target(current, TargetPathErrorKind::SymbolicLink));
                }
                if is_leaf {
                    if !metadata.is_file() {
                        return Err(ModError::target(
                            current,
                            TargetPathErrorKind::NotRegularFile,
                        ));
                    }
                } else if !metadata.is_dir() {
                    return Err(ModError::target(
                        current,
                        TargetPathErrorKind::ParentNotDirectory,
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing = true;
                if !is_leaf {
                    missing_directories
                        .push(RelativeGamePath::parse(&relative_parts.join("/"), limits)?);
                }
            }
            Err(error) => {
                return Err(ModError::io("inspect game target", current, error));
            }
        }
    }
    Ok(TargetInspection {
        target: root.canonical_path().join(path.as_ref()),
        exists: !missing,
        missing_directories,
    })
}

fn copy_stable_file(
    source: &Path,
    destination: &Path,
    limit: u64,
    operation: &'static str,
) -> Result<FileSHA256, ModError> {
    let before = fs::symlink_metadata(source)
        .map_err(|error| ModError::io("inspect source before copy", source, error))?;
    validate_regular_file(source, &before)?;
    if before.len() > limit {
        return Err(ModError::target(source, TargetPathErrorKind::TooLarge));
    }
    let stamp = file_stamp(&before);
    let mut input =
        File::open(source).map_err(|error| ModError::io("open source file", source, error))?;
    let opened = input
        .metadata()
        .map_err(|error| ModError::io("inspect open source file", source, error))?;
    if file_stamp(&opened) != stamp {
        return Err(ModError::target(source, TargetPathErrorKind::Changed));
    }
    let parent = destination.parent().ok_or_else(|| {
        ModError::io(
            "resolve recovery-file parent",
            destination,
            io::Error::new(io::ErrorKind::InvalidInput, "recovery file has no parent"),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| ModError::io("create recovery-file parent", parent, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| ModError::io("create temporary recovery file", parent, error))?;
    let (digest, copied) = copy_and_hash(
        &mut input,
        temporary.as_file_mut(),
        limit,
        source,
        operation,
        TargetPathErrorKind::TooLarge,
    )?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| ModError::io("flush recovery file", temporary.path(), error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| ModError::io("synchronize recovery file", temporary.path(), error))?;
    let after = fs::symlink_metadata(source)
        .map_err(|error| ModError::io("reinspect source after copy", source, error))?;
    validate_regular_file(source, &after)?;
    if copied != before.len() || file_stamp(&after) != stamp {
        return Err(ModError::target(source, TargetPathErrorKind::Changed));
    }
    temporary
        .persist_noclobber(destination)
        .map_err(|error| ModError::io("publish recovery file", destination, error.error))?;
    sync_directory(parent)?;
    Ok(digest)
}

fn hash_stable_file(path: &Path, limit: u64) -> Result<FileSHA256, ModError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("inspect target before hashing", path, error))?;
    validate_regular_file(path, &before)?;
    if before.len() > limit {
        return Err(ModError::target(path, TargetPathErrorKind::TooLarge));
    }
    let stamp = file_stamp(&before);
    let mut input =
        File::open(path).map_err(|error| ModError::io("open target for hashing", path, error))?;
    let mut sink = io::sink();
    let (digest, bytes) = copy_and_hash(
        &mut input,
        &mut sink,
        limit,
        path,
        "hash game target",
        TargetPathErrorKind::TooLarge,
    )?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("reinspect target after hashing", path, error))?;
    validate_regular_file(path, &after)?;
    if bytes != before.len() || file_stamp(&after) != stamp {
        return Err(ModError::target(path, TargetPathErrorKind::Changed));
    }
    Ok(digest)
}

fn copy_and_hash(
    input: &mut impl Read,
    output: &mut impl Write,
    limit: u64,
    path: &Path,
    operation: &'static str,
    limit_kind: TargetPathErrorKind,
) -> Result<(FileSHA256, u64), ModError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES].into_boxed_slice();
    let mut bytes = 0u64;
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| ModError::io(operation, path, error))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| ModError::target(path, limit_kind))?;
        if bytes > limit {
            return Err(ModError::target(path, limit_kind));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            ModError::io(
                operation,
                path,
                io::Error::new(io::ErrorKind::InvalidData, "reader exceeded its buffer"),
            )
        })?;
        output
            .write_all(chunk)
            .map_err(|error| ModError::io(operation, path, error))?;
        hasher.update(chunk);
    }
    Ok((FileSHA256::from_bytes(hasher.finalize().into()), bytes))
}

fn publish_file(
    source: &Path,
    destination: &Path,
    expected_digest: FileSHA256,
    limit: u64,
) -> Result<(), ModError> {
    let parent = destination.parent().ok_or_else(|| {
        ModError::io(
            "resolve game target parent",
            destination,
            io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"),
        )
    })?;
    let mut input = File::open(source)
        .map_err(|error| ModError::io("open transaction source", source, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| ModError::io("create adjacent game temporary", parent, error))?;
    let (actual_digest, _) = copy_and_hash(
        &mut input,
        temporary.as_file_mut(),
        limit,
        source,
        "copy transaction file",
        TargetPathErrorKind::TooLarge,
    )?;
    if actual_digest != expected_digest {
        return Err(ModError::target(source, TargetPathErrorKind::Changed));
    }
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| ModError::io("flush adjacent game temporary", temporary.path(), error))?;
    temporary.as_file().sync_all().map_err(|error| {
        ModError::io(
            "synchronize adjacent game temporary",
            temporary.path(),
            error,
        )
    })?;
    temporary
        .persist(destination)
        .map_err(|error| ModError::io("publish game file", destination, error.error))?;
    sync_directory(parent)
}

fn validate_rollback_target(path: &Path) -> Result<(), ModError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_regular_file(path, &metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ModError::io("inspect rollback target", path, error)),
    }
}

fn validate_regular_file(path: &Path, metadata: &fs::Metadata) -> Result<(), ModError> {
    if metadata.file_type().is_symlink() {
        return Err(ModError::target(path, TargetPathErrorKind::SymbolicLink));
    }
    if !metadata.is_file() {
        return Err(ModError::target(path, TargetPathErrorKind::NotRegularFile));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: Option<u32>,
    #[cfg(windows)]
    file_index: Option<u64>,
}

fn file_stamp(metadata: &fs::Metadata) -> FileStamp {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;

    FileStamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(windows)]
        volume_serial_number: metadata.volume_serial_number(),
        #[cfg(windows)]
        file_index: metadata.file_index(),
    }
}

fn publish_control_file(parent: &Path, destination: &Path, bytes: &[u8]) -> Result<(), ModError> {
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| ModError::io("create temporary operation image", parent, error))?;
    temporary.write_all(bytes).map_err(|error| {
        ModError::io("write temporary operation image", temporary.path(), error)
    })?;
    temporary.as_file_mut().flush().map_err(|error| {
        ModError::io("flush temporary operation image", temporary.path(), error)
    })?;
    temporary.as_file().sync_all().map_err(|error| {
        ModError::io(
            "synchronize temporary operation image",
            temporary.path(),
            error,
        )
    })?;
    temporary
        .persist(destination)
        .map_err(|error| ModError::io("publish operation image", destination, error.error))?;
    sync_directory(parent)
}

fn sync_parent_directory(path: &Path) -> Result<(), ModError> {
    let parent = path.parent().ok_or_else(|| {
        ModError::io(
            "resolve parent directory",
            path,
            io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"),
        )
    })?;
    sync_directory(parent)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), ModError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| ModError::io("synchronize transaction directory", path, error))
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> Result<(), ModError> {
    Ok(())
}

fn invalid_transaction_index(path: &Path) -> ModError {
    ModError::io(
        "read transaction plan",
        path,
        io::Error::new(
            io::ErrorKind::InvalidData,
            "transaction file index is invalid",
        ),
    )
}

fn injected_error(operation: &'static str, path: &Path) -> ModError {
    ModError::io(
        operation,
        path,
        io::Error::other("injected transaction failure"),
    )
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationImage {
    format_version: u64,
    #[serde(rename = "operationID")]
    operation_id: String,
    #[serde(rename = "installationID")]
    installation_id: String,
    #[serde(rename = "packageID")]
    package_id: String,
    kind: OperationKind,
    state: OperationState,
    game: String,
    configured_root: String,
    canonical_root: String,
    root_key: String,
    installed_at: String,
    files: Vec<OperationFileImage>,
    created_directories: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum OperationKind {
    Apply,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationFileImage {
    path: String,
    #[serde(rename = "installedSHA256", skip_serializing_if = "Option::is_none")]
    installed_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_existed: Option<bool>,
    #[serde(rename = "originalSHA256", skip_serializing_if = "Option::is_none")]
    original_sha256: Option<String>,
    committed: bool,
}
