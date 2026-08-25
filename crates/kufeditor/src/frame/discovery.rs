use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::PathBuf,
    rc::Rc,
};

use gpui::{Context, PathPromptOptions, Task};
use kufeditor_game::{
    DiscoveryError, DiscoveryReport, Game, GameInstallation, InstallationError,
    discover_steam_installations, steam_discovery_available,
};

use super::{AppFrame, discovery_status::DiscoveryKey};
use crate::{
    notices::{Notice, NoticeLevel, NoticeSource},
    state::RequestID,
};

pub(crate) fn game_folder_prompt_options() -> PathPromptOptions {
    PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Select game folder".into()),
    }
}

pub(crate) enum BrowsePromptResult {
    Selected(Vec<PathBuf>),
    Canceled,
    Failed(Notice),
}

pub(super) trait BrowsePromptLauncher {
    fn launch(
        &self,
        options: PathPromptOptions,
        cx: &mut Context<AppFrame>,
    ) -> Task<BrowsePromptResult>;
}

pub(super) struct PlatformBrowsePromptLauncher;

impl BrowsePromptLauncher for PlatformBrowsePromptLauncher {
    fn launch(
        &self,
        options: PathPromptOptions,
        cx: &mut Context<AppFrame>,
    ) -> Task<BrowsePromptResult> {
        let prompt = cx.prompt_for_paths(options);
        cx.spawn(async move |_, _| match prompt.await {
            Ok(Ok(Some(paths))) => BrowsePromptResult::Selected(paths),
            Ok(Ok(None)) => BrowsePromptResult::Canceled,
            Ok(Err(error)) => BrowsePromptResult::Failed(Notice::error(
                "Could not open the folder picker",
                error.as_ref(),
            )),
            Err(error) => BrowsePromptResult::Failed(Notice::error(
                "The folder picker did not respond",
                &error,
            )),
        })
    }
}

#[derive(Debug)]
pub(crate) enum SelectedRootError {
    NonUnicode { game: Game, path: PathBuf },
    Inspection(InstallationError),
}

impl Display for SelectedRootError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUnicode { game, path } => write!(
                formatter,
                "the selected {game} folder has a non-Unicode path: {}",
                path.display()
            ),
            Self::Inspection(_) => {
                formatter.write_str("could not inspect the selected game folder")
            }
        }
    }
}

impl Error for SelectedRootError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NonUnicode { .. } => None,
            Self::Inspection(error) => Some(error),
        }
    }
}

impl From<InstallationError> for SelectedRootError {
    fn from(error: InstallationError) -> Self {
        Self::Inspection(error)
    }
}

