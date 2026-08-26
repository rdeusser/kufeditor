use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use kufeditor_game::Game;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{Builder as TempBuilder, NamedTempFile};

use crate::{
    BackupErrorKind, BackupID, FileSHA256, GameRoot, GameRootKey, ModError, ModLimits, ModProgress,
    ModProgressPhase, ModProgressReporter, ModService, ModStorePaths, ModTimestamp,
    RelativeGamePath,
    library::{existing_mod_store_root, prepare_mod_store_root},
    manifest::{game_name, parse_game},
    progress::ContinueProgress,
    transaction::{OverlayTransaction, TransactionFailpoint, validate_game_root},
};

const BACKUP_VERSION: u64 = 1;
const MAX_BACKUP_METADATA_BYTES: usize = 16 * 1024 * 1024;
const MAX_BACKUP_LABEL_BYTES: usize = 256;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct CreateBackupRequest<'a> {
    root: &'a GameRoot,
    label: Option<String>,
}

impl<'a> CreateBackupRequest<'a> {
    pub fn new(root: &'a GameRoot, label: Option<String>) -> Result<Self, ModError> {
        if label
            .as_deref()
            .is_some_and(|label| label.trim().is_empty() || label.len() > MAX_BACKUP_LABEL_BYTES)
        {
            return Err(ModError::backup(
                root.canonical_path(),
                None,
                BackupErrorKind::InvalidLabel,
            ));
        }
        Ok(Self { root, label })
    }

    pub const fn root(&self) -> &GameRoot {
        self.root
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RestoreBackupRequest<'a> {
    root: &'a GameRoot,
    backup_id: BackupID,
}

impl<'a> RestoreBackupRequest<'a> {
    pub const fn new(root: &'a GameRoot, backup_id: BackupID) -> Self {
        Self { root, backup_id }
    }

    pub const fn root(&self) -> &GameRoot {
        self.root
    }

