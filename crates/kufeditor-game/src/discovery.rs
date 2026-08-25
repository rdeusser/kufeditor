use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{Game, GameInstallation, InstallationError};

const STEAM_GAME_FOLDERS: [(Game, &str); 2] = [
    (Game::Crusaders, "KUF Crusader"),
    (Game::Heroes, "KUF Heroes"),
];

#[derive(Debug)]
pub struct DiscoveryReport {
    installations: Vec<GameInstallation>,
    issues: Vec<DiscoveryIssue>,
}

impl DiscoveryReport {
    pub fn installations(&self) -> &[GameInstallation] {
        &self.installations
    }

    pub fn issues(&self) -> &[DiscoveryIssue] {
        &self.issues
    }
}

#[derive(Debug, Error)]
#[error(
    "could not inspect the {game} candidate at {} while reading {}: {source}",
    root.display(),
    path.display()
)]
pub struct DiscoveryIssue {
    pub game: Game,
    pub root: PathBuf,
    pub path: PathBuf,
    #[source]
    pub source: io::Error,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("automatic Steam discovery is unavailable on this platform")]
    Unavailable,
}

pub fn scan_steam_common_directories(common_directories: &[PathBuf]) -> DiscoveryReport {
    scan_with_metadata(common_directories, &|path| fs::metadata(path))
}

pub const fn steam_discovery_available() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
pub fn discover_steam_installations() -> Result<DiscoveryReport, DiscoveryError> {
    Ok(scan_steam_common_directories(
        &windows_steam_common_directories(),
    ))
}

#[cfg(not(windows))]
pub fn discover_steam_installations() -> Result<DiscoveryReport, DiscoveryError> {
    Err(DiscoveryError::Unavailable)
}

fn scan_with_metadata<F>(common_directories: &[PathBuf], metadata: &F) -> DiscoveryReport
where
    F: Fn(&Path) -> io::Result<fs::Metadata>,
{
    let mut installations = Vec::new();
    let mut issues = Vec::new();
    let mut seen_roots = HashSet::new();

    for common_directory in common_directories {
        for (game, folder) in STEAM_GAME_FOLDERS {
            let root = common_directory.join(folder);
            if !seen_roots.insert(root.clone()) {
                continue;
            }

            match GameInstallation::inspect_with_metadata(game, root, metadata) {
                Ok(installation) => installations.push(installation),
                Err(InstallationError::Metadata {
                    game,
                    root,
                    path,
                    source,
                }) => issues.push(DiscoveryIssue {
                    game,
                    root,
                    path,
                    source,
                }),
                Err(_) => {}
            }
        }
    }

    DiscoveryReport {
        installations,
        issues,
    }
}

#[cfg(any(windows, test))]
fn windows_steam_common_directories() -> Vec<PathBuf> {
    const PATTERNS: [&str; 3] = [
        "Steam\\steamapps\\common",
        "Program Files\\Steam\\steamapps\\common",
        "Program Files (x86)\\Steam\\steamapps\\common",
    ];

    ('A'..='Z')
        .flat_map(|drive| {
            PATTERNS
                .iter()
                .map(move |pattern| PathBuf::from(format!("{drive}:\\{pattern}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled temporary filesystem fixtures make setup failures fatal"
    )]

    use std::{fs, io, path::PathBuf};

    use super::{scan_with_metadata, windows_steam_common_directories};
    use crate::{Game, GameInstallation};
    use tempfile::TempDir;

    #[test]
    fn windows_common_directory_generation_keeps_drive_and_pattern_order() {
        let directories = windows_steam_common_directories();

        assert_eq!(directories.len(), 78);
        assert_eq!(
            directories.first(),
            Some(&PathBuf::from(r"A:\Steam\steamapps\common"))
        );
        assert_eq!(
            directories.get(1),
            Some(&PathBuf::from(r"A:\Program Files\Steam\steamapps\common"))
        );
        assert_eq!(
            directories.get(2),
            Some(&PathBuf::from(
                r"A:\Program Files (x86)\Steam\steamapps\common"
            ))
        );
        assert_eq!(
            directories.get(3),
            Some(&PathBuf::from(r"B:\Steam\steamapps\common"))
        );
        assert_eq!(
            directories.get(75),
            Some(&PathBuf::from(r"Z:\Steam\steamapps\common"))
        );
        assert_eq!(
            directories.get(76),
            Some(&PathBuf::from(r"Z:\Program Files\Steam\steamapps\common"))
        );
        assert_eq!(
            directories.last(),
            Some(&PathBuf::from(
                r"Z:\Program Files (x86)\Steam\steamapps\common"
            ))
        );
    }

    #[test]
    fn scanner_records_an_injected_metadata_error_and_continues() {
        let temporary = TempDir::new().unwrap();
        let common = temporary.path().join("common");
        let bad_root = common.join("KUF Crusader");
        let heroes_root = common.join("KUF Heroes");
        fs::create_dir_all(heroes_root.join("Data/SOX")).unwrap();

        let report = scan_with_metadata(&[common], &|path| {
            if path == bad_root {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                fs::metadata(path)
            }
        });

        assert_eq!(report.installations().len(), 1);
        assert_eq!(
            report.installations().first().map(GameInstallation::game),
            Some(Game::Heroes)
        );
        assert_eq!(report.issues().len(), 1);
        let issue = report.issues().first().unwrap();
        assert_eq!(issue.game, Game::Crusaders);
        assert_eq!(issue.root, bad_root);
        assert_eq!(issue.path, issue.root);
        assert_eq!(issue.source.kind(), io::ErrorKind::PermissionDenied);
    }
}
