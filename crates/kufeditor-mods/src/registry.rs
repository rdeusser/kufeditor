use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use kufeditor_game::Game;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{
    FileSHA256, GameRoot, GameRootKey, InstallationID, InstalledFileErrorKind, ModError, ModLimits,
    ModMetadata, ModPackageID, ModService, ModStorePaths, ModTimestamp, OperationID,
    PackageErrorKind, RegistryErrorKind, RelativeGamePath,
    library::{existing_mod_store_root, existing_package_directory, prepare_mod_store_root},
    manifest::{game_name, parse_game},
    package::inspect_package,
    progress::ContinueProgress,
};

const REGISTRY_VERSION: u64 = 1;
const MAX_REGISTRY_BYTES: usize = 16 * 1024 * 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstalledModStatus {
    Clean,
    Modified,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledFile {
    path: RelativeGamePath,
    installed_sha256: FileSHA256,
    original_existed: bool,
}

impl InstalledFile {
    pub const fn path(&self) -> &RelativeGamePath {
        &self.path
    }

    pub const fn installed_sha256(&self) -> FileSHA256 {
        self.installed_sha256
    }

    pub const fn original_existed(&self) -> bool {
        self.original_existed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstallationRecord {
    pub(crate) installation_id: InstallationID,
    pub(crate) package_id: ModPackageID,
    pub(crate) metadata: ModMetadata,
    pub(crate) game: Game,
    pub(crate) configured_root: PathBuf,
    pub(crate) canonical_root: PathBuf,
    pub(crate) root_key: GameRootKey,
    pub(crate) installed_at: ModTimestamp,
    pub(crate) operation_id: OperationID,
    pub(crate) files: Vec<InstalledFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InstallationPlanConflict {
    DuplicateName {
        installation_id: InstallationID,
    },
    PathOverlap {
        installation_id: InstallationID,
        path: RelativeGamePath,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledMod {
    record: InstallationRecord,
    status: Option<InstalledModStatus>,
}

impl InstalledMod {
    pub const fn installation_id(&self) -> InstallationID {
        self.record.installation_id
    }

    pub const fn package_id(&self) -> ModPackageID {
        self.record.package_id
    }

    pub const fn metadata(&self) -> &ModMetadata {
        &self.record.metadata
    }

    pub const fn game(&self) -> Game {
        self.record.game
    }

    pub fn configured_root(&self) -> &Path {
        &self.record.configured_root
    }

    pub fn canonical_root(&self) -> &Path {
        &self.record.canonical_root
    }

    pub const fn root_key(&self) -> GameRootKey {
        self.record.root_key
    }

    pub const fn installed_at(&self) -> &ModTimestamp {
        &self.record.installed_at
    }

    pub const fn operation_id(&self) -> OperationID {
        self.record.operation_id
    }

    pub fn files(&self) -> &[InstalledFile] {
        &self.record.files
    }

    pub const fn status(&self) -> Option<InstalledModStatus> {
        self.status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationIssueKind {
    InvalidRecord,
    DuplicateInstallationID,
    DuplicateName,
    PathConflict,
    Health,
}

#[derive(Debug)]
pub struct InstallationIssue {
    kind: InstallationIssueKind,
    record_index: usize,
    installation_id: Option<InstallationID>,
    path: Option<PathBuf>,
    error: ModError,
}

impl InstallationIssue {
    pub const fn kind(&self) -> InstallationIssueKind {
        self.kind
    }

    pub const fn record_index(&self) -> usize {
        self.record_index
    }

    pub const fn installation_id(&self) -> Option<InstallationID> {
        self.installation_id
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub const fn error(&self) -> &ModError {
        &self.error
    }
}

#[derive(Debug, Default)]
pub struct InstallationScan {
    installations: Vec<InstalledMod>,
    issues: Vec<InstallationIssue>,
}

impl InstallationScan {
    pub fn installations(&self) -> &[InstalledMod] {
        &self.installations
    }

    pub fn issues(&self) -> &[InstallationIssue] {
        &self.issues
    }
}

struct ParsedRegistry {
    records: Vec<(usize, InstallationRecord)>,
    issues: Vec<InstallationIssue>,
}

impl ModService {
    pub fn scan_installations(&self, root: &GameRoot) -> Result<InstallationScan, ModError> {
        let mut registry = read_registry(&self.paths, &self.limits)?;
        let mut installations = Vec::new();
        for (record_index, record) in registry.records {
            if record.root_key != root.key() {
                continue;
            }
            let installation_id = record.installation_id;
            let status = match installation_health(&record, root, &self.limits) {
                Ok(status) => Some(status),
                Err((path, error)) => {
                    registry.issues.push(InstallationIssue {
                        kind: InstallationIssueKind::Health,
                        record_index,
                        installation_id: Some(installation_id),
                        path: Some(path),
                        error,
                    });
                    None
                }
            };
            installations.push(InstalledMod { record, status });
        }
        installations.sort_by(|left, right| {
            left.metadata()
                .name()
                .cmp(right.metadata().name())
                .then_with(|| left.metadata().version().cmp(right.metadata().version()))
                .then_with(|| left.installation_id().cmp(&right.installation_id()))
        });
        registry
            .issues
            .sort_by_key(|issue| (issue.record_index, issue.kind as u8));
        Ok(InstallationScan {
            installations,
            issues: registry.issues,
        })
    }

    pub fn remove_package(&self, package: ModPackageID) -> Result<(), ModError> {
        let records = load_installation_records(&self.paths, &self.limits)?;
        if records
            .iter()
            .any(|installation| installation.package_id == package)
        {
            return Err(ModError::package(
                self.paths.packages().join(format!("{package}.zip")),
                None,
                PackageErrorKind::ReferencedPackage,
            ));
        }

        let destination = self.paths.packages().join(format!("{package}.zip"));
        let Some(_) = existing_package_directory(&self.paths)? else {
            return Err(ModError::package(
                destination,
                None,
                PackageErrorKind::MissingLibraryPackage,
            ));
        };
        match fs::symlink_metadata(&destination) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ModError::package(
                    destination,
                    None,
                    PackageErrorKind::MissingLibraryPackage,
                ));
            }
            Err(error) => {
                return Err(ModError::io(
                    "inspect package before removal",
                    destination,
                    error,
                ));
            }
        }
        let inspected = inspect_package(&destination, &self.limits, &mut ContinueProgress)?;
        if inspected.package_id() != package {
            return Err(ModError::package(
                destination,
                None,
                PackageErrorKind::DestinationCollision,
            ));
        }
        fs::remove_file(&destination)
            .map_err(|error| ModError::io("remove library package", destination, error))
    }
}

#[allow(dead_code, reason = "used by the apply transaction in Task 5")]
pub(crate) fn load_installation_records(
    paths: &ModStorePaths,
    limits: &ModLimits,
) -> Result<Vec<InstallationRecord>, ModError> {
    let registry = read_registry(paths, limits)?;
    if !registry.issues.is_empty() {
        return Err(ModError::registry(
            paths.installation_registry(),
            RegistryErrorKind::InvalidRecord,
        ));
    }
    let mut records = registry
        .records
        .into_iter()
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    records.sort_by_key(|record| record.installation_id);
    Ok(records)
}

#[allow(dead_code, reason = "used by the apply transaction in Task 5")]
pub(crate) fn installation_plan_conflict(
    records: &[InstallationRecord],
    root_key: GameRootKey,
    name: &str,
    files: &[RelativeGamePath],
) -> Option<InstallationPlanConflict> {
    let name_key = name.to_lowercase();
    let candidate_paths = files
        .iter()
        .map(|path| (path.portable_key(), path))
        .collect::<HashMap<_, _>>();
    for record in records.iter().filter(|record| record.root_key == root_key) {
        if record.metadata.name().to_lowercase() == name_key {
            return Some(InstallationPlanConflict::DuplicateName {
                installation_id: record.installation_id,
            });
        }
        for installed in &record.files {
            if let Some(candidate) = candidate_paths.get(installed.path.portable_key()) {
                return Some(InstallationPlanConflict::PathOverlap {
                    installation_id: record.installation_id,
                    path: (*candidate).clone(),
                });
            }
        }
    }
    None
}

fn read_registry(paths: &ModStorePaths, limits: &ModLimits) -> Result<ParsedRegistry, ModError> {
    let registry_path = paths.installation_registry();
    if existing_mod_store_root(paths)?.is_none() {
        return Ok(ParsedRegistry {
            records: Vec::new(),
            issues: Vec::new(),
        });
    }
    let metadata = match fs::symlink_metadata(&registry_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ParsedRegistry {
                records: Vec::new(),
                issues: Vec::new(),
            });
        }
        Err(error) => {
            return Err(ModError::io(
                "inspect installation registry",
                registry_path,
                error,
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::SymbolicLink,
        ));
    }
    if !metadata.is_file() {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::NotRegularFile,
        ));
    }
    if metadata.len() > MAX_REGISTRY_BYTES as u64 {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::TooLarge,
        ));
    }

    let file = File::open(&registry_path)
        .map_err(|error| ModError::io("open installation registry", &registry_path, error))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(MAX_REGISTRY_BYTES)
            .min(MAX_REGISTRY_BYTES),
    );
    file.take((MAX_REGISTRY_BYTES as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ModError::io("read installation registry", &registry_path, error))?;
    if bytes.len() > MAX_REGISTRY_BYTES {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::TooLarge,
        ));
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len() {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::Changed,
        ));
    }

    let image: InstallationRegistryImage = serde_json::from_slice(&bytes)
        .map_err(|_| ModError::registry(&registry_path, RegistryErrorKind::InvalidJSON))?;
    if image.format_version != REGISTRY_VERSION {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::UnsupportedVersion,
        ));
    }
    if u64::try_from(image.installations.len()).unwrap_or(u64::MAX) > limits.max_package_files {
        return Err(ModError::registry(
            registry_path,
            RegistryErrorKind::TooManyRecords,
        ));
    }
    Ok(parse_registry_records(
        &registry_path,
        image.installations,
        limits,
    ))
}

