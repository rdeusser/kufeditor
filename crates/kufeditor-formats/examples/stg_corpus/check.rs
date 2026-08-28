use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use kufeditor_formats::{FormatError, STGDocument, STGEventTarget, STGTailStatus};

const FNV64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV64_PRIME: u64 = 0x0000_0100_0000_01b3;
const MANIFEST_PREAMBLE: &str = "# kufeditor STG corpus manifest v1";
const MANIFEST_HEADER: &str = "path_fnv64\tfile_fnv64\tbytes\ttail\traw_region\traw_offset\tunits\tareas\tvariables\tblocks\tevents\tconditions\tactions\tfooter_entries\tsuffix_bytes";
const REGION_NAMES: [&str; 13] = [
    "source",
    "magic",
    "header",
    "units",
    "areas",
    "variables",
    "event_blocks",
    "events",
    "conditions",
    "actions",
    "parameters",
    "footer",
    "suffix",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TailExpectation {
    Parsed,
    Raw,
}

impl TailExpectation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::Raw => "raw",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionCount {
    Units,
    Areas,
    Variables,
    Blocks,
    Events,
    Conditions,
    Actions,
    FooterEntries,
    SuffixBytes,
}

impl RegionCount {
    pub const ALL: [Self; 9] = [
        Self::Units,
        Self::Areas,
        Self::Variables,
        Self::Blocks,
        Self::Events,
        Self::Conditions,
        Self::Actions,
        Self::FooterEntries,
        Self::SuffixBytes,
    ];

    const fn as_str(self) -> &'static str {
        match self {
            Self::Units => "units",
            Self::Areas => "areas",
            Self::Variables => "variables",
            Self::Blocks => "blocks",
            Self::Events => "events",
            Self::Conditions => "conditions",
            Self::Actions => "actions",
            Self::FooterEntries => "footer entries",
            Self::SuffixBytes => "suffix bytes",
        }
    }
}

impl fmt::Display for RegionCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegionCounts {
    pub units: Option<u64>,
    pub areas: Option<u64>,
    pub variables: Option<u64>,
    pub blocks: Option<u64>,
    pub events: Option<u64>,
    pub conditions: Option<u64>,
    pub actions: Option<u64>,
    pub footer_entries: Option<u64>,
    pub suffix_bytes: Option<u64>,
}

