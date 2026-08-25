use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{
    Action, AnyElement, AnyWindowHandle, App, AsyncApp, Context, Div, FocusHandle, Focusable,
    KeyDownEvent, PathPromptOptions, PromptLevel, Stateful, WeakEntity, Window, div, px,
};
use kufeditor_game::Game;
use kufeditor_workspace::{
    DocumentEdit, DocumentId, SaveToken, TroopField, TroopGroup, Workspace, load_path,
};

use crate::{
    actions::{OpenFile, Redo, Save, SaveAll, SaveAs, Undo},
    components,
    number_edit::{NumberCommand, NumberEdit, NumberOutcome},
    state::{Area, ClosePolicy, Notice, NoticeLevel, RequestId, ShellState},
    theme::Theme,
    views,
};

struct ActiveNumberEdit {
    target: NumberEditTarget,
    editor: NumberEdit,
}

impl ActiveNumberEdit {
    fn troop_field(document: DocumentId, record: usize, field: TroopField, value: i32) -> Self {
        Self {
            target: NumberEditTarget::TroopField {
                document,
                record,
                field,
            },
            editor: NumberEdit::new(i64::from(value), i64::from(i32::MIN), i64::from(i32::MAX)),
        }
    }
}

#[derive(Clone, Copy)]
enum NumberEditTarget {
    TroopField {
        document: DocumentId,
        record: usize,
        field: TroopField,
    },
}

impl NumberEditTarget {
    fn document(&self) -> DocumentId {
        match self {
            Self::TroopField { document, .. } => *document,
        }
    }

    fn is_troop_field(&self, document: DocumentId, record: usize, field: TroopField) -> bool {
        matches!(
            self,
            Self::TroopField {
                document: target_document,
                record: target_record,
                field: target_field,
            } if *target_document == document && *target_record == record && *target_field == field
        )
    }

    fn document_edit(
        &self,
        value: i64,
    ) -> Result<(DocumentId, DocumentEdit), std::num::TryFromIntError> {
        match *self {
            Self::TroopField {
                document,
                record,
                field,
            } => Ok((
                document,
                DocumentEdit::SetTroopField {
                    record,
                    field,
                    value: i32::try_from(value)?,
                },
            )),
        }
    }
}

fn invalid_number_notice() -> Notice {
    Notice::info("Enter a whole number within the allowed range")
}

pub struct AppFrame {
    workspace: Workspace,
    pub(crate) shell: ShellState,
    theme: Theme,
    focus: FocusHandle,
    active_document: Option<DocumentId>,
    selected_troop: usize,
    number_edit: Option<ActiveNumberEdit>,
    notice: Option<Notice>,
    window_handle: Option<AnyWindowHandle>,
    close_armed: bool,
    close_after_saves: bool,
    close_prompt_open: bool,
}

impl AppFrame {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            workspace: Workspace::new(),
            shell: ShellState::default(),
            theme: Theme::forged_steel(),
            focus: cx.focus_handle(),
            active_document: None,
            selected_troop: 0,
            number_edit: None,
            notice: None,
            window_handle: None,
            close_armed: false,
            close_after_saves: false,
            close_prompt_open: false,
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn activate_document(&mut self, document: DocumentId, selected_troop: usize) {
        self.active_document = Some(document);
        self.selected_troop = selected_troop;
        self.number_edit = None;
    }

    pub fn window_should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.window_handle = Some(window.window_handle());
        if std::mem::take(&mut self.close_armed) {
            return true;
        }
        if self.close_prompt_open || self.close_after_saves {
            return false;
        }

