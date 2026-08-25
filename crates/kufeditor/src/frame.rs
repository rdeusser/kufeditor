use gpui::prelude::*;
use gpui::{Action, App, Context, Div, FocusHandle, Focusable, Stateful, Window, div, px};
use kufeditor_game::Game;
use kufeditor_workspace::{DocumentId, Workspace};

use crate::{
    actions::{OpenFile, Redo, Save, SaveAll, SaveAs, Undo},
    components,
    state::{Area, ShellState},
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
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
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

    fn content(&self) -> Div {
        match self.shell.area() {
            Area::Home => views::home::render(&self.theme, self.shell.game()),
            Area::Files => views::files::render(
                &self.theme,
                &self.workspace,
                self.active_document,
                self.selected_troop,
            ),
            Area::Mods => views::mods::render(&self.theme),
            Area::Patches => views::patches::render(&self.theme),
        }
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
            .child(self.top_bar(cx))
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
                            .child(self.content()),
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
