use std::sync::Arc;

use gpui::Context;
use kufeditor_game::{CatalogLoad, Game, GameInstallation, NameDictionary, load_name_dictionary};
use kufeditor_workspace::DocumentKind;

use super::AppFrame;
use crate::{catalog_status::CatalogRequestError, save_catalog_status::SaveCatalogKey};

impl AppFrame {
    pub(crate) fn reconcile_save_catalog(&mut self, cx: &mut Context<Self>) {
        let previous_dictionary = self.visible_save_dictionary();
        let active_save = self.active_document.is_some_and(|document| {
            self.workspace.document_kind(document).ok() == Some(DocumentKind::CrusadersSave)
        });
        let root = self.game_paths.root(Game::Crusaders).map(ToOwned::to_owned);

        if !active_save {
            if self.save_catalog.dormant(root.as_deref()) {
                self.shell.invalidate_save_catalog();
            }
            self.reconcile_save_presentation_if_dictionary_changed(previous_dictionary, cx);
            cx.notify();
            return;
        }

        let Some(root) = root else {
            self.shell.invalidate_save_catalog();
            self.save_catalog.not_configured();
            self.reconcile_save_presentation_if_dictionary_changed(previous_dictionary, cx);
            cx.notify();
            return;
        };

        if self.save_catalog.activate(&root) {
            self.reconcile_save_presentation_if_dictionary_changed(previous_dictionary, cx);
            cx.notify();
            return;
        }

        self.shell.invalidate_save_catalog();
        let request = self.shell.begin_save_catalog();
        let key = SaveCatalogKey::new(request, root);
        self.save_catalog.begin(key.clone());
        #[cfg(test)]
        {
            self.task_launches.save_catalog += 1;
        }
        self.reconcile_save_presentation_if_dictionary_changed(previous_dictionary, cx);
        cx.notify();

        let work_key = key.clone();
        let task = cx.background_executor().spawn(async move {
            let installation = GameInstallation::inspect(Game::Crusaders, work_key.root())?;
            let catalog = load_name_dictionary(&installation.sox_directory())?;
            Ok::<CatalogLoad, CatalogRequestError>(catalog)
        });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_save_catalog_load(key, result, cx);
            });
        })
        .detach();
    }

    fn finish_save_catalog_load(
        &mut self,
        key: SaveCatalogKey,
        result: Result<CatalogLoad, CatalogRequestError>,
        cx: &mut Context<Self>,
    ) {
        if !self.shell.accepts_save_catalog(key.request())
            || self.game_paths.root(Game::Crusaders) != Some(key.root())
        {
            return;
        }

        let previous_dictionary = self.visible_save_dictionary();
        let accepted = match result {
            Ok(CatalogLoad { dictionary, issues }) => {
                self.save_catalog
                    .finish_ready(key, Arc::new(dictionary), issues.len())
            }
            Err(error) => self.save_catalog.finish_failed(key, error),
        };
        if accepted {
            self.reconcile_save_presentation_if_dictionary_changed(previous_dictionary, cx);
            cx.notify();
        }
    }

    fn visible_save_dictionary(&self) -> Option<Arc<NameDictionary>> {
        match self.save_catalog.status() {
            crate::save_catalog_status::SaveCatalogStatus::Ready { dictionary, .. } => {
                Some(Arc::clone(dictionary))
            }
            crate::save_catalog_status::SaveCatalogStatus::NotConfigured
            | crate::save_catalog_status::SaveCatalogStatus::Dormant
            | crate::save_catalog_status::SaveCatalogStatus::Loading { .. }
            | crate::save_catalog_status::SaveCatalogStatus::Failed { .. } => None,
        }
    }

    fn reconcile_save_presentation_if_dictionary_changed(
        &mut self,
        previous: Option<Arc<NameDictionary>>,
        cx: &mut Context<Self>,
    ) {
        let current = self.visible_save_dictionary();
        let unchanged = match (previous, current) {
            (Some(previous), Some(current)) => Arc::ptr_eq(&previous, &current),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        if unchanged {
            return;
        }
        let Some(document) = self.active_document.filter(|document| {
            self.workspace.document_kind(*document).ok() == Some(DocumentKind::CrusadersSave)
        }) else {
            return;
        };
        self.reconcile_save_presentation(document, cx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "controlled GPUI, save, and catalog fixtures make failures fatal"
    )]

    use std::{fs, mem::size_of, path::PathBuf, sync::Arc};

    use gpui::{AppContext, TestAppContext, WindowOptions};
    use kufeditor_game::{
        CatalogLoad, CatalogRole, Game, GameInstallation, InstallationError, load_name_dictionary,
    };
    use kufeditor_workspace::{
        Document, DocumentID, SaveDocument, SaveNumberTarget, SaveUnitField, TroopDocument,
        TroopField, load_path,
    };
    use tempfile::TempDir;

    use super::super::{ActiveNumberEdit, AppFrame};
    use crate::{
        catalog_status::{CatalogKey, CatalogRequestError, CatalogStatus},
        notices::{Notice, NoticeSource},
        number_edit::{NumberCommand, NumberOutcome},
        save_catalog_status::{SaveCatalogKey, SaveCatalogStatus},
        settings::SettingsStartup,
        state::SaveSection,
        test_support::SaveFixture,
        views::save::SaveRows,
    };

    const SAVE_CONTEXT_SIZE: usize = 0x438;
    const SAVE_MAIN_SIZE: usize = 0x154;
    const SAVE_PADDED_SIZE: usize = 0x8000;

    struct CatalogTree {
        _temporary: TempDir,
        root: PathBuf,
    }

    impl CatalogTree {
        fn with_troop_catalog() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().join("game");
            let sox = root.join("Data/SOX");
            fs::create_dir_all(sox.join("ENG")).unwrap();
            fs::write(
                sox.join(CatalogRole::TroopNames.relative_path()),
                indexed_table(&[(2, b"Footman")]),
            )
            .unwrap();
            Self {
                _temporary: temporary,
                root,
            }
        }

        fn empty() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path().join("game");
            fs::create_dir_all(root.join("Data/SOX")).unwrap();
            Self {
                _temporary: temporary,
                root,
            }
        }

        fn load(&self) -> CatalogLoad {
            let installation = GameInstallation::inspect(Game::Crusaders, &self.root).unwrap();
            load_name_dictionary(&installation.sox_directory()).unwrap()
        }
    }

    fn indexed_table(records: &[(u32, &[u8])]) -> Vec<u8> {
        let mut bytes = 100_u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&u32::try_from(records.len()).unwrap().to_le_bytes());
        for (id, value) in records {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(value);
        }
        bytes
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

    fn troop_document() -> TroopDocument {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(..8)
            .unwrap()
            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
        TroopDocument::parse(bytes).unwrap()
    }

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

    fn open_save(frame: &mut AppFrame, path: &str) -> DocumentID {
        frame.workspace.open_loaded(
            PathBuf::from(path),
            Document::Save(SaveDocument::parse(save_fixture()).unwrap()),
        )
    }

    fn open_sox(frame: &mut AppFrame, path: &str) -> DocumentID {
        frame
            .workspace
            .open_loaded(PathBuf::from(path), Document::Troop(troop_document()))
    }

    fn visible_key(frame: &AppFrame) -> SaveCatalogKey {
        match frame.save_catalog.status() {
            SaveCatalogStatus::Loading { key }
            | SaveCatalogStatus::Ready { key, .. }
            | SaveCatalogStatus::Failed { key, .. } => key.clone(),
            SaveCatalogStatus::NotConfigured | SaveCatalogStatus::Dormant => {
                panic!("save catalog does not have a visible key")
            }
        }
    }

    #[gpui::test]
    fn no_active_save_or_active_sox_keeps_the_save_catalog_dormant(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame.shell.select_game(Game::Heroes);
                frame.reconcile_save_catalog(cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));

                let sox = open_sox(frame, "TroopInfo.sox");
                frame.activate_document(sox, cx);

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));
                assert_eq!(frame.task_launches.save_catalog, 0);
            })
            .unwrap();
    }

    #[gpui::test]
    fn active_save_without_a_crusaders_root_is_not_configured(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let save = open_save(frame, "campaign.sav");
                frame.activate_document(save, cx);

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::NotConfigured
                ));
                assert_eq!(frame.task_launches.save_catalog, 0);
            })
            .unwrap();
    }

    #[gpui::test]
    fn active_save_starts_one_load_for_the_exact_crusaders_root(cx: &mut TestAppContext) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let save = open_save(frame, "campaign.sav");
                frame.activate_document(save, cx);

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key } if key.root() == root
                ));
                assert_eq!(frame.task_launches.save_catalog, 1);

                frame.reconcile_save_catalog(cx);
                assert_eq!(frame.task_launches.save_catalog, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn ready_dictionary_is_reused_by_multiple_save_tabs(cx: &mut TestAppContext) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let window = test_window(cx);
        let (first, second) = window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let first = open_save(frame, "first.sav");
                let second = open_save(frame, "second.sav");
                frame.activate_document(first, cx);
                (first, second)
            })
            .unwrap();
        cx.run_until_parked();

        let first_dictionary = window
            .update(cx, |frame, _, cx| {
                let dictionary = match frame.save_catalog.status() {
                    SaveCatalogStatus::Ready {
                        dictionary,
                        issue_count,
                        key,
                    } if key.root() == root && *issue_count > 0 => Arc::clone(dictionary),
                    _ => panic!("save catalog did not become ready"),
                };
                assert_eq!(dictionary.troop_name(2), Some("Footman"));
                assert_eq!(frame.task_launches.save_catalog, 1);

                frame.activate_document(second, cx);
                dictionary
            })
            .unwrap();

        window
            .update(cx, |frame, _, _| {
                let SaveCatalogStatus::Ready {
                    dictionary: second_dictionary,
                    ..
                } = frame.save_catalog.status()
                else {
                    panic!("ready dictionary was not reused");
                };
                assert!(Arc::ptr_eq(&first_dictionary, second_dictionary));
                assert_eq!(frame.task_launches.save_catalog, 1);
                assert_eq!(frame.active_document, Some(second));
                assert_ne!(frame.active_document, Some(first));
            })
            .unwrap();
    }

    #[gpui::test]
    fn ready_catalog_keeps_player_only_membership_and_the_active_unit_draft(
        cx: &mut TestAppContext,
    ) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let window = test_window(cx);
        let (document, target) = window
            .update(cx, |frame, app_window, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let document = frame.workspace.open_loaded(
                    PathBuf::from("campaign.sav"),
                    Document::Save(
                        SaveDocument::parse(
                            SaveFixture::new(2, 0, 0).with_unit_roles([0, 3]).build(),
                        )
                        .unwrap(),
                    ),
                );
                frame.activate_document(document, cx);
                let key = visible_key(frame);
                let rows = SaveRows::units(&frame.workspace, document, true).unwrap();
                let visibility = rows.unit_visibility().unwrap();
                frame
                    .save_presentations
                    .select_section(document, SaveSection::Units, false);
                frame
                    .save_presentations
                    .set_player_only(document, true, visibility, false);
                let target = SaveNumberTarget::Unit {
                    unit: 0,
                    field: SaveUnitField::TroopInfoIndex,
                };
                let value = frame.workspace.save_number(document, target).unwrap();
                let editor = frame
                    .workspace
                    .save_number_editor(document, target)
                    .unwrap();
                frame.begin_number_edit(
                    ActiveNumberEdit::save(document, target, value, editor).unwrap(),
                );
                assert_eq!(
                    frame
                        .number_edit
                        .as_mut()
                        .unwrap()
                        .editor
                        .apply(NumberCommand::Insert('9')),
                    NumberOutcome::Continue,
                );

                frame.finish_save_catalog_load(key, Ok(tree.load()), cx);
                app_window.focus(&frame.focus);
                (document, target)
            })
            .unwrap();

        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "enter");
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame
                        .save_presentations
                        .get(document)
                        .unwrap()
                        .inspected_unit(),
                    0,
                );
                assert!(frame.number_edit.is_none());
                assert_eq!(frame.workspace.save_number(document, target).unwrap(), 9);
                assert!(frame.workspace.is_dirty(document).unwrap());
                assert!(frame.workspace.can_undo(document).unwrap());
            })
            .unwrap();
    }

    #[gpui::test]
    fn retained_ready_catalog_keeps_raw_player_membership_on_save_activation(
        cx: &mut TestAppContext,
    ) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let save = frame.workspace.open_loaded(
                    PathBuf::from("campaign.sav"),
                    Document::Save(
                        SaveDocument::parse(
                            SaveFixture::new(2, 0, 0).with_unit_roles([0, 3]).build(),
                        )
                        .unwrap(),
                    ),
                );
                let sox = open_sox(frame, "TroopInfo.sox");
                frame.activate_document(save, cx);
                let key = visible_key(frame);
                let rows = SaveRows::units(&frame.workspace, save, true).unwrap();
                let visibility = rows.unit_visibility().unwrap();
                frame
                    .save_presentations
                    .set_player_only(save, true, visibility, false);

                frame.activate_document(sox, cx);
                frame.begin_number_edit(ActiveNumberEdit::troop_field(
                    sox,
                    0,
                    TroopField::MoveSpeed,
                    0,
                ));
                frame.finish_save_catalog_load(key, Ok(tree.load()), cx);
                assert!(frame.number_edit.is_some());

                frame.activate_document(save, cx);
                assert_eq!(
                    frame.save_presentations.get(save).unwrap().inspected_unit(),
                    0,
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn failed_same_root_does_not_restart(cx: &mut TestAppContext) {
        let tree = CatalogTree::empty();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let save = open_save(frame, "campaign.sav");
                frame.activate_document(save, cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, cx| {
                let failed = visible_key(frame);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Failed { key, .. } if key == &failed
                ));
                frame.reconcile_save_catalog(cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Failed { key, .. } if key == &failed
                ));
                assert_eq!(frame.task_launches.save_catalog, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn different_root_supersedes_old_success_and_failure(cx: &mut TestAppContext) {
        let dictionary = CatalogTree::with_troop_catalog();
        let old_root = PathBuf::from("/old/crusaders");
        let new_root = PathBuf::from("/new/crusaders");
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(old_root.clone()));
                let save = open_save(frame, "campaign.sav");
                frame.activate_document(save, cx);
                let old_key = visible_key(frame);

                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(new_root.clone()));
                frame.reconcile_save_catalog(cx);
                let new_key = visible_key(frame);

                assert_ne!(old_key.request(), new_key.request());
                assert_eq!(new_key.root(), new_root);
                assert_eq!(frame.task_launches.save_catalog, 2);

                frame.finish_save_catalog_load(old_key.clone(), Ok(dictionary.load()), cx);
                frame.finish_save_catalog_load(
                    old_key,
                    Err(CatalogRequestError::Installation(
                        InstallationError::RootMissing {
                            game: Game::Crusaders,
                            root: old_root,
                        },
                    )),
                    cx,
                );

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key } if key == &new_key
                ));
                assert!(frame.shell.accepts_save_catalog(new_key.request()));
            })
            .unwrap();
    }

    #[gpui::test]
    fn same_root_completion_is_accepted_while_a_sox_tab_is_active(cx: &mut TestAppContext) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let save = open_save(frame, "campaign.sav");
                let sox = open_sox(frame, "TroopInfo.sox");
                frame.activate_document(save, cx);
                let key = visible_key(frame);

                frame.activate_document(sox, cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));

                frame.finish_save_catalog_load(key, Ok(tree.load()), cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));

                frame.activate_document(save, cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Ready { dictionary, key, .. }
                        if key.root() == root && dictionary.troop_name(2) == Some("Footman")
                ));
                assert_eq!(frame.task_launches.save_catalog, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn successful_open_and_cross_document_selection_reconcile_the_save_catalog(
        cx: &mut TestAppContext,
    ) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let directory = tempfile::tempdir().unwrap();
        let save_path = directory.path().join("campaign.sav");
        fs::write(&save_path, save_fixture()).unwrap();
        let loaded = load_path(save_path.clone()).unwrap();
        let window = test_window(cx);

        let opened = window
            .update(cx, |frame, _, cx| {
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let request = frame.shell.begin_open();
                frame.finish_open_paths(request, vec![(save_path.clone(), Ok(loaded))], cx);

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { key } if key.root() == root
                ));
                assert_eq!(frame.task_launches.save_catalog, 1);
                frame.active_document.unwrap()
            })
            .unwrap();

        window
            .update(cx, |frame, _, cx| {
                let sox = open_sox(frame, "TroopInfo.sox");
                frame.activate_document(sox, cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Dormant
                ));

                frame.select_record(opened, 0, cx);
                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Loading { .. }
                ));
                assert_eq!(frame.task_launches.save_catalog, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn save_completion_preserves_global_catalog_settings_and_notices(cx: &mut TestAppContext) {
        let tree = CatalogTree::with_troop_catalog();
        let root = tree.root.clone();
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                frame.shell.select_game(Game::Heroes);
                let global_request = frame.shell.begin_catalog();
                let global_key = CatalogKey::new(global_request, Game::Heroes, "/heroes");
                frame.catalog.begin(global_key.clone());
                frame.notices.begin(
                    NoticeSource::Catalog,
                    global_request.get(),
                    Notice::info("Loading Heroes catalogs"),
                );
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let paths = frame.game_paths.clone();
                let recent_files = frame.recent_files.clone();
                let revision = frame.settings.latest_revision_for_test();
                let save = open_save(frame, "campaign.sav");
                frame.activate_document(save, cx);

                assert_eq!(frame.shell.game(), Game::Heroes);
                assert_eq!(frame.game_paths, paths);
                assert_eq!(frame.recent_files, recent_files);
                assert_eq!(frame.settings.latest_revision_for_test(), revision);
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Loading { key } if key == &global_key
                ));
                assert!(frame.shell.accepts_catalog(global_request));
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Loading Heroes catalogs")
                );

                let save_key = visible_key(frame);

                frame.finish_save_catalog_load(save_key, Ok(tree.load()), cx);

                assert_eq!(frame.shell.game(), Game::Heroes);
                assert_eq!(frame.game_paths, paths);
                assert_eq!(frame.recent_files, recent_files);
                assert_eq!(frame.settings.latest_revision_for_test(), revision);
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::Loading { key } if key == &global_key
                ));
                assert!(frame.shell.accepts_catalog(global_request));
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Loading Heroes catalogs")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn save_failure_stays_out_of_the_global_catalog_notice_stream(cx: &mut TestAppContext) {
        let window = test_window(cx);

        window
            .update(cx, |frame, _, cx| {
                let root = PathBuf::from("/missing/crusaders");
                frame
                    .game_paths
                    .set_root(Game::Crusaders, Some(root.clone()));
                let save = open_save(frame, "campaign.sav");
                frame.activate_document(save, cx);
                let key = visible_key(frame);
                assert!(frame.notices.current().is_none());

                frame.finish_save_catalog_load(
                    key.clone(),
                    Err(CatalogRequestError::Installation(
                        InstallationError::RootMissing {
                            game: Game::Crusaders,
                            root,
                        },
                    )),
                    cx,
                );

                assert!(matches!(
                    frame.save_catalog.status(),
                    SaveCatalogStatus::Failed {
                        key: current,
                        error: CatalogRequestError::Installation(
                            InstallationError::RootMissing { game, root: failed_root }
                        ),
                    } if current == &key
                        && *game == Game::Crusaders
                        && failed_root == key.root()
                ));
                assert!(frame.notices.current().is_none());
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::NotConfigured
                ));
            })
            .unwrap();
    }
}