impl RegionCounts {
    pub const fn get(&self, region: RegionCount) -> Option<u64> {
        match region {
            RegionCount::Units => self.units,
            RegionCount::Areas => self.areas,
            RegionCount::Variables => self.variables,
            RegionCount::Blocks => self.blocks,
            RegionCount::Events => self.events,
            RegionCount::Conditions => self.conditions,
            RegionCount::Actions => self.actions,
            RegionCount::FooterEntries => self.footer_entries,
            RegionCount::SuffixBytes => self.suffix_bytes,
        }
    }

    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "used by the integration-test module, but not the example test target"
    )]
    pub const fn set(&mut self, region: RegionCount, value: Option<u64>) {
        match region {
            RegionCount::Units => self.units = value,
            RegionCount::Areas => self.areas = value,
            RegionCount::Variables => self.variables = value,
            RegionCount::Blocks => self.blocks = value,
            RegionCount::Events => self.events = value,
            RegionCount::Conditions => self.conditions = value,
            RegionCount::Actions => self.actions = value,
            RegionCount::FooterEntries => self.footer_entries = value,
            RegionCount::SuffixBytes => self.suffix_bytes = value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestRow {
    pub path_hash: u64,
    pub file_hash: u64,
    pub byte_size: u64,
    pub tail: TailExpectation,
    pub raw_region: Option<String>,
    pub raw_offset: Option<u64>,
    pub counts: RegionCounts,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CorpusManifest {
    pub rows: Vec<ManifestRow>,
}

impl CorpusManifest {
    pub fn parse(source: &str) -> Result<Self, CorpusError> {
        if source.trim().is_empty() {
            return Err(CorpusError::EmptyManifest);
        }

        let mut lines = source.lines().enumerate();
        let Some((_, preamble)) = lines.next() else {
            return Err(CorpusError::EmptyManifest);
        };
        if preamble != MANIFEST_PREAMBLE {
            return Err(invalid_manifest(1, "invalid manifest preamble"));
        }
        let Some((header_index, header)) = lines.next() else {
            return Err(invalid_manifest(2, "missing manifest header"));
        };
        if header != MANIFEST_HEADER {
            return Err(invalid_manifest(
                header_index + 1,
                "invalid manifest header",
            ));
        }

        let mut rows = Vec::new();
        let mut path_hashes = HashSet::new();
        for (index, line) in lines {
            let line_number = index + 1;
            if line.is_empty() {
                return Err(invalid_manifest(line_number, "empty manifest row"));
            }
            let row = parse_manifest_row(line_number, line)?;
            if !path_hashes.insert(row.path_hash) {
                return Err(CorpusError::DuplicateManifestPathHash {
                    path_hash: row.path_hash,
                });
            }
            rows.push(row);
        }
        if rows.is_empty() {
            return Err(CorpusError::EmptyManifest);
        }

        Ok(Self { rows })
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        output.push_str(MANIFEST_PREAMBLE);
        output.push('\n');
        output.push_str(MANIFEST_HEADER);
        output.push('\n');
        for row in &self.rows {
            use std::fmt::Write as _;
            let _ = writeln!(
                output,
                "{path_hash:016x}\t{file_hash:016x}\t{byte_size}\t{tail}\t{raw_region}\t{raw_offset}\t{units}\t{areas}\t{variables}\t{blocks}\t{events}\t{conditions}\t{actions}\t{footer_entries}\t{suffix_bytes}",
                path_hash = row.path_hash,
                file_hash = row.file_hash,
                byte_size = row.byte_size,
                tail = row.tail.as_str(),
                raw_region = optional_text(row.raw_region.as_deref()),
                raw_offset = optional_number(row.raw_offset),
                units = optional_number(row.counts.units),
                areas = optional_number(row.counts.areas),
                variables = optional_number(row.counts.variables),
                blocks = optional_number(row.counts.blocks),
                events = optional_number(row.counts.events),
                conditions = optional_number(row.counts.conditions),
                actions = optional_number(row.counts.actions),
                footer_entries = optional_number(row.counts.footer_entries),
                suffix_bytes = optional_number(row.counts.suffix_bytes),
            );
        }
        output
    }

    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "used by the integration-test module, but not the example test target"
    )]
    pub fn identity(&self) -> u64 {
        fnv64(self.render().as_bytes())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CorpusSummary {
    pub file_count: usize,
    pub parsed_tail_count: usize,
    pub raw_tail_count: usize,
    pub byte_count: u64,
    pub maximum_file_size: u64,
    pub corpus_hash: u64,
}

#[derive(Clone, Debug)]
struct CorpusFile {
    relative_path: String,
    row: ManifestRow,
}

#[derive(Clone, Debug)]
pub struct Corpus {
    files: Vec<CorpusFile>,
    summary: CorpusSummary,
}

impl Corpus {
    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "used by the integration-test module, but not the example test target"
    )]
    pub fn relative_paths(&self) -> Vec<&str> {
        self.files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect()
    }

    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "used by the integration-test module, but not the example test target"
    )]
    pub const fn summary(&self) -> CorpusSummary {
        self.summary
    }

    pub fn manifest(&self) -> CorpusManifest {
        CorpusManifest {
            rows: self.files.iter().map(|file| file.row.clone()).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckReport {
    pub summary: CorpusSummary,
    pub manifest_hash: u64,
}

#[derive(Debug)]
pub enum CorpusError {
    Usage,
    RootNotDirectory {
        root: PathBuf,
    },
    IO {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    NonUTF8Path {
        path: PathBuf,
    },
    InvalidRelativePath {
        path: PathBuf,
    },
    EmptyCorpus,
    EmptyManifest,
    NoParsedTails,
    InvalidManifest {
        line: usize,
        message: String,
    },
    DuplicateManifestPathHash {
        path_hash: u64,
    },
    DuplicateCorpusPathHash {
        first: String,
        second: String,
        path_hash: u64,
    },
    Parse {
        path: String,
        source: Box<FormatError>,
    },
    Inspect {
        path: String,
        source: Box<FormatError>,
    },
    InconsistentDocument {
        path: String,
        field: &'static str,
    },
    Encode {
        path: String,
        source: Box<FormatError>,
    },
    NonExactEncoding {
        path: String,
    },
    NumberOverflow {
        field: &'static str,
        value: usize,
    },
    AggregateOverflow {
        field: &'static str,
    },
    MissingFile {
        path_hash: u64,
    },
    ExtraFile {
        path: String,
    },
    ManifestOrder {
        path: String,
    },
    ChangedBytes {
        path: String,
        expected_hash: u64,
        actual_hash: u64,
        expected_size: u64,
        actual_size: u64,
    },
    ParsedToRaw {
        path: String,
    },
    RawToParsed {
        path: String,
    },
    ChangedRawFailure {
        path: String,
        expected_region: Option<String>,
        actual_region: Option<String>,
        expected_offset: Option<u64>,
        actual_offset: Option<u64>,
    },
    ChangedCount {
        path: String,
        region: RegionCount,
        expected: Option<u64>,
        actual: Option<u64>,
    },
}

impl fmt::Display for CorpusError {
    #[allow(
        clippy::too_many_lines,
        reason = "the typed CLI error taxonomy is rendered exhaustively in one place"
    )]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => formatter.write_str(
                "usage: stg_corpus bootstrap <explicit-root> <candidate-manifest> | stg_corpus check <checked-manifest> <explicit-root>",
            ),
            Self::RootNotDirectory { root } => {
                write!(formatter, "corpus root is not a directory: {}", root.display())
            }
            Self::IO {
                action,
                path,
                source,
            } => write!(formatter, "failed to {action} {}: {source}", path.display()),
            Self::NonUTF8Path { path } => {
                write!(formatter, "corpus path is not UTF8: {}", path.display())
            }
            Self::InvalidRelativePath { path } => write!(
                formatter,
                "corpus path cannot be normalized as a relative path: {}",
                path.display()
            ),
            Self::EmptyCorpus => formatter.write_str("STG corpus is empty"),
            Self::EmptyManifest => formatter.write_str("STG corpus manifest is empty"),
            Self::NoParsedTails => {
                formatter.write_str("STG corpus manifest contains zero parsed tails")
            }
            Self::InvalidManifest { line, message } => {
                write!(formatter, "invalid STG corpus manifest at line {line}: {message}")
            }
            Self::DuplicateManifestPathHash { path_hash } => write!(
                formatter,
                "STG corpus manifest repeats path hash {path_hash:016x}"
            ),
            Self::DuplicateCorpusPathHash {
                first,
                second,
                path_hash,
            } => write!(
                formatter,
                "STG corpus paths {first} and {second} collide at hash {path_hash:016x}"
            ),
            Self::Parse { path, source } => {
                write!(formatter, "failed to parse STG corpus file {path}: {source}")
            }
            Self::Inspect { path, source } => {
                write!(formatter, "failed to inspect STG corpus file {path}: {source}")
            }
            Self::InconsistentDocument { path, field } => {
                write!(formatter, "parsed STG corpus file {path} has no {field}")
            }
            Self::Encode { path, source } => {
                write!(formatter, "failed to encode STG corpus file {path}: {source}")
            }
            Self::NonExactEncoding { path } => {
                write!(formatter, "STG corpus file did not encode exactly: {path}")
            }
            Self::NumberOverflow { field, value } => {
                write!(formatter, "STG corpus {field} {value} does not fit u64")
            }
            Self::AggregateOverflow { field } => {
                write!(formatter, "STG corpus {field} overflowed u64")
            }
            Self::MissingFile { path_hash } => {
                write!(formatter, "STG corpus is missing path hash {path_hash:016x}")
            }
            Self::ExtraFile { path } => write!(formatter, "STG corpus has extra file {path}"),
            Self::ManifestOrder { path } => write!(
                formatter,
                "STG corpus manifest is not in normalized relative-path order at {path}"
            ),
            Self::ChangedBytes {
                path,
                expected_hash,
                actual_hash,
                expected_size,
                actual_size,
            } => write!(
                formatter,
                "STG corpus bytes changed for {path}: expected {expected_hash:016x}/{expected_size}, found {actual_hash:016x}/{actual_size}"
            ),
            Self::ParsedToRaw { path } => {
                write!(formatter, "STG corpus file regressed from parsed to raw: {path}")
            }
            Self::RawToParsed { path } => write!(
                formatter,
                "STG corpus file changed unexpectedly from raw to parsed: {path}"
            ),
            Self::ChangedRawFailure {
                path,
                expected_region,
                actual_region,
                expected_offset,
                actual_offset,
            } => write!(
                formatter,
                "STG corpus raw failure changed for {path}: expected {expected_region:?}@{expected_offset:?}, found {actual_region:?}@{actual_offset:?}"
            ),
            Self::ChangedCount {
                path,
                region,
                expected,
                actual,
            } => write!(
                formatter,
                "STG corpus {region} count changed for {path}: expected {expected:?}, found {actual:?}"
            ),
        }
    }
}

