use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zip::{
    CompressionMethod, DateTime, System, ZIP64_BYTES_THR, ZipArchive, ZipWriter,
    write::SimpleFileOptions,
};

use crate::{
    GameRoot, ManifestErrorKind, ModError, ModLimits, ModManifest, ModMetadata, ModPackageID,
    ModProgress, ModProgressPhase, ModProgressReporter, ModService, PackageErrorKind,
    RelativeGamePath, SourceFileErrorKind,
};

const BUFFER_BYTES: usize = 64 * 1024;
const UNIX_TYPE_MASK: u32 = 0o170_000;
const UNIX_REGULAR_FILE: u32 = 0o100_000;
const UNIX_DIRECTORY: u32 = 0o040_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModPackageInfo {
    package_id: ModPackageID,
    library_path: PathBuf,
    manifest: ModManifest,
    compressed_bytes: u64,
    uncompressed_bytes: u64,
    file_count: u64,
}

impl ModPackageInfo {
    pub const fn package_id(&self) -> ModPackageID {
        self.package_id
    }

    pub fn library_path(&self) -> &Path {
        &self.library_path
    }

    pub const fn manifest(&self) -> &ModManifest {
        &self.manifest
    }

    pub const fn compressed_bytes(&self) -> u64 {
        self.compressed_bytes
    }

    pub const fn uncompressed_bytes(&self) -> u64 {
        self.uncompressed_bytes
    }

    pub const fn file_count(&self) -> u64 {
        self.file_count
    }

    pub(crate) fn at_path(mut self, path: PathBuf) -> Self {
        self.library_path = path;
        self
    }

    pub(crate) fn same_content(&self, other: &Self) -> bool {
        self.package_id == other.package_id
            && self.manifest == other.manifest
            && self.compressed_bytes == other.compressed_bytes
            && self.uncompressed_bytes == other.uncompressed_bytes
            && self.file_count == other.file_count
    }
}

#[derive(Debug)]
pub struct CreateModRequest<'a> {
    manifest: ModManifest,
    root: &'a GameRoot,
    sources: Vec<CreationSource>,
    output: &'a Path,
}

impl<'a> CreateModRequest<'a> {
    pub fn new(
        metadata: ModMetadata,
        root: &'a GameRoot,
        files: Vec<RelativeGamePath>,
        output: &'a Path,
    ) -> Result<Self, ModError> {
        let manifest = ModManifest::new(metadata, root.game(), files)?;
        let sources = manifest
            .files()
            .iter()
            .map(|path| inspect_creation_source(root, path))
            .collect::<Result<Vec<_>, _>>()?;
        validate_creation_output(output, &sources)?;
        Ok(Self {
            manifest,
            root,
            sources,
            output,
        })
    }

    pub const fn manifest(&self) -> &ModManifest {
        &self.manifest
    }

    pub const fn root(&self) -> &GameRoot {
        self.root
    }

    pub fn files(&self) -> &[RelativeGamePath] {
        self.manifest.files()
    }

