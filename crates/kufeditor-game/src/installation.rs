use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::Game;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GamePaths {
    crusaders: Option<PathBuf>,
    heroes: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameInstallation {
    game: Game,
    root: PathBuf,
}

impl GamePaths {
    pub fn root(&self, game: Game) -> Option<&Path> {
        match game {
            Game::Crusaders => self.crusaders.as_deref(),
            Game::Heroes => self.heroes.as_deref(),
        }
    }

    pub fn set_root(&mut self, game: Game, root: Option<PathBuf>) {
        match game {
            Game::Crusaders => self.crusaders = root,
            Game::Heroes => self.heroes = root,
        }
    }
}

impl GameInstallation {
    pub fn inspect(game: Game, root: impl Into<PathBuf>) -> Result<Self, InstallationError> {
        Self::inspect_with_metadata(game, root.into(), &|path| fs::metadata(path))
    }

    pub const fn game(&self) -> Game {
        self.game
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sox_directory(&self) -> PathBuf {
        self.root.join("Data/SOX")
    }

    pub(crate) fn inspect_with_metadata<F>(
        game: Game,
        root: PathBuf,
        metadata: &F,
    ) -> Result<Self, InstallationError>
    where
        F: Fn(&Path) -> io::Result<fs::Metadata>,
    {
        match metadata(&root) {
            Ok(value) if value.is_dir() => {}
            Ok(_) => return Err(InstallationError::RootNotDirectory { game, root }),
            Err(source) if source.kind() == ErrorKind::NotFound => {
                return Err(InstallationError::RootMissing { game, root });
            }
            Err(source) => {
                return Err(InstallationError::Metadata {
                    game,
                    root: root.clone(),
                    path: root,
                    source,
                });
            }
        }

        let sox_path = root.join("Data/SOX");
        match metadata(&sox_path) {
            Ok(value) if value.is_dir() => Ok(Self { game, root }),
            Ok(_) => Err(InstallationError::SOXNotDirectory {
                game,
                root,
                sox_path,
            }),
            Err(source) if source.kind() == ErrorKind::NotFound => {
                Err(InstallationError::SOXMissing {
                    game,
                    root,
                    sox_path,
                })
            }
            Err(source) => Err(InstallationError::Metadata {
                game,
                root,
                path: sox_path,
                source,
            }),
        }
    }
}

#[derive(Debug, Error)]
pub enum InstallationError {
    #[error("the {game} root does not exist: {}", root.display())]
    RootMissing { game: Game, root: PathBuf },

    #[error("the {game} root is not a directory: {}", root.display())]
    RootNotDirectory { game: Game, root: PathBuf },

    #[error("the {game} SOX directory does not exist: {}", sox_path.display())]
    SOXMissing {
        game: Game,
        root: PathBuf,
        sox_path: PathBuf,
    },

    #[error("the {game} SOX path is not a directory: {}", sox_path.display())]
    SOXNotDirectory {
        game: Game,
        root: PathBuf,
        sox_path: PathBuf,
    },

    #[error("could not read metadata for {}: {source}", path.display())]
    Metadata {
        game: Game,
        root: PathBuf,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