    pub const fn backup_id(&self) -> BackupID {
        self.backup_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupFileInfo {
    path: RelativeGamePath,
    bytes: u64,
    sha256: FileSHA256,
}

impl BackupFileInfo {
    pub const fn path(&self) -> &RelativeGamePath {
        &self.path
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn sha256(&self) -> FileSHA256 {
        self.sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupInfo {
    backup_id: BackupID,
    directory: PathBuf,
    label: Option<String>,
    game: Game,
    root_key: GameRootKey,
    created_at: ModTimestamp,
    file_count: u64,
    total_bytes: u64,
    content_sha256: FileSHA256,
    files: Vec<BackupFileInfo>,
}

impl BackupInfo {
    pub const fn backup_id(&self) -> BackupID {
        self.backup_id
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub const fn game(&self) -> Game {
        self.game
    }

    pub const fn root_key(&self) -> GameRootKey {
        self.root_key
    }

    pub const fn created_at(&self) -> &ModTimestamp {
        &self.created_at
    }

    pub const fn file_count(&self) -> u64 {
        self.file_count
    }

    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub const fn content_sha256(&self) -> FileSHA256 {
        self.content_sha256
    }

    pub fn files(&self) -> &[BackupFileInfo] {
        &self.files
    }
}

#[derive(Debug)]
pub struct BackupIssue {
    path: PathBuf,
    error: ModError,
}

impl BackupIssue {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn error(&self) -> &ModError {
        &self.error
    }
}

#[derive(Debug, Default)]
pub struct BackupScan {
    backups: Vec<BackupInfo>,
    issues: Vec<BackupIssue>,
}

impl BackupScan {
    pub fn backups(&self) -> &[BackupInfo] {
        &self.backups
    }

    pub fn issues(&self) -> &[BackupIssue] {
        &self.issues
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreBackupReport {
    backup_id: BackupID,
    committed_paths: Vec<RelativeGamePath>,
}

impl RestoreBackupReport {
    pub const fn backup_id(&self) -> BackupID {
        self.backup_id
    }

    pub fn committed_paths(&self) -> &[RelativeGamePath] {
        &self.committed_paths
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackupSource {
    path: RelativeGamePath,
    absolute_path: PathBuf,
    bytes: u64,
    stamp: BackupFileStamp,
}

impl ModService {
    pub fn create_backup(
        &self,
        request: CreateBackupRequest<'_>,
        progress: &mut impl ModProgressReporter,
    ) -> Result<BackupInfo, ModError> {
        validate_game_root(request.root)?;
        let sources = scan_backup_sources(request.root, &self.limits, progress)?;
        let backup_root = prepare_backup_root(&self.paths, request.root)?;
        let temporary = TempBuilder::new()
            .prefix(".backup-")
            .tempdir_in(&backup_root)
            .map_err(|error| {
                ModError::io("create temporary backup directory", &backup_root, error)
            })?;
        let files_root = temporary.path().join("files");
        fs::create_dir(&files_root)
            .map_err(|error| ModError::io("create backup payload directory", &files_root, error))?;
        let files = copy_backup_sources(&sources, &files_root, &self.limits, progress)?;
        require_unchanged_backup_sources(request.root, &sources, &self.limits)?;

        let created_at = ModTimestamp::now()?;
        let content_sha256 = backup_content_digest(&files);
        let backup_id = compute_backup_id(
            request.root.key(),
            &created_at,
            request.label.as_deref(),
            content_sha256,
        );
        let file_count = u64::try_from(files.len()).unwrap_or(u64::MAX);
        let total_bytes = checked_backup_bytes(&files, request.root.canonical_path(), backup_id)?;
        let metadata = BackupImage::from_parts(
            backup_id,
            &request,
            &created_at,
            file_count,
            total_bytes,
            content_sha256,
            &files,
        );
        write_backup_metadata(temporary.path(), &metadata)?;
        sync_directory(&files_root)?;
        sync_directory(temporary.path())?;

        let destination = publish_backup(temporary.path(), &backup_root, backup_id, progress)?;
        Ok(BackupInfo {
            backup_id,
            directory: destination,
            label: request.label,
            game: request.root.game(),
            root_key: request.root.key(),
            created_at,
            file_count,
            total_bytes,
            content_sha256,
            files,
        })
    }

    pub fn scan_backups(&self, root: &GameRoot) -> Result<BackupScan, ModError> {
        validate_game_root(root)?;
        let Some(backup_root) = existing_backup_root(&self.paths, root)? else {
            return Ok(BackupScan::default());
        };
        let mut paths = fs::read_dir(&backup_root)
            .map_err(|error| ModError::io("scan backup directory", &backup_root, error))?
            .map(|entry| {
                entry.map(|entry| entry.path()).map_err(|error| {
                    ModError::io("read backup directory entry", &backup_root, error)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        let mut scan = BackupScan::default();
        for path in paths {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                scan.issues.push(BackupIssue {
                    error: ModError::backup(&path, None, BackupErrorKind::InvalidMetadata),
                    path,
                });
                continue;
            };
            if name.starts_with(".backup-") {
                continue;
            }
            let Ok(backup_id) = BackupID::parse(name) else {
                scan.issues.push(BackupIssue {
                    error: ModError::backup(&path, None, BackupErrorKind::InvalidMetadata),
                    path,
                });
                continue;
            };
            match inspect_backup(&path, backup_id, root, &self.limits) {
                Ok(backup) => scan.backups.push(backup),
                Err(error) => scan.issues.push(BackupIssue { path, error }),
            }
        }
        scan.backups.sort_by(|left, right| {
            left.created_at
                .as_str()
                .cmp(right.created_at.as_str())
                .then_with(|| left.backup_id.cmp(&right.backup_id))
        });
        scan.issues
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(scan)
    }

    pub fn restore_backup(
        &self,
        request: RestoreBackupRequest<'_>,
        progress: &mut impl ModProgressReporter,
    ) -> Result<RestoreBackupReport, ModError> {
        self.restore_backup_with_failpoint(request, progress, &TransactionFailpoint::default())
    }

    fn restore_backup_with_failpoint(
        &self,
        request: RestoreBackupRequest<'_>,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<RestoreBackupReport, ModError> {
        validate_game_root(request.root)?;
        let Some(backup_root) = existing_backup_root(&self.paths, request.root)? else {
            return Err(ModError::backup(
                self.paths
                    .backups()
                    .join(request.root.key().to_string())
                    .join(request.backup_id.to_string()),
                Some(request.backup_id),
                BackupErrorKind::Missing,
            ));
        };
        let directory = backup_root.join(request.backup_id.to_string());
        let backup = inspect_backup(&directory, request.backup_id, request.root, &self.limits)?;
        let mut transaction = OverlayTransaction::begin_backup_restore(
            &self.paths,
            request.root,
            request.backup_id,
            backup.files(),
        )?;
        if let Err(error) =
            transaction.stage_backup(backup.directory(), &self.limits, progress, failpoint)
        {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        match inspect_backup(&directory, request.backup_id, request.root, &self.limits) {
            Ok(verified) if verified == backup => {}
            Ok(_) => {
                let error = ModError::backup(
                    &directory,
                    Some(request.backup_id),
                    BackupErrorKind::SourceChanged,
                );
                return Err(transaction.recover_error(error, progress, failpoint));
            }
            Err(error) => return Err(transaction.recover_error(error, progress, failpoint)),
        }
        if let Err(error) = transaction.create_recovery(&self.limits, progress, failpoint) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        if let Err(error) = transaction.commit(&self.limits, progress, failpoint) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        let committed_paths = transaction.committed_paths();
        transaction.finish_success()?;
        Ok(RestoreBackupReport {
            backup_id: request.backup_id,
            committed_paths,
        })
    }

    pub fn delete_backup(&self, root: &GameRoot, backup_id: BackupID) -> Result<(), ModError> {
        validate_game_root(root)?;
        let Some(backup_root) = existing_backup_root(&self.paths, root)? else {
            return Err(ModError::backup(
                self.paths
                    .backups()
                    .join(root.key().to_string())
                    .join(backup_id.to_string()),
                Some(backup_id),
                BackupErrorKind::Missing,
            ));
        };
        let directory = backup_root.join(backup_id.to_string());
        inspect_backup(&directory, backup_id, root, &self.limits)?;
        fs::remove_dir_all(&directory)
            .map_err(|error| ModError::io("delete backup directory", &directory, error))?;
        sync_directory(&backup_root)
    }
}

fn copy_backup_sources(
    sources: &[BackupSource],
    files_root: &Path,
    limits: &ModLimits,
    progress: &mut impl ModProgressReporter,
) -> Result<Vec<BackupFileInfo>, ModError> {
    let total = u64::try_from(sources.len()).unwrap_or(u64::MAX);
    let mut files = Vec::with_capacity(sources.len());
    for (index, source) in sources.iter().enumerate() {
        let destination = files_root.join(source.path.as_ref());
        let sha256 = copy_backup_source(source, &destination, limits)?;
        files.push(BackupFileInfo {
            path: source.path.clone(),
            bytes: source.bytes,
            sha256,
        });
        let completed = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::CopyingBackup,
                completed,
                total,
                path: Some(source.path.clone()),
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "backup creation",
            });
        }
    }
    Ok(files)
}

fn require_unchanged_backup_sources(
    root: &GameRoot,
    sources: &[BackupSource],
    limits: &ModLimits,
) -> Result<(), ModError> {
    let verified = scan_backup_sources(root, limits, &mut ContinueProgress)?;
    if verified == sources {
        Ok(())
    } else {
        Err(ModError::backup(
            root.canonical_path(),
            None,
            BackupErrorKind::SourceChanged,
        ))
    }
}

fn checked_backup_bytes(
    files: &[BackupFileInfo],
    root: &Path,
    backup_id: BackupID,
) -> Result<u64, ModError> {
    files.iter().try_fold(0u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or_else(|| ModError::backup(root, Some(backup_id), BackupErrorKind::TooLarge))
    })
}

fn publish_backup(
    temporary: &Path,
    backup_root: &Path,
    backup_id: BackupID,
    progress: &mut impl ModProgressReporter,
) -> Result<PathBuf, ModError> {
    if progress
        .report(&ModProgress {
            phase: ModProgressPhase::PublishingBackup,
            completed: 0,
            total: 1,
            path: None,
        })
        .is_break()
    {
        return Err(ModError::Canceled {
            operation: "backup creation",
        });
    }
    let destination = backup_root.join(backup_id.to_string());
    if destination.exists() {
        return Err(ModError::backup(
            destination,
            Some(backup_id),
            BackupErrorKind::DestinationCollision,
        ));
    }
    fs::rename(temporary, &destination)
        .map_err(|error| ModError::io("publish backup directory", &destination, error))?;
    sync_directory(backup_root)?;
    let _ = progress.report(&ModProgress {
        phase: ModProgressPhase::PublishingBackup,
        completed: 1,
        total: 1,
        path: None,
    });
    Ok(destination)
}

fn scan_backup_sources(
    root: &GameRoot,
    limits: &ModLimits,
    progress: &mut impl ModProgressReporter,
) -> Result<Vec<BackupSource>, ModError> {
    let mut directories = vec![root.canonical_path().to_path_buf()];
    let mut sources = Vec::new();
    let mut portable_paths = HashSet::new();
    let mut total_bytes = 0u64;
    let mut entry_count = 0u64;
    while let Some(directory) = directories.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| ModError::io("scan game directory for backup", &directory, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| ModError::io("read game directory entry", &directory, error))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let relative = backup_relative_path(root.canonical_path(), &path, limits)?;
            if !portable_paths.insert(relative.portable_key().to_owned()) {
                return Err(ModError::backup(path, None, BackupErrorKind::UnsafePath));
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ModError::io("inspect backup source", &path, error))?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                return Err(ModError::backup(path, None, BackupErrorKind::SymbolicLink));
            }
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(ModError::backup(
                    path,
                    None,
                    BackupErrorKind::UnsupportedObject,
                ));
            }
            entry_count = entry_count.saturating_add(1);
            if entry_count > limits.max_backup_files {
                return Err(ModError::backup(
                    root.canonical_path(),
                    None,
                    BackupErrorKind::TooManyFiles,
                ));
            }
            if metadata.len() > limits.max_file_bytes {
                return Err(ModError::backup(path, None, BackupErrorKind::TooLarge));
            }
            total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
                ModError::backup(root.canonical_path(), None, BackupErrorKind::TooLarge)
            })?;
            if total_bytes > limits.max_backup_bytes {
                return Err(ModError::backup(
                    root.canonical_path(),
                    None,
                    BackupErrorKind::TooLarge,
                ));
            }
            sources.push(BackupSource {
                path: relative.clone(),
                absolute_path: path,
                bytes: metadata.len(),
                stamp: backup_file_stamp(&metadata),
            });
            if progress
                .report(&ModProgress {
                    phase: ModProgressPhase::ScanningBackup,
                    completed: entry_count,
                    total: limits.max_backup_files,
                    path: Some(relative),
                })
                .is_break()
            {
                return Err(ModError::Canceled {
                    operation: "backup creation",
                });
            }
        }
    }
    sources.sort_by(|left, right| left.path.portable_key().cmp(right.path.portable_key()));
    Ok(sources)
}

fn copy_backup_source(
    source: &BackupSource,
    destination: &Path,
    limits: &ModLimits,
) -> Result<FileSHA256, ModError> {
    let current = resolve_backup_source(&source.absolute_path, &source.path)?;
    let before = fs::symlink_metadata(&current)
        .map_err(|error| ModError::io("inspect backup source before copy", &current, error))?;
    if !before.is_file() || backup_file_stamp(&before) != source.stamp {
        return Err(ModError::backup(
            current,
            None,
            BackupErrorKind::SourceChanged,
        ));
    }
    let mut input = File::open(&current)
        .map_err(|error| ModError::io("open backup source", &current, error))?;
    let opened = input
        .metadata()
        .map_err(|error| ModError::io("inspect open backup source", &current, error))?;
    if backup_file_stamp(&opened) != source.stamp {
        return Err(ModError::backup(
            current,
            None,
            BackupErrorKind::SourceChanged,
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        ModError::io(
            "resolve backup destination parent",
            destination,
            io::Error::new(io::ErrorKind::InvalidInput, "backup file has no parent"),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| ModError::io("create backup destination parent", parent, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| ModError::io("create temporary backup file", parent, error))?;
    let (sha256, copied) = copy_and_hash_backup(
        &mut input,
        temporary.as_file_mut(),
        limits.max_file_bytes,
        &current,
    )?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| ModError::io("flush backup file", temporary.path(), error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| ModError::io("synchronize backup file", temporary.path(), error))?;
    let after = fs::symlink_metadata(&current)
        .map_err(|error| ModError::io("reinspect backup source", &current, error))?;
    if copied != source.bytes || backup_file_stamp(&after) != source.stamp {
        return Err(ModError::backup(
            current,
            None,
            BackupErrorKind::SourceChanged,
        ));
    }
    temporary
        .persist_noclobber(destination)
        .map_err(|error| ModError::io("publish backup file", destination, error.error))?;
    sync_directory(parent)?;
    Ok(sha256)
}

fn resolve_backup_source(
    absolute_path: &Path,
    relative_path: &RelativeGamePath,
) -> Result<PathBuf, ModError> {
    let Some(root) = absolute_path
        .ancestors()
        .nth(relative_path.component_count())
    else {
        return Err(ModError::backup(
            absolute_path,
            None,
            BackupErrorKind::SourceChanged,
        ));
    };
    let mut current = root.to_path_buf();
    for (index, component) in relative_path.as_str().split('/').enumerate() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| ModError::backup(&current, None, BackupErrorKind::SourceChanged))?;
        if metadata.file_type().is_symlink() {
            return Err(ModError::backup(
                current,
                None,
                BackupErrorKind::SymbolicLink,
            ));
        }
        let is_file = index.saturating_add(1) == relative_path.component_count();
        if (is_file && !metadata.is_file()) || (!is_file && !metadata.is_dir()) {
            return Err(ModError::backup(
                current,
                None,
                BackupErrorKind::SourceChanged,
            ));
        }
    }
    Ok(current)
}

fn inspect_backup(
    directory: &Path,
    expected_id: BackupID,
    root: &GameRoot,
    limits: &ModLimits,
) -> Result<BackupInfo, ModError> {
    require_backup_directory(directory, Some(expected_id))?;
    let metadata_path = directory.join("backup-v1.json");
    let image = read_backup_image(&metadata_path, Some(expected_id))?;
    let (game, created_at) = validate_backup_metadata(&image, &metadata_path, expected_id, root)?;
    let (files, total_bytes, content_sha256) =
        validate_backup_payload(directory, &image, &metadata_path, expected_id, limits)?;
    if compute_backup_id(
        root.key(),
        &created_at,
        image.label.as_deref(),
        content_sha256,
    ) != expected_id
    {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::IDMismatch,
        ));
    }
    Ok(BackupInfo {
        backup_id: expected_id,
        directory: directory.to_path_buf(),
        label: image.label,
        game,
        root_key: root.key(),
        created_at,
        file_count: image.file_count,
        total_bytes,
        content_sha256,
        files,
    })
}

fn validate_backup_metadata(
    image: &BackupImage,
    metadata_path: &Path,
    expected_id: BackupID,
    root: &GameRoot,
) -> Result<(Game, ModTimestamp), ModError> {
    if image.format_version != BACKUP_VERSION {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::UnsupportedVersion,
        ));
    }
    let game = parse_game(&image.game).map_err(|_| {
        ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::InvalidMetadata,
        )
    })?;
    if game != root.game()
        || GameRootKey::parse(&image.root_key).ok() != Some(root.key())
        || image.canonical_root != root.canonical_path().to_string_lossy()
    {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::WrongRoot,
        ));
    }
    if image.configured_root.is_empty()
        || image
            .label
            .as_deref()
            .is_some_and(|label| label.trim().is_empty() || label.len() > MAX_BACKUP_LABEL_BYTES)
    {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::InvalidMetadata,
        ));
    }
    let backup_id = BackupID::parse(&image.backup_id).map_err(|_| {
        ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::InvalidMetadata,
        )
    })?;
    if backup_id != expected_id {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::IDMismatch,
        ));
    }
    let created_at = ModTimestamp::parse(&image.created_at).map_err(|_| {
        ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::InvalidMetadata,
        )
    })?;
    Ok((game, created_at))
}