fn parse_registry_records(
    registry_path: &Path,
    images: Vec<Value>,
    limits: &ModLimits,
) -> ParsedRegistry {
    let mut records = Vec::with_capacity(images.len());
    let mut issues = Vec::new();
    for (record_index, value) in images.into_iter().enumerate() {
        let installation_id = value
            .get("installationID")
            .and_then(Value::as_str)
            .and_then(|value| InstallationID::parse(value).ok());
        match serde_json::from_value::<InstallationImage>(value)
            .map_err(|_| ModError::registry(registry_path, RegistryErrorKind::InvalidRecord))
            .and_then(|image| parse_installation(image, limits))
        {
            Ok(record) => records.push((record_index, record)),
            Err(error) => issues.push(InstallationIssue {
                kind: InstallationIssueKind::InvalidRecord,
                record_index,
                installation_id,
                path: None,
                error,
            }),
        }
    }
    append_relationship_issues(registry_path, &records, &mut issues);
    ParsedRegistry { records, issues }
}

fn parse_installation(
    image: InstallationImage,
    limits: &ModLimits,
) -> Result<InstallationRecord, ModError> {
    let installation_id = InstallationID::parse(&image.installation_id)?;
    let package_id = ModPackageID::parse(&image.package_id)?;
    let game = parse_game(&image.game)?;
    let created = image
        .created
        .as_deref()
        .map(ModTimestamp::parse)
        .transpose()?;
    let metadata = ModMetadata::new(
        image.name,
        image.version,
        image.author,
        image.description,
        created,
    )?;
    let configured_root = PathBuf::from(image.configured_root);
    let canonical_root = PathBuf::from(image.canonical_root);
    let root_key = GameRootKey::parse(&image.root_key)?;
    if configured_root.as_os_str().is_empty()
        || !canonical_root.is_absolute()
        || root_key != GameRootKey::for_root(game, &canonical_root)
    {
        return Err(ModError::registry(
            canonical_root,
            RegistryErrorKind::InvalidRecord,
        ));
    }
    let installed_at = ModTimestamp::parse(&image.installed_at)?;
    let operation_id = OperationID::parse(&image.operation_id)?;
    if image.files.is_empty()
        || u64::try_from(image.files.len()).unwrap_or(u64::MAX) > limits.max_package_files
    {
        return Err(ModError::registry(
            canonical_root,
            RegistryErrorKind::InvalidRecord,
        ));
    }
    let mut portable_paths = HashSet::with_capacity(image.files.len());
    let mut files = Vec::with_capacity(image.files.len());
    for file in image.files {
        let path = RelativeGamePath::parse(&file.path, limits)?;
        if !portable_paths.insert(path.portable_key().to_owned()) {
            return Err(ModError::registry(
                canonical_root,
                RegistryErrorKind::InvalidRecord,
            ));
        }
        files.push(InstalledFile {
            path,
            installed_sha256: FileSHA256::parse(&file.installed_sha256)?,
            original_existed: file.original_existed,
        });
    }
    files.sort_by(|left, right| left.path.portable_key().cmp(right.path.portable_key()));
    Ok(InstallationRecord {
        installation_id,
        package_id,
        metadata,
        game,
        configured_root,
        canonical_root,
        root_key,
        installed_at,
        operation_id,
        files,
    })
}

