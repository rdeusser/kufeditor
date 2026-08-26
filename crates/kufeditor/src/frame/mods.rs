use std::path::PathBuf;

use gpui::Context;
use kufeditor_game::Game;
use kufeditor_mods::{
    BackupScan, GameRoot, InstallationScan, ModLibraryScan, ModService, ModStorePaths,
};

use super::AppFrame;
use crate::{
    mod_status::{
        BackupSnapshot, InstalledModSnapshot, ModCollectionSnapshot, ModContextChange,
        ModIssueScope, ModIssueSnapshot, ModPackageSnapshot, ModRequestKey, ModRootCompletion,
        ModScanCompletion, ModScanScope, ModSection,
    },
    state::Area,
};

impl AppFrame {
    pub(crate) fn start_mod_library_scan(&mut self, cx: &mut Context<Self>) {
        self.start_mod_scan_scope(ModScanScope::LibraryOnly, cx);
    }

    pub(crate) fn start_mod_scan(&mut self, cx: &mut Context<Self>) {
        self.start_mod_scan_scope(ModScanScope::Full, cx);
    }

    fn start_mod_scan_scope(&mut self, scope: ModScanScope, cx: &mut Context<Self>) {
        if matches!(
            self.sync_mod_context(),
            ModContextChange::BlockedByOperation
        ) {
            return;
        }
        let game = self.shell.game();
        let configured_root = self.game_paths.root(game).map(ToOwned::to_owned);
        let key = self
            .mods
            .begin_scan(scope, configured_root.as_ref().is_some());
        #[cfg(test)]
        {
            self.task_launches.mods += 1;
        }
        cx.notify();

        let service = self.mod_service.clone();
        let stores = self.mod_stores.clone();
        let task = cx
            .background_executor()
            .spawn(async move { scan_mods(&service, &stores, game, configured_root, scope) });
        cx.spawn(async move |entity, cx| {
            let completion = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_mod_scan(key, completion, cx);
            });
        })
        .detach();
    }

    pub(crate) fn finish_mod_scan(
        &mut self,
        key: ModRequestKey,
        completion: ModScanCompletion,
        cx: &mut Context<Self>,
    ) {
        if self.mods.finish_scan(key, completion) {
            cx.notify();
        }
    }

    pub(crate) fn select_mod_section(&mut self, section: ModSection, cx: &mut Context<Self>) {
        if self.mods.section() == section {
            return;
        }
        self.mods.select_section(section);
        cx.notify();
    }

    pub(super) fn active_mod_context_changed(&mut self, cx: &mut Context<Self>) {
        let change = self.sync_mod_context();
        if change == ModContextChange::Changed && self.shell.area() == Area::Mods {
            self.start_mod_scan(cx);
        }
    }

    fn sync_mod_context(&mut self) -> ModContextChange {
        self.mods.set_context(
            self.shell.game(),
            self.root_revisions.revision(self.shell.game()),
        )
    }
}

fn scan_mods(
    service: &ModService,
    stores: &ModStorePaths,
    game: Game,
    configured_root: Option<PathBuf>,
    scope: ModScanScope,
) -> ModScanCompletion {
    let library = scan_library(service);
    let root = match scope {
        ModScanScope::LibraryOnly => ModRootCompletion::NotRequested,
        ModScanScope::Full => configured_root.map_or(ModRootCompletion::MissingRoot, |root| {
            scan_root(service, stores, game, root)
        }),
    };
    ModScanCompletion::new(library, root)
}

fn scan_library(
    service: &ModService,
) -> Result<ModCollectionSnapshot<ModPackageSnapshot>, ModIssueSnapshot> {
    service.scan_library().map_or_else(
        |error| {
            Err(ModIssueSnapshot::from_error(
                ModIssueScope::Library,
                "library-scan",
                "Could not scan the mod library",
                &error,
            ))
        },
        |scan| Ok(library_snapshot(&scan)),
    )
}

fn library_snapshot(scan: &ModLibraryScan) -> ModCollectionSnapshot<ModPackageSnapshot> {
    let rows = scan.packages().iter().map(Into::into).collect();
    let issues = scan
        .issues()
        .iter()
        .enumerate()
        .map(|(index, issue)| {
            ModIssueSnapshot::from_error(
                ModIssueScope::Library,
                format!("library-{index}"),
                format!("Could not use {}", issue.path().display()),
                issue.error(),
            )
        })
        .collect();
    ModCollectionSnapshot::new(rows, issues)
}