        match ClosePolicy::from_dirty_count(self.dirty_count()) {
            ClosePolicy::Allow => true,
            ClosePolicy::PromptForUnsaved { count } => {
                self.close_prompt_open = true;
                let message = format!(
                    "{count} unsaved {}. Save before closing?",
                    if count == 1 { "document" } else { "documents" }
                );
                let answer = window.prompt(
                    PromptLevel::Warning,
                    &message,
                    None,
                    &["Save All", "Discard Changes", "Cancel"],
                    cx,
                );
                cx.spawn_in(window, async move |entity, cx| {
                    let answer = answer.await.ok();
                    let _ = entity.update_in(cx, move |frame, window, cx| {
                        frame.finish_close_prompt(answer, window, cx);
                    });
                })
                .detach();
                false
            }
        }
    }

    fn dirty_count(&self) -> usize {
        self.workspace
            .document_ids()
            .iter()
            .filter(|document_id| self.workspace.is_dirty(**document_id).unwrap_or(false))
            .count()
    }

    fn finish_close_prompt(
        &mut self,
        answer: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_prompt_open = false;
        match answer {
            Some(0) => {
                self.close_after_saves = true;
                self.continue_close_after_saves(cx);
            }
            Some(1) => {
                self.close_armed = true;
                window.remove_window();
            }
            Some(_) | None => {}
        }
        cx.notify();
    }

    fn continue_close_after_saves(&mut self, cx: &mut Context<Self>) {
        if !self.close_after_saves {
            return;
        }

        let document_ids = self.workspace.document_ids().to_vec();
        let mut dirty_or_saving = false;
        for document_id in document_ids {
            let dirty = self.workspace.is_dirty(document_id).unwrap_or(false);
            let saving = self
                .workspace
                .save_in_progress(document_id)
                .unwrap_or(false);
            dirty_or_saving |= dirty || saving;
            if dirty && !saving && !self.start_save(document_id, None, cx) {
                return;
            }
        }

        if dirty_or_saving {
            return;
        }
        self.close_after_saves = false;
        self.close_armed = true;
        let Some(window_handle) = self.window_handle else {
            return;
        };
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, _| window.remove_window());
        });
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.number_edit.is_none() {
            return;
        }
        let Some(command) = number_command(event) else {
            return;
        };
        cx.stop_propagation();

        let outcome = self
            .number_edit
            .as_mut()
            .map_or(NumberOutcome::Cancel, |edit| edit.editor.apply(command));
        match outcome {
            NumberOutcome::Continue => {}
            NumberOutcome::Invalid => self.notice = Some(invalid_number_notice()),
            NumberOutcome::Cancel => self.number_edit = None,
            NumberOutcome::Commit(value) => self.commit_number_edit(value),
        }
        cx.notify();
    }

    fn commit_number_edit(&mut self, value: i64) {
        let Some(target_document) = self.number_edit.as_ref().map(|edit| edit.target.document())
        else {
            return;
        };
        if self.active_document != Some(target_document) {
            self.number_edit = None;
            self.notice = Some(Notice::info("The active document changed; edit canceled"));
            return;
        }

        let Some(edit) = self.number_edit.as_ref() else {
            return;
        };
        let (document, document_edit) = match edit.target.document_edit(value) {
            Ok(edit) => edit,
            Err(error) => {
                self.notice = Some(Notice::error("Could not update TroopInfo", &error));
                return;
            }
        };
        match self.workspace.apply(document, document_edit) {
            Ok(()) => {
                self.number_edit = None;
                self.notice = None;
            }
            Err(error) => {
                self.notice = Some(Notice::error("Could not update TroopInfo", &error));
            }
        }
    }

    fn open_action(&mut self, _: &OpenFile, _: &mut Window, cx: &mut Context<Self>) {
        self.number_edit = None;
        let request = self.shell.begin_open();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open".into()),
        });
        self.notice = Some(Notice::info("Choose one or more .sox files"));
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let paths = match prompt.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => {
                    set_open_notice(&entity, cx, request, None);
                    return;
                }
                Ok(Err(error)) => {
                    let notice = Notice::error("Could not open the file picker", error.as_ref());
                    set_open_notice(&entity, cx, request, Some(notice));
                    return;
                }
                Err(error) => {
                    let notice = Notice::error("The file picker did not respond", &error);
                    set_open_notice(&entity, cx, request, Some(notice));
                    return;
                }
            };

            let tasks = paths
                .into_iter()
                .map(|path| {
                    cx.background_executor().spawn(async move {
                        let loaded = load_path(path.clone());
                        (path, loaded)
                    })
                })
                .collect::<Vec<_>>();
            let mut opened_any = false;

            for task in tasks {
                let (path, result) = task.await;
                let activate = !opened_any;
                let opened = entity
                    .update(cx, move |frame, cx| {
                        if !frame.shell.accepts_open(request) {
                            return false;
                        }
                        match result {
                            Ok(loaded) => {
                                let name = display_name(&path);
                                let document_id = frame.workspace.insert_loaded(loaded);
                                if activate {
                                    frame.activate_document(document_id, 0);
                                    frame.shell.select_area(Area::Files);
                                }
                                frame.notice = Some(Notice::success(format!("Opened {name}")));
                                cx.notify();
                                true
                            }
                            Err(error) => {
                                frame.notice = Some(Notice::error(
                                    format!("Could not open {}", display_name(&path)),
                                    &error,
                                ));
                                cx.notify();
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                opened_any |= opened;
            }
        })
        .detach();
    }

    fn save_action(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        self.number_edit = None;
        if let Some(document_id) = self.active_document {
            self.start_save(document_id, None, cx);
        }
    }

    fn save_all_action(&mut self, _: &SaveAll, _: &mut Window, cx: &mut Context<Self>) {
        self.number_edit = None;
        let document_ids = self.workspace.document_ids().to_vec();
        let mut started = false;
        for document_id in document_ids {
            let dirty = self.workspace.is_dirty(document_id).unwrap_or(false);
            let busy = self
                .workspace
                .save_in_progress(document_id)
                .unwrap_or(false);
            if dirty && !busy {
                self.start_save(document_id, None, cx);
                started = true;
            }
        }
        if !started {
            self.notice = Some(Notice::info("All documents are already saved"));
            cx.notify();
        }
    }

    fn save_as_action(&mut self, _: &SaveAs, _: &mut Window, cx: &mut Context<Self>) {
        self.number_edit = None;
        let Some(document_id) = self.active_document else {
            return;
        };
        let current_path = match self.workspace.path(document_id) {
            Ok(path) => path.to_path_buf(),
            Err(error) => {
                self.notice = Some(Notice::error("Could not determine the save path", &error));
                cx.notify();
                return;
            }
        };
        let parent = current_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let suggested = current_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned);
        let prompt = cx.prompt_for_new_path(&parent, suggested.as_deref());

        cx.spawn(async move |entity, cx| match prompt.await {
            Ok(Ok(Some(path))) => {
                let _ = entity.update(cx, move |frame, cx| {
                    frame.start_save(document_id, Some(path), cx);
                });
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                let _ = entity.update(cx, move |frame, cx| {
                    frame.notice = Some(Notice::error("Could not open Save As", error.as_ref()));
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = entity.update(cx, move |frame, cx| {
                    frame.notice = Some(Notice::error("Save As did not respond", &error));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn undo_action(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.move_history(false, cx);
    }

    fn redo_action(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.move_history(true, cx);
    }

    fn move_history(&mut self, redo: bool, cx: &mut Context<Self>) {
        self.number_edit = None;
        let Some(document_id) = self.active_document else {
            return;
        };
        let result = if redo {
            self.workspace.redo(document_id)
        } else {
            self.workspace.undo(document_id)
        };
        match result {
            Ok(true) => self.notice = None,
            Ok(false) => {}
            Err(error) => {
                self.notice = Some(Notice::error("Could not change document history", &error));
            }
        }
        cx.notify();
    }

    fn start_save(
        &mut self,
        document_id: DocumentId,
        target: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> bool {
        let request = match self.workspace.prepare_save(document_id, target) {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some(Notice::error("Could not start save", &error));
                self.close_after_saves = false;
                self.close_armed = false;
                cx.notify();
                return false;
            }
        };
        let request_document = request.document_id();
        let token = request.token();
        let task = cx.background_executor().spawn(async move { request.run() });
        self.notice = Some(Notice::info("Saving document"));
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_save_result(request_document, token, result);
                frame.continue_close_after_saves(cx);
                cx.notify();
            });
        })
        .detach();
        true
    }

    fn finish_save_result(
        &mut self,
        document_id: DocumentId,
        token: SaveToken,
        result: Result<kufeditor_workspace::SavedDocument, kufeditor_workspace::WorkspaceError>,
    ) {
        match result {
            Ok(saved) => match self.workspace.finish_save(saved) {
                Ok(()) => {
                    let name = self
                        .workspace
                        .path(document_id)
                        .map_or_else(|_| "document".to_owned(), display_name);
                    self.notice = Some(Notice::success(format!("Saved {name}")));
                }
                Err(error) => {
                    self.close_after_saves = false;
                    self.close_armed = false;
                    self.notice = Some(Notice::error("Could not finish save", &error));
                }
            },
            Err(error) => match self.workspace.finish_save_failure(document_id, token) {
                Ok(()) => {
                    self.close_after_saves = false;
                    self.close_armed = false;
                    self.notice = Some(Notice::error("Could not save document", &error));
                }
                Err(cleanup_error) => {
                    self.close_after_saves = false;
                    self.close_armed = false;
                    self.notice = Some(Notice::error(
                        "Could not reconcile failed save",
                        &cleanup_error,
                    ));
                }
            },
        }
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .h(px(54.0))
            .px(px(18.0))
            .gap(px(8.0))
            .bg(self.theme.surface)
            .border_b_1()
            .border_color(self.theme.border)
            .child(
                div()
                    .flex_none()
                    .w(px(172.0))
                    .text_size(px(18.0))
                    .text_color(self.theme.accent)
                    .child("KufEditor"),
            )
            .child(self.file_actions())
            .child(div().flex_1())
            .child(self.game_picker(cx))
    }

    fn file_actions(&self) -> Div {
        let has_document = self.active_document.is_some();
        let can_undo = self
            .active_document
            .is_some_and(|id| self.workspace.can_undo(id).unwrap_or(false));
        let can_redo = self
            .active_document
            .is_some_and(|id| self.workspace.can_redo(id).unwrap_or(false));

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(action_button(
                &self.theme,
                "toolbar-open",
                "Open",
                true,
                OpenFile,
            ))
            .child(action_button(
                &self.theme,
                "toolbar-save",
                "Save",
                has_document,
                Save,
            ))
            .child(action_button(
                &self.theme,
                "toolbar-save-as",
                "Save as",
                has_document,
                SaveAs,
            ))
            .child(action_button(
                &self.theme,
                "toolbar-save-all",
                "Save all",
                has_document,
                SaveAll,
            ))
            .child(action_button(
                &self.theme,
                "toolbar-undo",
                "Undo",
                can_undo,
                Undo,
            ))
            .child(action_button(
                &self.theme,
                "toolbar-redo",
                "Redo",
                can_redo,
                Redo,
            ))
    }

    fn game_picker(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .children(Game::ALL.into_iter().map(|game| {
                let id = match game {
                    Game::Crusaders => "game-crusaders",
                    Game::Heroes => "game-heroes",
                };
                let selected = self.shell.game() == game;
                components::toolbar_button(&self.theme, id, game.label(), true)
                    .when(selected, |button| {
                        button
                            .bg(self.theme.accent_dim)
                            .border_color(self.theme.accent)
                            .text_color(self.theme.accent)
                    })
                    .on_click(cx.listener(move |frame, _, _, cx| {
                        frame.shell.select_game(game);
                        frame.number_edit = None;
                        cx.notify();
                    }))
            }))
    }

    fn navigation(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(196.0))
            .p(px(12.0))
            .gap(px(8.0))
            .bg(self.theme.surface)
            .border_r_1()
            .border_color(self.theme.border)
            .children(Area::ALL.into_iter().map(|area| {
                components::rail_item(
                    &self.theme,
                    area.element_id(),
                    area.label(),
                    self.shell.area() == area,
                )
                .on_click(cx.listener(move |frame, _, _, cx| {
                    frame.shell.select_area(area);
                    frame.number_edit = None;
                    cx.notify();
                }))
            }))
            .child(div().flex_1())
            .child(components::disabled_rail_item(
                &self.theme,
                "rail-settings",
                "Settings · later",
            ))
    }

    fn content(&self, cx: &mut Context<Self>) -> Div {
        match self.shell.area() {
            Area::Home => views::home::render(&self.theme, self.shell.game()),
            Area::Files => {
                let editor = self
                    .active_document
                    .map(|document_id| self.troop_editor(document_id, cx));
                views::files::render(&self.theme, self.document_tabs(cx), editor)
            }
            Area::Mods => views::mods::render(&self.theme),
            Area::Patches => views::patches::render(&self.theme),
        }
    }

    fn troop_editor(&self, document_id: DocumentId, cx: &mut Context<Self>) -> Div {
        let record_count = match self.workspace.record_count(document_id) {
            Ok(count) => count,
            Err(error) => {
                return div()
                    .size_full()
                    .p(px(28.0))
                    .text_color(self.theme.text_dim)
                    .child(format!("Could not read TroopInfo: {error}"));
            }
        };
        let selected = self.selected_troop.min(record_count.saturating_sub(1));
        let records = self.troop_records(document_id, record_count, selected, cx);
        let groups = if record_count == 0 {
            vec![
                div()
                    .text_color(self.theme.text_dim)
                    .child("This file has no troop records.")
                    .into_any_element(),
            ]
        } else {
            self.troop_groups(document_id, selected, cx)
        };
        let diagnostics = self.troop_diagnostics(document_id);
        views::troop::render(&self.theme, records, groups, diagnostics)
    }

    fn troop_records(
        &self,
        document_id: DocumentId,
        record_count: usize,
        selected: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        (0..record_count)
            .map(|record| {
                views::troop::record_row(
                    &self.theme,
                    ("troop-record", record),
                    record,
                    record == selected,
                )
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.activate_document(document_id, record);
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect()
    }

    fn troop_groups(
        &self,
        document_id: DocumentId,
        record: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        TroopGroup::ALL
            .into_iter()
            .map(|group| {
                let fields = TroopField::ALL
                    .into_iter()
                    .enumerate()
                    .filter(|(_, field)| field.group() == group)
                    .map(|(index, field)| self.troop_field(document_id, record, field, index, cx))
                    .collect();
                let help = (group == TroopGroup::Resistances)
                    .then_some("Damage %: 0 immune, 100 normal, 200 vulnerable");
                let derived = (group == TroopGroup::Formation).then(|| {
                    let width = self
                        .workspace
                        .troop_value(document_id, record, TroopField::DefaultUnitNumX)
                        .unwrap_or(0);
                    let depth = self
                        .workspace
                        .troop_value(document_id, record, TroopField::DefaultUnitNumY)
                        .unwrap_or(0);
                    ("Units Total", width.saturating_mul(depth))
                });
                views::troop::group(&self.theme, group.label(), fields, help, derived)
                    .into_any_element()
            })
            .collect()
    }

    fn troop_field(
        &self,
        document_id: DocumentId,
        record: usize,
        field: TroopField,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = self.workspace.troop_value(document_id, record, field);
        let active_edit = self
            .number_edit
            .as_ref()
            .filter(|edit| edit.target.is_troop_field(document_id, record, field));
        let display = active_edit.map_or_else(
            || {
                value
                    .as_ref()
                    .map_or_else(|_| "—".to_owned(), i32::to_string)
            },
            |edit| edit.editor.draft().to_owned(),
        );
        let row = views::troop::field_row(
            &self.theme,
            ("troop-field", index),
            field.label(),
            display,
            active_edit.is_some(),
            active_edit.is_some_and(|edit| edit.editor.invalid()),
        );

        match value {
            Ok(value) => row
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.number_edit = Some(ActiveNumberEdit::troop_field(
                        document_id,
                        record,
                        field,
                        value,
                    ));
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element(),
            Err(_) => row.into_any_element(),
        }
    }

    fn troop_diagnostics(&self, document_id: DocumentId) -> Vec<AnyElement> {
        let diagnostics = self.workspace.diagnostics(document_id).unwrap_or_default();
        if diagnostics.is_empty() {
            return vec![views::troop::no_diagnostics(&self.theme).into_any_element()];
        }

        diagnostics
            .into_iter()
            .map(|diagnostic| {
                views::troop::diagnostic_row(
                    &self.theme,
                    diagnostic.severity,
                    format!(
                        "{} · {}",
                        views::troop::troop_name(diagnostic.record),
                        diagnostic.field.label()
                    ),
                    diagnostic.message,
                )
                .into_any_element()
            })
            .collect()
    }

    fn document_tabs(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.workspace
            .document_ids()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, document_id)| {
                let title = self
                    .workspace
                    .title(document_id)
                    .unwrap_or_else(|error| format!("Unavailable: {error}"));
                let dirty = self.workspace.is_dirty(document_id).unwrap_or(false);
                components::document_tab(
                    &self.theme,
                    ("document-tab", index),
                    title,
                    self.active_document == Some(document_id),
                    dirty,
                )
                .on_click(cx.listener(move |frame, _, _, cx| {
                    frame.activate_document(document_id, 0);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect()
    }

    fn notice_bar(&self) -> Option<AnyElement> {
        self.notice.as_ref().map(|notice| {
            let label = match notice.level() {
                NoticeLevel::Info => "INFO",
                NoticeLevel::Success => "SAVED",
                NoticeLevel::Error => "ERROR",
            };
            div()
                .id("workspace-notice")
                .flex()
                .items_center()
                .gap(px(10.0))
                .px(px(18.0))
                .py(px(8.0))
                .bg(self.theme.accent_dim)
                .border_b_1()
                .border_color(self.theme.accent)
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(self.theme.accent)
                        .child(label),
                )
                .child(
                    div()
                        .flex_none()
                        .text_color(self.theme.text)
                        .child(notice.summary().to_owned()),
                )
                .children((!notice.detail().is_empty()).then(|| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(self.theme.text_dim)
                        .child(notice.detail().to_owned())
                }))
                .into_any_element()
        })
    }
}

fn action_button<A: Action>(
    theme: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
    action: A,
) -> Stateful<Div> {
    components::toolbar_button(theme, id, label, enabled).when(enabled, |button| {
        button.on_click(move |_, window: &mut Window, cx| {
            window.dispatch_action(action.boxed_clone(), cx);
        })
    })
}

fn set_open_notice(
    entity: &WeakEntity<AppFrame>,
    cx: &mut AsyncApp,
    request: RequestId,
    notice: Option<Notice>,
) {
    let _ = entity.update(cx, move |frame, cx| {
        if frame.shell.accepts_open(request) {
            frame.notice = notice;
            cx.notify();
        }
    });
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn number_command(event: &KeyDownEvent) -> Option<NumberCommand> {
    match event.keystroke.key.as_str() {
        "enter" => Some(NumberCommand::Commit),
        "escape" => Some(NumberCommand::Cancel),
        "backspace" => Some(NumberCommand::Backspace),
        "up" => Some(NumberCommand::Increment),
        "down" => Some(NumberCommand::Decrement),
        _ => {
            let mut characters = event.keystroke.key_char.as_deref()?.chars();
            let character = characters.next()?;
            characters
                .next()
                .is_none()
                .then_some(NumberCommand::Insert(character))
        }
    }
}

impl Focusable for AppFrame {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for AppFrame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.window_handle = Some(window.window_handle());
        div()
            .id("kufeditor-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(self.theme.background)
            .text_color(self.theme.text)
            .font_family("Inter")
            .text_size(px(14.0))
            .track_focus(&self.focus)
            .key_context("KufEditor")
            .on_key_down(cx.listener(Self::key_down))
            .on_action(cx.listener(Self::open_action))
            .on_action(cx.listener(Self::save_action))
            .on_action(cx.listener(Self::save_all_action))
            .on_action(cx.listener(Self::save_as_action))
            .on_action(cx.listener(Self::undo_action))
            .on_action(cx.listener(Self::redo_action))
            .child(self.top_bar(cx))
            .children(self.notice_bar())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.navigation(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(self.content(cx)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "the GPUI test creates one controlled in-memory window"
    )]

    use std::path::PathBuf;

    use gpui::{AppContext, TestAppContext, WindowOptions};
    use kufeditor_workspace::{Document, DocumentId, TroopDocument, TroopField};

    use super::{ActiveNumberEdit, AppFrame, invalid_number_notice};
    use crate::state::{Area, NoticeLevel};

    #[test]
    fn invalid_number_notice_explains_the_allowed_range() {
        let notice = invalid_number_notice();

        assert_eq!(notice.level(), NoticeLevel::Info);
        assert_eq!(
            notice.summary(),
            "Enter a whole number within the allowed range"
        );
    }

    fn open_troop(frame: &mut AppFrame, path: &str, move_speed: i32) -> DocumentId {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(0..4)
            .unwrap()
            .copy_from_slice(&100_u32.to_le_bytes());
        bytes
            .get_mut(4..8)
            .unwrap()
            .copy_from_slice(&1_u32.to_le_bytes());
        let mut document = TroopDocument::parse(bytes).unwrap();
        document
            .set_value(0, TroopField::MoveSpeed, move_speed)
            .unwrap();
        frame
            .workspace
            .open_loaded(PathBuf::from(path), Document::Troop(document))
    }

    #[gpui::test]
    fn inactive_number_edit_cannot_survive_activation_or_commit(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(AppFrame::new))
                .unwrap()
        });

        window
            .update(cx, |frame, _, _| {
                let first = open_troop(frame, "first.sox", 100);
                let second = open_troop(frame, "second.sox", 200);
                frame.activate_document(first, 0);
                frame.number_edit = Some(ActiveNumberEdit::troop_field(
                    first,
                    0,
                    TroopField::MoveSpeed,
                    100,
                ));

                frame.activate_document(second, 0);

                assert_eq!(frame.active_document, Some(second));
                assert!(frame.number_edit.is_none());

                frame.number_edit = Some(ActiveNumberEdit::troop_field(
                    first,
                    0,
                    TroopField::MoveSpeed,
                    100,
                ));
                frame.commit_number_edit(101);

                assert!(frame.number_edit.is_none());
                assert_eq!(
                    frame
                        .workspace
                        .troop_value(first, 0, TroopField::MoveSpeed)
                        .unwrap(),
                    100
                );
                assert!(!frame.workspace.is_dirty(first).unwrap());
            })
            .unwrap();
    }

    #[gpui::test]
    fn app_frame_opens_at_home(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(AppFrame::new))
                .unwrap()
        });

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.shell.area(), Area::Home);
            })
            .unwrap();
    }

    #[gpui::test]
    fn troop_editor_builds_for_a_loaded_document(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| cx.new(AppFrame::new))
                .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                let mut bytes = vec![0_u8; 8 + 148 + 64];
                bytes
                    .get_mut(0..4)
                    .unwrap()
                    .copy_from_slice(&100_u32.to_le_bytes());
                bytes
                    .get_mut(4..8)
                    .unwrap()
                    .copy_from_slice(&1_u32.to_le_bytes());
                bytes
                    .get_mut(108..112)
                    .unwrap()
                    .copy_from_slice(&800_i32.to_le_bytes());
                let document = TroopDocument::parse(bytes).unwrap();
                let document_id = frame
                    .workspace
                    .open_loaded(PathBuf::from("TroopInfo.sox"), Document::Troop(document));

                frame.active_document = Some(document_id);
                frame.shell.select_area(Area::Files);
                let _editor = frame.troop_editor(document_id, cx);

                assert_eq!(frame.workspace.record_count(document_id).unwrap(), 1);
            })
            .unwrap();
    }
}