fn validate_backup_payload(
    directory: &Path,
    image: &BackupImage,
    metadata_path: &Path,
    expected_id: BackupID,
    limits: &ModLimits,
) -> Result<(Vec<BackupFileInfo>, u64, FileSHA256), ModError> {
    let files = scan_backup_payload(&directory.join("files"), limits, Some(expected_id))?;
    let expected_files = parse_backup_files(&image.files, limits, metadata_path, expected_id)?;
    let total_bytes = files.iter().try_fold(0u64, |total, file| {
        total.checked_add(file.bytes).ok_or_else(|| {
            ModError::backup(metadata_path, Some(expected_id), BackupErrorKind::TooLarge)
        })
    })?;
    if files != expected_files
        || image.file_count != u64::try_from(files.len()).unwrap_or(u64::MAX)
        || image.total_bytes != total_bytes
    {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::PayloadMismatch,
        ));
    }
    let content_sha256 = backup_content_digest(&files);
    if FileSHA256::parse(&image.content_sha256).ok() != Some(content_sha256) {
        return Err(ModError::backup(
            metadata_path,
            Some(expected_id),
            BackupErrorKind::PayloadMismatch,
        ));
    }
    Ok((files, total_bytes, content_sha256))
}

fn scan_backup_payload(
    root: &Path,
    limits: &ModLimits,
    backup_id: Option<BackupID>,
) -> Result<Vec<BackupFileInfo>, ModError> {
    require_backup_directory(root, backup_id)?;
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut portable_paths = HashSet::new();
    let mut total_bytes = 0u64;
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| ModError::io("scan backup payload", &directory, error))?;
        for entry in entries {
            let entry = entry
                .map_err(|error| ModError::io("read backup payload entry", &directory, error))?;
            let path = entry.path();
            let relative = payload_relative_path(root, &path, limits, backup_id)?;
            if !portable_paths.insert(relative.portable_key().to_owned()) {
                return Err(ModError::backup(
                    path,
                    backup_id,
                    BackupErrorKind::PayloadMismatch,
                ));
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ModError::io("inspect backup payload", &path, error))?;
            if metadata.file_type().is_symlink() {
                return Err(ModError::backup(
                    path,
                    backup_id,
                    BackupErrorKind::SymbolicLink,
                ));
            }
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(ModError::backup(
                    path,
                    backup_id,
                    BackupErrorKind::UnsupportedObject,
                ));
            }
            if u64::try_from(files.len()).unwrap_or(u64::MAX) >= limits.max_backup_files
                || metadata.len() > limits.max_file_bytes
            {
                return Err(ModError::backup(
                    path,
                    backup_id,
                    if metadata.len() > limits.max_file_bytes {
                        BackupErrorKind::TooLarge
                    } else {
                        BackupErrorKind::TooManyFiles
                    },
                ));
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| ModError::backup(&path, backup_id, BackupErrorKind::TooLarge))?;
            if total_bytes > limits.max_backup_bytes {
                return Err(ModError::backup(path, backup_id, BackupErrorKind::TooLarge));
            }
            let sha256 = hash_backup_file(&path, limits.max_file_bytes, backup_id)?;
            files.push(BackupFileInfo {
                path: relative,
                bytes: metadata.len(),
                sha256,
            });
        }
    }
    files.sort_by(|left, right| left.path.portable_key().cmp(right.path.portable_key()));
    Ok(files)
}