impl Error for CorpusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IO { source, .. } => Some(source),
            Self::Parse { source, .. }
            | Self::Inspect { source, .. }
            | Self::Encode { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = FNV64_OFFSET_BASIS;
    update_fnv64(&mut hash, bytes);
    hash
}

pub fn scan_corpus(root: &Path) -> Result<Corpus, CorpusError> {
    let metadata = fs::metadata(root).map_err(|source| io_error("inspect", root, source))?;
    if !metadata.is_dir() {
        return Err(CorpusError::RootNotDirectory {
            root: root.to_path_buf(),
        });
    }

    let mut paths = Vec::new();
    collect_stg_paths(root, root, &mut paths)?;
    paths.sort_by(|left, right| left.relative.cmp(&right.relative));
    if paths.is_empty() {
        return Err(CorpusError::EmptyCorpus);
    }

    let mut files = Vec::with_capacity(paths.len());
    let mut path_hashes = BTreeMap::new();
    let mut parsed_tail_count = 0_usize;
    let mut raw_tail_count = 0_usize;
    let mut byte_count = 0_u64;
    let mut maximum_file_size = 0_u64;
    let mut corpus_hash = FNV64_OFFSET_BASIS;

    for path in paths {
        let bytes = fs::read(&path.full).map_err(|source| io_error("read", &path.full, source))?;
        let byte_size = to_u64(bytes.len(), "file size")?;
        byte_count = byte_count
            .checked_add(byte_size)
            .ok_or(CorpusError::AggregateOverflow {
                field: "byte count",
            })?;
        maximum_file_size = maximum_file_size.max(byte_size);
        update_corpus_hash(&mut corpus_hash, &path.relative, &bytes)?;

        let path_hash = fnv64(path.relative.as_bytes());
        if let Some(first) = path_hashes.insert(path_hash, path.relative.clone()) {
            return Err(CorpusError::DuplicateCorpusPathHash {
                first,
                second: path.relative,
                path_hash,
            });
        }

        let file_hash = fnv64(&bytes);
        let document = STGDocument::parse(bytes.clone()).map_err(|source| CorpusError::Parse {
            path: path.relative.clone(),
            source: Box::new(source),
        })?;
        let encoded = document.encode().map_err(|source| CorpusError::Encode {
            path: path.relative.clone(),
            source: Box::new(source),
        })?;
        if encoded != bytes {
            return Err(CorpusError::NonExactEncoding {
                path: path.relative,
            });
        }

        let (tail, raw_region, raw_offset, counts) = inspect_document(&path.relative, &document)?;
        match tail {
            TailExpectation::Parsed => parsed_tail_count += 1,
            TailExpectation::Raw => raw_tail_count += 1,
        }
        files.push(CorpusFile {
            relative_path: path.relative,
            row: ManifestRow {
                path_hash,
                file_hash,
                byte_size,
                tail,
                raw_region,
                raw_offset,
                counts,
            },
        });
    }

    Ok(Corpus {
        summary: CorpusSummary {
            file_count: files.len(),
            parsed_tail_count,
            raw_tail_count,
            byte_count,
            maximum_file_size,
            corpus_hash,
        },
        files,
    })
}

