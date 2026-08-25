use std::{
    error::Error,
    fmt, fs,
    io::{self, ErrorKind, Read},
    path::{Path, PathBuf},
};

use kufeditor_game::{Game, GamePaths};
use kufeditor_workspace::RecentFiles;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

pub(crate) const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SettingsImageV1 {
    #[serde(default = "version_one")]
    version: u64,
    #[serde(default)]
    active_game: SettingsGame,
    #[serde(default)]
    crusaders_path: Option<String>,
    #[serde(default)]
    heroes_path: Option<String>,
    #[serde(default = "default_recent_limit")]
    max_recent_files: usize,
    #[serde(default)]
    recent_files: Vec<String>,
}

impl SettingsImageV1 {
    pub(super) fn into_runtime(self) -> (Game, GamePaths, RecentFiles) {
        let mut paths = GamePaths::default();
        paths.set_root(Game::Crusaders, self.crusaders_path.map(PathBuf::from));
        paths.set_root(Game::Heroes, self.heroes_path.map(PathBuf::from));
        let recent = RecentFiles::from_persisted(
            self.max_recent_files,
            self.recent_files.into_iter().map(PathBuf::from).collect(),
        );
        (self.active_game.into(), paths, recent)
    }
}

impl Default for SettingsImageV1 {
    fn default() -> Self {
        Self {
            version: version_one(),
            active_game: SettingsGame::default(),
            crusaders_path: None,
            heroes_path: None,
            max_recent_files: default_recent_limit(),
            recent_files: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum SettingsGame {
    #[default]
    Crusaders,
    Heroes,
}

impl From<Game> for SettingsGame {
    fn from(game: Game) -> Self {
        match game {
            Game::Crusaders => Self::Crusaders,
            Game::Heroes => Self::Heroes,
        }
    }
}

impl From<SettingsGame> for Game {
    fn from(game: SettingsGame) -> Self {
        match game {
            SettingsGame::Crusaders => Self::Crusaders,
            SettingsGame::Heroes => Self::Heroes,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SettingsImageError {
    NonUnicodePath { field: &'static str, path: PathBuf },
}

impl fmt::Display for SettingsImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUnicodePath { field, path } => write!(
                formatter,
                "{field} contains a non-Unicode path: {}",
                path.display()
            ),
        }
    }
}

impl Error for SettingsImageError {}

#[derive(Debug)]
pub(crate) enum SettingsLoadError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    TooLarge {
        path: PathBuf,
        max_bytes: u64,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnsupportedVersion {
        found: u64,
    },
}

impl fmt::Display for SettingsLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(formatter, "could not read {}", path.display()),
            Self::TooLarge { path, max_bytes } => write!(
                formatter,
                "{} is larger than the {max_bytes}-byte settings limit",
                path.display()
            ),
            Self::Json { path, .. } => {
                write!(
                    formatter,
                    "could not parse settings from {}",
                    path.display()
                )
            }
            Self::UnsupportedVersion { found } => {
                write!(formatter, "settings version {found} is not supported")
            }
        }
    }
}

impl Error for SettingsLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::TooLarge { .. } | Self::UnsupportedVersion { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SettingsSaveError {
    CreateParent {
        path: PathBuf,
        source: io::Error,
    },
    CreateTemporary {
        directory: PathBuf,
        source: io::Error,
    },
    Serialize {
        source: serde_json::Error,
    },
    Write {
        path: PathBuf,
        source: io::Error,
    },
    Flush {
        path: PathBuf,
        source: io::Error,
    },
    Sync {
        path: PathBuf,
        source: io::Error,
    },
    Persist {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for SettingsSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateParent { path, .. } => {
                write!(
                    formatter,
                    "could not create settings directory {}",
                    path.display()
                )
            }
            Self::CreateTemporary { directory, .. } => write!(
                formatter,
                "could not create a temporary settings file in {}",
                directory.display()
            ),
            Self::Serialize { .. } => {
                formatter.write_str("could not serialize application settings")
            }
            Self::Write { path, .. } => {
                write!(formatter, "could not write settings for {}", path.display())
            }
            Self::Flush { path, .. } => {
                write!(formatter, "could not flush settings for {}", path.display())
            }
            Self::Sync { path, .. } => {
                write!(
                    formatter,
                    "could not synchronize settings for {}",
                    path.display()
                )
            }
            Self::Persist { path, .. } => {
                write!(
                    formatter,
                    "could not replace settings at {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for SettingsSaveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CreateParent { source, .. }
            | Self::CreateTemporary { source, .. }
            | Self::Write { source, .. }
            | Self::Flush { source, .. }
            | Self::Sync { source, .. }
            | Self::Persist { source, .. } => Some(source),
            Self::Serialize { source } => Some(source),
        }
    }
}

pub(crate) fn image_from_runtime(
    game: Game,
    paths: &GamePaths,
    recent: &RecentFiles,
) -> Result<SettingsImageV1, SettingsImageError> {
    Ok(SettingsImageV1 {
        version: version_one(),
        active_game: game.into(),
        crusaders_path: unicode_path("crusaders_path", paths.root(Game::Crusaders))?,
        heroes_path: unicode_path("heroes_path", paths.root(Game::Heroes))?,
        max_recent_files: recent.limit(),
        recent_files: recent
            .paths()
            .iter()
            .map(|path| unicode_path("recent_files", Some(path.as_path())))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect(),
    })
}

pub(crate) fn load_image(path: &Path) -> Result<Option<SettingsImageV1>, SettingsLoadError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SettingsLoadError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| SettingsLoadError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(SettingsLoadError::TooLarge {
            path: path.to_path_buf(),
            max_bytes: MAX_SETTINGS_BYTES,
        });
    }

    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| SettingsLoadError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(found) = value.get("version").and_then(serde_json::Value::as_u64)
        && found != version_one()
    {
        return Err(SettingsLoadError::UnsupportedVersion { found });
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|source| SettingsLoadError::Json {
            path: path.to_path_buf(),
            source,
        })
}

pub(crate) fn save_image(path: &Path, image: &SettingsImageV1) -> Result<(), SettingsSaveError> {
    use std::io::Write;

    let parent = destination_directory(path);
    fs::create_dir_all(parent).map_err(|source| SettingsSaveError::CreateParent {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| SettingsSaveError::CreateTemporary {
            directory: parent.to_path_buf(),
            source,
        })?;
    let bytes = serde_json::to_vec_pretty(image)
        .map_err(|source| SettingsSaveError::Serialize { source })?;
    temporary
        .write_all(&bytes)
        .map_err(|source| SettingsSaveError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .flush()
        .map_err(|source| SettingsSaveError::Flush {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| SettingsSaveError::Sync {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| SettingsSaveError::Persist {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    Ok(())
}

fn destination_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn unicode_path(
    field: &'static str,
    path: Option<&Path>,
) -> Result<Option<String>, SettingsImageError> {
    path.map(|path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| SettingsImageError::NonUnicodePath {
                field,
                path: path.to_path_buf(),
            })
    })
    .transpose()
}

const fn version_one() -> u64 {
    1
}

const fn default_recent_limit() -> usize {
    kufeditor_workspace::DEFAULT_RECENT_FILE_LIMIT
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use kufeditor_game::{Game, GamePaths};
    use kufeditor_workspace::RecentFiles;
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        MAX_SETTINGS_BYTES, SettingsImageError, SettingsLoadError, SettingsSaveError,
        destination_directory, image_from_runtime, load_image, save_image,
    };

    fn write_fixture(contents: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, contents).unwrap();
        (directory, path)
    }

    #[test]
    fn a_missing_file_has_no_image() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");

        assert!(load_image(&path).unwrap().is_none());
    }

    #[test]
    fn an_empty_object_loads_version_one_defaults() {
        let (_directory, path) = write_fixture(b"{}");

        let (game, paths, recent) = load_image(&path).unwrap().unwrap().into_runtime();

        assert_eq!(game, Game::Crusaders);
        assert_eq!(paths, GamePaths::default());
        assert_eq!(recent, RecentFiles::default());
    }

    #[test]
    fn complete_version_one_uses_lowercase_games_and_snake_case_keys() {
        let input = br#"{
            "version": 1,
            "active_game": "heroes",
            "crusaders_path": "/games/Crusaders",
            "heroes_path": "/games/Heroes",
            "max_recent_files": 15,
            "recent_files": ["/files/first.sox", "/files/second.sox"]
        }"#;
        let (_directory, path) = write_fixture(input);

        let image = load_image(&path).unwrap().unwrap();
        let value = serde_json::to_value(&image).unwrap();
        assert_eq!(
            value,
            json!({
                "version": 1,
                "active_game": "heroes",
                "crusaders_path": "/games/Crusaders",
                "heroes_path": "/games/Heroes",
                "max_recent_files": 15,
                "recent_files": ["/files/first.sox", "/files/second.sox"]
            })
        );

        let (game, paths, recent) = image.into_runtime();
        assert_eq!(game, Game::Heroes);
        assert_eq!(
            paths.root(Game::Crusaders),
            Some(std::path::Path::new("/games/Crusaders"))
        );
        assert_eq!(
            paths.root(Game::Heroes),
            Some(std::path::Path::new("/games/Heroes"))
        );
        assert_eq!(recent.limit(), 15);
        assert_eq!(
            recent.paths(),
            [
                PathBuf::from("/files/first.sox"),
                PathBuf::from("/files/second.sox")
            ]
        );
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let (_directory, path) =
            write_fixture(br#"{"version":1,"active_game":"crusaders","future_setting":true}"#);

        let (game, _, _) = load_image(&path).unwrap().unwrap().into_runtime();
        assert_eq!(game, Game::Crusaders);
    }

    #[test]
    fn malformed_json_keeps_the_json_error() {
        let (_directory, path) = write_fixture(br#"{"version":1"#);

        assert!(matches!(
            load_image(&path),
            Err(SettingsLoadError::Json { path: error_path, .. }) if error_path == path
        ));
    }

    #[test]
    fn exactly_one_mebibyte_is_accepted() {
        let mut input = b"{}".to_vec();
        input.resize(usize::try_from(MAX_SETTINGS_BYTES).unwrap(), b' ');
        let (_directory, path) = write_fixture(&input);

        assert!(load_image(&path).unwrap().is_some());
    }

    #[test]
    fn one_byte_over_one_mebibyte_is_rejected_before_json_parsing() {
        let input = vec![b' '; usize::try_from(MAX_SETTINGS_BYTES).unwrap() + 1];
        let (_directory, path) = write_fixture(&input);

        assert!(matches!(
            load_image(&path),
            Err(SettingsLoadError::TooLarge {
                path: error_path,
                max_bytes: MAX_SETTINGS_BYTES,
            }) if error_path == path
        ));
    }

    #[test]
    fn version_two_is_rejected_before_version_one_fields_are_read() {
        for input in [
            br#"{"version":2,"active_game":"heroes","max_recent_files":10}"#.as_slice(),
            br#"{"version":2,"active_game":{"invalid":true}}"#.as_slice(),
        ] {
            let (_directory, path) = write_fixture(input);
            assert!(matches!(
                load_image(&path),
                Err(SettingsLoadError::UnsupportedVersion { found: 2 })
            ));
        }
    }

    #[test]
    fn persisted_recent_files_are_normalized_deduplicated_and_truncated() {
        let input = br#"{
            "max_recent_files": 6,
            "recent_files": ["a", "b", "a", "c", "d", "e", "f"]
        }"#;
        let (_directory, path) = write_fixture(input);

        let (_, _, recent) = load_image(&path).unwrap().unwrap().into_runtime();

        assert_eq!(recent.limit(), 5);
        assert_eq!(
            recent.paths(),
            [
                PathBuf::from("a"),
                PathBuf::from("b"),
                PathBuf::from("c"),
                PathBuf::from("d"),
                PathBuf::from("e")
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_non_unicode_runtime_path_is_rejected_without_lossy_conversion() {
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff, b'x']));
        let mut paths = GamePaths::default();
        paths.set_root(Game::Crusaders, Some(path.clone()));

        let error =
            image_from_runtime(Game::Crusaders, &paths, &RecentFiles::default()).unwrap_err();

        assert!(matches!(
            error,
            SettingsImageError::NonUnicodePath {
                field: "crusaders_path",
                path: error_path,
            } if error_path == path
        ));
    }

    #[test]
    fn saving_replaces_an_existing_file_with_one_pretty_complete_image() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, b"old contents").unwrap();
        let mut paths = GamePaths::default();
        paths.set_root(Game::Heroes, Some(PathBuf::from("/games/Heroes")));
        let recent = RecentFiles::from_persisted(10, vec![PathBuf::from("/files/first.sox")]);
        let image = image_from_runtime(Game::Heroes, &paths, &recent).unwrap();

        save_image(&path, &image).unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with("{\n"));
        assert!(!saved.contains("old contents"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
            serde_json::to_value(image).unwrap()
        );
    }

