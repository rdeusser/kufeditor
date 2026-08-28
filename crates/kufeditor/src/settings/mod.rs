use std::{error::Error, fmt, path::PathBuf};

use kufeditor_game::{Game, GamePaths};
use kufeditor_workspace::RecentFiles;

mod path;
mod store;
mod write_pump;

pub(crate) use path::settings_path;
pub(crate) use store::{SettingsImageError, SettingsImageV1, image_from_runtime};
pub(crate) use write_pump::{
    SettingsQueueResult, SettingsRevision, SettingsWriteCompletion, SettingsWritePump,
};

#[derive(Debug)]
pub(crate) struct SettingsStartup {
    pub(crate) path: PathBuf,
    pub(crate) active_game: Game,
    pub(crate) game_paths: GamePaths,
    pub(crate) recent_files: RecentFiles,
    pub(crate) persistence: PersistenceMode,
    pub(crate) warning: Option<SettingsStartupWarning>,
}

impl SettingsStartup {
    pub(crate) fn load(path: PathBuf) -> Self {
        match store::load_image(&path) {
            Ok(Some(image)) => {
                let (active_game, game_paths, recent_files) = image.into_runtime();
                Self {
                    path,
                    active_game,
                    game_paths,
                    recent_files,
                    persistence: PersistenceMode::Enabled,
                    warning: None,
                }
            }
            Ok(None) => Self::defaults(path, PersistenceMode::Enabled, None),
            Err(store::SettingsLoadError::UnsupportedVersion { found }) => Self::defaults(
                path,
                PersistenceMode::ProtectedUnsupportedVersion { found },
                Some(SettingsStartupWarning::UnsupportedVersion { found }),
            ),
            Err(error) => Self::defaults(
                path,
                PersistenceMode::Enabled,
                Some(SettingsStartupWarning::Load(error)),
            ),
        }
    }

    fn defaults(
        path: PathBuf,
        persistence: PersistenceMode,
        warning: Option<SettingsStartupWarning>,
    ) -> Self {
        Self {
            path,
            active_game: Game::default(),
            game_paths: GamePaths::default(),
            recent_files: RecentFiles::default(),
            persistence,
            warning,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PersistenceMode {
    Enabled,
    ProtectedUnsupportedVersion { found: u64 },
}

#[derive(Debug)]
pub(crate) enum SettingsStartupWarning {
    Load(store::SettingsLoadError),
    UnsupportedVersion { found: u64 },
}

impl fmt::Display for SettingsStartupWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(_) => formatter.write_str("application settings could not be loaded"),
            Self::UnsupportedVersion { found } => write!(
                formatter,
                "settings version {found} is newer than this application supports"
            ),
        }
    }
}

impl Error for SettingsStartupWarning {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Load(error) => Some(error),
            Self::UnsupportedVersion { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use kufeditor_game::{Game, GamePaths};
    use kufeditor_workspace::RecentFiles;
    use tempfile::tempdir;

    use super::{
        PersistenceMode, SettingsStartup, SettingsStartupWarning, store::SettingsLoadError,
    };

    #[test]
    fn missing_settings_start_with_defaults_and_enabled_persistence() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");

        let startup = SettingsStartup::load(path.clone());

        assert_eq!(startup.path, path);
        assert_eq!(startup.active_game, Game::Crusaders);
        assert_eq!(startup.game_paths, GamePaths::default());
        assert_eq!(startup.recent_files, RecentFiles::default());
        assert_eq!(startup.persistence, PersistenceMode::Enabled);
        assert!(startup.warning.is_none());
    }

    #[test]
    fn malformed_settings_start_with_defaults_and_a_load_warning() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, b"{").unwrap();

        let startup = SettingsStartup::load(path);

        assert_eq!(startup.active_game, Game::Crusaders);
        assert_eq!(startup.persistence, PersistenceMode::Enabled);
        assert!(matches!(
            startup.warning,
            Some(SettingsStartupWarning::Load(SettingsLoadError::JSON { .. }))
        ));
    }

    #[test]
    fn a_future_version_protects_the_source_file_for_the_session() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, br#"{"version":2,"active_game":{"invalid":true}}"#).unwrap();

        let startup = SettingsStartup::load(path);

        assert_eq!(startup.active_game, Game::Crusaders);
        assert_eq!(
            startup.persistence,
            PersistenceMode::ProtectedUnsupportedVersion { found: 2 }
        );
        assert!(matches!(
            startup.warning,
            Some(SettingsStartupWarning::UnsupportedVersion { found: 2 })
        ));
    }

    #[test]
    fn loading_settings_never_mutates_the_source_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original = br#"{"version":2,"future":{"kept":true}}"#;
        fs::write(&path, original).unwrap();

        let _startup = SettingsStartup::load(path.clone());

        assert_eq!(fs::read(path).unwrap(), original);
    }
}