    pub const fn output(&self) -> &Path {
        self.output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedMod {
    package: ModPackageInfo,
}

impl CreatedMod {
    pub fn output_path(&self) -> &Path {
        self.package.library_path()
    }

    pub const fn package_id(&self) -> ModPackageID {
        self.package.package_id()
    }

    pub const fn manifest(&self) -> &ModManifest {
        self.package.manifest()
    }

    pub const fn compressed_bytes(&self) -> u64 {
        self.package.compressed_bytes()
    }

    pub const fn uncompressed_bytes(&self) -> u64 {
        self.package.uncompressed_bytes()
    }

    pub const fn file_count(&self) -> u64 {
        self.package.file_count()
    }
}

#[derive(Debug)]
struct CreationSource {
    relative_path: RelativeGamePath,
    absolute_path: PathBuf,
    components: Vec<SourceComponentSnapshot>,
    file_stamp: SourceMetadataStamp,
}

impl CreationSource {
    const fn bytes(&self) -> u64 {
        self.file_stamp.len
    }
}

#[derive(Debug)]
struct SourceComponentSnapshot {
    path: PathBuf,
    stamp: SourceMetadataStamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceMetadataStamp {
    kind: SourceMetadataKind,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceMetadataKind {
    Directory,
    File,
    SymbolicLink,
    Other,
}

impl ModService {
    pub fn create_package(
        &self,
        request: CreateModRequest<'_>,
        progress: &mut impl ModProgressReporter,
    ) -> Result<CreatedMod, ModError> {
        let CreateModRequest {
            manifest,
            root: _,
            sources,
            output,
        } = request;
        let (manifest_bytes, total_source_bytes) =
            prepare_creation(&manifest, &sources, &self.limits)?;
        let output_parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = NamedTempFile::new_in(output_parent)
            .map_err(|error| ModError::io("create temporary mod package", output_parent, error))?;
        let temporary_path = temporary.path().to_path_buf();
        write_creation_image(
            temporary.as_file_mut(),
            &temporary_path,
            &sources,
            &manifest_bytes,
            total_source_bytes,
            &self.limits,
            progress,
        )?;
        let inspected = inspect_package(&temporary_path, &self.limits, progress)?;
        if inspected.manifest() != &manifest
            || inspected.uncompressed_bytes() != total_source_bytes
            || inspected.file_count() != u64::try_from(sources.len()).unwrap_or(u64::MAX)
        {
            return Err(ModError::package(
                &temporary_path,
                None,
                PackageErrorKind::PayloadMismatch,
            ));
        }
        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::PublishingPackage,
                completed: 0,
                total: 1,
                path: None,
            })
            .is_break()
        {
            return Err(ModError::Canceled {
                operation: "package creation",
            });
        }

        let output = output.to_path_buf();
        let file = temporary
            .persist(&output)
            .map_err(|error| ModError::io("save created mod package", &output, error.error))?;
        drop(file);
        Ok(CreatedMod {
            package: inspected.at_path(output),
        })
    }
}

fn prepare_creation(
    manifest: &ModManifest,
    sources: &[CreationSource],
    limits: &ModLimits,
) -> Result<(Vec<u8>, u64), ModError> {
    let file_count = u64::try_from(sources.len()).unwrap_or(u64::MAX);
    if file_count > limits.max_package_files {
        return Err(ModError::manifest(ManifestErrorKind::TooManyFiles));
    }
    let manifest_bytes = manifest.to_json()?;
    if u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX) > limits.max_manifest_bytes {
        return Err(ModError::manifest(ManifestErrorKind::TooLarge));
    }

    let mut total_source_bytes = 0u64;
    for source in sources {
        verify_creation_source(source)?;
        if source.bytes() > limits.max_file_bytes {
            return Err(ModError::source(
                &source.absolute_path,
                SourceFileErrorKind::TooLarge,
            ));
        }
        total_source_bytes = total_source_bytes
            .checked_add(source.bytes())
            .ok_or_else(|| {
                ModError::source(&source.absolute_path, SourceFileErrorKind::TooLarge)
            })?;
        if total_source_bytes > limits.max_uncompressed_bytes {
            return Err(ModError::source(
                &source.absolute_path,
                SourceFileErrorKind::TooLarge,
            ));
        }
    }
    Ok((manifest_bytes, total_source_bytes))
}

fn write_creation_image(
    output: &mut File,
    image_path: &Path,
    sources: &[CreationSource],
    manifest_bytes: &[u8],
    total_source_bytes: u64,
    limits: &ModLimits,
    progress: &mut impl ModProgressReporter,
) -> Result<(), ModError> {
    let mut writer = ZipWriter::new(output);
    writer
        .start_file(
            "mod.json",
            creation_options(
                u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX) >= ZIP64_BYTES_THR,
            ),
        )
        .map_err(|error| ModError::zip(image_path, error))?;
    writer
        .write_all(manifest_bytes)
        .map_err(|error| ModError::io("write mod manifest", image_path, error))?;