fn append_relationship_issues(
    registry_path: &Path,
    records: &[(usize, InstallationRecord)],
    issues: &mut Vec<InstallationIssue>,
) {
    let mut installation_ids = HashMap::new();
    let mut names = HashMap::new();
    let mut paths = HashMap::new();
    for (record_index, record) in records {
        if installation_ids
            .insert(record.installation_id, *record_index)
            .is_some()
        {
            issues.push(relationship_issue(
                registry_path,
                *record_index,
                record.installation_id,
                InstallationIssueKind::DuplicateInstallationID,
            ));
        }
        let name_key = (record.root_key, record.metadata.name().to_lowercase());
        if names.insert(name_key, *record_index).is_some() {
            issues.push(relationship_issue(
                registry_path,
                *record_index,
                record.installation_id,
                InstallationIssueKind::DuplicateName,
            ));
        }
        for file in &record.files {
            let path_key = (record.root_key, file.path.portable_key().to_owned());
            if paths.insert(path_key, *record_index).is_some() {
                let mut issue = relationship_issue(
                    registry_path,
                    *record_index,
                    record.installation_id,
                    InstallationIssueKind::PathConflict,
                );
                issue.path = Some(record.canonical_root.join(file.path.as_ref()));
                issues.push(issue);
            }
        }
    }
}