fn parse_backup_files(
    images: &[BackupFileImage],
    limits: &ModLimits,
    metadata_path: &Path,
    backup_id: BackupID,
) -> Result<Vec<BackupFileInfo>, ModError> {
    if u64::try_from(images.len()).unwrap_or(u64::MAX) > limits.max_backup_files {
        return Err(ModError::backup(
            metadata_path,
            Some(backup_id),
            BackupErrorKind::TooManyFiles,
        ));
    }
    let mut files = Vec::with_capacity(images.len());
    let mut portable_paths = HashSet::with_capacity(images.len());
    for image in images {
        let path = RelativeGamePath::parse(&image.path, limits).map_err(|_| {
            ModError::backup(
                metadata_path,
                Some(backup_id),
                BackupErrorKind::InvalidMetadata,
            )
        })?;
        if !portable_paths.insert(path.portable_key().to_owned()) {
            return Err(ModError::backup(
                metadata_path,
                Some(backup_id),
                BackupErrorKind::InvalidMetadata,
            ));
        }
        let sha256 = FileSHA256::parse(&image.sha256).map_err(|_| {
            ModError::backup(
                metadata_path,
                Some(backup_id),
                BackupErrorKind::InvalidMetadata,
            )
        })?;
        files.push(BackupFileInfo {
            path,
            bytes: image.bytes,
            sha256,
        });
    }
    files.sort_by(|left, right| left.path.portable_key().cmp(right.path.portable_key()));
    Ok(files)
}