    let mut completed = 0u64;
    for source in sources {
        write_creation_source(
            &mut writer,
            image_path,
            source,
            total_source_bytes,
            &mut completed,
            limits,
            progress,
        )?;
    }
    let file = writer
        .finish()
        .map_err(|error| ModError::zip(image_path, error))?;
    file.flush()
        .map_err(|error| ModError::io("flush created mod package", image_path, error))?;
    file.sync_all()
        .map_err(|error| ModError::io("synchronize created mod package", image_path, error))
}

fn write_creation_source(
    writer: &mut ZipWriter<&mut File>,
    image_path: &Path,
    source: &CreationSource,
    total_source_bytes: u64,
    completed: &mut u64,
    limits: &ModLimits,
    progress: &mut impl ModProgressReporter,
) -> Result<(), ModError> {
    verify_creation_source(source)?;
    let mut input = File::open(&source.absolute_path)
        .map_err(|error| ModError::io("open package source", &source.absolute_path, error))?;
    let opened_stamp = source_metadata_stamp(&input.metadata().map_err(|error| {
        ModError::io("inspect open package source", &source.absolute_path, error)
    })?);
    if opened_stamp != source.file_stamp {
        return Err(ModError::source(
            &source.absolute_path,
            SourceFileErrorKind::Changed,
        ));
    }
    writer
        .start_file(
            source.relative_path.as_str(),
            creation_options(source.bytes() >= ZIP64_BYTES_THR),
        )
        .map_err(|error| ModError::zip(image_path, error))?;

    let mut buffer = vec![0u8; BUFFER_BYTES].into_boxed_slice();
    let mut source_bytes = 0u64;
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| ModError::io("read package source", &source.absolute_path, error))?;
        if read == 0 {
            break;
        }
        source_bytes = source_bytes
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                ModError::source(&source.absolute_path, SourceFileErrorKind::TooLarge)
            })?;
        if source_bytes > source.bytes() || source_bytes > limits.max_file_bytes {
            return Err(ModError::source(
                &source.absolute_path,
                SourceFileErrorKind::Changed,
            ));
        }
        *completed = completed
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                ModError::source(&source.absolute_path, SourceFileErrorKind::TooLarge)
            })?;
        if *completed > limits.max_uncompressed_bytes {
            return Err(ModError::source(
                &source.absolute_path,
                SourceFileErrorKind::TooLarge,
            ));
        }
        writer
            .write_all(checked_read_bytes(&buffer, read, &source.absolute_path)?)
            .map_err(|error| ModError::io("write package payload", image_path, error))?;
        report_creation_progress(progress, *completed, total_source_bytes, source)?;
    }
    if source_bytes == 0 {
        report_creation_progress(progress, *completed, total_source_bytes, source)?;
    }
    let closed_stamp = source_metadata_stamp(&input.metadata().map_err(|error| {
        ModError::io(
            "reinspect open package source",
            &source.absolute_path,
            error,
        )
    })?);
    if source_bytes != source.bytes() || closed_stamp != source.file_stamp {
        return Err(ModError::source(
            &source.absolute_path,
            SourceFileErrorKind::Changed,
        ));
    }
    verify_creation_source(source)
}

fn report_creation_progress(
    reporter: &mut impl ModProgressReporter,
    completed: u64,
    total: u64,
    source: &CreationSource,
) -> Result<(), ModError> {
    if reporter
        .report(&ModProgress {
            phase: ModProgressPhase::CreatingPackage,
            completed,
            total,
            path: Some(source.relative_path.clone()),
        })
        .is_break()
    {
        Err(ModError::Canceled {
            operation: "package creation",
        })
    } else {
        Ok(())
    }
}