pub fn verify_manifest(
    expected: &CorpusManifest,
    actual: &Corpus,
) -> Result<CorpusSummary, CorpusError> {
    if expected.rows.is_empty() {
        return Err(CorpusError::EmptyManifest);
    }
    if !expected
        .rows
        .iter()
        .any(|row| row.tail == TailExpectation::Parsed)
    {
        return Err(CorpusError::NoParsedTails);
    }

    let mut expected_hashes = HashSet::new();
    for row in &expected.rows {
        if !expected_hashes.insert(row.path_hash) {
            return Err(CorpusError::DuplicateManifestPathHash {
                path_hash: row.path_hash,
            });
        }
    }
    let actual_by_hash: BTreeMap<_, _> = actual
        .files
        .iter()
        .map(|file| (file.row.path_hash, file))
        .collect();

    for expected_row in &expected.rows {
        let Some(actual_file) = actual_by_hash.get(&expected_row.path_hash) else {
            return Err(CorpusError::MissingFile {
                path_hash: expected_row.path_hash,
            });
        };
        compare_row(expected_row, actual_file)?;
    }
    for actual_file in &actual.files {
        if !expected_hashes.contains(&actual_file.row.path_hash) {
            return Err(CorpusError::ExtraFile {
                path: actual_file.relative_path.clone(),
            });
        }
    }
    for (expected_row, actual_file) in expected.rows.iter().zip(&actual.files) {
        if expected_row.path_hash != actual_file.row.path_hash {
            return Err(CorpusError::ManifestOrder {
                path: actual_file.relative_path.clone(),
            });
        }
    }

    Ok(actual.summary)
}