impl AppFrame {
    pub(crate) fn browse_game_root(&mut self, game: Game, cx: &mut Context<Self>) {
        let request = self.begin_browse_request(game);
        let launcher = Rc::clone(&self.browse_prompt_launcher);
        let task = launcher.launch(game_folder_prompt_options(), cx);
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                let _ = frame.finish_browse_prompt(game, request, result, cx);
            });
        })
        .detach();
    }

    fn begin_browse_request(&mut self, game: Game) -> RequestID {
        let request = self.shell.begin_browse(game);
        self.notices.begin(
            NoticeSource::Browse(game),
            request.get(),
            Notice::info(format!("Choose the {game} game folder")),
        );
        request
    }

    fn finish_browse_prompt(
        &mut self,
        game: Game,
        request: RequestID,
        result: BrowsePromptResult,
        cx: &mut Context<Self>,
    ) -> Result<(), SelectedRootError> {
        match result {
            BrowsePromptResult::Canceled => {
                self.notices
                    .complete(NoticeSource::Browse(game), request.get(), None);
            }
            BrowsePromptResult::Failed(notice) => {
                self.notices
                    .complete(NoticeSource::Browse(game), request.get(), Some(notice));
            }
            BrowsePromptResult::Selected(paths) => {
                let Some(path) = paths.into_iter().next() else {
                    self.notices
                        .complete(NoticeSource::Browse(game), request.get(), None);
                    cx.notify();
                    return Ok(());
                };
                if path.to_str().is_none() {
                    let error = SelectedRootError::NonUnicode { game, path };
                    self.notices.complete(
                        NoticeSource::Browse(game),
                        request.get(),
                        Some(Notice::error(
                            "Could not use the selected game folder",
                            &error,
                        )),
                    );
                    cx.notify();
                    return Err(error);
                }

                #[cfg(test)]
                {
                    self.task_launches.inspection += 1;
                }
                let task = cx.background_executor().spawn(async move {
                    GameInstallation::inspect(game, path).map_err(SelectedRootError::from)
                });
                cx.spawn(async move |entity, cx| {
                    let result = task.await;
                    let _ = entity.update(cx, move |frame, cx| {
                        frame.finish_browse_inspection(game, request, result, cx);
                    });
                })
                .detach();
            }
        }
        cx.notify();
        Ok(())
    }

    fn finish_browse_inspection(
        &mut self,
        game: Game,
        request: RequestID,
        result: Result<GameInstallation, SelectedRootError>,
        cx: &mut Context<Self>,
    ) {
        if !self.shell.accepts_browse(game, request) {
            return;
        }

        let installation = match result {
            Ok(installation) => installation,
            Err(error) => {
                self.notices.complete(
                    NoticeSource::Browse(game),
                    request.get(),
                    Some(Notice::error(
                        "Could not use the selected game folder",
                        &error,
                    )),
                );
                cx.notify();
                return;
            }
        };
        let root = installation.root().to_path_buf();
        let previous = self.game_paths.root(game).map(ToOwned::to_owned);
        if previous.as_ref() == Some(&root) {
            if game == Game::Crusaders {
                self.reconcile_save_catalog(cx);
            }
            self.notices
                .complete(NoticeSource::Browse(game), request.get(), None);
            cx.notify();
            return;
        }

        self.game_paths.set_root(game, Some(root));
        if !self.schedule_settings_write(self.shell.game(), cx) {
            self.game_paths.set_root(game, previous);
            if game == Game::Crusaders {
                self.reconcile_save_catalog(cx);
            }
            self.notices
                .complete(NoticeSource::Browse(game), request.get(), None);
            return;
        }
        #[cfg(test)]
        {
            self.task_launches.settings += 1;
        }
        self.invalidate_discovery();
        self.root_revisions.bump(game);
        self.notices
            .complete(NoticeSource::Browse(game), request.get(), None);
        if self.shell.game() == game {
            #[cfg(test)]
            {
                self.task_launches.catalog += 1;
            }
            self.start_catalog_load(cx);
        }
        if game == Game::Crusaders {
            self.reconcile_save_catalog(cx);
        }
        cx.notify();
    }

    pub(crate) fn clear_game_root(&mut self, game: Game, cx: &mut Context<Self>) {
        self.shell.invalidate_browse(game);
        self.notices.clear(NoticeSource::Browse(game));
        self.invalidate_discovery();
        self.root_revisions.bump(game);

        let previous = self.game_paths.root(game).map(ToOwned::to_owned);
        if previous.is_none() {
            if game == Game::Crusaders {
                self.reconcile_save_catalog(cx);
            }
            cx.notify();
            return;
        }
        self.game_paths.set_root(game, None);
        if !self.schedule_settings_write(self.shell.game(), cx) {
            self.game_paths.set_root(game, previous);
            if game == Game::Crusaders {
                self.reconcile_save_catalog(cx);
            }
            return;
        }
        #[cfg(test)]
        {
            self.task_launches.settings += 1;
        }
        if self.shell.game() == game {
            #[cfg(test)]
            {
                self.task_launches.catalog += 1;
            }
            self.start_catalog_load(cx);
        }
        if game == Game::Crusaders {
            self.reconcile_save_catalog(cx);
        }
        cx.notify();
    }

    pub(crate) fn start_discovery(&mut self, cx: &mut Context<Self>) -> bool {
        if !steam_discovery_available() {
            self.notices.replace(
                NoticeSource::Discovery,
                Notice::plain(
                    NoticeLevel::Warning,
                    "Automatic Steam discovery is unavailable on this platform",
                ),
            );
            cx.notify();
            return false;
        }

        let key = self.begin_discovery_request();
        #[cfg(test)]
        {
            self.task_launches.discovery += 1;
        }
        let task = cx
            .background_executor()
            .spawn(async move { discover_steam_installations() });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_discovery(key, result, cx);
            });
        })
        .detach();
        true
    }

    fn begin_discovery_request(&mut self) -> DiscoveryKey {
        let request = self.shell.begin_discovery();
        let key = DiscoveryKey::new(request, self.root_revisions, &self.game_paths);
        self.discovery.begin(key.clone());
        self.notices.begin(
            NoticeSource::Discovery,
            request.get(),
            Notice::info("Detecting Steam installations"),
        );
        key
    }

    fn finish_discovery(
        &mut self,
        key: DiscoveryKey,
        result: Result<DiscoveryReport, DiscoveryError>,
        cx: &mut Context<Self>,
    ) {
        if !self.shell.accepts_discovery(key.request()) {
            return;
        }
        let failure_notice = result
            .as_ref()
            .err()
            .map(|error| Notice::error("Could not detect Steam installations", error));
        let request = key.request();
        let previous_paths = self.game_paths.clone();
        let previous_revisions = self.root_revisions;
        match self
            .discovery
            .finish(key, result, &mut self.game_paths, &mut self.root_revisions)
        {
            super::discovery_status::DiscoveryFinish::Stale => return,
            super::discovery_status::DiscoveryFinish::Failed => {
                self.notices
                    .complete(NoticeSource::Discovery, request.get(), failure_notice);
            }
            super::discovery_status::DiscoveryFinish::Ready(update) => {
                if !update.changed_games.is_empty() {
                    if self.schedule_settings_write(self.shell.game(), cx) {
                        #[cfg(test)]
                        {
                            self.task_launches.settings += 1;
                        }
                        if update.changed_games.contains(&self.shell.game()) {
                            #[cfg(test)]
                            {
                                self.task_launches.catalog += 1;
                            }
                            self.start_catalog_load(cx);
                        }
                    } else {
                        self.game_paths = previous_paths;
                        self.root_revisions = previous_revisions;
                    }
                }
                self.reconcile_save_catalog(cx);

                let notice = if update.installation_count == 0 {
                    Some(Notice::plain(
                        NoticeLevel::Warning,
                        "No Steam installations were found",
                    ))
                } else if update.issue_count > 0 {
                    let issue = if update.issue_count == 1 {
                        "issue"
                    } else {
                        "issues"
                    };
                    Some(Notice::plain(
                        NoticeLevel::Warning,
                        format!(
                            "Found Steam installations with {} {issue}",
                            update.issue_count
                        ),
                    ))
                } else {
                    None
                };
                self.notices
                    .complete(NoticeSource::Discovery, request.get(), notice);
            }
        }
        cx.notify();
    }

    fn invalidate_discovery(&mut self) {
        self.shell.invalidate_discovery();
        self.discovery.invalidate();
        self.notices.clear(NoticeSource::Discovery);
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled GPUI and temporary installation fixtures make failures fatal"
    )]

    use std::{error::Error, fs, mem::size_of, path::PathBuf};

    use gpui::{AppContext, TestAppContext, WindowOptions};
    use kufeditor_game::{
        DiscoveryError, DiscoveryReport, Game, GameInstallation, scan_steam_common_directories,
        steam_discovery_available,
    };
    use kufeditor_workspace::{Document, DocumentID, SaveDocument};

    use super::{BrowsePromptResult, SelectedRootError, game_folder_prompt_options};
    use crate::{
        catalog_status::CatalogStatus,
        frame::{
            AppFrame,
            discovery_status::{DiscoveryStatus, RootRevisions},
        },
        notices::{Notice, NoticeLevel, NoticeSource},
        save_catalog_status::{SaveCatalogKey, SaveCatalogStatus},
        settings::SettingsStartup,
    };

    const SAVE_CONTEXT_SIZE: usize = 0x438;
    const SAVE_MAIN_SIZE: usize = 0x154;
    const SAVE_PADDED_SIZE: usize = 0x8000;

    struct InstallationFixture {
        _temporary: tempfile::TempDir,
        crusaders: PathBuf,
        heroes: PathBuf,
    }

    impl InstallationFixture {
        fn complete() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let crusaders = temporary.path().join("crusaders");
            let heroes = temporary.path().join("heroes");
            fs::create_dir_all(crusaders.join("Data/SOX")).unwrap();
            fs::create_dir_all(heroes.join("Data/SOX")).unwrap();
            Self {
                _temporary: temporary,
                crusaders,
                heroes,
            }
        }
    }

    struct ReportFixture {
        _temporary: tempfile::TempDir,
        report: DiscoveryReport,
        crusaders: Option<PathBuf>,
        heroes: Option<PathBuf>,
    }

    impl ReportFixture {
        fn empty() -> Self {
            Self {
                _temporary: tempfile::tempdir().unwrap(),
                report: scan_steam_common_directories(&[]),
                crusaders: None,
                heroes: None,
            }
        }

        fn complete() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let common = temporary.path().join("common");
            let crusaders = common.join("KUF Crusader");
            let heroes = common.join("KUF Heroes");
            fs::create_dir_all(crusaders.join("Data/SOX")).unwrap();
            fs::create_dir_all(heroes.join("Data/SOX")).unwrap();
            let report = scan_steam_common_directories(&[common]);
            Self {
                _temporary: temporary,
                report,
                crusaders: Some(crusaders),
                heroes: Some(heroes),
            }
        }

        #[cfg(unix)]
        fn with_issue() -> Self {
            use std::os::unix::fs::symlink;

            let temporary = tempfile::tempdir().unwrap();
            let common = temporary.path().join("common");
            let crusaders = common.join("KUF Crusader");
            let heroes = common.join("KUF Heroes");
            fs::create_dir_all(crusaders.join("Data/SOX")).unwrap();
            symlink("KUF Heroes", &heroes).unwrap();
            let report = scan_steam_common_directories(&[common]);
            Self {
                _temporary: temporary,
                report,
                crusaders: Some(crusaders),
                heroes: None,
            }
        }
    }

    fn test_window(cx: &mut TestAppContext) -> gpui::WindowHandle<AppFrame> {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        drop(file);
        let startup = SettingsStartup::load(path);
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        })
    }

    fn begin_browse(frame: &mut AppFrame, game: Game) -> crate::state::RequestID {
        frame.begin_browse_request(game)
    }

    fn begin_discovery(frame: &mut AppFrame) -> crate::frame::discovery_status::DiscoveryKey {
        frame.begin_discovery_request()
    }

    fn save_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0x6e);
        append_u32(&mut bytes, u32::MAX);
        bytes.resize(bytes.len() + SAVE_CONTEXT_SIZE - size_of::<u32>(), 0);
        append_u32(&mut bytes, 0);
        bytes.resize(bytes.len() + SAVE_MAIN_SIZE, 0);
        append_u32(&mut bytes, 0);
        append_i32(&mut bytes, -1);
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0);
        for _ in 0..20 {
            append_u32(&mut bytes, 0);
        }
        append_u32(&mut bytes, 0);
        bytes.resize(SAVE_PADDED_SIZE, 0);
        let length = u32::try_from(bytes.len()).unwrap();
        bytes
            .get_mut(..size_of::<u32>())
            .unwrap()
            .copy_from_slice(&length.to_le_bytes());
        bytes
    }

    fn append_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn append_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn open_active_save(frame: &mut AppFrame, cx: &mut gpui::Context<AppFrame>) -> DocumentID {
        let document = frame.workspace.open_loaded(
            PathBuf::from("campaign.sav"),
            Document::Save(SaveDocument::parse(save_fixture()).unwrap()),
        );
        frame.activate_document(document, cx);
        document
    }

    #[test]
    fn folder_prompt_requests_one_directory_with_the_stable_prompt() {
        let options = game_folder_prompt_options();

        assert!(!options.files);
        assert!(options.directories);
        assert!(!options.multiple);
        assert_eq!(
            options.prompt.as_ref().map(ToString::to_string),
            Some("Select game folder".to_owned())
        );
    }

    #[cfg(unix)]
    #[gpui::test]
    fn non_unicode_selection_returns_typed_error_before_inspection_or_mutation(
        cx: &mut TestAppContext,
    ) {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let window = test_window(cx);
        let selected = PathBuf::from(OsString::from_vec(vec![b'/', 0xff]));
        window
            .update(cx, |frame, _, cx| {
                let request = begin_browse(frame, Game::Crusaders);
                let error = frame
                    .finish_browse_prompt(
                        Game::Crusaders,
                        request,
                        BrowsePromptResult::Selected(vec![selected.clone()]),
                        cx,
                    )
                    .unwrap_err();

                assert!(matches!(
                    error,
                    SelectedRootError::NonUnicode {
                        game: Game::Crusaders,
                        ref path,
                    } if path == &selected
                ));
                assert!(error.source().is_none());
                assert_eq!(frame.task_launches.inspection, 0);
                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 0);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.task_launches.inspection, 0);
                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
            })
            .unwrap();
    }

    #[gpui::test]
    fn selected_root_inspection_runs_as_background_work(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let selected = fixture.crusaders.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let request = begin_browse(frame, Game::Crusaders);
                assert!(
                    frame
                        .finish_browse_prompt(
                            Game::Crusaders,
                            request,
                            BrowsePromptResult::Selected(vec![selected.clone()]),
                            cx,
                        )
                        .is_ok()
                );

                assert_eq!(frame.task_launches.inspection, 1);
                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
            })
            .unwrap();

        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(selected.as_path())
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn canceled_and_failed_prompts_affect_only_the_current_browse_slot(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let stale = begin_browse(frame, Game::Crusaders);
                let current = begin_browse(frame, Game::Crusaders);
                let heroes = begin_browse(frame, Game::Heroes);

                assert!(
                    frame
                        .finish_browse_prompt(
                            Game::Crusaders,
                            stale,
                            BrowsePromptResult::Canceled,
                            cx,
                        )
                        .is_ok()
                );
                assert!(frame.notices.complete(
                    NoticeSource::Browse(Game::Crusaders),
                    current.get(),
                    Some(Notice::info("current Crusaders browse")),
                ));
                assert!(frame.notices.complete(
                    NoticeSource::Browse(Game::Heroes),
                    heroes.get(),
                    Some(Notice::info("current Heroes browse")),
                ));
                assert!(
                    frame
                        .finish_browse_prompt(
                            Game::Heroes,
                            heroes,
                            BrowsePromptResult::Canceled,
                            cx,
                        )
                        .is_ok()
                );
                assert!(!frame.notices.complete(
                    NoticeSource::Browse(Game::Heroes),
                    heroes.get(),
                    Some(Notice::info("canceled Heroes browse")),
                ));

                let failed = begin_browse(frame, Game::Crusaders);
                assert!(
                    frame
                        .finish_browse_prompt(
                            Game::Crusaders,
                            failed,
                            BrowsePromptResult::Failed(Notice::plain(
                                NoticeLevel::Error,
                                "Folder picker failed",
                            )),
                            cx,
                        )
                        .is_ok()
                );
                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 0);
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Error);
                assert_eq!(notice.summary(), "Folder picker failed");
            })
            .unwrap();
    }

    #[gpui::test]
    fn invalid_or_stale_browse_results_preserve_root_and_revision(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                let revision = frame.root_revisions.revision(Game::Crusaders);
                let invalid_request = begin_browse(frame, Game::Crusaders);
                frame.finish_browse_inspection(
                    Game::Crusaders,
                    invalid_request,
                    Err(SelectedRootError::Inspection(
                        kufeditor_game::InstallationError::RootMissing {
                            game: Game::Crusaders,
                            root: PathBuf::from("/missing"),
                        },
                    )),
                    cx,
                );
                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(fixture.crusaders.as_path())
                );
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), revision);

                let stale = begin_browse(frame, Game::Crusaders);
                let _current = begin_browse(frame, Game::Crusaders);
                frame.finish_browse_inspection(
                    Game::Crusaders,
                    stale,
                    Ok(GameInstallation::inspect(Game::Crusaders, &fixture.heroes).unwrap()),
                    cx,
                );
                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(fixture.crusaders.as_path())
                );
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), revision);
            })
            .unwrap();
    }

    #[gpui::test]
    fn current_browse_changes_one_root_revision_snapshot_and_active_catalog(
        cx: &mut TestAppContext,
    ) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                open_active_save(frame, cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::NotConfigured
                ));
                let request = begin_browse(frame, Game::Crusaders);
                frame.finish_browse_inspection(
                    Game::Crusaders,
                    request,
                    Ok(GameInstallation::inspect(Game::Crusaders, &fixture.crusaders).unwrap()),
                    cx,
                );

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(fixture.crusaders.as_path())
                );
                assert_eq!(frame.game_paths.root(Game::Heroes), None);
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert_eq!(frame.root_revisions.revision(Game::Heroes), 0);
                assert_eq!(frame.settings.latest_revision_for_test().unwrap().get(), 1);
                assert_eq!(frame.task_launches.settings, 1);
                assert_eq!(frame.task_launches.catalog, 1);
                assert_eq!(frame.task_launches.save_catalog, 1);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == fixture.crusaders
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn same_root_browse_completion_reconciles_the_existing_crusaders_root(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                open_active_save(frame, cx);
                let stale_request = frame.shell.begin_save_catalog();
                frame
                    .save_catalog
                    .begin(SaveCatalogKey::new(stale_request, "/stale/crusaders"));
                let request = begin_browse(frame, Game::Crusaders);

                frame.finish_browse_inspection(
                    Game::Crusaders,
                    request,
                    Ok(GameInstallation::inspect(Game::Crusaders, &fixture.crusaders).unwrap()),
                    cx,
                );

                assert_eq!(frame.task_launches.settings, 0);
                assert_eq!(frame.task_launches.save_catalog, 2);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == fixture.crusaders
                            && key.request() != stale_request
                ));
            })
            .unwrap();
    }

    #[cfg(unix)]
    #[gpui::test]
    fn browse_snapshot_rollback_reconciles_the_restored_crusaders_root(cx: &mut TestAppContext) {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let previous = InstallationFixture::complete();
        let selected = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(previous.crusaders.clone()));
                open_active_save(frame, cx);
                let stale_request = frame.shell.begin_save_catalog();
                frame
                    .save_catalog
                    .begin(SaveCatalogKey::new(stale_request, "/stale/crusaders"));
                frame
                    .recent_files
                    .add(PathBuf::from(OsString::from_vec(vec![b'/', 0xff])));
                let request = begin_browse(frame, Game::Crusaders);

                frame.finish_browse_inspection(
                    Game::Crusaders,
                    request,
                    Ok(GameInstallation::inspect(Game::Crusaders, &selected.crusaders).unwrap()),
                    cx,
                );

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(previous.crusaders.as_path())
                );
                assert_eq!(frame.task_launches.settings, 0);
                assert_eq!(frame.task_launches.save_catalog, 2);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == previous.crusaders
                            && key.request() != stale_request
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn heroes_browse_does_not_restart_the_crusaders_save_catalog(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                open_active_save(frame, cx);
                let save_key = match frame.save_catalog.status() {
                    SaveCatalogStatus::Loading { key } => key.clone(),
                    _ => panic!("configured active save must start loading"),
                };
                let request = begin_browse(frame, Game::Heroes);

                frame.finish_browse_inspection(
                    Game::Heroes,
                    request,
                    Ok(GameInstallation::inspect(Game::Heroes, &fixture.heroes).unwrap()),
                    cx,
                );

                assert_eq!(frame.task_launches.save_catalog, 1);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key } if key == &save_key
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn clear_invalidates_async_roots_and_resets_the_active_catalog(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                open_active_save(frame, cx);
                let save_key = match frame.save_catalog.status() {
                    SaveCatalogStatus::Loading { key } => key.clone(),
                    _ => panic!("configured active save must start loading"),
                };
                frame.start_catalog_load(cx);
                let browse = begin_browse(frame, Game::Crusaders);
                let discovery = begin_discovery(frame);

                frame.clear_game_root(Game::Crusaders, cx);

                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::NotConfigured
                ));
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::NotConfigured
                ));
                assert!(!frame.shell.accepts_save_catalog(save_key.request()));
                assert!(!frame.notices.complete(
                    NoticeSource::Browse(Game::Crusaders),
                    browse.get(),
                    None,
                ));
                assert!(!frame.notices.complete(
                    NoticeSource::Discovery,
                    discovery.request().get(),
                    None,
                ));

                frame.finish_browse_inspection(
                    Game::Crusaders,
                    browse,
                    Ok(GameInstallation::inspect(Game::Crusaders, &fixture.crusaders).unwrap()),
                    cx,
                );
                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
                assert!(!frame.notices.complete(
                    NoticeSource::Browse(Game::Crusaders),
                    browse.get(),
                    Some(Notice::info("stale browse")),
                ));
            })
            .unwrap();
    }

    #[cfg(unix)]
    #[gpui::test]
    fn clear_snapshot_rollback_reconciles_the_restored_crusaders_root(cx: &mut TestAppContext) {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                open_active_save(frame, cx);
                let stale_request = frame.shell.begin_save_catalog();
                frame
                    .save_catalog
                    .begin(SaveCatalogKey::new(stale_request, "/stale/crusaders"));
                frame
                    .recent_files
                    .add(PathBuf::from(OsString::from_vec(vec![b'/', 0xff])));

                frame.clear_game_root(Game::Crusaders, cx);

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(fixture.crusaders.as_path())
                );
                assert_eq!(frame.task_launches.settings, 0);
                assert_eq!(frame.task_launches.save_catalog, 2);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == fixture.crusaders
                            && key.request() != stale_request
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn clear_while_sox_is_active_invalidates_retained_save_work(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                open_active_save(frame, cx);
                let save_request = match frame.save_catalog.status() {
                    SaveCatalogStatus::Loading { key } => key.request(),
                    _ => panic!("configured active save must start loading"),
                };
                let sox = frame.workspace.open_loaded(
                    PathBuf::from("TroopInfo.sox"),
                    Document::Troop({
                        let mut bytes = vec![0_u8; 8 + 148 + 64];
                        bytes
                            .get_mut(..8)
                            .unwrap()
                            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
                        kufeditor_workspace::TroopDocument::parse(bytes).unwrap()
                    }),
                );
                frame.activate_document(sox, cx);

                frame.clear_game_root(Game::Crusaders, cx);

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));
                assert!(!frame.shell.accepts_save_catalog(save_request));
            })
            .unwrap();
    }

    #[gpui::test]
    fn clearing_inactive_root_leaves_active_catalog_unchanged(cx: &mut TestAppContext) {
        let fixture = InstallationFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(fixture.crusaders.clone()));
                frame
                    .game_paths
                    .set_root(Game::Heroes, Some(fixture.heroes.clone()));
                frame.start_catalog_load(cx);
                let before = match frame.catalog.status() {
                    CatalogStatus::Loading { key }
                    | CatalogStatus::Ready { key, .. }
                    | CatalogStatus::Failed { key, .. } => key.clone(),
                    CatalogStatus::NotConfigured => panic!("active root must start a catalog"),
                };

                frame.clear_game_root(Game::Heroes, cx);

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(fixture.crusaders.as_path())
                );
                assert_eq!(frame.game_paths.root(Game::Heroes), None);
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Loading { key }
                    | CatalogStatus::Ready { key, .. }
                    | CatalogStatus::Failed { key, .. } if key == &before
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn clear_on_empty_root_rejects_browse_and_discovery_completions_without_a_write(
        cx: &mut TestAppContext,
    ) {
        let fixture = InstallationFixture::complete();
        let report = ReportFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let browse = begin_browse(frame, Game::Crusaders);
                let discovery = begin_discovery(frame);
                let stale_request = frame.shell.begin_save_catalog();
                frame
                    .save_catalog
                    .begin(SaveCatalogKey::new(stale_request, "/stale/crusaders"));

                frame.clear_game_root(Game::Crusaders, cx);

                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert!(frame.settings.latest_revision_for_test().is_none());
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));
                assert!(!frame.shell.accepts_save_catalog(stale_request));
                frame.finish_browse_inspection(
                    Game::Crusaders,
                    browse,
                    Ok(GameInstallation::inspect(Game::Crusaders, &fixture.crusaders).unwrap()),
                    cx,
                );
                frame.finish_discovery(discovery.clone(), Ok(report.report), cx);
                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
                assert_eq!(frame.game_paths.root(Game::Heroes), None);
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));
                assert!(frame.settings.latest_revision_for_test().is_none());
                assert!(!frame.notices.complete(
                    NoticeSource::Browse(Game::Crusaders),
                    browse.get(),
                    Some(Notice::info("stale")),
                ));
                assert!(!frame.notices.complete(
                    NoticeSource::Discovery,
                    discovery.request().get(),
                    Some(Notice::info("stale")),
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn unsupported_discovery_stops_before_request_or_task_launch(cx: &mut TestAppContext) {
        if steam_discovery_available() {
            return;
        }
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                assert!(!frame.start_discovery(cx));
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));
                assert_eq!(frame.task_launches.discovery, 0);
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert!(notice.summary().contains("unavailable"));
            })
            .unwrap();
    }

    #[gpui::test]
    fn beginning_discovery_installs_loading_and_one_current_notice(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, _| {
                let key = begin_discovery(frame);
                assert!(matches!(
                    frame.discovery.status(),
                    DiscoveryStatus::Loading { key: current } if current == &key
                ));
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Detecting Steam installations")
                );
                assert!(frame.notices.complete(
                    NoticeSource::Discovery,
                    key.request().get(),
                    Some(Notice::info("current discovery")),
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn zero_installations_becomes_ready_with_warning(cx: &mut TestAppContext) {
        let fixture = ReportFixture::empty();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let key = begin_discovery(frame);
                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert!(matches!(
                    frame.discovery.status(),
                    DiscoveryStatus::Ready { report, .. }
                        if report.installations().is_empty() && report.issues().is_empty()
                ));
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert_eq!(notice.summary(), "No Steam installations were found");
            })
            .unwrap();
    }

    #[gpui::test]
    fn accepted_discovery_with_unchanged_roots_reconciles_the_save_catalog(
        cx: &mut TestAppContext,
    ) {
        let fixture = ReportFixture::complete();
        let expected_crusaders = fixture.crusaders.clone().unwrap();
        let expected_heroes = fixture.heroes.clone().unwrap();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(expected_crusaders.clone()));
                frame
                    .game_paths
                    .set_root(Game::Heroes, Some(expected_heroes.clone()));
                open_active_save(frame, cx);
                let stale_request = frame.shell.begin_save_catalog();
                frame
                    .save_catalog
                    .begin(SaveCatalogKey::new(stale_request, "/stale/crusaders"));
                let key = begin_discovery(frame);

                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert_eq!(frame.root_revisions, RootRevisions::default());
                assert_eq!(frame.task_launches.settings, 0);
                assert_eq!(frame.task_launches.save_catalog, 2);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == expected_crusaders
                            && key.request() != stale_request
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn zero_installation_discovery_reconciles_the_configured_save_catalog(cx: &mut TestAppContext) {
        let installation = InstallationFixture::complete();
        let fixture = ReportFixture::empty();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(installation.crusaders.clone()));
                open_active_save(frame, cx);
                let stale_request = frame.shell.begin_save_catalog();
                frame
                    .save_catalog
                    .begin(SaveCatalogKey::new(stale_request, "/stale/crusaders"));
                let key = begin_discovery(frame);

                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert_eq!(frame.task_launches.settings, 0);
                assert_eq!(frame.task_launches.save_catalog, 2);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == installation.crusaders
                            && key.request() != stale_request
                ));
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert_eq!(notice.summary(), "No Steam installations were found");
            })
            .unwrap();
    }

    #[gpui::test]
    fn heroes_only_discovery_restores_matching_save_catalog_without_restarting(
        cx: &mut TestAppContext,
    ) {
        let fixture = ReportFixture::complete();
        let expected_crusaders = fixture.crusaders.clone().unwrap();
        let expected_heroes = fixture.heroes.clone().unwrap();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(expected_crusaders.clone()));
                open_active_save(frame, cx);
                let save_key = match frame.save_catalog.status() {
                    SaveCatalogStatus::Loading { key } => key.clone(),
                    _ => panic!("configured active save must start loading"),
                };
                assert!(
                    !frame
                        .save_catalog
                        .dormant(Some(expected_crusaders.as_path()))
                );
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));
                let key = begin_discovery(frame);

                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert_eq!(
                    frame.game_paths.root(Game::Heroes),
                    Some(expected_heroes.as_path())
                );
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 0);
                assert_eq!(frame.root_revisions.revision(Game::Heroes), 1);
                assert_eq!(frame.task_launches.save_catalog, 1);
                assert!(frame.shell.accepts_save_catalog(save_key.request()));
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key } if key == &save_key
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn accepted_browse_during_discovery_loading_makes_completion_stale(cx: &mut TestAppContext) {
        let installation = InstallationFixture::complete();
        let report = ReportFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let discovery = begin_discovery(frame);
                let browse = begin_browse(frame, Game::Crusaders);
                frame.finish_browse_inspection(
                    Game::Crusaders,
                    browse,
                    Ok(
                        GameInstallation::inspect(Game::Crusaders, &installation.crusaders)
                            .unwrap(),
                    ),
                    cx,
                );
                frame.finish_discovery(discovery.clone(), Ok(report.report), cx);

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(installation.crusaders.as_path())
                );
                assert_eq!(frame.game_paths.root(Game::Heroes), None);
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert_eq!(frame.root_revisions.revision(Game::Heroes), 0);
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));
                assert!(!frame.notices.complete(
                    NoticeSource::Discovery,
                    discovery.request().get(),
                    Some(Notice::info("stale discovery")),
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn one_discovered_root_bumps_and_writes_once(cx: &mut TestAppContext) {
        let fixture = ReportFixture::complete();
        let expected_crusaders = fixture.crusaders.clone().unwrap();
        let configured_heroes = PathBuf::from("/configured/heroes");
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Heroes, Some(configured_heroes.clone()));
                open_active_save(frame, cx);
                let key = begin_discovery(frame);
                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(expected_crusaders.as_path())
                );
                assert_eq!(
                    frame.game_paths.root(Game::Heroes),
                    Some(configured_heroes.as_path())
                );
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert_eq!(frame.root_revisions.revision(Game::Heroes), 0);
                assert_eq!(frame.settings.latest_revision_for_test().unwrap().get(), 1);
                assert_eq!(frame.task_launches.settings, 1);
                assert_eq!(frame.task_launches.catalog, 1);
                assert_eq!(frame.task_launches.save_catalog, 1);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key }
                        if key.root() == expected_crusaders
                ));
            })
            .unwrap();
    }

    #[cfg(unix)]
    #[gpui::test]
    fn discovery_settings_validation_rolls_back_roots_revisions_and_catalog_work(
        cx: &mut TestAppContext,
    ) {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let fixture = ReportFixture::complete();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .recent_files
                    .add(PathBuf::from(OsString::from_vec(vec![b'/', 0xff])));
                open_active_save(frame, cx);
                let key = begin_discovery(frame);
                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert_eq!(frame.game_paths.root(Game::Crusaders), None);
                assert_eq!(frame.game_paths.root(Game::Heroes), None);
                assert_eq!(frame.root_revisions, RootRevisions::default());
                assert_eq!(frame.task_launches.settings, 0);
                assert_eq!(frame.task_launches.catalog, 0);
                assert_eq!(frame.task_launches.save_catalog, 0);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::NotConfigured
                ));
                assert!(matches!(
                    frame.discovery.status(),
                    DiscoveryStatus::Ready { .. }
                ));
                assert_eq!(frame.notices.current().unwrap().level(), NoticeLevel::Error);
            })
            .unwrap();
    }

    #[cfg(unix)]
    #[gpui::test]
    fn installations_with_issues_apply_roots_and_warn_with_issue_count(cx: &mut TestAppContext) {
        let fixture = ReportFixture::with_issue();
        let expected = fixture.crusaders.clone().unwrap();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let key = begin_discovery(frame);
                frame.finish_discovery(key, Ok(fixture.report), cx);

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(expected.as_path())
                );
                assert_eq!(frame.game_paths.root(Game::Heroes), None);
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert_eq!(notice.summary(), "Found Steam installations with 1 issue");
            })
            .unwrap();
    }

    #[gpui::test]
    fn two_discovered_roots_bump_once_write_once_and_restart_active_catalog(
        cx: &mut TestAppContext,
    ) {
        let fixture = ReportFixture::complete();
        let expected_crusaders = fixture.crusaders.clone().unwrap();
        let expected_heroes = fixture.heroes.clone().unwrap();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let key = begin_discovery(frame);
                frame.finish_discovery(key.clone(), Ok(fixture.report), cx);

                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(expected_crusaders.as_path())
                );
                assert_eq!(
                    frame.game_paths.root(Game::Heroes),
                    Some(expected_heroes.as_path())
                );
                assert_eq!(frame.root_revisions.revision(Game::Crusaders), 1);
                assert_eq!(frame.root_revisions.revision(Game::Heroes), 1);
                assert_eq!(frame.settings.latest_revision_for_test().unwrap().get(), 1);
                assert_eq!(frame.task_launches.settings, 1);
                assert_eq!(frame.task_launches.catalog, 1);
                assert!(!frame.notices.complete(
                    NoticeSource::Discovery,
                    key.request().get(),
                    None,
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn current_discovery_failure_becomes_failed_with_error_notice(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let key = begin_discovery(frame);
                frame.finish_discovery(key, Err(DiscoveryError::Unavailable), cx);

                assert!(matches!(
                    frame.discovery.status(),
                    DiscoveryStatus::Failed {
                        error: DiscoveryError::Unavailable,
                        ..
                    }
                ));
                assert_eq!(frame.notices.current().unwrap().level(), NoticeLevel::Error);
            })
            .unwrap();
    }
}
