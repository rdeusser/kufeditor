use gpui::Context;
use kufeditor_game::{CatalogLoad, GameInstallation, load_name_dictionary};

use super::AppFrame;
use crate::{
    catalog_status::{CatalogKey, CatalogRequestError},
    notices::{Notice, NoticeLevel, NoticeSource},
};

impl AppFrame {
    pub(crate) fn start_catalog_load(&mut self, cx: &mut Context<Self>) {
        let game = self.shell.game();
        let Some(root) = self.game_paths.root(game).map(ToOwned::to_owned) else {
            self.shell.invalidate_catalog();
            self.catalog.not_configured();
            self.notices.clear(NoticeSource::Catalog);
            cx.notify();
            return;
        };

        let request = self.shell.begin_catalog();
        let key = CatalogKey::new(request, game, root);
        self.catalog.begin(key.clone());
        self.notices.begin(
            NoticeSource::Catalog,
            request.get(),
            Notice::info("Loading game catalogs"),
        );
        cx.notify();

        let work_key = key.clone();
        let task = cx.background_executor().spawn(async move {
            let installation = GameInstallation::inspect(work_key.game(), work_key.root())?;
            let catalog = load_name_dictionary(&installation.sox_directory())?;
            Ok::<CatalogLoad, CatalogRequestError>(catalog)
        });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_catalog_load(key, result, cx);
            });
        })
        .detach();
    }

    fn finish_catalog_load(
        &mut self,
        key: CatalogKey,
        result: Result<CatalogLoad, CatalogRequestError>,
        cx: &mut Context<Self>,
    ) {
        if !self.shell.accepts_catalog(key.request()) {
            return;
        }
        let request = key.request();

        match result {
            Ok(CatalogLoad { dictionary, issues }) => {
                let issue_count = issues.len();
                if !self.catalog.finish_ready(key, dictionary) {
                    return;
                }
                let notice = (issue_count > 0).then(|| {
                    let issue_label = if issue_count == 1 { "issue" } else { "issues" };
                    Notice::plain(
                        NoticeLevel::Warning,
                        format!("Loaded game catalogs with {issue_count} {issue_label}"),
                    )
                });
                self.notices
                    .complete(NoticeSource::Catalog, request.get(), notice);
            }
            Err(error) => {
                let notice = Notice::error("Could not load game catalogs", &error);
                if !self.catalog.finish_failed(key, error) {
                    return;
                }
                self.notices
                    .complete(NoticeSource::Catalog, request.get(), Some(notice));
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "the GPUI tests use controlled temporary paths and windows"
    )]

    use std::{fs, io, path::PathBuf};

    use gpui::{AppContext, TestAppContext, WindowOptions};
    use kufeditor_game::{CatalogLoadError, CatalogRole, Game, InstallationError};
    use tempfile::TempDir;

    use super::super::AppFrame;
    use crate::{
        catalog_status::{CatalogKey, CatalogRequestError, CatalogStatus},
        notices::{Notice, NoticeLevel, NoticeSource},
        settings::SettingsStartup,
    };

    fn test_startup() -> SettingsStartup {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        drop(file);
        SettingsStartup::load(path)
    }

    fn test_window(cx: &mut TestAppContext) -> gpui::WindowHandle<AppFrame> {
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        })
    }

    fn begin_catalog(frame: &mut AppFrame, game: Game, root: &str) -> CatalogKey {
        let request = frame.shell.begin_catalog();
        let key = CatalogKey::new(request, game, root);
        frame.catalog.begin(key.clone());
        frame.notices.begin(
            NoticeSource::Catalog,
            request.get(),
            Notice::info("Loading game catalogs"),
        );
        key
    }

    fn installation_errors() -> Vec<InstallationError> {
        vec![
            InstallationError::RootMissing {
                game: Game::Crusaders,
                root: PathBuf::from("/missing"),
            },
            InstallationError::RootNotDirectory {
                game: Game::Crusaders,
                root: PathBuf::from("/file"),
            },
            InstallationError::SoxMissing {
                game: Game::Crusaders,
                root: PathBuf::from("/game"),
                sox_path: PathBuf::from("/game/Data/SOX"),
            },
            InstallationError::SoxNotDirectory {
                game: Game::Crusaders,
                root: PathBuf::from("/game"),
                sox_path: PathBuf::from("/game/Data/SOX"),
            },
            InstallationError::Metadata {
                game: Game::Crusaders,
                root: PathBuf::from("/game"),
                path: PathBuf::from("/game/Data/SOX"),
                source: io::Error::other("fixture metadata failure"),
            },
        ]
    }

    struct CatalogTree {
        _temporary: TempDir,
        root: PathBuf,
    }

    impl CatalogTree {
        fn complete() -> Self {
            let temporary = TempDir::new().unwrap();
            let root = temporary.path().join("game");
            let sox = root.join("Data/SOX");
            let text = root.join("Data/Text/ENG");
            fs::create_dir_all(sox.join("ENG")).unwrap();
            fs::create_dir_all(&text).unwrap();
            fs::write(
                sox.join(CatalogRole::TroopNames.relative_path()),
                indexed_table(&[(2, b"Footman")]),
            )
            .unwrap();
            fs::write(
                sox.join(CatalogRole::CharacterNames.relative_path()),
                indexed_table(&[(7, b"Gerald")]),
            )
            .unwrap();
            fs::write(
                sox.join(CatalogRole::LeaderPools.relative_path()),
                indexed_table(&[(1, b"Alpha Beta")]),
            )
            .unwrap();
            fs::write(
                sox.join(CatalogRole::SpecialNameKeys.relative_path()),
                special_names_table(&[(b"Hero", b"Default Hero")]),
            )
            .unwrap();
            fs::write(
                sox.join(CatalogRole::SpecialDisplayNames.relative_path()),
                sequential_table(&[b"Localized Hero"]),
            )
            .unwrap();
            fs::write(
                sox.join(CatalogRole::ItemAttributes.relative_path()),
                indexed_fields_table(&[(3, &[b"Flame", b"Adds fire"])]),
            )
            .unwrap();
            fs::write(
                sox.join(CatalogRole::ItemTypePrefixes.relative_path()),
                indexed_fields_table(&[(4, &[b"Fine", b"Rare", b"Epic"])]),
            )
            .unwrap();
            fs::write(
                text.join("WeaponNames_ENG.txt"),
                b"1\n1\n9\nSword\nLong Sword\n",
            )
            .unwrap();
            Self {
                _temporary: temporary,
                root,
            }
        }

        fn remove(&self, role: CatalogRole) {
            let path = match role {
                CatalogRole::WeaponNames => self.root.join("Data").join(role.relative_path()),
                _ => self.root.join("Data/SOX").join(role.relative_path()),
            };
            fs::remove_file(path).unwrap();
        }
    }

    fn indexed_table(records: &[(u32, &[u8])]) -> Vec<u8> {
        let mut bytes = table_header(records.len());
        for (id, value) in records {
            bytes.extend_from_slice(&id.to_le_bytes());
            push_field(&mut bytes, value);
        }
        bytes
    }

    fn sequential_table(records: &[&[u8]]) -> Vec<u8> {
        let mut bytes = table_header(records.len());
        for value in records {
            push_field(&mut bytes, value);
        }
        bytes
    }

    fn indexed_fields_table(records: &[(u32, &[&[u8]])]) -> Vec<u8> {
        let mut bytes = table_header(records.len());
        for (id, fields) in records {
            bytes.extend_from_slice(&id.to_le_bytes());
            for field in *fields {
                push_field(&mut bytes, field);
            }
        }
        bytes
    }

    fn special_names_table(records: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut bytes = table_header(records.len());
        for (key, default_value) in records {
            push_field(&mut bytes, key);
            push_field(&mut bytes, default_value);
        }
        bytes.extend_from_slice(&[b' '; 64]);
        bytes
    }

    fn table_header(record_count: usize) -> Vec<u8> {
        let mut bytes = 100_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&u32::try_from(record_count).unwrap().to_le_bytes());
        bytes
    }

    fn push_field(bytes: &mut Vec<u8>, field: &[u8]) {
        bytes.extend_from_slice(&u16::try_from(field.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(field);
    }

    #[gpui::test]
    fn startup_without_an_active_root_is_not_configured(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame.start_catalog_load(cx);

                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::NotConfigured
                ));
                assert!(frame.catalog.ready_value().is_none());
            })
            .unwrap();
    }

    #[gpui::test]
    fn configured_catalog_is_loading_before_background_work(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                frame.start_catalog_load(cx);

                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Loading { .. }
                ));
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Loading game catalogs")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn every_installation_failure_becomes_exact_failed_state_without_mutating_paths(
        cx: &mut TestAppContext,
    ) {
        let window = test_window(cx);

        for error in installation_errors() {
            let expected = error.to_string();
            window
                .update(cx, |frame, _, cx| {
                    frame.game_paths.set_root(
                        Game::Crusaders,
                        Some(PathBuf::from("/configured/crusaders")),
                    );
                    frame
                        .game_paths
                        .set_root(Game::Heroes, Some(PathBuf::from("/configured/heroes")));
                    let paths = frame.game_paths.clone();
                    let key = begin_catalog(frame, Game::Crusaders, "/configured/crusaders");

                    frame.finish_catalog_load(
                        key.clone(),
                        Err(CatalogRequestError::Installation(error)),
                        cx,
                    );

                    assert_eq!(frame.game_paths, paths);
                    assert!(matches!(
                        frame.catalog.status(),
                        CatalogStatus::Failed {
                            key: actual,
                            error: CatalogRequestError::Installation(actual_error),
                        } if actual == &key && actual_error.to_string() == expected
                    ));
                    let notice = frame.notices.current().unwrap();
                    assert_eq!(notice.level(), NoticeLevel::Error);
                    assert!(notice.detail().contains(&expected));
                })
                .unwrap();
        }
    }

    #[gpui::test]
    fn catalog_load_failure_becomes_exact_failed_state_without_mutating_paths(
        cx: &mut TestAppContext,
    ) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let configured = PathBuf::from("/configured/crusaders");
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(configured.clone()));
                let paths = frame.game_paths.clone();
                let key = begin_catalog(frame, Game::Crusaders, "/configured/crusaders");
                let load_error = CatalogLoadError::InvalidSoxDirectory {
                    path: configured.join("Data/SOX"),
                };
                let expected = load_error.to_string();

                frame.finish_catalog_load(
                    key.clone(),
                    Err(CatalogRequestError::Load(load_error)),
                    cx,
                );

                assert_eq!(frame.game_paths, paths);
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Failed {
                        key: actual,
                        error: CatalogRequestError::Load(actual_error),
                    } if actual == &key && actual_error.to_string() == expected
                ));
            })
            .unwrap();
    }

    #[gpui::test]
    fn stale_inspection_and_load_results_change_no_catalog_state_or_notice(
        cx: &mut TestAppContext,
    ) {
        let window = test_window(cx);

        for error in [
            CatalogRequestError::Installation(InstallationError::RootMissing {
                game: Game::Crusaders,
                root: PathBuf::from("/old"),
            }),
            CatalogRequestError::Load(CatalogLoadError::InvalidSoxDirectory {
                path: PathBuf::from("/old/Data/SOX"),
            }),
        ] {
            window
                .update(cx, |frame, _, cx| {
                    let stale = begin_catalog(frame, Game::Crusaders, "/old");
                    let current = begin_catalog(frame, Game::Heroes, "/current");

                    frame.finish_catalog_load(stale, Err(error), cx);

                    assert!(matches!(
                        frame.catalog.status(),
                        CatalogStatus::Loading { key } if key == &current
                    ));
                    assert_eq!(
                        frame.notices.current().map(Notice::summary),
                        Some("Loading game catalogs")
                    );
                })
                .unwrap();
        }
    }

    #[gpui::test]
    fn starting_an_unconfigured_request_clears_the_catalog_notice(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let _ = begin_catalog(frame, Game::Crusaders, "/old");
                frame.shell.select_game(Game::Heroes);

                frame.start_catalog_load(cx);

                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::NotConfigured
                ));
                assert!(frame.notices.current().is_none());
            })
            .unwrap();
    }

    #[gpui::test]
    fn background_success_installs_the_exact_dictionary_and_clears_the_notice(
        cx: &mut TestAppContext,
    ) {
        let tree = CatalogTree::complete();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                frame.start_catalog_load(cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Ready { key, value }
                        if key.game() == Game::Crusaders
                            && key.root() == root
                            && value.troop_name(2) == Some("Footman")
                ));
                assert!(frame.notices.current().is_none());
            })
            .unwrap();
    }

    #[gpui::test]
    fn background_success_with_issues_keeps_the_dictionary_and_warns_with_the_count(
        cx: &mut TestAppContext,
    ) {
        let tree = CatalogTree::complete();
        tree.remove(CatalogRole::LeaderPools);
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                frame.start_catalog_load(cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame
                        .catalog
                        .ready_value()
                        .and_then(|dictionary| dictionary.troop_name(2)),
                    Some("Footman")
                );
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert_eq!(notice.summary(), "Loaded game catalogs with 1 issue");
            })
            .unwrap();
    }

    #[gpui::test]
    fn a_new_catalog_request_removes_the_previous_dictionary_before_work(cx: &mut TestAppContext) {
        let tree = CatalogTree::complete();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                frame.start_catalog_load(cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                assert!(frame.catalog.ready_value().is_some());

                frame.start_catalog_load(cx);

                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Loading { .. }
                ));
                assert!(frame.catalog.ready_value().is_none());
            })
            .unwrap();
    }

    #[gpui::test]
    fn changing_the_active_game_schedules_one_snapshot_and_one_catalog_request(
        cx: &mut TestAppContext,
    ) {
        let tree = CatalogTree::complete();
        let root = tree.root.clone();
        let directory = tempfile::tempdir().unwrap();
        let settings_path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(settings_path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.game_paths.set_root(Game::Heroes, Some(root.clone()));
                frame.select_game(Game::Heroes, cx);

                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Loading { key }
                        if key.game() == Game::Heroes && key.root() == root
                ));
            })
            .unwrap();
        cx.run_until_parked();

        let saved = serde_json::from_slice::<serde_json::Value>(&fs::read(&settings_path).unwrap())
            .unwrap();
        assert_eq!(
            saved.get("active_game").and_then(serde_json::Value::as_str),
            Some("heroes")
        );
        window
            .update(cx, |frame, _, cx| {
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Ready { key, .. }
                        if key.game() == Game::Heroes && key.root() == root
                ));
                fs::remove_file(&settings_path).unwrap();

                frame.select_game(Game::Heroes, cx);
            })
            .unwrap();
        cx.run_until_parked();

        assert!(!settings_path.exists());
        window
            .update(cx, |frame, _, _| {
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Ready { .. }
                ));
            })
            .unwrap();
    }
}
