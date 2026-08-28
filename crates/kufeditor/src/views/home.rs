use std::path::PathBuf;

use gpui::{AnyElement, Div, Window, div, prelude::*, px};
use kufeditor_game::Game;

use crate::{actions::OpenFile, components, theme::Theme};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RecentFileProjection {
    pub(crate) path: PathBuf,
    pub(crate) label: String,
    pub(crate) secondary: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RecentFilesProjection {
    Empty(&'static str),
    Rows(Vec<RecentFileProjection>),
}

pub(crate) fn project_recent_files(paths: &[PathBuf]) -> RecentFilesProjection {
    if paths.is_empty() {
        return RecentFilesProjection::Empty("No recent files yet");
    }

    RecentFilesProjection::Rows(
        paths
            .iter()
            .map(|path| RecentFileProjection {
                path: path.clone(),
                label: path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned(),
                secondary: path.display().to_string(),
            })
            .collect(),
    )
}

pub fn render(theme: &Theme, game: Game, recent_rows: Vec<AnyElement>) -> Div {
    div().size_full().flex().justify_center().p(px(40.0)).child(
        div()
            .w_full()
            .max_w(px(720.0))
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(
                components::surface(theme)
                    .w_full()
                    .p(px(36.0))
                    .flex()
                    .flex_col()
                    .gap(px(18.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.accent)
                            .child("KINGDOM UNDER FIRE EDITOR"),
                    )
                    .child(
                        div()
                            .text_size(px(30.0))
                            .text_color(theme.text)
                            .child("Edit data files, mods, executable patches, and backups."),
                    )
                    .child(
                        div()
                            .max_w(px(560.0))
                            .text_size(px(15.0))
                            .text_color(theme.text_dim)
                            .child(format!("Current game: {}", game.label())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                components::primary_button(theme, "home-open-file", "Open files")
                                    .on_click(|_, window: &mut Window, cx| {
                                        window.dispatch_action(Box::new(OpenFile), cx);
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.text_dim)
                                    .child("Open SOX files or Crusaders SAV and STG files."),
                            ),
                    ),
            )
            .child(
                components::surface(theme)
                    .w_full()
                    .p(px(24.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.accent)
                            .child("RECENT FILES"),
                    )
                    .child(
                        div()
                            .id("home-recent-files")
                            .max_h(px(220.0))
                            .overflow_y_scroll()
                            .children(recent_rows),
                    ),
            ),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{RecentFileProjection, RecentFilesProjection, project_recent_files};

    #[test]
    fn recent_projection_keeps_file_names_and_full_paths() {
        let troop_path = PathBuf::from("/games/KUF/TroopInfo.sox");
        let skill_path = PathBuf::from("relative/SkillInfo.sox");
        let paths = [troop_path.clone(), skill_path.clone()];

        let projection = project_recent_files(&paths);

        assert_eq!(
            projection,
            RecentFilesProjection::Rows(vec![
                RecentFileProjection {
                    path: troop_path,
                    label: "TroopInfo.sox".to_owned(),
                    secondary: "/games/KUF/TroopInfo.sox".to_owned(),
                },
                RecentFileProjection {
                    path: skill_path,
                    label: "SkillInfo.sox".to_owned(),
                    secondary: "relative/SkillInfo.sox".to_owned(),
                },
            ])
        );
    }

    #[test]
    fn empty_recent_projection_explains_that_there_are_no_files() {
        assert_eq!(
            project_recent_files(&[]),
            RecentFilesProjection::Empty("No recent files yet")
        );
    }
}