fn relationship_issue(
    registry_path: &Path,
    record_index: usize,
    installation_id: InstallationID,
    kind: InstallationIssueKind,
) -> InstallationIssue {
    InstallationIssue {
        kind,
        record_index,
        installation_id: Some(installation_id),
        path: None,
        error: ModError::registry(registry_path, RegistryErrorKind::InvalidRecord),
    }
}

fn installation_health(
    record: &InstallationRecord,
    root: &GameRoot,
    limits: &ModLimits,
) -> Result<InstalledModStatus, (PathBuf, ModError)> {
    let mut has_missing = false;
    let mut has_modified = false;
    for file in &record.files {
        let target = root.canonical_path().join(file.path.as_ref());
        match hash_installed_file(root, &file.path, limits) {
            Ok(Some(digest)) if digest == file.installed_sha256 => {}
            Ok(Some(_)) => has_modified = true,
            Ok(None) => has_missing = true,
            Err(error) => return Err((target, error)),
        }
    }
    Ok(if has_missing {
        InstalledModStatus::Missing
    } else if has_modified {
        InstalledModStatus::Modified
    } else {
        InstalledModStatus::Clean
    })
}

fn hash_installed_file(
    root: &GameRoot,
    relative_path: &RelativeGamePath,
    limits: &ModLimits,
) -> Result<Option<FileSHA256>, ModError> {
    let Some(current) = validate_installed_path(root, relative_path)? else {
        return Ok(None);
    };

    let before = fs::symlink_metadata(&current)
        .map_err(|error| ModError::io("inspect installed file before hashing", &current, error))?;
    validate_health_component(&current, &before, true)?;
    if before.len() > limits.max_file_bytes {
        return Err(ModError::installed_file(
            current,
            InstalledFileErrorKind::TooLarge,
        ));
    }
    let before_stamp = health_file_stamp(&before);
    let mut file = File::open(&current)
        .map_err(|error| ModError::io("open installed file for health", &current, error))?;
    let opened = file
        .metadata()
        .map_err(|error| ModError::io("inspect open installed file", &current, error))?;
    if health_file_stamp(&opened) != before_stamp {
        return Err(ModError::installed_file(
            current,
            InstalledFileErrorKind::Changed,
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER_BYTES].into_boxed_slice();
    let mut bytes = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| ModError::io("hash installed file", &current, error))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| ModError::installed_file(&current, InstalledFileErrorKind::TooLarge))?;
        if bytes > limits.max_file_bytes {
            return Err(ModError::installed_file(
                current,
                InstalledFileErrorKind::TooLarge,
            ));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            ModError::io(
                "validate health read length",
                &current,
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "reader returned more bytes than the supplied buffer",
                ),
            )
        })?;
        hasher.update(chunk);
    }
    let after = fs::symlink_metadata(&current)
        .map_err(|error| ModError::io("reinspect installed file after hashing", &current, error))?;
    validate_health_component(&current, &after, true)?;
    if bytes != before.len() || health_file_stamp(&after) != before_stamp {
        return Err(ModError::installed_file(
            current,
            InstalledFileErrorKind::Changed,
        ));
    }
    if validate_installed_path(root, relative_path)?.is_none() {
        return Err(ModError::installed_file(
            current,
            InstalledFileErrorKind::Changed,
        ));
    }
    Ok(Some(FileSHA256::from_bytes(hasher.finalize().into())))
}