pub fn check_corpus(manifest_path: &Path, root: &Path) -> Result<CheckReport, CorpusError> {
    let source = fs::read_to_string(manifest_path)
        .map_err(|error| io_error("read", manifest_path, error))?;
    let manifest_hash = fnv64(source.as_bytes());
    let manifest = CorpusManifest::parse(&source)?;
    let corpus = scan_corpus(root)?;
    let summary = verify_manifest(&manifest, &corpus)?;
    Ok(CheckReport {
        summary,
        manifest_hash,
    })
}

fn bootstrap_corpus(root: &Path, candidate_path: &Path) -> Result<CheckReport, CorpusError> {
    let corpus = scan_corpus(root)?;
    if corpus.summary.parsed_tail_count == 0 {
        return Err(CorpusError::NoParsedTails);
    }
    let manifest = corpus.manifest();
    let source = manifest.render();
    let mut candidate = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(candidate_path)
        .map_err(|error| io_error("create", candidate_path, error))?;
    candidate
        .write_all(source.as_bytes())
        .map_err(|error| io_error("write", candidate_path, error))?;
    candidate
        .flush()
        .map_err(|error| io_error("flush", candidate_path, error))?;

    Ok(CheckReport {
        summary: corpus.summary,
        manifest_hash: fnv64(source.as_bytes()),
    })
}

pub fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<String, CorpusError> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    let (mode, report) = match arguments.as_slice() {
        [mode, root, candidate] if mode == OsStr::new("bootstrap") => (
            "bootstrap-candidate",
            bootstrap_corpus(Path::new(root), Path::new(candidate))?,
        ),
        [mode, manifest, root] if mode == OsStr::new("check") => {
            ("check", check_corpus(Path::new(manifest), Path::new(root))?)
        }
        _ => return Err(CorpusError::Usage),
    };
    let gate = if mode == "check" { "passed" } else { "not-run" };
    Ok(format_report(mode, gate, report))
}

fn format_report(mode: &str, gate: &str, report: CheckReport) -> String {
    format!(
        "mode={mode}\ngate={gate}\nmanifest_fnv64={manifest_hash:016x}\ncorpus_fnv64={corpus_hash:016x}\nfiles={file_count}\nparsed_tails={parsed_tail_count}\nraw_tails={raw_tail_count}\nbytes={byte_count}\nmaximum_file_size={maximum_file_size}",
        manifest_hash = report.manifest_hash,
        corpus_hash = report.summary.corpus_hash,
        file_count = report.summary.file_count,
        parsed_tail_count = report.summary.parsed_tail_count,
        raw_tail_count = report.summary.raw_tail_count,
        byte_count = report.summary.byte_count,
        maximum_file_size = report.summary.maximum_file_size,
    )
}

