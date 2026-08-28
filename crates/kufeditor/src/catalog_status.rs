use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::{Path, PathBuf},
};

use kufeditor_game::{CatalogLoadError, Game, InstallationError};

use crate::state::RequestID;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogKey {
    request: RequestID,
    game: Game,
    root: PathBuf,
}

pub(crate) enum CatalogStatus<T, E> {
    NotConfigured,
    Loading {
        key: CatalogKey,
    },
    Ready {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "the terminal request identity remains part of the cached catalog state"
            )
        )]
        key: CatalogKey,
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "the global name dictionary remains resident across UI transitions"
            )
        )]
        value: T,
        issue_count: usize,
    },
    Failed {
        #[cfg_attr(
            not(test),
            expect(
                dead_code,
                reason = "the terminal request identity remains part of the cached catalog state"
            )
        )]
        key: CatalogKey,
        error: E,
    },
}

pub(crate) struct CatalogSession<T, E> {
    status: CatalogStatus<T, E>,
}

impl CatalogKey {
    pub(crate) fn new(request: RequestID, game: Game, root: impl Into<PathBuf>) -> Self {
        Self {
            request,
            game,
            root: root.into(),
        }
    }

    pub(crate) const fn request(&self) -> RequestID {
        self.request
    }

    pub(crate) const fn game(&self) -> Game {
        self.game
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug)]
pub(crate) enum CatalogRequestError {
    Installation(InstallationError),
    Load(CatalogLoadError),
}

impl Display for CatalogRequestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Installation(_) => formatter.write_str("could not read the game folder"),
            Self::Load(_) => formatter.write_str("could not load game names"),
        }
    }
}

impl Error for CatalogRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Installation(error) => Some(error),
            Self::Load(error) => Some(error),
        }
    }
}

impl From<InstallationError> for CatalogRequestError {
    fn from(error: InstallationError) -> Self {
        Self::Installation(error)
    }
}

impl From<CatalogLoadError> for CatalogRequestError {
    fn from(error: CatalogLoadError) -> Self {
        Self::Load(error)
    }
}

impl<T, E> CatalogSession<T, E> {
    pub(crate) fn begin(&mut self, key: CatalogKey) {
        self.status = CatalogStatus::Loading { key };
    }

    pub(crate) fn not_configured(&mut self) {
        self.status = CatalogStatus::NotConfigured;
    }

    pub(crate) fn finish_ready(&mut self, key: CatalogKey, value: T, issue_count: usize) -> bool {
        if !matches!(&self.status, CatalogStatus::Loading { key: current } if current == &key) {
            return false;
        }
        self.status = CatalogStatus::Ready {
            key,
            value,
            issue_count,
        };
        true
    }

    pub(crate) fn finish_failed(&mut self, key: CatalogKey, error: E) -> bool {
        if !matches!(&self.status, CatalogStatus::Loading { key: current } if current == &key) {
            return false;
        }
        self.status = CatalogStatus::Failed { key, error };
        true
    }

    pub(crate) const fn status(&self) -> &CatalogStatus<T, E> {
        &self.status
    }

    #[cfg(test)]
    pub(crate) fn ready_value(&self) -> Option<&T> {
        match &self.status {
            CatalogStatus::Ready { value, .. } => Some(value),
            CatalogStatus::NotConfigured
            | CatalogStatus::Loading { .. }
            | CatalogStatus::Failed { .. } => None,
        }
    }
}

impl<T, E> Default for CatalogSession<T, E> {
    fn default() -> Self {
        Self {
            status: CatalogStatus::NotConfigured,
        }
    }
}

#[cfg(test)]
mod tests {
    use kufeditor_game::Game;

    use super::{CatalogKey, CatalogSession, CatalogStatus};
    use crate::state::ShellState;

    #[test]
    fn only_the_complete_current_key_can_finish_ready() {
        let mut shell = ShellState::default();
        let first = CatalogKey::new(shell.begin_catalog(), Game::Crusaders, "/first");
        let second = CatalogKey::new(shell.begin_catalog(), Game::Heroes, "/second");

        let mut session = CatalogSession::<&'static str, &'static str>::default();
        session.begin(first.clone());
        session.begin(second.clone());

        assert!(!session.finish_ready(first, "stale", 0));
        assert!(session.finish_ready(second.clone(), "current", 0));
        assert_eq!(session.ready_value(), Some(&"current"));
        assert!(matches!(
            session.status(),
            CatalogStatus::Ready {
                key,
                value: "current",
                issue_count: 0,
            } if key == &second
        ));
    }

    #[test]
    fn ready_status_retains_catalog_issue_count_without_hiding_the_value() {
        let mut shell = ShellState::default();
        let key = CatalogKey::new(shell.begin_catalog(), Game::Crusaders, "/game");
        let mut session = CatalogSession::<&'static str, &'static str>::default();
        session.begin(key.clone());

        assert!(session.finish_ready(key.clone(), "dictionary", 3));
        assert_eq!(session.ready_value(), Some(&"dictionary"));
        assert!(matches!(
            session.status(),
            CatalogStatus::Ready {
                key: current,
                value: "dictionary",
                issue_count: 3,
            } if current == &key
        ));
    }

    #[test]
    fn non_ready_transitions_remove_the_previous_payload() {
        let mut shell = ShellState::default();
        let first = CatalogKey::new(shell.begin_catalog(), Game::Crusaders, "/first");
        let second = CatalogKey::new(shell.begin_catalog(), Game::Heroes, "/second");
        let mut session = CatalogSession::<&'static str, &'static str>::default();

        session.begin(first.clone());
        assert!(session.finish_ready(first, "ready", 0));
        session.begin(second.clone());
        assert_eq!(session.ready_value(), None);

        assert!(session.finish_ready(second.clone(), "ready again", 0));
        session.not_configured();
        assert_eq!(session.ready_value(), None);
        assert!(matches!(session.status(), CatalogStatus::NotConfigured));

        session.begin(second.clone());
        assert!(session.finish_ready(second.clone(), "one last value", 0));
        session.begin(second.clone());
        assert!(session.finish_failed(second.clone(), "load failed"));
        assert_eq!(session.ready_value(), None);
        assert!(matches!(
            session.status(),
            CatalogStatus::Failed { key, error: "load failed" } if key == &second
        ));
    }

    #[test]
    fn catalog_keys_include_request_game_and_exact_root() {
        let mut shell = ShellState::default();
        let request = shell.begin_catalog();
        let current = CatalogKey::new(request, Game::Crusaders, "/games/current");
        let wrong_game = CatalogKey::new(request, Game::Heroes, "/games/current");
        let wrong_root = CatalogKey::new(request, Game::Crusaders, "/games/other");
        let mut session = CatalogSession::<&'static str, &'static str>::default();

        session.begin(current.clone());

        assert!(!session.finish_ready(wrong_game, "wrong game", 1));
        assert!(!session.finish_ready(wrong_root, "wrong root", 1));
        assert!(matches!(
            session.status(),
            CatalogStatus::Loading { key } if key == &current
        ));
    }
}
