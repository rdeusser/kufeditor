#![allow(
    clippy::unwrap_used,
    reason = "controlled temporary filesystem fixtures make setup failures fatal"
)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use kufeditor_game::{
    Game, GameInstallation, GamePaths, InstallationError, scan_steam_common_directories,
};
use tempfile::TempDir;

#[test]
fn game_paths_keep_configured_roots_independent() {
    let mut paths = GamePaths::default();
    assert_eq!(paths.root(Game::Crusaders), None);
    assert_eq!(paths.root(Game::Heroes), None);

    paths.set_root(
        Game::Crusaders,
        Some(PathBuf::from("C:/Games/KUF Crusader")),
    );
    paths.set_root(Game::Heroes, Some(PathBuf::from("C:/Games/KUF Heroes")));
    assert_eq!(
        paths.root(Game::Crusaders),
        Some(Path::new("C:/Games/KUF Crusader")),
    );
    assert_eq!(
        paths.root(Game::Heroes),
        Some(Path::new("C:/Games/KUF Heroes")),
    );

    paths.set_root(Game::Crusaders, None);
    assert_eq!(paths.root(Game::Crusaders), None);
    assert_eq!(
        paths.root(Game::Heroes),
        Some(Path::new("C:/Games/KUF Heroes")),
    );
}

#[test]
fn installation_inspection_reports_missing_root() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("missing");

    let error = GameInstallation::inspect(Game::Crusaders, &root).unwrap_err();

    match error {
        InstallationError::RootMissing {
            game: Game::Crusaders,
            root: actual_root,
        } => assert_eq!(actual_root, root),
        other => panic!("expected RootMissing, got {other:?}"),
    }
}

#[test]
fn installation_inspection_reports_file_root() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("root-file");
    fs::write(&root, []).unwrap();

    let error = GameInstallation::inspect(Game::Heroes, &root).unwrap_err();

    match error {
        InstallationError::RootNotDirectory {
            game: Game::Heroes,
            root: actual_root,
        } => assert_eq!(actual_root, root),
        other => panic!("expected RootNotDirectory, got {other:?}"),
    }
}

#[test]
fn installation_inspection_reports_missing_sox_directory() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("game");
    fs::create_dir(&root).unwrap();
    let expected_sox = root.join("Data/SOX");

    let error = GameInstallation::inspect(Game::Crusaders, &root).unwrap_err();

    match error {
        InstallationError::SoxMissing {
            game: Game::Crusaders,
            root: actual_root,
            sox_path,
        } => {
            assert_eq!(actual_root, root);
            assert_eq!(sox_path, expected_sox);
        }
        other => panic!("expected SoxMissing, got {other:?}"),
    }
}

#[test]
fn installation_inspection_reports_file_sox_path() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("game");
    let sox = root.join("Data/SOX");
    fs::create_dir_all(sox.parent().unwrap()).unwrap();
    fs::write(&sox, []).unwrap();

    let error = GameInstallation::inspect(Game::Heroes, &root).unwrap_err();

    match error {
        InstallationError::SoxNotDirectory {
            game: Game::Heroes,
            root: actual_root,
            sox_path,
        } => {
            assert_eq!(actual_root, root);
            assert_eq!(sox_path, sox);
        }
        other => panic!("expected SoxNotDirectory, got {other:?}"),
    }
}

#[test]
fn installation_inspection_retains_a_valid_root_and_derives_its_sox_directory() {
    let temporary = TempDir::new().unwrap();
    let root = temporary.path().join("game");
    let expected_sox = root.join("Data/SOX");
    fs::create_dir_all(&expected_sox).unwrap();

    let installation = GameInstallation::inspect(Game::Heroes, &root).unwrap();

    assert_eq!(installation.game(), Game::Heroes);
    assert_eq!(installation.root(), root);
    assert_eq!(installation.sox_directory(), expected_sox);
}

#[test]
fn discovery_preserves_common_directory_and_game_folder_order() {
    let temporary = TempDir::new().unwrap();
    let common_one = temporary.path().join("common-1");
    let common_two = temporary.path().join("common-2");
    create_valid_installation(&common_one, "KUF Crusader");
    create_valid_installation(&common_one, "KUF Heroes");
    create_valid_installation(&common_two, "KUF Crusader");

    let report = scan_steam_common_directories(&[common_one.clone(), common_two.clone()]);

    assert!(report.issues().is_empty());
    assert_eq!(
        installation_locations(&report),
        vec![
            (Game::Crusaders, common_one.join("KUF Crusader")),
            (Game::Heroes, common_one.join("KUF Heroes")),
            (Game::Crusaders, common_two.join("KUF Crusader")),
        ]
    );
}

#[test]
fn discovery_deduplicates_exact_common_directories() {
    let temporary = TempDir::new().unwrap();
    let common = temporary.path().join("common");
    create_valid_installation(&common, "KUF Crusader");

    let report = scan_steam_common_directories(&[common.clone(), common.clone()]);

    assert!(report.issues().is_empty());
    assert_eq!(
        installation_locations(&report),
        vec![(Game::Crusaders, common.join("KUF Crusader"))]
    );
}

#[test]
fn discovery_ignores_missing_candidates() {
    let temporary = TempDir::new().unwrap();
    let missing_common = temporary.path().join("missing-common");

    let report = scan_steam_common_directories(&[missing_common]);

    assert!(report.installations().is_empty());
    assert!(report.issues().is_empty());
}

#[test]
fn discovery_accepts_only_known_steam_folder_names() {
    let temporary = TempDir::new().unwrap();
    let common = temporary.path().join("common");
    create_valid_installation(&common, "KUF Crusader");
    create_valid_installation(&common, "KUF Heroes");
    create_valid_installation(&common, "Kingdom Under Fire The Crusaders");
    create_valid_installation(&common, "KUF Crusaders");

    let report = scan_steam_common_directories(std::slice::from_ref(&common));

    assert!(report.issues().is_empty());
    assert_eq!(
        installation_locations(&report),
        vec![
            (Game::Crusaders, common.join("KUF Crusader")),
            (Game::Heroes, common.join("KUF Heroes")),
        ]
    );
}

fn create_valid_installation(common: &Path, folder: &str) {
    fs::create_dir_all(common.join(folder).join("Data/SOX")).unwrap();
}

fn installation_locations(report: &kufeditor_game::DiscoveryReport) -> Vec<(Game, PathBuf)> {
    report
        .installations()
        .iter()
        .map(|installation| (installation.game(), installation.root().to_path_buf()))
        .collect()
}