#[derive(Debug)]
struct CorpusPath {
    full: PathBuf,
    relative: String,
}

fn collect_stg_paths(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<CorpusPath>,
) -> Result<(), CorpusError> {
    let entries = fs::read_dir(directory).map_err(|error| io_error("read", directory, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error("read", directory, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| io_error("inspect", &entry.path(), error))?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_stg_paths(root, &path, paths)?;
        } else if file_type.is_file() && has_stg_extension(&path) {
            let relative = normalize_relative_path(root, &path)?;
            paths.push(CorpusPath {
                full: path,
                relative,
            });
        }
    }
    Ok(())
}

fn has_stg_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("stg"))
}

fn normalize_relative_path(root: &Path, path: &Path) -> Result<String, CorpusError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CorpusError::InvalidRelativePath {
            path: path.to_path_buf(),
        })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(CorpusError::InvalidRelativePath {
                path: path.to_path_buf(),
            });
        };
        let Some(part) = part.to_str() else {
            return Err(CorpusError::NonUTF8Path {
                path: path.to_path_buf(),
            });
        };
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(CorpusError::InvalidRelativePath {
            path: path.to_path_buf(),
        });
    }
    Ok(parts.join("/"))
}

fn inspect_document(
    relative_path: &str,
    document: &STGDocument,
) -> Result<(TailExpectation, Option<String>, Option<u64>, RegionCounts), CorpusError> {
    let units = Some(to_u64(document.unit_count(), "unit count")?);
    match document.tail_status() {
        STGTailStatus::Raw { failure, .. } => Ok((
            TailExpectation::Raw,
            Some(region_name(failure.region()).to_owned()),
            Some(to_u64(failure.offset(), "raw failure offset")?),
            RegionCounts {
                units,
                ..RegionCounts::default()
            },
        )),
        STGTailStatus::Parsed { suffix } => {
            let areas = required_count(document.area_count(), relative_path, "area count")?;
            let variables =
                required_count(document.variable_count(), relative_path, "variable count")?;
            let blocks = required_count(
                document.event_block_count(),
                relative_path,
                "event block count",
            )?;
            let footer_entries =
                required_count(document.footer_count(), relative_path, "footer count")?;
            let mut events = 0_u64;
            let mut conditions = 0_u64;
            let mut actions = 0_u64;
            let block_count =
                document
                    .event_block_count()
                    .ok_or_else(|| CorpusError::InconsistentDocument {
                        path: relative_path.to_owned(),
                        field: "event block count",
                    })?;
            for block_index in 0..block_count {
                let block =
                    document
                        .event_block(block_index)
                        .map_err(|source| CorpusError::Inspect {
                            path: relative_path.to_owned(),
                            source: Box::new(source),
                        })?;
                events = add_count(events, block.event_count, "event count")?;
                for event_index in 0..block.event_count {
                    let event = document
                        .event(STGEventTarget {
                            block: block_index,
                            event: event_index,
                        })
                        .map_err(|source| CorpusError::Inspect {
                            path: relative_path.to_owned(),
                            source: Box::new(source),
                        })?;
                    conditions = add_count(conditions, event.condition_count, "condition count")?;
                    actions = add_count(actions, event.action_count, "action count")?;
                }
            }

            Ok((
                TailExpectation::Parsed,
                None,
                None,
                RegionCounts {
                    units,
                    areas: Some(areas),
                    variables: Some(variables),
                    blocks: Some(blocks),
                    events: Some(events),
                    conditions: Some(conditions),
                    actions: Some(actions),
                    footer_entries: Some(footer_entries),
                    suffix_bytes: Some(to_u64(suffix.len(), "suffix byte count")?),
                },
            ))
        }
    }
}

