use std::{
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "../examples/stg_corpus/check.rs"]
mod check;
#[path = "support/stg.rs"]
#[allow(
    dead_code,
    reason = "the shared STG fixture exposes mutation offsets used by another integration test"
)]
mod stg_support;

use check::{
    CorpusError, CorpusManifest, RegionCount, TailExpectation, check_corpus, scan_corpus,
    verify_manifest,
};
use stg_support::{
    SYNTHETIC_EMPTY_STG_PATH, SYNTHETIC_PARSED_STG_PATH, SYNTHETIC_RAW_STG_PATH,
    complete_stg_fixture, synthetic_raw_stg_fixture, write_synthetic_stg_corpus,
};

static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn new(label: &str) -> io::Result<Self> {
        let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "kufeditor-stg-corpus-{}-{label}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn stg_corpus_scans_mixed_case_extensions_in_root_independent_order() {
    let scratch = ScratchDirectory::new("scan").unwrap();
    write_synthetic_stg_corpus(scratch.path()).unwrap();

    let corpus = scan_corpus(scratch.path()).unwrap();
    let summary = corpus.summary();

    assert_eq!(
        corpus.relative_paths(),
        [
            SYNTHETIC_PARSED_STG_PATH,
            SYNTHETIC_EMPTY_STG_PATH,
            SYNTHETIC_RAW_STG_PATH,
        ]
    );
    assert_eq!(summary.file_count, 3);
    assert_eq!(summary.byte_count, 3_649);
    assert_eq!(summary.maximum_file_size, 1_826);
    assert_eq!(summary.parsed_tail_count, 2);
    assert_eq!(summary.raw_tail_count, 1);

    let rows = &corpus.manifest().rows;
    let parsed = rows
        .iter()
        .find(|row| row.path_hash == check::fnv64(SYNTHETIC_PARSED_STG_PATH.as_bytes()))
        .unwrap();
    assert_eq!(parsed.tail, TailExpectation::Parsed);
    assert_eq!(parsed.counts.units, Some(1));
    assert_eq!(parsed.counts.areas, Some(1));
    assert_eq!(parsed.counts.variables, Some(4));
    assert_eq!(parsed.counts.blocks, Some(2));
    assert_eq!(parsed.counts.events, Some(2));
    assert_eq!(parsed.counts.conditions, Some(1));
    assert_eq!(parsed.counts.actions, Some(1));
    assert_eq!(parsed.counts.footer_entries, Some(2));
    assert_eq!(parsed.counts.suffix_bytes, Some(4));

    let raw = rows
        .iter()
        .find(|row| row.path_hash == check::fnv64(SYNTHETIC_RAW_STG_PATH.as_bytes()))
        .unwrap();
    assert_eq!(raw.tail, TailExpectation::Raw);
    assert_eq!(raw.raw_region.as_deref(), Some("areas"));
    assert_eq!(raw.raw_offset, Some(1_172));
    assert_eq!(raw.counts.units, Some(1));
    assert_eq!(raw.counts.areas, None);

    assert_eq!(summary.corpus_hash, 0x5e1a_f90d_4870_666f);
    assert_eq!(corpus.manifest().identity(), 0x84bb_1a02_4707_3191);
}

#[test]
fn stg_corpus_names_the_relative_path_of_an_invalid_file() {
    let scratch = ScratchDirectory::new("invalid").unwrap();
    write_synthetic_stg_corpus(scratch.path()).unwrap();
    let invalid = "nested/bad.STG";
    fs::write(scratch.path().join(invalid), b"invalid").unwrap();

    let error = scan_corpus(scratch.path()).unwrap_err();

    assert!(matches!(error, CorpusError::Parse { .. }));
    assert!(error.to_string().contains(invalid));
}

#[test]
fn stg_corpus_check_rejects_empty_and_drifted_inputs() {
    let scratch = ScratchDirectory::new("drift").unwrap();
    write_synthetic_stg_corpus(scratch.path()).unwrap();
    let corpus = scan_corpus(scratch.path()).unwrap();
    let expected = corpus.manifest();

    let empty_root = ScratchDirectory::new("empty-root").unwrap();
    assert!(matches!(
        scan_corpus(empty_root.path()),
        Err(CorpusError::EmptyCorpus)
    ));

    assert!(matches!(
        verify_manifest(&CorpusManifest::default(), &corpus),
        Err(CorpusError::EmptyManifest)
    ));

    let mut no_parsed = expected.clone();
    no_parsed
        .rows
        .retain(|row| row.tail == TailExpectation::Raw);
    assert!(matches!(
        verify_manifest(&no_parsed, &corpus),
        Err(CorpusError::NoParsedTails)
    ));

    let mut missing = expected.clone();
    let mut fabricated = missing.rows.first().unwrap().clone();
    fabricated.path_hash ^= 1;
    missing.rows.push(fabricated);
    assert!(matches!(
        verify_manifest(&missing, &corpus),
        Err(CorpusError::MissingFile { .. })
    ));

    let mut extra = expected.clone();
    extra.rows.pop();
    assert!(matches!(
        verify_manifest(&extra, &corpus),
        Err(CorpusError::ExtraFile { .. })
    ));

    let changed_path = scratch.path().join(SYNTHETIC_PARSED_STG_PATH);
    let mut changed_bytes = fs::read(&changed_path).unwrap();
    let last = changed_bytes.last_mut().unwrap();
    *last ^= 1;
    fs::write(&changed_path, changed_bytes).unwrap();
    let changed = scan_corpus(scratch.path()).unwrap();
    assert!(matches!(
        verify_manifest(&expected, &changed),
        Err(CorpusError::ChangedBytes { .. })
    ));
}

