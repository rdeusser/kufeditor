use std::path::{Path, PathBuf};

use gpui::prelude::*;
use gpui::{
    Action, AnyElement, App, AsyncApp, Context, Div, FocusHandle, Focusable, PathPromptOptions,
    Stateful, WeakEntity, Window, div, px,
};
use kufeditor_game::Game;
use kufeditor_workspace::{DocumentId, SaveToken, Workspace, load_path};

use crate::{
    actions::{OpenFile, Redo, Save, SaveAll, SaveAs, Undo},
    components,
    state::{Area, Notice, NoticeLevel, RequestId, ShellState},
    theme::Theme,
    views,
};

pub struct AppFrame {
    workspace: Workspace,
    pub(crate) shell: ShellState,
    theme: Theme,
    focus: FocusHandle,
    active_document: Option<DocumentId>,
    selected_troop: usize,
    notice: Option<Notice>,
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
            notice: None,
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn open_action(&mut self, _: &OpenFile, _: &mut Window, cx: &mut Context<Self>) {
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
                    let notice =
                        Notice::error("Could not open the file picker", error.as_ref());
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
                                    frame.active_document = Some(document_id);
                                    frame.selected_troop = 0;
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
        if let Some(document_id) = self.active_document {
            self.start_save(document_id, None, cx);
        }
    }

    fn save_all_action(&mut self, _: &SaveAll, _: &mut Window, cx: &mut Context<Self>) {
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
    ) {
        let request = match self.workspace.prepare_save(document_id, target) {
            Ok(request) => request,
            Err(error) => {
                self.notice = Some(Notice::error("Could not start save", &error));
                cx.notify();
                return;
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
                cx.notify();
            });
        })
        .detach();
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
                    self.notice = Some(Notice::error("Could not finish save", &error));
                }
            },
            Err(error) => match self.workspace.finish_save_failure(document_id, token) {
                Ok(()) => {
                    self.notice = Some(Notice::error("Could not save document", &error));
                }
                Err(cleanup_error) => {
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
            Area::Files => views::files::render(
                &self.theme,
                &self.workspace,
                self.active_document,
                self.selected_troop,
                self.document_tabs(cx),
            ),
            Area::Mods => views::mods::render(&self.theme),
            Area::Patches => views::patches::render(&self.theme),
        }
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
                    frame.active_document = Some(document_id);
                    frame.selected_troop = 0;
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

impl Focusable for AppFrame {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for AppFrame {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

    use gpui::{AppContext, TestAppContext, WindowOptions};

    use super::AppFrame;
    use crate::state::Area;

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
}
