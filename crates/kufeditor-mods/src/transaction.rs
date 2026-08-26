use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zip::ZipArchive;

use crate::{
    FileSHA256, GameRoot, GameRootErrorKind, GameRootKey, InstallationID, InstalledFile, ModError,
    ModLimits, ModPackageID, ModPackageInfo, ModProgress, ModProgressPhase, ModProgressReporter,
    ModStorePaths, ModTimestamp, OperationID, PackageErrorKind, RelativeGamePath,
    TargetPathErrorKind, library::prepare_mod_store_root, manifest::game_name,
    package::inspect_package, progress::ContinueProgress,
};

const OPERATION_VERSION: u64 = 1;
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
        let (operation_id, directory) = create_operation_directory(&operations, root, package_id)?;
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
            game: game_name(self.root.game()),
            configured_root: self.root.configured_path().to_string_lossy().into_owned(),
            canonical_root: self.root.canonical_path().to_string_lossy().into_owned(),
            root_key: self.root.key().to_string(),
            installed_at: self.installed_at.as_str(),
            files: self
                .files
                .iter()
                .map(|file| OperationFileImage {
                    path: file.path.as_str(),
                    installed_sha256: file.installed_sha256.map(|digest| digest.to_string()),
                    original_existed: file.original_existed,
                    original_sha256: file.original_sha256.map(|digest| digest.to_string()),
                    committed: file.committed,
                })
                .collect(),
            created_directories: self
                .created_directories
                .iter()
                .map(RelativeGamePath::as_str)
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
    root: &GameRoot,
    package_id: ModPackageID,
) -> Result<(OperationID, PathBuf), ModError> {
    for _ in 0..128 {
        let operation_id = next_operation_id(root.key(), package_id);
        let directory = operations.join(operation_id.to_string());
        match fs::create_dir(&directory) {
            Ok(()) => return Ok((operation_id, directory)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(ModError::io("create apply operation", directory, error));
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

fn next_operation_id(root_key: GameRootKey, package_id: ModPackageID) -> OperationID {
    let sequence = NEXT_OPERATION.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut hasher = Sha256::new();
    hasher.update(b"kufeditor-operation-v1\0");
    hasher.update(root_key.as_bytes());
    hasher.update(package_id.as_bytes());
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationImage<'a> {
    format_version: u64,
    #[serde(rename = "operationID")]
    operation_id: String,
    #[serde(rename = "installationID")]
    installation_id: String,
    #[serde(rename = "packageID")]
    package_id: String,
    kind: OperationKind,
    state: OperationState,
    game: &'static str,
    configured_root: String,
    canonical_root: String,
    root_key: String,
    installed_at: &'a str,
    files: Vec<OperationFileImage<'a>>,
    created_directories: Vec<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum OperationKind {
    Apply,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationFileImage<'a> {
    path: &'a str,
    #[serde(rename = "installedSHA256", skip_serializing_if = "Option::is_none")]
    installed_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_existed: Option<bool>,
    #[serde(rename = "originalSHA256", skip_serializing_if = "Option::is_none")]
    original_sha256: Option<String>,
    committed: bool,
}