fn creation_options(large_file: bool) -> SimpleFileOptions {
    SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644)
        .large_file(large_file)
        .system(System::Unix)
}

fn inspect_creation_source(
    root: &GameRoot,
    relative_path: &RelativeGamePath,
) -> Result<CreationSource, ModError> {
    let mut current = root.canonical_path().to_path_buf();
    let mut components = Vec::with_capacity(relative_path.component_count().saturating_add(1));
    let root_metadata = initial_source_metadata(&current)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ModError::source(
            &current,
            SourceFileErrorKind::SymbolicLink,
        ));
    }
    if !root_metadata.is_dir() {
        return Err(ModError::source(
            &current,
            SourceFileErrorKind::ParentNotDirectory,
        ));
    }
    components.push(SourceComponentSnapshot {
        path: current.clone(),
        stamp: source_metadata_stamp(&root_metadata),
    });

    for (index, component) in relative_path.as_str().split('/').enumerate() {
        current.push(component);
        let metadata = initial_source_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(ModError::source(
                &current,
                SourceFileErrorKind::SymbolicLink,
            ));
        }
        let is_file = index.saturating_add(1) == relative_path.component_count();
        if is_file && !metadata.is_file() {
            return Err(ModError::source(
                &current,
                SourceFileErrorKind::NotRegularFile,
            ));
        }
        if !is_file && !metadata.is_dir() {
            return Err(ModError::source(
                &current,
                SourceFileErrorKind::ParentNotDirectory,
            ));
        }
        components.push(SourceComponentSnapshot {
            path: current.clone(),
            stamp: source_metadata_stamp(&metadata),
        });
    }
    let file_stamp = components
        .last()
        .map(|component| component.stamp)
        .ok_or_else(|| ModError::source(&current, SourceFileErrorKind::Missing))?;
    Ok(CreationSource {
        relative_path: relative_path.clone(),
        absolute_path: current,
        components,
        file_stamp,
    })
}

fn initial_source_metadata(path: &Path) -> Result<fs::Metadata, ModError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(ModError::source(path, SourceFileErrorKind::Missing))
        }
        Err(error) => Err(ModError::io("inspect package source", path, error)),
    }
}

fn validate_creation_output(output: &Path, sources: &[CreationSource]) -> Result<(), ModError> {
    let canonical_output = match fs::canonicalize(output) {
        Ok(path) => Some(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(ModError::io("inspect mod package output", output, error)),
    };
    if let Some(source) = canonical_output
        .and_then(|output| sources.iter().find(|source| source.absolute_path == output))
    {
        Err(ModError::source(
            &source.absolute_path,
            SourceFileErrorKind::OutputCollision,
        ))
    } else {
        Ok(())
    }
}

fn verify_creation_source(source: &CreationSource) -> Result<(), ModError> {
    for component in &source.components {
        let metadata = match fs::symlink_metadata(&component.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ModError::source(
                    &component.path,
                    SourceFileErrorKind::Changed,
                ));
            }
            Err(error) => {
                return Err(ModError::io(
                    "reinspect package source",
                    &component.path,
                    error,
                ));
            }
        };
        if source_metadata_stamp(&metadata) != component.stamp {
            return Err(ModError::source(
                &component.path,
                SourceFileErrorKind::Changed,
            ));
        }
    }
    Ok(())
}