fn scan_root(
    service: &ModService,
    stores: &ModStorePaths,
    game: Game,
    configured_root: PathBuf,
) -> ModRootCompletion {
    let root = match GameRoot::inspect(game, configured_root, stores) {
        Ok(root) => root,
        Err(error) => {
            return ModRootCompletion::Failed(ModIssueSnapshot::from_error(
                ModIssueScope::Root,
                "game-root",
                "Could not inspect the configured game root",
                &error,
            ));
        }
    };
    let installations = service.scan_installations(&root).map_or_else(
        |error| {
            ModCollectionSnapshot::new(
                Vec::new(),
                vec![ModIssueSnapshot::from_error(
                    ModIssueScope::Installed,
                    "installation-scan",
                    "Could not scan installed mods",
                    &error,
                )],
            )
        },
        |scan| installation_snapshot(&scan),
    );
    let backups = service.scan_backups(&root).map_or_else(
        |error| {
            ModCollectionSnapshot::new(
                Vec::new(),
                vec![ModIssueSnapshot::from_error(
                    ModIssueScope::Backups,
                    "backup-scan",
                    "Could not scan backups",
                    &error,
                )],
            )
        },
        |scan| backup_snapshot(&scan),
    );
    ModRootCompletion::Ready {
        configured_root: root.configured_path().display().to_string(),
        installations,
        backups,
    }
}

fn installation_snapshot(scan: &InstallationScan) -> ModCollectionSnapshot<InstalledModSnapshot> {
    let rows = scan.installations().iter().map(Into::into).collect();
    let issues = scan
        .issues()
        .iter()
        .map(|issue| {
            let identity = issue.installation_id().map_or_else(
                || format!("installation-record-{}", issue.record_index()),
                |installation| format!("installation-{installation}"),
            );
            ModIssueSnapshot::from_error(
                ModIssueScope::Installed,
                identity,
                "Could not verify an installed mod",
                issue.error(),
            )
        })
        .collect();
    ModCollectionSnapshot::new(rows, issues)
}

fn backup_snapshot(scan: &BackupScan) -> ModCollectionSnapshot<BackupSnapshot> {
    let rows = scan.backups().iter().map(Into::into).collect();
    let issues = scan
        .issues()
        .iter()
        .enumerate()
        .map(|(index, issue)| {
            ModIssueSnapshot::from_error(
                ModIssueScope::Backups,
                format!("backup-{index}"),
                format!("Could not use {}", issue.path().display()),
                issue.error(),
            )
        })
        .collect();
    ModCollectionSnapshot::new(rows, issues)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "the GPUI tests use controlled temporary settings and game roots"
    )]

    use std::fs;

    use gpui::{AppContext, TestAppContext, WindowOptions};
    use kufeditor_game::Game;

    use super::AppFrame;
    use crate::{
        mod_status::{ModLibraryState, ModRootState},
        settings::SettingsStartup,
        state::Area,
    };

    #[gpui::test]
    fn settings_parent_remains_the_mod_store_when_settings_are_protected(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let settings_path = directory.path().join("settings.json");
        fs::write(&settings_path, br#"{"version":2,"future":true}"#).unwrap();
        let window = test_window(cx, SettingsStartup::load(settings_path));

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.mod_stores.application_data(), directory.path());
            })
            .unwrap();
    }

    #[gpui::test]
    fn library_starts_independently_and_the_mods_route_adds_the_root_scan(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let game_root = directory.path().join("game");
        fs::create_dir(&game_root).unwrap();
        let mut startup = SettingsStartup::load(directory.path().join("settings.json"));
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(game_root));
        let window = test_window(cx, startup);

        window
            .update(cx, |frame, _, cx| {
                frame.start_mod_library_scan(cx);
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Loading
                ));
                assert!(matches!(frame.mods.root_state(), ModRootState::Idle));
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Ready(_)
                ));
                frame.select_area(Area::Mods, cx);
                assert!(matches!(frame.mods.root_state(), ModRootState::Loading));
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Ready(_)
                ));
                assert!(matches!(
                    frame.mods.root_state(),
                    ModRootState::Ready { .. }
                ));
                assert_eq!(frame.task_launches.mods, 2);
            })
            .unwrap();
    }

    #[gpui::test]
    fn a_game_change_discards_the_pending_library_completion(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let window = test_window(
            cx,
            SettingsStartup::load(directory.path().join("settings.json")),
        );

        window
            .update(cx, |frame, _, cx| {
                frame.start_mod_library_scan(cx);
                frame.shell.select_game(Game::Heroes);
                frame.active_mod_context_changed(cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.mods.game(), Game::Heroes);
                assert!(matches!(frame.mods.library_state(), ModLibraryState::Idle));
                assert_eq!(frame.task_launches.mods, 1);
            })
            .unwrap();
    }

    fn test_window(
        cx: &mut TestAppContext,
        startup: SettingsStartup,
    ) -> gpui::WindowHandle<AppFrame> {
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        })
    }
}
