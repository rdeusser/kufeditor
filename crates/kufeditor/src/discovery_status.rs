use kufeditor_game::{DiscoveryError, DiscoveryReport, Game, GamePaths};

use crate::state::RequestID;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RootRevisions {
    crusaders: u64,
    heroes: u64,
}

impl RootRevisions {
    pub(crate) const fn revision(&self, game: Game) -> u64 {
        match game {
            Game::Crusaders => self.crusaders,
            Game::Heroes => self.heroes,
        }
    }

    pub(crate) fn bump(&mut self, game: Game) {
        match game {
            Game::Crusaders => self.crusaders += 1,
            Game::Heroes => self.heroes += 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveryKey {
    request: RequestID,
    revisions: RootRevisions,
    crusaders_was_none: bool,
    heroes_was_none: bool,
}

impl DiscoveryKey {
    pub(crate) fn new(request: RequestID, revisions: RootRevisions, paths: &GamePaths) -> Self {
        Self {
            request,
            revisions,
            crusaders_was_none: paths.root(Game::Crusaders).is_none(),
            heroes_was_none: paths.root(Game::Heroes).is_none(),
        }
    }

    pub(crate) const fn request(&self) -> RequestID {
        self.request
    }

    const fn root_was_none(&self, game: Game) -> bool {
        match game {
            Game::Crusaders => self.crusaders_was_none,
            Game::Heroes => self.heroes_was_none,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) enum DiscoveryStatus {
    #[default]
    Idle,
    Loading {
        key: DiscoveryKey,
    },
    Ready {
        key: DiscoveryKey,
        report: DiscoveryReport,
    },
    Failed {
        key: DiscoveryKey,
        error: DiscoveryError,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct DiscoveryUpdate {
    pub(crate) changed_games: Vec<Game>,
    pub(crate) installation_count: usize,
    pub(crate) issue_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryFinish {
    Stale,
    Ready(DiscoveryUpdate),
    Failed,
}

impl DiscoveryStatus {
    pub(crate) fn begin(&mut self, key: DiscoveryKey) {
        *self = Self::Loading { key };
    }

    pub(crate) fn invalidate(&mut self) {
        *self = Self::Idle;
    }

    pub(crate) fn finish(
        &mut self,
        key: DiscoveryKey,
        result: Result<DiscoveryReport, DiscoveryError>,
        paths: &mut GamePaths,
        revisions: &mut RootRevisions,
    ) -> DiscoveryFinish {
        if !matches!(self, Self::Loading { key: current } if current == &key) {
            return DiscoveryFinish::Stale;
        }

        let report = match result {
            Ok(report) => report,
            Err(error) => {
                *self = Self::Failed { key, error };
                return DiscoveryFinish::Failed;
            }
        };

        let installation_count = report.installations().len();
        let issue_count = report.issues().len();
        let mut changed_games = Vec::new();
        for installation in report.installations() {
            let game = installation.game();
            if changed_games.contains(&game)
                || !key.root_was_none(game)
                || paths.root(game).is_some()
                || revisions.revision(game) != key.revisions.revision(game)
            {
                continue;
            }

            paths.set_root(game, Some(installation.root().to_path_buf()));
            revisions.bump(game);
            changed_games.push(game);
        }

        *self = Self::Ready { key, report };
        DiscoveryFinish::Ready(DiscoveryUpdate {
            changed_games,
            installation_count,
            issue_count,
        })
    }

    pub(crate) const fn status(&self) -> &Self {
        self
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled temporary installation fixtures make setup failures fatal"
    )]

    use std::{fs, path::PathBuf};

    use kufeditor_game::{
        DiscoveryError, DiscoveryReport, Game, GamePaths, scan_steam_common_directories,
    };

    use super::{DiscoveryFinish, DiscoveryKey, DiscoveryStatus, RootRevisions};
    use crate::state::ShellState;

    struct ReportFixture {
        _temporary: tempfile::TempDir,
        report: DiscoveryReport,
        crusaders_roots: Vec<PathBuf>,
        heroes_roots: Vec<PathBuf>,
    }

    impl ReportFixture {
        fn with_common_directories(count: usize) -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let mut common_directories = Vec::new();
            let mut crusaders_roots = Vec::new();
            let mut heroes_roots = Vec::new();
            for index in 0..count {
                let common = temporary.path().join(format!("common-{index}"));
                let crusaders = common.join("KUF Crusader");
                let heroes = common.join("KUF Heroes");
                fs::create_dir_all(crusaders.join("Data/SOX")).unwrap();
                fs::create_dir_all(heroes.join("Data/SOX")).unwrap();
                common_directories.push(common);
                crusaders_roots.push(crusaders);
                heroes_roots.push(heroes);
            }
            let report = scan_steam_common_directories(&common_directories);
            Self {
                _temporary: temporary,
                report,
                crusaders_roots,
                heroes_roots,
            }
        }
    }

    fn begin(
        status: &mut DiscoveryStatus,
        paths: &GamePaths,
        revisions: RootRevisions,
    ) -> DiscoveryKey {
        let mut shell = ShellState::default();
        let key = DiscoveryKey::new(shell.begin_discovery(), revisions, paths);
        status.begin(key.clone());
        key
    }

    #[test]
    fn begin_and_invalidate_replace_loading_and_cached_results_with_idle() {
        let mut paths = GamePaths::default();
        let mut revisions = RootRevisions::default();
        let mut status = DiscoveryStatus::Idle;
        let key = begin(&mut status, &paths, revisions);
        assert!(
            matches!(status.status(), DiscoveryStatus::Loading { key: current } if current == &key)
        );

        let fixture = ReportFixture::with_common_directories(0);
        assert!(matches!(
            status.finish(key, Ok(fixture.report), &mut paths, &mut revisions),
            DiscoveryFinish::Ready(_)
        ));
        assert!(matches!(status.status(), DiscoveryStatus::Ready { .. }));

        status.invalidate();
        assert!(matches!(status.status(), DiscoveryStatus::Idle));
    }

    #[test]
    fn stale_completion_after_invalidation_changes_nothing() {
        let mut paths = GamePaths::default();
        let mut revisions = RootRevisions::default();
        let mut status = DiscoveryStatus::Idle;
        let key = begin(&mut status, &paths, revisions);
        status.invalidate();
        let fixture = ReportFixture::with_common_directories(1);

        assert!(matches!(
            status.finish(key, Ok(fixture.report), &mut paths, &mut revisions),
            DiscoveryFinish::Stale
        ));
        assert!(matches!(status.status(), DiscoveryStatus::Idle));
        assert_eq!(paths, GamePaths::default());
        assert_eq!(revisions, RootRevisions::default());
    }

    #[test]
    fn first_installation_for_each_game_wins() {
        let mut paths = GamePaths::default();
        let mut revisions = RootRevisions::default();
        let mut status = DiscoveryStatus::Idle;
        let key = begin(&mut status, &paths, revisions);
        let fixture = ReportFixture::with_common_directories(2);
        let expected_crusaders = fixture.crusaders_roots.first().unwrap().clone();
        let expected_heroes = fixture.heroes_roots.first().unwrap().clone();

        let finish = status.finish(key, Ok(fixture.report), &mut paths, &mut revisions);

        assert!(matches!(
            finish,
            DiscoveryFinish::Ready(update)
                if update.changed_games == vec![Game::Crusaders, Game::Heroes]
                    && update.installation_count == 4
                    && update.issue_count == 0
        ));
        assert_eq!(
            paths.root(Game::Crusaders),
            Some(expected_crusaders.as_path())
        );
        assert_eq!(paths.root(Game::Heroes), Some(expected_heroes.as_path()));
        assert_eq!(revisions.revision(Game::Crusaders), 1);
        assert_eq!(revisions.revision(Game::Heroes), 1);
    }

    #[test]
    fn roots_configured_at_or_after_start_are_not_replaced() {
        let configured_at_start = PathBuf::from("/configured/crusaders");
        let configured_after_start = PathBuf::from("/configured/heroes");
        let mut paths = GamePaths::default();
        paths.set_root(Game::Crusaders, Some(configured_at_start.clone()));
        let mut revisions = RootRevisions::default();
        let mut status = DiscoveryStatus::Idle;
        let key = begin(&mut status, &paths, revisions);
        paths.set_root(Game::Heroes, Some(configured_after_start.clone()));
        let fixture = ReportFixture::with_common_directories(1);

        let finish = status.finish(key, Ok(fixture.report), &mut paths, &mut revisions);

        assert!(matches!(
            finish,
            DiscoveryFinish::Ready(update) if update.changed_games.is_empty()
        ));
        assert_eq!(
            paths.root(Game::Crusaders),
            Some(configured_at_start.as_path())
        );
        assert_eq!(
            paths.root(Game::Heroes),
            Some(configured_after_start.as_path())
        );
        assert_eq!(revisions, RootRevisions::default());
    }

    #[test]
    fn changed_revision_blocks_replacement_even_when_root_is_none() {
        let mut paths = GamePaths::default();
        let mut revisions = RootRevisions::default();
        let mut status = DiscoveryStatus::Idle;
        let key = begin(&mut status, &paths, revisions);
        revisions.bump(Game::Crusaders);
        let fixture = ReportFixture::with_common_directories(1);

        let finish = status.finish(key, Ok(fixture.report), &mut paths, &mut revisions);

        assert!(matches!(
            finish,
            DiscoveryFinish::Ready(update) if update.changed_games == vec![Game::Heroes]
        ));
        assert_eq!(paths.root(Game::Crusaders), None);
        assert_eq!(revisions.revision(Game::Crusaders), 1);
        assert_eq!(revisions.revision(Game::Heroes), 1);
    }

    #[test]
    fn one_or_two_accepted_roots_report_exact_changed_games() {
        for configured_game in [None, Some(Game::Crusaders), Some(Game::Heroes)] {
            let mut paths = GamePaths::default();
            if let Some(game) = configured_game {
                paths.set_root(game, Some(PathBuf::from(format!("/configured/{game}"))));
            }
            let mut revisions = RootRevisions::default();
            let mut status = DiscoveryStatus::Idle;
            let key = begin(&mut status, &paths, revisions);
            let fixture = ReportFixture::with_common_directories(1);

            let finish = status.finish(key, Ok(fixture.report), &mut paths, &mut revisions);
            let expected = match configured_game {
                None => vec![Game::Crusaders, Game::Heroes],
                Some(Game::Crusaders) => vec![Game::Heroes],
                Some(Game::Heroes) => vec![Game::Crusaders],
            };
            assert!(matches!(
                finish,
                DiscoveryFinish::Ready(update) if update.changed_games == expected
            ));
        }
    }

    #[test]
    fn failed_current_request_becomes_failed() {
        let mut paths = GamePaths::default();
        let mut revisions = RootRevisions::default();
        let mut status = DiscoveryStatus::Idle;
        let key = begin(&mut status, &paths, revisions);

        assert!(matches!(
            status.finish(
                key.clone(),
                Err(DiscoveryError::Unavailable),
                &mut paths,
                &mut revisions,
            ),
            DiscoveryFinish::Failed
        ));
        assert!(matches!(
            status.status(),
            DiscoveryStatus::Failed { key: current, error: DiscoveryError::Unavailable }
                if current == &key
        ));
    }
}