fn source_metadata_stamp(metadata: &fs::Metadata) -> SourceMetadataStamp {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;

    let file_type = metadata.file_type();
    let kind = if file_type.is_dir() {
        SourceMetadataKind::Directory
    } else if file_type.is_file() {
        SourceMetadataKind::File
    } else if file_type.is_symlink() {
        SourceMetadataKind::SymbolicLink
    } else {
        SourceMetadataKind::Other
    };
    let (len, modified) = if kind == SourceMetadataKind::File {
        (metadata.len(), metadata.modified().ok())
    } else {
        (0, None)
    };
    SourceMetadataStamp {
        kind,
        len,
        modified,
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

pub(crate) fn inspect_package(
    path: &Path,
    limits: &ModLimits,
    reporter: &mut impl ModProgressReporter,
) -> Result<ModPackageInfo, ModError> {
    let (package_id, compressed_bytes) = hash_package_image(path, limits)?;
    let declared_entry_count = declared_entry_count(path, compressed_bytes)?;
    let maximum_entries = limits.max_package_files.saturating_add(1);
    if declared_entry_count.is_some_and(|count| count > maximum_entries) {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::TooManyEntries,
        ));
    }
    let file = File::open(path).map_err(|error| ModError::io("open ZIP package", path, error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| ModError::zip(path, error))?;
    let entry_count = u64::try_from(archive.len()).unwrap_or(u64::MAX);
    if entry_count > maximum_entries {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::TooManyEntries,
        ));
    }
    if declared_entry_count.is_some_and(|count| count != entry_count) {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::DuplicateEntry,
        ));
    }
    let mut inspection = PackageInspection::new(archive.len());
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ModError::zip(path, error))?;
        inspection.inspect_entry(path, limits, index, entry_count, &mut entry, reporter)?;
    }
    drop(archive);

    let PackageInspection {
        manifest_bytes,
        mut payloads,
        uncompressed_bytes,
        file_count,
        ..
    } = inspection;
    let manifest_bytes = manifest_bytes
        .ok_or_else(|| ModError::package(path, None, PackageErrorKind::MissingManifest))?;
    let manifest = ModManifest::from_json(&manifest_bytes, limits)?;
    payloads.sort_by(|left, right| left.portable_key().cmp(right.portable_key()));
    if payloads != manifest.files() {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::PayloadMismatch,
        ));
    }

    let (verified_id, verified_bytes) = hash_package_image(path, limits)?;
    if verified_id != package_id || verified_bytes != compressed_bytes {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::SourceChanged,
        ));
    }

    Ok(ModPackageInfo {
        package_id,
        library_path: path.to_path_buf(),
        manifest,
        compressed_bytes,
        uncompressed_bytes,
        file_count,
    })
}

struct PackageInspection {
    manifest_bytes: Option<Vec<u8>>,
    payloads: Vec<RelativeGamePath>,
    portable_entries: HashSet<String>,
    uncompressed_bytes: u64,
    file_count: u64,
}

impl PackageInspection {
    fn new(entry_count: usize) -> Self {
        Self {
            manifest_bytes: None,
            payloads: Vec::new(),
            portable_entries: HashSet::with_capacity(entry_count),
            uncompressed_bytes: 0,
            file_count: 0,
        }
    }

    fn inspect_entry<R: Read + Seek>(
        &mut self,
        package: &Path,
        limits: &ModLimits,
        index: usize,
        entry_count: u64,
        entry: &mut zip::read::ZipFile<'_, R>,
        reporter: &mut impl ModProgressReporter,
    ) -> Result<(), ModError> {
        let entry_name = std::str::from_utf8(entry.name_raw())
            .map_err(|_| ModError::package(package, None, PackageErrorKind::EntryNameNotUTF8))?
            .to_owned();
        validate_entry_kind(package, &entry_name, entry)?;
        validate_compression(package, &entry_name, entry.compression())?;
        let is_directory = entry.is_dir();
        let relative_path = relative_entry_path(package, &entry_name, is_directory, limits)?;
        if !is_directory
            && relative_path.as_str() != "mod.json"
            && relative_path.as_str().rsplit('/').next() == Some("mod.json")
        {
            return Err(ModError::package(
                package,
                Some(entry_name),
                PackageErrorKind::NestedManifest,
            ));
        }
        if !self
            .portable_entries
            .insert(relative_path.portable_key().to_owned())
        {
            let kind = if !is_directory && entry_name == "mod.json" {
                PackageErrorKind::DuplicateManifest
            } else {
                PackageErrorKind::DuplicateEntry
            };
            return Err(ModError::package(package, Some(entry_name), kind));
        }

        let progress_path = if is_directory || entry_name == "mod.json" {
            None
        } else {
            Some(relative_path.clone())
        };
        if is_directory {
            inspect_directory(package, &entry_name, entry)?;
        } else if entry_name == "mod.json" {
            self.inspect_manifest(package, &entry_name, entry, limits)?;
        } else {
            self.inspect_payload(package, entry_name, relative_path, entry, limits)?;
        }
        report_inspection_progress(reporter, index, entry_count, progress_path)
    }