fn validate_installed_path(
    root: &GameRoot,
    relative_path: &RelativeGamePath,
) -> Result<Option<PathBuf>, ModError> {
    let mut current = root.canonical_path().to_path_buf();
    let root_metadata = match fs::symlink_metadata(&current) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ModError::io("inspect game root health", current, error)),
    };
    validate_health_component(&current, &root_metadata, false)?;
    for (index, component) in relative_path.as_str().split('/').enumerate() {
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(ModError::io(
                    "inspect installed file health",
                    current,
                    error,
                ));
            }
        };
        let is_file = index.saturating_add(1) == relative_path.component_count();
        validate_health_component(&current, &metadata, is_file)?;
    }
    Ok(Some(current))
}

fn validate_health_component(
    path: &Path,
    metadata: &fs::Metadata,
    is_file: bool,
) -> Result<(), ModError> {
    if metadata.file_type().is_symlink() {
        return Err(ModError::installed_file(
            path,
            InstalledFileErrorKind::SymbolicLink,
        ));
    }
    if (is_file && !metadata.is_file()) || (!is_file && !metadata.is_dir()) {
        return Err(ModError::installed_file(
            path,
            InstalledFileErrorKind::NotRegularFile,
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HealthFileStamp {
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

fn health_file_stamp(metadata: &fs::Metadata) -> HealthFileStamp {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;

    HealthFileStamp {
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

#[allow(dead_code, reason = "used by the apply transaction in Task 5")]
pub(crate) fn store_installations(
    paths: &ModStorePaths,
    records: &[InstallationRecord],
) -> Result<(), ModError> {
    let bytes = serialize_registry(paths, records)?;
    publish_registry_image_with_hook(paths, &bytes, |_| Ok(()))
}

fn serialize_registry(
    paths: &ModStorePaths,
    records: &[InstallationRecord],
) -> Result<Vec<u8>, ModError> {
    let mut records = records.iter().collect::<Vec<_>>();
    records.sort_by_key(|record| record.installation_id);
    let installations = records
        .into_iter()
        .map(InstallationImageOwned::from_record)
        .collect::<Vec<_>>();
    let mut bytes = serde_json::to_vec_pretty(&InstallationRegistryImageOwned {
        format_version: REGISTRY_VERSION,
        installations,
    })
    .map_err(|_| {
        ModError::registry(
            paths.installation_registry(),
            RegistryErrorKind::InvalidRecord,
        )
    })?;
    bytes.push(b'\n');
    if bytes.len() > MAX_REGISTRY_BYTES {
        return Err(ModError::registry(
            paths.installation_registry(),
            RegistryErrorKind::TooLarge,
        ));
    }
    Ok(bytes)
}

fn publish_registry_image_with_hook(
    paths: &ModStorePaths,
    bytes: &[u8],
    before_publish: impl FnOnce(&Path) -> Result<(), ModError>,
) -> Result<(), ModError> {
    let root = prepare_mod_store_root(paths)?;
    let registry_path = paths.installation_registry();
    let mut temporary = NamedTempFile::new_in(&root)
        .map_err(|error| ModError::io("create temporary installation registry", &root, error))?;
    temporary.write_all(bytes).map_err(|error| {
        ModError::io(
            "write temporary installation registry",
            temporary.path(),
            error,
        )
    })?;
    temporary.flush().map_err(|error| {
        ModError::io(
            "flush temporary installation registry",
            temporary.path(),
            error,
        )
    })?;
    temporary.as_file().sync_all().map_err(|error| {
        ModError::io(
            "synchronize temporary installation registry",
            temporary.path(),
            error,
        )
    })?;
    before_publish(temporary.path())?;
    let file = temporary.persist(&registry_path).map_err(|error| {
        ModError::io("publish installation registry", &registry_path, error.error)
    })?;
    drop(file);
    sync_directory(&root)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), ModError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| ModError::io("synchronize mod store", path, error))
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> Result<(), ModError> {
    Ok(())
}

#[derive(Deserialize)]
struct InstallationRegistryImage {
    #[serde(rename = "formatVersion")]
    format_version: u64,
    installations: Vec<Value>,
}

#[derive(Deserialize)]
struct InstallationImage {
    #[serde(rename = "installationID")]
    installation_id: String,
    #[serde(rename = "packageID")]
    package_id: String,
    name: String,
    version: String,
    author: Option<String>,
    description: Option<String>,
    created: Option<String>,
    game: String,
    #[serde(rename = "configuredRoot")]
    configured_root: String,
    #[serde(rename = "canonicalRoot")]
    canonical_root: String,
    #[serde(rename = "rootKey")]
    root_key: String,
    #[serde(rename = "installedAt")]
    installed_at: String,
    #[serde(rename = "operationID")]
    operation_id: String,
    files: Vec<InstalledFileImage>,
}

#[derive(Deserialize)]
struct InstalledFileImage {
    path: String,
    #[serde(rename = "installedSHA256")]
    installed_sha256: String,
    #[serde(rename = "originalExisted")]
    original_existed: bool,
}

#[derive(Serialize)]
struct InstallationRegistryImageOwned {
    #[serde(rename = "formatVersion")]
    format_version: u64,
    installations: Vec<InstallationImageOwned>,
}

#[derive(Serialize)]
struct InstallationImageOwned {
    #[serde(rename = "installationID")]
    installation_id: String,
    #[serde(rename = "packageID")]
    package_id: String,
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<String>,
    game: &'static str,
    #[serde(rename = "configuredRoot")]
    configured_root: String,
    #[serde(rename = "canonicalRoot")]
    canonical_root: String,
    #[serde(rename = "rootKey")]
    root_key: String,
    #[serde(rename = "installedAt")]
    installed_at: String,
    #[serde(rename = "operationID")]
    operation_id: String,
    files: Vec<InstalledFileImageOwned>,
}

impl InstallationImageOwned {
    fn from_record(record: &InstallationRecord) -> Self {
        Self {
            installation_id: record.installation_id.to_string(),
            package_id: record.package_id.to_string(),
            name: record.metadata.name().to_owned(),
            version: record.metadata.version().to_owned(),
            author: record.metadata.author().map(str::to_owned),
            description: record.metadata.description().map(str::to_owned),
            created: record
                .metadata
                .created()
                .map(|value| value.as_str().to_owned()),
            game: game_name(record.game),
            configured_root: record.configured_root.to_string_lossy().into_owned(),
            canonical_root: record.canonical_root.to_string_lossy().into_owned(),
            root_key: record.root_key.to_string(),
            installed_at: record.installed_at.as_str().to_owned(),
            operation_id: record.operation_id.to_string(),
            files: record
                .files
                .iter()
                .map(|file| InstalledFileImageOwned {
                    path: file.path.as_str().to_owned(),
                    installed_sha256: file.installed_sha256.to_string(),
                    original_existed: file.original_existed,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct InstalledFileImageOwned {
    path: String,
    #[serde(rename = "installedSHA256")]
    installed_sha256: String,
    #[serde(rename = "originalExisted")]
    original_existed: bool,
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use tempfile::tempdir;

    use kufeditor_game::Game;

    use super::{
        InstallationPlanConflict, InstallationRecord, InstalledFile, installation_plan_conflict,
        publish_registry_image_with_hook, store_installations,
    };
    use crate::{
        FileSHA256, GameRootKey, InstallationID, ModError, ModLimits, ModMetadata, ModPackageID,
        ModStorePaths, ModTimestamp, OperationID, RelativeGamePath,
    };

    #[test]
    fn installation_identity_depends_only_on_stable_operation_inputs() {
        let root = GameRootKey::from_bytes([1; 32]);
        let package = ModPackageID::from_bytes([2; 32]);
        let operation = OperationID::from_bytes([3; 32]);

        let first = InstallationID::for_installation(root, package, operation);
        let repeated = InstallationID::for_installation(root, package, operation);
        let other_operation =
            InstallationID::for_installation(root, package, OperationID::from_bytes([4; 32]));

        assert_eq!(first, repeated);
        assert_ne!(first, other_operation);
    }

    #[test]
    fn candidate_planning_rejects_names_and_portable_paths_owned_by_the_same_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let limits = ModLimits::default();
        let root_key = GameRootKey::from_bytes([1; 32]);
        let owned_path = RelativeGamePath::parse("Data/file.sox", &limits)?;
        let record = InstallationRecord {
            installation_id: InstallationID::from_bytes([2; 32]),
            package_id: ModPackageID::from_bytes([3; 32]),
            metadata: ModMetadata::new("Existing", "1", None, None, None)?,
            game: Game::Heroes,
            configured_root: "/game".into(),
            canonical_root: "/game".into(),
            root_key,
            installed_at: ModTimestamp::parse("2026-08-26T12:00:00Z")?,
            operation_id: OperationID::from_bytes([4; 32]),
            files: vec![InstalledFile {
                path: owned_path,
                installed_sha256: FileSHA256::from_bytes([5; 32]),
                original_existed: true,
            }],
        };

        assert!(matches!(
            installation_plan_conflict(std::slice::from_ref(&record), root_key, "existing", &[]),
            Some(InstallationPlanConflict::DuplicateName { .. })
        ));
        assert!(matches!(
            installation_plan_conflict(
                &[record],
                root_key,
                "Different",
                &[RelativeGamePath::parse("data/FILE.SOX", &limits)?]
            ),
            Some(InstallationPlanConflict::PathOverlap { .. })
        ));
        Ok(())
    }

    #[test]
    fn publication_failure_preserves_registry_bytes_and_removes_the_temporary_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let paths = ModStorePaths::new(directory.path().join("application-data"));
        fs::create_dir_all(paths.root())?;
        fs::write(paths.installation_registry(), b"previous registry bytes")?;

        let result = publish_registry_image_with_hook(&paths, b"replacement", |path| {
            Err(ModError::io(
                "inject registry publication failure",
                path,
                io::Error::other("injected failure"),
            ))
        });

        assert!(result.is_err());
        assert_eq!(
            fs::read(paths.installation_registry())?,
            b"previous registry bytes"
        );
        assert_eq!(fs::read_dir(paths.root())?.count(), 1);
        Ok(())
    }

    #[test]
    fn empty_registry_has_one_canonical_versioned_image() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempdir()?;
        let paths = ModStorePaths::new(directory.path().join("application-data"));

        store_installations(&paths, &[])?;

        assert_eq!(
            fs::read(paths.installation_registry())?,
            b"{\n  \"formatVersion\": 1,\n  \"installations\": []\n}\n"
        );
        Ok(())
    }
}
