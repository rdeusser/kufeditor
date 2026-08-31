use std::path::PathBuf;

use gpui::{AnyElement, Div, Stateful, Window, div, prelude::*, px};

use crate::{actions::OpenFile, components, theme::Theme};

const EMPTY_COPY: &str = "Open a SOX (.sox) data file, a Crusaders SAV (.sav) file, or a Crusaders STG (.stg) file to begin editing.";

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
        return RecentFilesProjection::Empty("No recent files");
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

pub fn render(
    theme: &Theme,
    tabs: Vec<AnyElement>,
    editor: Option<Div>,
    recent_rows: Vec<AnyElement>,
) -> Div {
    let has_tabs = !tabs.is_empty();
    let body = editor.map_or_else(
        || empty_canvas(theme).into_any_element(),
        |editor| {
            div()
                .id("files-active-editor")
                .debug_selector(|| "files-active-editor".to_owned())
                .size_full()
                .child(editor)
                .into_any_element()
        },
    );
    div()
        .size_full()
        .flex()
        .child(file_navigator(theme, recent_rows))
        .child(document_canvas(theme, tabs, has_tabs, body))
}

fn document_canvas(
    theme: &Theme,
    tabs: Vec<AnyElement>,
    has_tabs: bool,
    body: AnyElement,
) -> Stateful<Div> {
    div()
        .id("files-document-canvas")
        .debug_selector(|| "files-document-canvas".to_owned())
        .size_full()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_none()
                .min_w_0()
                .h(px(40.0))
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .when(!has_tabs, |bar| {
                    bar.px(px(14.0)).items_center().child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.text_dim)
                            .child("No document"),
                    )
                })
                .children(tabs),
        )
        .child(div().flex_1().min_h_0().child(body))
}

fn file_navigator(theme: &Theme, recent_rows: Vec<AnyElement>) -> Stateful<Div> {
    div()
        .id("files-navigator")
        .debug_selector(|| "files-navigator".to_owned())
        .flex()
        .flex_col()
        .flex_none()
        .w(px(260.0))
        .min_h_0()
        .bg(theme.surface)
        .border_r_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(48.0))
                .px(px(12.0))
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(11.0))
                .text_color(theme.text_dim)
                .child("RECENT FILES"),
        )
        .child(
            div()
                .id("files-recent-files")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(recent_rows),
        )
}

fn empty_canvas(theme: &Theme) -> Stateful<Div> {
    div()
        .id("files-empty-canvas")
        .debug_selector(|| "files-empty-canvas".to_owned())
        .size_full()
        .p(px(32.0))
        .bg(theme.background)
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(20.0))
                .text_color(theme.text)
                .child("Open a file to edit"),
        )
        .child(
            div()
                .max_w(px(620.0))
                .text_color(theme.text_dim)
                .child(EMPTY_COPY),
        )
        .child(div().pt(px(4.0)).child(
            components::primary_button(theme, "files-open-file", "Open file").on_click(
                |_, window: &mut Window, cx| {
                    window.dispatch_action(Box::new(OpenFile), cx);
                },
            ),
        ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{EMPTY_COPY, RecentFileProjection, RecentFilesProjection, project_recent_files};

    #[test]
    fn files_supports_stg_in_empty_copy() {
        assert!(EMPTY_COPY.contains(".sox"));
        assert!(EMPTY_COPY.contains(".sav"));
        assert!(EMPTY_COPY.contains(".stg"));
    }

    #[test]
    fn recent_projection_keeps_file_names_and_full_paths() {
        let troop_path = PathBuf::from("/games/KUF/TroopInfo.sox");
        let skill_path = PathBuf::from("relative/SkillInfo.sox");
        let paths = [troop_path.clone(), skill_path.clone()];

        assert_eq!(
            project_recent_files(&paths),
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
    fn empty_recent_projection_names_the_empty_list() {
        assert_eq!(
            project_recent_files(&[]),
            RecentFilesProjection::Empty("No recent files")
        );
    }
}