    fn inspect_manifest<R: Read + Seek>(
        &mut self,
        package: &Path,
        entry_name: &str,
        entry: &mut zip::read::ZipFile<'_, R>,
        limits: &ModLimits,
    ) -> Result<(), ModError> {
        if self.manifest_bytes.is_some() {
            return Err(ModError::package(
                package,
                Some(entry_name.to_owned()),
                PackageErrorKind::DuplicateManifest,
            ));
        }
        let declared = entry.size();
        if declared > limits.max_manifest_bytes {
            return Err(ModError::package(
                package,
                Some(entry_name.to_owned()),
                PackageErrorKind::FileTooLarge,
            ));
        }
        let (actual, bytes) =
            read_entry_bytes(package, entry_name, entry, limits.max_manifest_bytes, true)?;
        if actual != declared {
            return Err(ModError::package(
                package,
                Some(entry_name.to_owned()),
                PackageErrorKind::EntrySizeMismatch,
            ));
        }
        self.manifest_bytes = Some(bytes);
        Ok(())
    }

    fn inspect_payload<R: Read + Seek>(
        &mut self,
        package: &Path,
        entry_name: String,
        relative_path: RelativeGamePath,
        entry: &mut zip::read::ZipFile<'_, R>,
        limits: &ModLimits,
    ) -> Result<(), ModError> {
        self.file_count = self
            .file_count
            .checked_add(1)
            .ok_or_else(|| ModError::package(package, None, PackageErrorKind::TooManyEntries))?;
        if self.file_count > limits.max_package_files {
            return Err(ModError::package(
                package,
                Some(entry_name),
                PackageErrorKind::TooManyEntries,
            ));
        }
        let declared = entry.size();
        if declared > limits.max_file_bytes {
            return Err(ModError::package(
                package,
                Some(entry_name),
                PackageErrorKind::FileTooLarge,
            ));
        }
        self.uncompressed_bytes = self
            .uncompressed_bytes
            .checked_add(declared)
            .ok_or_else(|| ModError::package(package, None, PackageErrorKind::TotalDataTooLarge))?;
        if self.uncompressed_bytes > limits.max_uncompressed_bytes {
            return Err(ModError::package(
                package,
                Some(entry_name),
                PackageErrorKind::TotalDataTooLarge,
            ));
        }
        let (actual, _) =
            read_entry_bytes(package, &entry_name, entry, limits.max_file_bytes, false)?;
        if actual != declared {
            return Err(ModError::package(
                package,
                Some(entry_name),
                PackageErrorKind::EntrySizeMismatch,
            ));
        }
        self.payloads.push(relative_path);
        Ok(())
    }
}

fn inspect_directory<R: Read + Seek>(
    package: &Path,
    entry_name: &str,
    entry: &mut zip::read::ZipFile<'_, R>,
) -> Result<(), ModError> {
    if entry.size() != 0 || entry.compressed_size() != 0 {
        return Err(ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::DirectoryData,
        ));
    }
    let actual = read_entry_bytes(package, entry_name, entry, 0, false)?.0;
    if actual == 0 {
        Ok(())
    } else {
        Err(ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::DirectoryData,
        ))
    }
}