fn backup_relative_path(
    root: &Path,
    path: &Path,
    limits: &ModLimits,
) -> Result<RelativeGamePath, ModError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ModError::backup(path, None, BackupErrorKind::SourceChanged))?;
    let value = relative
        .to_str()
        .ok_or_else(|| ModError::backup(path, None, BackupErrorKind::UnsafePath))?;
    RelativeGamePath::parse(&value.replace('\\', "/"), limits)
        .map_err(|_| ModError::backup(path, None, BackupErrorKind::UnsafePath))
}

fn payload_relative_path(
    root: &Path,
    path: &Path,
    limits: &ModLimits,
    backup_id: Option<BackupID>,
) -> Result<RelativeGamePath, ModError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ModError::backup(path, backup_id, BackupErrorKind::PayloadMismatch))?;
    let value = relative
        .to_str()
        .ok_or_else(|| ModError::backup(path, backup_id, BackupErrorKind::UnsafePath))?;
    RelativeGamePath::parse(&value.replace('\\', "/"), limits)
        .map_err(|_| ModError::backup(path, backup_id, BackupErrorKind::UnsafePath))
}

fn copy_and_hash_backup(
    input: &mut impl Read,
    output: &mut impl Write,
    limit: u64,
    path: &Path,
) -> Result<(FileSHA256, u64), ModError> {
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES].into_boxed_slice();
    let mut hasher = Sha256::new();
    let mut bytes = 0u64;
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| ModError::io("read backup file", path, error))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| ModError::backup(path, None, BackupErrorKind::TooLarge))?;
        if bytes > limit {
            return Err(ModError::backup(path, None, BackupErrorKind::TooLarge));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            ModError::io(
                "validate backup read length",
                path,
                io::Error::new(io::ErrorKind::InvalidData, "reader exceeded its buffer"),
            )
        })?;
        output
            .write_all(chunk)
            .map_err(|error| ModError::io("write backup file", path, error))?;
        hasher.update(chunk);
    }
    Ok((FileSHA256::from_bytes(hasher.finalize().into()), bytes))
}