fn compare_row(expected: &ManifestRow, actual_file: &CorpusFile) -> Result<(), CorpusError> {
    let actual = &actual_file.row;
    let path = &actual_file.relative_path;
    match (expected.tail, actual.tail) {
        (TailExpectation::Parsed, TailExpectation::Raw) => {
            return Err(CorpusError::ParsedToRaw { path: path.clone() });
        }
        (TailExpectation::Raw, TailExpectation::Parsed) => {
            return Err(CorpusError::RawToParsed { path: path.clone() });
        }
        (TailExpectation::Parsed, TailExpectation::Parsed)
        | (TailExpectation::Raw, TailExpectation::Raw) => {}
    }
    if expected.tail == TailExpectation::Raw
        && (expected.raw_region != actual.raw_region || expected.raw_offset != actual.raw_offset)
    {
        return Err(CorpusError::ChangedRawFailure {
            path: path.clone(),
            expected_region: expected.raw_region.clone(),
            actual_region: actual.raw_region.clone(),
            expected_offset: expected.raw_offset,
            actual_offset: actual.raw_offset,
        });
    }
    for region in RegionCount::ALL {
        let expected_count = expected.counts.get(region);
        let actual_count = actual.counts.get(region);
        if expected_count != actual_count {
            return Err(CorpusError::ChangedCount {
                path: path.clone(),
                region,
                expected: expected_count,
                actual: actual_count,
            });
        }
    }
    if expected.file_hash != actual.file_hash || expected.byte_size != actual.byte_size {
        return Err(CorpusError::ChangedBytes {
            path: path.clone(),
            expected_hash: expected.file_hash,
            actual_hash: actual.file_hash,
            expected_size: expected.byte_size,
            actual_size: actual.byte_size,
        });
    }
    Ok(())
}

fn parse_manifest_row(line_number: usize, line: &str) -> Result<ManifestRow, CorpusError> {
    let fields: Vec<_> = line.split('\t').collect();
    let Ok(
        [
            path_hash,
            file_hash,
            byte_size,
            tail,
            raw_region,
            raw_offset,
            units,
            areas,
            variables,
            blocks,
            events,
            conditions,
            actions,
            footer_entries,
            suffix_bytes,
        ],
    ) = <[&str; 15]>::try_from(fields)
    else {
        return Err(invalid_manifest(
            line_number,
            "manifest row must contain 15 tab-separated fields",
        ));
    };

    let tail = match tail {
        "parsed" => TailExpectation::Parsed,
        "raw" => TailExpectation::Raw,
        _ => return Err(invalid_manifest(line_number, "tail must be parsed or raw")),
    };
    let raw_region = parse_optional_region(line_number, raw_region)?;
    let raw_offset = parse_optional_u64(line_number, "raw_offset", raw_offset)?;
    let counts = RegionCounts {
        units: parse_optional_u64(line_number, "units", units)?,
        areas: parse_optional_u64(line_number, "areas", areas)?,
        variables: parse_optional_u64(line_number, "variables", variables)?,
        blocks: parse_optional_u64(line_number, "blocks", blocks)?,
        events: parse_optional_u64(line_number, "events", events)?,
        conditions: parse_optional_u64(line_number, "conditions", conditions)?,
        actions: parse_optional_u64(line_number, "actions", actions)?,
        footer_entries: parse_optional_u64(line_number, "footer_entries", footer_entries)?,
        suffix_bytes: parse_optional_u64(line_number, "suffix_bytes", suffix_bytes)?,
    };
    validate_row_shape(
        line_number,
        tail,
        raw_region.as_deref(),
        raw_offset,
        &counts,
    )?;

    Ok(ManifestRow {
        path_hash: parse_hash(line_number, "path_fnv64", path_hash)?,
        file_hash: parse_hash(line_number, "file_fnv64", file_hash)?,
        byte_size: parse_u64(line_number, "bytes", byte_size)?,
        tail,
        raw_region,
        raw_offset,
        counts,
    })
}