fn report_inspection_progress(
    reporter: &mut impl ModProgressReporter,
    index: usize,
    entry_count: u64,
    path: Option<RelativeGamePath>,
) -> Result<(), ModError> {
    let completed = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
    if reporter
        .report(&ModProgress {
            phase: ModProgressPhase::InspectingPackage,
            completed,
            total: entry_count,
            path,
        })
        .is_break()
    {
        Err(ModError::Canceled {
            operation: "package inspection",
        })
    } else {
        Ok(())
    }
}

fn validate_entry_kind<R: Read + Seek>(
    package: &Path,
    entry_name: &str,
    entry: &zip::read::ZipFile<'_, R>,
) -> Result<(), ModError> {
    if entry.encrypted() {
        return Err(ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::EncryptedEntry,
        ));
    }
    if entry.is_symlink() {
        return Err(ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::SymbolicLinkEntry,
        ));
    }
    let Some(mode) = entry.unix_mode() else {
        return Ok(());
    };
    let file_type = mode & UNIX_TYPE_MASK;
    let supported = if entry.is_dir() {
        file_type == 0 || file_type == UNIX_DIRECTORY
    } else {
        file_type == 0 || file_type == UNIX_REGULAR_FILE
    };
    if supported {
        Ok(())
    } else {
        Err(ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::UnsupportedEntryType,
        ))
    }
}

fn validate_compression(
    package: &Path,
    entry_name: &str,
    compression: CompressionMethod,
) -> Result<(), ModError> {
    if matches!(
        compression,
        CompressionMethod::Stored | CompressionMethod::Deflated
    ) {
        Ok(())
    } else {
        Err(ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::UnsupportedCompression,
        ))
    }
}

fn relative_entry_path(
    package: &Path,
    entry_name: &str,
    is_directory: bool,
    limits: &ModLimits,
) -> Result<RelativeGamePath, ModError> {
    let path = if is_directory {
        entry_name.strip_suffix('/').unwrap_or(entry_name)
    } else {
        entry_name
    };
    RelativeGamePath::parse(path, limits).map_err(|_| {
        ModError::package(
            package,
            Some(entry_name.to_owned()),
            PackageErrorKind::UnsafeEntryPath,
        )
    })
}

fn read_entry_bytes(
    package: &Path,
    entry_name: &str,
    reader: &mut impl Read,
    limit: u64,
    retain: bool,
) -> Result<(u64, Vec<u8>), ModError> {
    let mut buffer = vec![0u8; BUFFER_BYTES].into_boxed_slice();
    let mut bytes = Vec::new();
    let mut actual = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| ModError::io("read ZIP entry", package, error))?;
        if read == 0 {
            break;
        }
        actual = actual
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                ModError::package(
                    package,
                    Some(entry_name.to_owned()),
                    PackageErrorKind::FileTooLarge,
                )
            })?;
        if actual > limit {
            return Err(ModError::package(
                package,
                Some(entry_name.to_owned()),
                PackageErrorKind::FileTooLarge,
            ));
        }
        if retain {
            bytes.extend_from_slice(checked_read_bytes(&buffer, read, package)?);
        }
    }
    Ok((actual, bytes))
}

pub(crate) fn hash_package_image(
    path: &Path,
    limits: &ModLimits,
) -> Result<(ModPackageID, u64), ModError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| ModError::io("inspect package", path, error))?;
    if metadata.file_type().is_symlink() {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::SymbolicLink,
        ));
    }
    if !metadata.is_file() {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::NotRegularFile,
        ));
    }
    if metadata.len() > limits.max_zip_bytes {
        return Err(ModError::package(path, None, PackageErrorKind::ZIPTooLarge));
    }

    let mut file = File::open(path).map_err(|error| ModError::io("open package", path, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; BUFFER_BYTES].into_boxed_slice();
    let mut actual = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| ModError::io("hash package", path, error))?;
        if read == 0 {
            break;
        }
        actual = actual
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| ModError::package(path, None, PackageErrorKind::ZIPTooLarge))?;
        if actual > limits.max_zip_bytes {
            return Err(ModError::package(path, None, PackageErrorKind::ZIPTooLarge));
        }
        hasher.update(checked_read_bytes(&buffer, read, path)?);
    }
    if actual != metadata.len() {
        return Err(ModError::package(
            path,
            None,
            PackageErrorKind::SourceChanged,
        ));
    }
    Ok((ModPackageID::from_bytes(hasher.finalize().into()), actual))
}