fn hash_backup_file(
    path: &Path,
    limit: u64,
    backup_id: Option<BackupID>,
) -> Result<FileSHA256, ModError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("inspect backup file before hashing", path, error))?;
    if before.file_type().is_symlink() {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::SymbolicLink,
        ));
    }
    if !before.is_file() || before.len() > limit {
        return Err(ModError::backup(
            path,
            backup_id,
            if before.len() > limit {
                BackupErrorKind::TooLarge
            } else {
                BackupErrorKind::PayloadMismatch
            },
        ));
    }
    let stamp = backup_file_stamp(&before);
    let mut file = File::open(path)
        .map_err(|error| ModError::io("open backup file for hashing", path, error))?;
    let opened = file
        .metadata()
        .map_err(|error| ModError::io("inspect open backup file", path, error))?;
    if backup_file_stamp(&opened) != stamp {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::SourceChanged,
        ));
    }
    let mut sink = io::sink();
    let (digest, bytes) = copy_and_hash_backup(&mut file, &mut sink, limit, path)?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("reinspect backup file", path, error))?;
    if bytes != before.len() || backup_file_stamp(&after) != stamp {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::SourceChanged,
        ));
    }
    Ok(digest)
}

fn backup_content_digest(files: &[BackupFileInfo]) -> FileSHA256 {
    let mut hasher = Sha256::new();
    hasher.update(b"kufeditor-backup-content-v1\0");
    for file in files {
        let path = file.path.as_str().as_bytes();
        hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(path);
        hasher.update(file.bytes.to_le_bytes());
        hasher.update(file.sha256.as_bytes());
    }
    FileSHA256::from_bytes(hasher.finalize().into())
}

fn compute_backup_id(
    root_key: GameRootKey,
    created_at: &ModTimestamp,
    label: Option<&str>,
    content_sha256: FileSHA256,
) -> BackupID {
    let mut hasher = Sha256::new();
    hasher.update(b"kufeditor-backup-v1\0");
    hasher.update(root_key.as_bytes());
    hasher.update(created_at.as_str().as_bytes());
    hasher.update([0]);
    if let Some(label) = label {
        hasher.update(label.as_bytes());
    }
    hasher.update([0]);
    hasher.update(content_sha256.as_bytes());
    BackupID::from_bytes(hasher.finalize().into())
}

fn prepare_backup_root(paths: &ModStorePaths, root: &GameRoot) -> Result<PathBuf, ModError> {
    prepare_mod_store_root(paths)?;
    let backups = paths.backups();
    prepare_backup_directory(&backups)?;
    let root_directory = backups.join(root.key().to_string());
    prepare_backup_directory(&root_directory)?;
    Ok(root_directory)
}

fn existing_backup_root(
    paths: &ModStorePaths,
    root: &GameRoot,
) -> Result<Option<PathBuf>, ModError> {
    if existing_mod_store_root(paths)?.is_none() {
        return Ok(None);
    }
    let backups = paths.backups();
    if !existing_backup_directory(&backups)? {
        return Ok(None);
    }
    let root_directory = backups.join(root.key().to_string());
    existing_backup_directory(&root_directory).map(|exists| exists.then_some(root_directory))
}

fn prepare_backup_directory(path: &Path) -> Result<(), ModError> {
    if existing_backup_directory(path)? {
        return Ok(());
    }
    fs::create_dir(path)
        .map_err(|error| ModError::io("create backup storage directory", path, error))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("inspect created backup storage", path, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ModError::backup(path, None, BackupErrorKind::NotDirectory));
    }
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn existing_backup_directory(path: &Path) -> Result<bool, ModError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(ModError::backup(path, None, BackupErrorKind::SymbolicLink));
            }
            if !metadata.is_dir() {
                return Err(ModError::backup(path, None, BackupErrorKind::NotDirectory));
            }
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ModError::io("inspect backup storage", path, error)),
    }
}

fn require_backup_directory(path: &Path, backup_id: Option<BackupID>) -> Result<(), ModError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::SymbolicLink,
        )),
        Ok(metadata) if !metadata.is_dir() => Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::NotDirectory,
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(ModError::backup(path, backup_id, BackupErrorKind::Missing))
        }
        Err(error) => Err(ModError::io("inspect backup directory", path, error)),
    }
}