#[test]
fn stg_corpus_check_rejects_tail_and_region_count_drift() {
    let scratch = ScratchDirectory::new("tail-drift").unwrap();
    write_synthetic_stg_corpus(scratch.path()).unwrap();
    let baseline = scan_corpus(scratch.path()).unwrap();
    let expected = baseline.manifest();

    fs::write(
        scratch.path().join(SYNTHETIC_PARSED_STG_PATH),
        synthetic_raw_stg_fixture(),
    )
    .unwrap();
    let parsed_to_raw = scan_corpus(scratch.path()).unwrap();
    assert!(matches!(
        verify_manifest(&expected, &parsed_to_raw),
        Err(CorpusError::ParsedToRaw { .. })
    ));

    write_synthetic_stg_corpus(scratch.path()).unwrap();
    fs::write(
        scratch.path().join(SYNTHETIC_RAW_STG_PATH),
        complete_stg_fixture().bytes,
    )
    .unwrap();
    let raw_to_parsed = scan_corpus(scratch.path()).unwrap();
    assert!(matches!(
        verify_manifest(&expected, &raw_to_parsed),
        Err(CorpusError::RawToParsed { .. })
    ));

    let raw_row = expected
        .rows
        .iter()
        .position(|row| row.tail == TailExpectation::Raw)
        .unwrap();
    for change in [RawFailureChange::Region, RawFailureChange::Offset] {
        let mut changed = expected.clone();
        match change {
            RawFailureChange::Region => {
                changed.rows.get_mut(raw_row).unwrap().raw_region = Some("footer".to_owned());
            }
            RawFailureChange::Offset => {
                changed.rows.get_mut(raw_row).unwrap().raw_offset = Some(9_999);
            }
        }
        assert!(matches!(
            verify_manifest(&changed, &baseline),
            Err(CorpusError::ChangedRawFailure { .. })
        ));
    }

    let parsed_row = expected
        .rows
        .iter()
        .position(|row| row.tail == TailExpectation::Parsed)
        .unwrap();
    for region in RegionCount::ALL {
        let mut changed = expected.clone();
        let row = changed.rows.get_mut(parsed_row).unwrap();
        let current = row.counts.get(region).unwrap();
        row.counts.set(region, Some(current + 1));
        assert!(matches!(
            verify_manifest(&changed, &baseline),
            Err(CorpusError::ChangedCount {
                region: actual,
                ..
            }) if actual == region
        ));
    }
}

#[test]
fn stg_corpus_manifest_parser_rejects_invalid_rows() {
    assert!(matches!(
        CorpusManifest::parse(""),
        Err(CorpusError::EmptyManifest)
    ));
    assert!(matches!(
        CorpusManifest::parse("not a corpus manifest\n"),
        Err(CorpusError::InvalidManifest { .. })
    ));
}

#[test]
fn stg_corpus_cli_keeps_bootstrap_separate_from_the_checked_gate() {
    let scratch = ScratchDirectory::new("cli").unwrap();
    write_synthetic_stg_corpus(scratch.path()).unwrap();
    let candidate = scratch.path().join("candidate.tsv");

    let bootstrap = check::run([
        OsString::from("bootstrap"),
        scratch.path().as_os_str().to_owned(),
        candidate.as_os_str().to_owned(),
    ])
    .unwrap();

    assert!(bootstrap.contains("mode=bootstrap-candidate"));
    assert!(bootstrap.contains("gate=not-run"));
    assert!(!bootstrap.contains("gate=passed"));
    assert!(!bootstrap.contains(scratch.path().to_string_lossy().as_ref()));
    assert_eq!(
        fs::read_to_string(&candidate).unwrap(),
        scan_corpus(scratch.path()).unwrap().manifest().render()
    );
    assert!(matches!(
        check::run([
            OsString::from("bootstrap"),
            scratch.path().as_os_str().to_owned(),
            candidate.as_os_str().to_owned(),
        ]),
        Err(CorpusError::IO { .. })
    ));

    let checked_manifest = checked_manifest_path();
    let checked_manifest_before = fs::read(&checked_manifest).unwrap();
    let checked = check::run([
        OsString::from("check"),
        checked_manifest.as_os_str().to_owned(),
        scratch.path().as_os_str().to_owned(),
    ])
    .unwrap();
    assert!(checked.contains("mode=check"));
    assert!(checked.contains("gate=passed"));
    assert!(!checked.contains(scratch.path().to_string_lossy().as_ref()));
    assert_eq!(fs::read(checked_manifest).unwrap(), checked_manifest_before);

    assert!(matches!(
        check::run([OsString::from("check")]),
        Err(CorpusError::Usage)
    ));
}

#[test]
fn checked_synthetic_corpus_gate() {
    let scratch = ScratchDirectory::new("checked-gate").unwrap();
    write_synthetic_stg_corpus(scratch.path()).unwrap();
    let manifest = checked_manifest_path();

    let report = check_corpus(&manifest, scratch.path()).unwrap();

    assert_eq!(report.summary.file_count, 3);
    assert_eq!(report.summary.parsed_tail_count, 2);
    assert_eq!(report.summary.raw_tail_count, 1);
    assert_eq!(report.summary.byte_count, 3_649);
    assert_eq!(report.summary.maximum_file_size, 1_826);
    assert_eq!(report.summary.corpus_hash, 0x5e1a_f90d_4870_666f);
    assert_eq!(report.manifest_hash, 0x84bb_1a02_4707_3191);
}

fn checked_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stg-synthetic-manifest.tsv")
}

#[derive(Clone, Copy)]
enum RawFailureChange {
    Region,
    Offset,
}