    #[test]
    fn temporary_files_use_the_destination_directory() {
        let path = PathBuf::from("/settings-parent/settings.json");

        assert_eq!(
            destination_directory(&path),
            std::path::Path::new("/settings-parent")
        );
    }

    #[test]
    fn a_relative_destination_uses_the_current_directory() {
        assert_eq!(
            destination_directory(std::path::Path::new("settings.json")),
            std::path::Path::new(".")
        );
    }

    #[test]
    fn a_regular_file_parent_reports_parent_creation() {
        let directory = tempdir().unwrap();
        let parent = directory.path().join("not-a-directory");
        fs::write(&parent, b"file").unwrap();
        let path = parent.join("settings.json");
        let image = image_from_runtime(
            Game::Crusaders,
            &GamePaths::default(),
            &RecentFiles::default(),
        )
        .unwrap();

        let error = save_image(&path, &image).unwrap_err();

        assert!(matches!(
            error,
            SettingsSaveError::CreateParent {
                path: error_path,
                source,
            } if error_path == parent && !source.to_string().is_empty()
        ));
    }

    #[test]
    fn a_replacement_failure_retains_its_stage_path_and_source() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::create_dir(&path).unwrap();
        let image = image_from_runtime(
            Game::Crusaders,
            &GamePaths::default(),
            &RecentFiles::default(),
        )
        .unwrap();

        let error = save_image(&path, &image).unwrap_err();

        assert!(matches!(
            error,
            SettingsSaveError::Persist {
                path: error_path,
                source,
            } if error_path == path && !source.to_string().is_empty()
        ));
    }
}