fn write_backup_metadata(directory: &Path, image: &BackupImage) -> Result<(), ModError> {
    let mut bytes = serde_json::to_vec_pretty(image).map_err(|error| {
        ModError::io(
            "serialize backup metadata",
            directory,
            io::Error::new(io::ErrorKind::InvalidData, error),
        )
    })?;
    bytes.push(b'\n');
    if bytes.len() > MAX_BACKUP_METADATA_BYTES {
        return Err(ModError::backup(directory, None, BackupErrorKind::TooLarge));
    }
    let path = directory.join("backup-v1.json");
    let mut temporary = NamedTempFile::new_in(directory)
        .map_err(|error| ModError::io("create temporary backup metadata", directory, error))?;
    temporary
        .write_all(&bytes)
        .map_err(|error| ModError::io("write backup metadata", temporary.path(), error))?;
    temporary
        .as_file_mut()
        .flush()
        .map_err(|error| ModError::io("flush backup metadata", temporary.path(), error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| ModError::io("synchronize backup metadata", temporary.path(), error))?;
    temporary
        .persist_noclobber(&path)
        .map_err(|error| ModError::io("publish backup metadata", &path, error.error))?;
    sync_directory(directory)
}

fn read_backup_image(path: &Path, backup_id: Option<BackupID>) -> Result<BackupImage, ModError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ModError::backup(path, backup_id, BackupErrorKind::InvalidMetadata)
        } else {
            ModError::io("inspect backup metadata", path, error)
        }
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_BACKUP_METADATA_BYTES as u64
    {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::InvalidMetadata,
        ));
    }
    let before_stamp = backup_file_stamp(&metadata);
    let file =
        File::open(path).map_err(|error| ModError::io("open backup metadata", path, error))?;
    let opened = file
        .metadata()
        .map_err(|error| ModError::io("inspect open backup metadata", path, error))?;
    if backup_file_stamp(&opened) != before_stamp {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::SourceChanged,
        ));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(MAX_BACKUP_METADATA_BYTES)
            .min(MAX_BACKUP_METADATA_BYTES),
    );
    file.take((MAX_BACKUP_METADATA_BYTES as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ModError::io("read backup metadata", path, error))?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| ModError::io("reinspect backup metadata", path, error))?;
    if bytes.len() > MAX_BACKUP_METADATA_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len()
        || backup_file_stamp(&after) != before_stamp
    {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::SourceChanged,
        ));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| ModError::backup(path, backup_id, BackupErrorKind::InvalidMetadata))?;
    let Some(version) = value.get("formatVersion").and_then(Value::as_u64) else {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::InvalidMetadata,
        ));
    };
    if version != BACKUP_VERSION {
        return Err(ModError::backup(
            path,
            backup_id,
            BackupErrorKind::UnsupportedVersion,
        ));
    }
    serde_json::from_value(value)
        .map_err(|_| ModError::backup(path, backup_id, BackupErrorKind::InvalidMetadata))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BackupFileStamp {
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

fn backup_file_stamp(metadata: &fs::Metadata) -> BackupFileStamp {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;

    BackupFileStamp {
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

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), ModError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| ModError::io("synchronize backup directory", path, error))
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> Result<(), ModError> {
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupImage {
    format_version: u64,
    #[serde(rename = "backupID")]
    backup_id: String,
    label: Option<String>,
    game: String,
    configured_root: String,
    canonical_root: String,
    root_key: String,
    created_at: String,
    file_count: u64,
    total_bytes: u64,
    #[serde(rename = "contentSHA256")]
    content_sha256: String,
    files: Vec<BackupFileImage>,
}

impl BackupImage {
    fn from_parts(
        backup_id: BackupID,
        request: &CreateBackupRequest<'_>,
        created_at: &ModTimestamp,
        file_count: u64,
        total_bytes: u64,
        content_sha256: FileSHA256,
        files: &[BackupFileInfo],
    ) -> Self {
        Self {
            format_version: BACKUP_VERSION,
            backup_id: backup_id.to_string(),
            label: request.label.clone(),
            game: game_name(request.root.game()).to_owned(),
            configured_root: request
                .root
                .configured_path()
                .to_string_lossy()
                .into_owned(),
            canonical_root: request.root.canonical_path().to_string_lossy().into_owned(),
            root_key: request.root.key().to_string(),
            created_at: created_at.as_str().to_owned(),
            file_count,
            total_bytes,
            content_sha256: content_sha256.to_string(),
            files: files
                .iter()
                .map(|file| BackupFileImage {
                    path: file.path.as_str().to_owned(),
                    bytes: file.bytes,
                    sha256: file.sha256.to_string(),
                })
                .collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct BackupFileImage {
    path: String,
    bytes: u64,
    #[serde(rename = "SHA256")]
    sha256: String,
}

#[cfg(test)]
mod tests {
    use std::{fs, ops::ControlFlow};

    use kufeditor_game::Game;
    use tempfile::{TempDir, tempdir};

    use super::{CreateBackupRequest, ModService, RestoreBackupRequest};
    use crate::{
        BackupInfo, GameRoot, ModError, ModProgress, ModProgressPhase, ModProgressReporter,
        ModStorePaths, RecoveryReport, RelativeGamePath, transaction::TransactionFailpoint,
    };

    struct ContinueProgress;

    impl ModProgressReporter for ContinueProgress {
        fn report(&mut self, _: &ModProgress) -> ControlFlow<()> {
            ControlFlow::Continue(())
        }
    }

    #[test]
    fn restore_failures_before_game_writes_remove_the_temporary_operation()
    -> Result<(), Box<dyn std::error::Error>> {
        for state in [
            crate::OperationState::Planned,
            crate::OperationState::Staged,
            crate::OperationState::Recoverable,
        ] {
            let fixture = RestoreFixture::new()?;

            let result = fixture.service.restore_backup_with_failpoint(
                RestoreBackupRequest::new(&fixture.root, fixture.backup.backup_id()),
                &mut ContinueProgress,
                &TransactionFailpoint::after_state(state),
            );

            assert!(result.is_err(), "{state:?} failpoint must stop restore");
            fixture.assert_current_game()?;
            assert_eq!(fs::read_dir(fixture.stores.operations())?.count(), 0);
        }
        Ok(())
    }

    #[test]
    fn every_restore_commit_failure_rolls_back_exact_paths_in_reverse_order()
    -> Result<(), Box<dyn std::error::Error>> {
        for committed_count in 1..=3 {
            let fixture = RestoreFixture::new()?;

            let error = fixture
                .service
                .restore_backup_with_failpoint(
                    RestoreBackupRequest::new(&fixture.root, fixture.backup.backup_id()),
                    &mut ContinueProgress,
                    &TransactionFailpoint::after_committed_paths(committed_count),
                )
                .expect_err("the restore commit failpoint must stop restore");
            let recovery = recovery(&error)?;

            fixture.assert_current_game()?;
            assert_eq!(
                path_strings(recovery.committed()),
                ["a.sox", "b.sox", "c.sox"]
                    .get(..committed_count)
                    .ok_or("invalid restore committed fixture range")?
            );
            assert_eq!(
                path_strings(recovery.rolled_back()),
                ["c.sox", "b.sox", "a.sox"]
                    .get((3 - committed_count)..)
                    .ok_or("invalid restore rollback fixture range")?
            );
            assert!(recovery.rollback_failed().is_empty());
            assert_eq!(
                path_strings(recovery.unchanged()),
                ["a.sox", "b.sox", "c.sox"]
                    .get(committed_count..)
                    .ok_or("invalid restore unchanged fixture range")?
            );
            fixture.assert_retained_operation()?;
        }
        Ok(())
    }

    #[test]
    fn restore_rollback_failure_reports_one_path_and_continues_other_rollbacks()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = RestoreFixture::new()?;
        let failpoint = TransactionFailpoint::after_committed_paths(3).with_rollback_attempt(2);

        let error = fixture
            .service
            .restore_backup_with_failpoint(
                RestoreBackupRequest::new(&fixture.root, fixture.backup.backup_id()),
                &mut ContinueProgress,
                &failpoint,
            )
            .expect_err("the restore commit failpoint must stop restore");
        let recovery = recovery(&error)?;

        assert_eq!(fs::read(fixture.root_path.join("a.sox"))?, b"current-a");
        assert_eq!(fs::read(fixture.root_path.join("b.sox"))?, b"backup-b");
        assert_eq!(fs::read(fixture.root_path.join("c.sox"))?, b"current-c");
        assert_eq!(path_strings(recovery.rolled_back()), ["c.sox", "a.sox"]);
        assert_eq!(path_strings(recovery.rollback_failed()), ["b.sox"]);
        fixture.assert_retained_operation()?;
        Ok(())
    }

    #[test]
    fn restore_cancellation_cleans_before_writes_and_rolls_back_after_writes()
    -> Result<(), Box<dyn std::error::Error>> {
        for (phase, completed, retains_operation) in [
            (ModProgressPhase::StagingBackupRestore, 1, false),
            (ModProgressPhase::CreatingRestoreRecovery, 1, false),
            (ModProgressPhase::RestoringBackup, 0, false),
            (ModProgressPhase::RestoringBackup, 1, true),
        ] {
            let fixture = RestoreFixture::new()?;
            let mut progress = CancelAt { phase, completed };

            let error = fixture
                .service
                .restore_backup(
                    RestoreBackupRequest::new(&fixture.root, fixture.backup.backup_id()),
                    &mut progress,
                )
                .expect_err("the selected progress point must cancel restore");

            fixture.assert_current_game()?;
            assert_eq!(error.recovery_report().is_some(), retains_operation);
            assert_eq!(
                fs::read_dir(fixture.stores.operations())?.count(),
                usize::from(retains_operation)
            );
        }
        Ok(())
    }

    fn recovery(error: &ModError) -> Result<&RecoveryReport, Box<dyn std::error::Error>> {
        error
            .recovery_report()
            .ok_or_else(|| "missing backup-restore recovery report".into())
    }

    fn path_strings(paths: &[RelativeGamePath]) -> Vec<&str> {
        paths.iter().map(RelativeGamePath::as_str).collect()
    }

    struct CancelAt {
        phase: ModProgressPhase,
        completed: u64,
    }

    impl ModProgressReporter for CancelAt {
        fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
            if progress.phase == self.phase && progress.completed == self.completed {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    struct RestoreFixture {
        _directory: TempDir,
        stores: ModStorePaths,
        service: ModService,
        root_path: std::path::PathBuf,
        root: GameRoot,
        backup: BackupInfo,
    }

    impl RestoreFixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let directory = tempdir()?;
            let stores = ModStorePaths::new(directory.path().join("application-data"));
            let root_path = directory.path().join("game");
            fs::create_dir(&root_path)?;
            fs::write(root_path.join("a.sox"), b"backup-a")?;
            fs::write(root_path.join("b.sox"), b"backup-b")?;
            fs::write(root_path.join("c.sox"), b"backup-c")?;
            let root = GameRoot::inspect(Game::Heroes, root_path.clone(), &stores)?;
            let service = ModService::new(stores.clone());
            let backup = service.create_backup(
                CreateBackupRequest::new(&root, None)?,
                &mut ContinueProgress,
            )?;
            fs::write(root_path.join("a.sox"), b"current-a")?;
            fs::remove_file(root_path.join("b.sox"))?;
            fs::write(root_path.join("c.sox"), b"current-c")?;
            Ok(Self {
                _directory: directory,
                stores,
                service,
                root_path,
                root,
                backup,
            })
        }

        fn assert_current_game(&self) -> Result<(), Box<dyn std::error::Error>> {
            assert_eq!(fs::read(self.root_path.join("a.sox"))?, b"current-a");
            assert!(!self.root_path.join("b.sox").exists());
            assert_eq!(fs::read(self.root_path.join("c.sox"))?, b"current-c");
            Ok(())
        }

        fn assert_retained_operation(&self) -> Result<(), Box<dyn std::error::Error>> {
            let operation = fs::read_dir(self.stores.operations())?
                .next()
                .transpose()?
                .ok_or("missing retained restore operation")?
                .path();
            assert!(operation.join("restore-v1.json").is_file());
            assert!(operation.join("staged/a.sox").is_file());
            assert!(operation.join("before/a.sox").is_file());
            Ok(())
        }
    }
}
