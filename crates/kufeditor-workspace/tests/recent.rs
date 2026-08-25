use std::path::{Path, PathBuf};

use kufeditor_workspace::{
    DEFAULT_RECENT_FILE_LIMIT, RECENT_FILE_LIMITS, RecentFiles, normalize_recent_limit,
};

#[test]
fn single_add_moves_exact_duplicate_to_front() {
    let mut recent = RecentFiles::default();
    assert!(recent.add(PathBuf::from("A.sox")));
    assert!(recent.add(PathBuf::from("B.sox")));
    assert_eq!(
        recent.paths(),
        &[PathBuf::from("B.sox"), PathBuf::from("A.sox")],
    );

    assert!(recent.add(PathBuf::from("A.sox")));
    assert_eq!(
        recent.paths(),
        &[PathBuf::from("A.sox"), PathBuf::from("B.sox")],
    );
}

#[test]
fn paths_are_compared_exactly_without_case_or_canonicalization() {
    let mut recent = RecentFiles::default();

    assert!(recent.add(PathBuf::from("A.sox")));
    assert!(recent.add(PathBuf::from("a.sox")));
    assert!(recent.add(PathBuf::from("./A.sox")));

    assert_eq!(
        recent.paths(),
        &[
            PathBuf::from("./A.sox"),
            PathBuf::from("a.sox"),
            PathBuf::from("A.sox"),
        ],
    );
}

#[test]
fn adding_an_already_front_path_reports_no_change() {
    let mut recent =
        RecentFiles::from_persisted(10, vec![PathBuf::from("A.sox"), PathBuf::from("B.sox")]);

    assert!(!recent.add(PathBuf::from("A.sox")));
    assert_eq!(
        recent.paths(),
        &[PathBuf::from("A.sox"), PathBuf::from("B.sox")],
    );
}

#[test]
fn batch_add_keeps_first_occurrences_in_input_order() {
    let mut recent = RecentFiles::default();

    assert!(recent.add_batch(vec![
        PathBuf::from("A.sox"),
        PathBuf::from("B.sox"),
        PathBuf::from("A.sox"),
        PathBuf::from("C.sox"),
        PathBuf::from("B.sox"),
    ]));
    assert_eq!(
        recent.paths(),
        &[
            PathBuf::from("A.sox"),
            PathBuf::from("B.sox"),
            PathBuf::from("C.sox"),
        ],
    );
}

#[test]
fn batch_add_moves_existing_duplicates_into_the_batch_prefix() {
    let mut recent = RecentFiles::from_persisted(
        10,
        vec![
            PathBuf::from("D.sox"),
            PathBuf::from("E.sox"),
            PathBuf::from("B.sox"),
        ],
    );

    assert!(recent.add_batch(vec![
        PathBuf::from("A.sox"),
        PathBuf::from("B.sox"),
        PathBuf::from("C.sox"),
    ]));
    assert_eq!(
        recent.paths(),
        &[
            PathBuf::from("A.sox"),
            PathBuf::from("B.sox"),
            PathBuf::from("C.sox"),
            PathBuf::from("D.sox"),
            PathBuf::from("E.sox"),
        ],
    );
}

#[test]
fn supported_limits_and_nearest_normalization_are_stable() {
    assert_eq!(RECENT_FILE_LIMITS, [5, 10, 15, 20]);
    assert_eq!(DEFAULT_RECENT_FILE_LIMIT, 10);

    assert_eq!(normalize_recent_limit(7), 5);
    assert_eq!(normalize_recent_limit(8), 10);
    assert_eq!(normalize_recent_limit(12), 10);
    assert_eq!(normalize_recent_limit(13), 15);
    assert_eq!(normalize_recent_limit(17), 15);
    assert_eq!(normalize_recent_limit(18), 20);
}

#[test]
fn unsupported_limits_clamp_to_the_nearest_supported_limit() {
    assert_eq!(normalize_recent_limit(0), 5);
    assert_eq!(normalize_recent_limit(4), 5);
    assert_eq!(normalize_recent_limit(21), 20);
    assert_eq!(normalize_recent_limit(usize::MAX), 20);
}

#[test]
fn reducing_limit_truncates_immediately_and_reports_only_changes() {
    let mut recent = RecentFiles::new(20);
    assert!(recent.add_batch(vec![
        PathBuf::from("A.sox"),
        PathBuf::from("B.sox"),
        PathBuf::from("C.sox"),
        PathBuf::from("D.sox"),
        PathBuf::from("E.sox"),
        PathBuf::from("F.sox"),
        PathBuf::from("G.sox"),
    ]));

    assert!(recent.set_limit(10));
    assert_eq!(recent.limit(), 10);
    assert_eq!(recent.paths().len(), 7);

    assert!(!recent.set_limit(8));
    assert_eq!(recent.limit(), 10);

    assert!(recent.set_limit(5));
    assert_eq!(recent.limit(), 5);
    assert_eq!(
        recent.paths(),
        &[
            PathBuf::from("A.sox"),
            PathBuf::from("B.sox"),
            PathBuf::from("C.sox"),
            PathBuf::from("D.sox"),
            PathBuf::from("E.sox"),
        ],
    );

    assert!(!recent.set_limit(7));
    assert!(!recent.set_limit(5));
}

#[test]
fn persisted_paths_keep_first_exact_occurrence_and_active_limit() {
    let recent = RecentFiles::from_persisted(
        5,
        vec![
            PathBuf::from("A.sox"),
            PathBuf::from("B.sox"),
            PathBuf::from("A.sox"),
            PathBuf::from("a.sox"),
            PathBuf::from("C.sox"),
            PathBuf::from("B.sox"),
            PathBuf::from("D.sox"),
            PathBuf::from("E.sox"),
        ],
    );

    assert_eq!(recent.limit(), 5);
    assert_eq!(
        recent.paths(),
        &[
            PathBuf::from("A.sox"),
            PathBuf::from("B.sox"),
            PathBuf::from("a.sox"),
            PathBuf::from("C.sox"),
            PathBuf::from("D.sox"),
        ],
    );
}

#[test]
fn remove_and_clear_report_whether_they_changed_the_collection() {
    let mut recent =
        RecentFiles::from_persisted(10, vec![PathBuf::from("A.sox"), PathBuf::from("B.sox")]);

    assert!(recent.remove(Path::new("A.sox")));
    assert_eq!(recent.paths(), &[PathBuf::from("B.sox")]);
    assert!(!recent.remove(Path::new("A.sox")));

    assert!(recent.clear());
    assert!(recent.paths().is_empty());
    assert!(!recent.clear());
}

#[test]
fn default_has_no_paths_and_uses_the_default_limit() {
    let recent = RecentFiles::default();

    assert_eq!(recent.limit(), DEFAULT_RECENT_FILE_LIMIT);
    assert!(recent.paths().is_empty());
}