fn validate_row_shape(
    line_number: usize,
    tail: TailExpectation,
    raw_region: Option<&str>,
    raw_offset: Option<u64>,
    counts: &RegionCounts,
) -> Result<(), CorpusError> {
    match tail {
        TailExpectation::Parsed => {
            if raw_region.is_some() || raw_offset.is_some() {
                return Err(invalid_manifest(
                    line_number,
                    "parsed row must not contain a raw failure",
                ));
            }
            if RegionCount::ALL
                .into_iter()
                .any(|region| counts.get(region).is_none())
            {
                return Err(invalid_manifest(
                    line_number,
                    "parsed row must contain every region count",
                ));
            }
        }
        TailExpectation::Raw => {
            if raw_region.is_none() || raw_offset.is_none() {
                return Err(invalid_manifest(
                    line_number,
                    "raw row must contain a failure region and offset",
                ));
            }
            if counts.units.is_none()
                || RegionCount::ALL
                    .into_iter()
                    .filter(|region| *region != RegionCount::Units)
                    .any(|region| counts.get(region).is_some())
            {
                return Err(invalid_manifest(
                    line_number,
                    "raw row must contain only the unit count",
                ));
            }
        }
    }
    Ok(())
}

fn parse_hash(line: usize, field: &'static str, value: &str) -> Result<u64, CorpusError> {
    if value.len() != 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_manifest(
            line,
            format!("{field} must be 16 hexadecimal digits"),
        ));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| invalid_manifest(line, format!("{field} is outside the u64 range")))
}

fn parse_u64(line: usize, field: &'static str, value: &str) -> Result<u64, CorpusError> {
    value
        .parse()
        .map_err(|_| invalid_manifest(line, format!("{field} must be an unsigned integer")))
}

fn parse_optional_u64(
    line: usize,
    field: &'static str,
    value: &str,
) -> Result<Option<u64>, CorpusError> {
    if value == "-" {
        Ok(None)
    } else {
        parse_u64(line, field, value).map(Some)
    }
}

fn parse_optional_region(line: usize, value: &str) -> Result<Option<String>, CorpusError> {
    if value == "-" {
        return Ok(None);
    }
    if REGION_NAMES.contains(&value) {
        Ok(Some(value.to_owned()))
    } else {
        Err(invalid_manifest(line, "unknown raw failure region"))
    }
}

fn region_name(region: kufeditor_formats::STGRegion) -> &'static str {
    use kufeditor_formats::STGRegion;
    match region {
        STGRegion::Source => "source",
        STGRegion::Magic => "magic",
        STGRegion::Header => "header",
        STGRegion::Units => "units",
        STGRegion::Areas => "areas",
        STGRegion::Variables => "variables",
        STGRegion::EventBlocks => "event_blocks",
        STGRegion::Events => "events",
        STGRegion::Conditions => "conditions",
        STGRegion::Actions => "actions",
        STGRegion::Parameters => "parameters",
        STGRegion::Footer => "footer",
        STGRegion::Suffix => "suffix",
    }
}

fn optional_text(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn optional_number(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_owned(), |number| number.to_string())
}

fn required_count(
    value: Option<usize>,
    path: &str,
    field: &'static str,
) -> Result<u64, CorpusError> {
    let Some(value) = value else {
        return Err(CorpusError::InconsistentDocument {
            path: path.to_owned(),
            field,
        });
    };
    to_u64(value, field)
}

fn add_count(total: u64, value: usize, field: &'static str) -> Result<u64, CorpusError> {
    total
        .checked_add(to_u64(value, field)?)
        .ok_or(CorpusError::AggregateOverflow { field })
}

fn to_u64(value: usize, field: &'static str) -> Result<u64, CorpusError> {
    u64::try_from(value).map_err(|_| CorpusError::NumberOverflow { field, value })
}

fn update_corpus_hash(
    hash: &mut u64,
    relative_path: &str,
    bytes: &[u8],
) -> Result<(), CorpusError> {
    let path_length = to_u64(relative_path.len(), "relative path length")?;
    let byte_length = to_u64(bytes.len(), "file size")?;
    update_fnv64(hash, &path_length.to_le_bytes());
    update_fnv64(hash, relative_path.as_bytes());
    update_fnv64(hash, &byte_length.to_le_bytes());
    update_fnv64(hash, bytes);
    Ok(())
}

fn update_fnv64(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV64_PRIME);
    }
}

fn invalid_manifest(line: usize, message: impl Into<String>) -> CorpusError {
    CorpusError::InvalidManifest {
        line,
        message: message.into(),
    }
}

fn io_error(action: &'static str, path: &Path, source: io::Error) -> CorpusError {
    CorpusError::IO {
        action,
        path: path.to_path_buf(),
        source,
    }
}