fn checked_read_bytes<'a>(
    buffer: &'a [u8],
    read: usize,
    path: &Path,
) -> Result<&'a [u8], ModError> {
    buffer.get(..read).ok_or_else(|| {
        ModError::io(
            "validate read length",
            path,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "reader returned more bytes than the supplied buffer",
            ),
        )
    })
}

fn declared_entry_count(path: &Path, file_bytes: u64) -> Result<Option<u64>, ModError> {
    const END_BYTES: u64 = 22;
    const MAX_COMMENT_BYTES: u64 = u16::MAX as u64;
    const LOCATOR_BYTES: usize = 20;
    const ZIP64_END_BYTES: usize = 56;
    const END_SIGNATURE: [u8; 4] = 0x0605_4b50u32.to_le_bytes();
    const LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
    const ZIP64_END_SIGNATURE: u32 = 0x0606_4b50;

    let tail_bytes = file_bytes.min(END_BYTES + MAX_COMMENT_BYTES);
    let tail_len = usize::try_from(tail_bytes).unwrap_or(usize::MAX);
    let mut tail = vec![0u8; tail_len];
    let mut file = File::open(path)
        .map_err(|error| ModError::io("open ZIP central directory", path, error))?;
    file.seek(SeekFrom::Start(file_bytes.saturating_sub(tail_bytes)))
        .and_then(|_| file.read_exact(&mut tail))
        .map_err(|error| ModError::io("read ZIP central directory", path, error))?;

    let end_offset = tail
        .windows(END_SIGNATURE.len())
        .enumerate()
        .rev()
        .find_map(|(offset, signature)| {
            if signature != END_SIGNATURE {
                return None;
            }
            let comment_bytes = usize::from(read_u16(&tail, offset.checked_add(20)?)?);
            (offset.checked_add(22)?.checked_add(comment_bytes)? == tail.len()).then_some(offset)
        });
    let Some(end_offset) = end_offset else {
        return Ok(None);
    };
    let Some(classic_count) = read_u16(&tail, end_offset + 10) else {
        return Ok(None);
    };
    if classic_count != u16::MAX {
        return Ok(Some(u64::from(classic_count)));
    }

    let end_global_offset = file_bytes
        .saturating_sub(tail_bytes)
        .saturating_add(u64::try_from(end_offset).unwrap_or(u64::MAX));
    let Some(locator_offset) =
        end_global_offset.checked_sub(u64::try_from(LOCATOR_BYTES).unwrap_or(u64::MAX))
    else {
        return Ok(Some(u64::from(classic_count)));
    };
    let mut locator = [0u8; LOCATOR_BYTES];
    file.seek(SeekFrom::Start(locator_offset))
        .and_then(|_| file.read_exact(&mut locator))
        .map_err(|error| ModError::io("read ZIP64 locator", path, error))?;
    if read_u32(&locator, 0) != Some(LOCATOR_SIGNATURE) {
        return Ok(Some(u64::from(classic_count)));
    }
    let Some(zip64_offset) = read_u64(&locator, 8) else {
        return Ok(None);
    };
    let mut zip64_end = [0u8; ZIP64_END_BYTES];
    file.seek(SeekFrom::Start(zip64_offset))
        .and_then(|_| file.read_exact(&mut zip64_end))
        .map_err(|error| ModError::io("read ZIP64 central directory", path, error))?;
    if read_u32(&zip64_end, 0) != Some(ZIP64_END_SIGNATURE) {
        return Ok(None);
    }
    Ok(read_u64(&zip64_end, 32))
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}
