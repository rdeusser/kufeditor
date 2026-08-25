use std::fmt::Display;

use gpui::{Context, Div, Stateful, div, prelude::*, px};
use kufeditor_game::{Game, GamePaths};
use kufeditor_workspace::{RECENT_FILE_LIMITS, RecentFiles};

use crate::{
    catalog_status::CatalogStatus,
    components,
    frame::{AppFrame, discovery_status::DiscoveryStatus},
    theme::Theme,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstallationProjection {
    pub(crate) game: Game,
    pub(crate) label: &'static str,
    pub(crate) path: String,
    pub(crate) browse_id: &'static str,
    pub(crate) clear_id: &'static str,
    pub(crate) clear_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CatalogProjection {
    NotConfigured,
    Loading,
    Ready,
    ReadyWithIssues { issue_count: usize },
    Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryProjection {
    Unavailable {
        reason: String,
    },
    Idle,
    Loading,
    Ready {
        request: u64,
        installation_count: usize,
        issue_count: usize,
    },
    Failed {
        request: u64,
        error: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecentLimitProjection {
    pub(crate) limit: usize,
    pub(crate) element_id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsLayoutProjection {
    BoundedVerticalScroll,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettingsProjection {
    pub(crate) installations: [InstallationProjection; 2],
    pub(crate) catalog: CatalogProjection,
    pub(crate) discovery: DiscoveryProjection,
    pub(crate) auto_detect_id: &'static str,
    pub(crate) auto_detect_enabled: bool,
    pub(crate) recent_limits: [RecentLimitProjection; 4],
    pub(crate) clear_recents_id: &'static str,
    pub(crate) clear_recents_enabled: bool,
    pub(crate) layout: SettingsLayoutProjection,
}

pub(crate) fn project_settings<T, E>(
    paths: &GamePaths,
    catalog: &CatalogStatus<T, E>,
    discovery: &DiscoveryStatus,
    recent: &RecentFiles,
    discovery_available: bool,
) -> SettingsProjection
where
    E: Display,
{
    SettingsProjection {
        installations: Game::ALL.map(|game| installation_projection(paths, game)),
        catalog: match catalog {
            CatalogStatus::NotConfigured => CatalogProjection::NotConfigured,
            CatalogStatus::Loading { .. } => CatalogProjection::Loading,
            CatalogStatus::Ready { issue_count: 0, .. } => CatalogProjection::Ready,
            CatalogStatus::Ready { issue_count, .. } => CatalogProjection::ReadyWithIssues {
                issue_count: *issue_count,
            },
            CatalogStatus::Failed { error, .. } => CatalogProjection::Failed(error.to_string()),
        },
        discovery: discovery_projection(discovery, discovery_available),
        auto_detect_id: "settings-auto-detect",
        auto_detect_enabled: discovery_available
            && !matches!(discovery, DiscoveryStatus::Loading { .. }),
        recent_limits: RECENT_FILE_LIMITS.map(|limit| RecentLimitProjection {
            limit,
            element_id: recent_limit_id(limit),
            label: recent_limit_label(limit),
            selected: recent.limit() == limit,
        }),
        clear_recents_id: "settings-clear-recents",
        clear_recents_enabled: !recent.paths().is_empty(),
        layout: SettingsLayoutProjection::BoundedVerticalScroll,
    }
}

fn installation_projection(paths: &GamePaths, game: Game) -> InstallationProjection {
    let (browse_id, clear_id) = match game {
        Game::Crusaders => ("settings-browse-crusaders", "settings-clear-crusaders"),
        Game::Heroes => ("settings-browse-heroes", "settings-clear-heroes"),
    };
    InstallationProjection {
        game,
        label: game.label(),
        path: paths.root(game).map_or_else(
            || "Not configured".to_owned(),
            |path| path.display().to_string(),
        ),
        browse_id,
        clear_id,
        clear_enabled: paths.root(game).is_some(),
    }
}

fn discovery_projection(discovery: &DiscoveryStatus, available: bool) -> DiscoveryProjection {
    if !available {
        return DiscoveryProjection::Unavailable {
            reason: "Automatic Steam discovery is available only on Windows".to_owned(),
        };
    }
    match discovery {
        DiscoveryStatus::Idle => DiscoveryProjection::Idle,
        DiscoveryStatus::Loading { .. } => DiscoveryProjection::Loading,
        DiscoveryStatus::Ready { key, report } => DiscoveryProjection::Ready {
            request: key.request().get(),
            installation_count: report.installations().len(),
            issue_count: report.issues().len(),
        },
        DiscoveryStatus::Failed { key, error } => DiscoveryProjection::Failed {
            request: key.request().get(),
            error: error.to_string(),
        },
    }
}

const fn recent_limit_id(limit: usize) -> &'static str {
    match limit {
        5 => "settings-recent-limit-5",
        10 => "settings-recent-limit-10",
        15 => "settings-recent-limit-15",
        20 => "settings-recent-limit-20",
        _ => unreachable!(),
    }
}

const fn recent_limit_label(limit: usize) -> &'static str {
    match limit {
        5 => "5",
        10 => "10",
        15 => "15",
        20 => "20",
        _ => unreachable!(),
    }
}

pub(crate) fn render(
    theme: &Theme,
    projection: SettingsProjection,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let installation_rows = projection
        .installations
        .into_iter()
        .map(|installation| installation_row(theme, installation, cx))
        .collect::<Vec<_>>();
    let auto_detect = action_button(
        theme,
        projection.auto_detect_id,
        "Auto-detect",
        projection.auto_detect_enabled,
    )
    .when(projection.auto_detect_enabled, |button| {
        button.on_click(cx.listener(|frame, _, _, cx| {
            frame.start_discovery(cx);
        }))
    });
    let recent_limits = projection
        .recent_limits
        .into_iter()
        .map(|limit| {
            components::choice_button(theme, limit.element_id, limit.label, limit.selected)
                .on_click(cx.listener(move |frame, _, _, cx| {
                    frame.set_recent_limit(limit.limit, cx);
                }))
        })
        .collect::<Vec<_>>();
    let clear_recents = action_button(
        theme,
        projection.clear_recents_id,
        "Clear recent files",
        projection.clear_recents_enabled,
    )
    .when(projection.clear_recents_enabled, |button| {
        button.on_click(cx.listener(|frame, _, _, cx| {
            frame.clear_recent_files(cx);
        }))
    });
    let discovery_text = discovery_text(&projection.discovery);

    settings_layout([
        installations_surface(theme, installation_rows, auto_detect, discovery_text),
        catalog_surface(theme, &projection.catalog),
        recent_files_surface(theme, recent_limits, clear_recents),
    ])
}

fn settings_layout(surfaces: [Div; 3]) -> Stateful<Div> {
    settings_scroll_root().child(settings_bounded_content().children(surfaces))
}

fn settings_scroll_root() -> Stateful<Div> {
    div()
        .id("settings-content")
        .size_full()
        .overflow_y_scroll()
        .p(px(28.0))
        .flex()
        .justify_center()
}

fn settings_bounded_content() -> Div {
    div()
        .w_full()
        .max_w(px(860.0))
        .flex()
        .flex_col()
        .gap(px(18.0))
}

fn installations_surface(
    theme: &Theme,
    rows: Vec<Div>,
    auto_detect: Stateful<Div>,
    discovery: String,
) -> Div {
    components::surface(theme)
        .w_full()
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(section_title(theme, "Game installations"))
        .children(rows)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(auto_detect)
                .child(div().text_color(theme.text_dim).child(discovery)),
        )
}

fn catalog_surface(theme: &Theme, catalog: &CatalogProjection) -> Div {
    components::surface(theme)
        .w_full()
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(section_title(theme, "Active catalog"))
        .child(
            div()
                .text_color(theme.text_dim)
                .child(catalog_text(catalog)),
        )
}

fn recent_files_surface(theme: &Theme, limits: Vec<Stateful<Div>>, clear: Stateful<Div>) -> Div {
    components::surface(theme)
        .w_full()
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(section_title(theme, "Recent files"))
        .child(
            div()
                .text_color(theme.text_dim)
                .child("Maximum recent files"),
        )
        .child(div().flex().gap(px(8.0)).children(limits))
        .child(clear)
}

fn installation_row(
    theme: &Theme,
    installation: InstallationProjection,
    cx: &mut Context<AppFrame>,
) -> Div {
    let game = installation.game;
    let browse = action_button(theme, installation.browse_id, "Browse", true).on_click(
        cx.listener(move |frame, _, _, cx| {
            frame.browse_game_root(game, cx);
        }),
    );
    let clear = action_button(
        theme,
        installation.clear_id,
        "Clear",
        installation.clear_enabled,
    )
    .when(installation.clear_enabled, |button| {
        button.on_click(cx.listener(move |frame, _, _, cx| {
            frame.clear_game_root(game, cx);
        }))
    });

    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(div().text_color(theme.text).child(installation.label))
                .child(
                    div()
                        .text_color(theme.text_dim)
                        .text_ellipsis()
                        .child(installation.path),
                ),
        )
        .child(browse)
        .child(clear)
}

fn action_button(
    theme: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
) -> Stateful<Div> {
    if enabled {
        components::toolbar_button(theme, id, label, true)
    } else {
        components::disabled_rail_item(theme, id, label)
    }
}

fn section_title(theme: &Theme, label: &'static str) -> Div {
    div()
        .text_size(px(20.0))
        .text_color(theme.text)
        .child(label)
}

fn catalog_text(catalog: &CatalogProjection) -> String {
    match catalog {
        CatalogProjection::NotConfigured => "Not configured".to_owned(),
        CatalogProjection::Loading => "Loading game catalogs".to_owned(),
        CatalogProjection::Ready => "Ready".to_owned(),
        CatalogProjection::ReadyWithIssues { issue_count } => {
            let issue = if *issue_count == 1 { "issue" } else { "issues" };
            format!("Ready with {issue_count} {issue}")
        }
        CatalogProjection::Failed(error) => format!("Failed: {error}"),
    }
}

fn discovery_text(discovery: &DiscoveryProjection) -> String {
    match discovery {
        DiscoveryProjection::Unavailable { reason } => reason.clone(),
        DiscoveryProjection::Idle => "Scan Steam libraries for game folders".to_owned(),
        DiscoveryProjection::Loading => "Scanning Steam libraries".to_owned(),
        DiscoveryProjection::Ready {
            installation_count,
            issue_count,
            ..
        } => format!("Found {installation_count} installations with {issue_count} issues"),
        DiscoveryProjection::Failed { error, .. } => format!("Discovery failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gpui::{Overflow, Styled, px};
    use kufeditor_game::{DiscoveryError, Game, GamePaths, scan_steam_common_directories};
    use kufeditor_workspace::{RECENT_FILE_LIMITS, RecentFiles};

    use super::{
        CatalogProjection, DiscoveryProjection, SettingsLayoutProjection, catalog_text,
        project_settings, settings_bounded_content, settings_scroll_root,
    };
    use crate::{
        catalog_status::{CatalogKey, CatalogStatus},
        frame::discovery_status::{DiscoveryKey, DiscoveryStatus, RootRevisions},
        state::ShellState,
    };

    fn catalog_key() -> CatalogKey {
        let mut shell = ShellState::default();
        CatalogKey::new(shell.begin_catalog(), Game::Crusaders, "/game")
    }

    fn discovery_key(paths: &GamePaths) -> DiscoveryKey {
        let mut shell = ShellState::default();
        DiscoveryKey::new(shell.begin_discovery(), RootRevisions::default(), paths)
    }

    #[test]
    fn settings_view_game_installations_project_unconfigured_and_configured_roots() {
        let catalog = CatalogStatus::<(), &'static str>::NotConfigured;
        let discovery = DiscoveryStatus::Idle;
        let recent = RecentFiles::default();
        let mut paths = GamePaths::default();

        let empty = project_settings(&paths, &catalog, &discovery, &recent, true);
        assert_eq!(empty.installations[0].path, "Not configured");
        assert_eq!(empty.installations[1].path, "Not configured");
        assert!(!empty.installations[0].clear_enabled);
        assert!(!empty.installations[1].clear_enabled);

        paths.set_root(Game::Heroes, Some(PathBuf::from("/games/Heroes")));
        let configured = project_settings(&paths, &catalog, &discovery, &recent, true);
        assert_eq!(configured.installations[0].path, "Not configured");
        assert_eq!(configured.installations[1].path, "/games/Heroes");
        assert!(!configured.installations[0].clear_enabled);
        assert!(configured.installations[1].clear_enabled);
    }

    #[test]
    fn settings_view_catalog_projection_covers_not_configured_loading_ready_and_failed() {
        let paths = GamePaths::default();
        let discovery = DiscoveryStatus::Idle;
        let recent = RecentFiles::default();
        let key = catalog_key();

        let cases = [
            (
                CatalogStatus::NotConfigured,
                CatalogProjection::NotConfigured,
            ),
            (
                CatalogStatus::Loading { key: key.clone() },
                CatalogProjection::Loading,
            ),
            (
                CatalogStatus::Ready {
                    key: key.clone(),
                    value: (),
                    issue_count: 0,
                },
                CatalogProjection::Ready,
            ),
            (
                CatalogStatus::Failed {
                    key,
                    error: "fixture catalog failure",
                },
                CatalogProjection::Failed("fixture catalog failure".to_owned()),
            ),
        ];

        for (catalog, expected) in cases {
            assert_eq!(
                project_settings(&paths, &catalog, &discovery, &recent, true).catalog,
                expected
            );
        }
    }

    #[test]
    fn settings_view_catalog_ready_with_issues_keeps_and_renders_the_exact_count() {
        let projection = project_settings(
            &GamePaths::default(),
            &CatalogStatus::<(), &'static str>::Ready {
                key: catalog_key(),
                value: (),
                issue_count: 3,
            },
            &DiscoveryStatus::Idle,
            &RecentFiles::default(),
            true,
        );

        assert_eq!(
            projection.catalog,
            CatalogProjection::ReadyWithIssues { issue_count: 3 }
        );
        assert_eq!(catalog_text(&projection.catalog), "Ready with 3 issues");
    }

    #[test]
    fn settings_view_discovery_projection_covers_unavailable_loading_ready_empty_and_failed() {
        let paths = GamePaths::default();
        let catalog = CatalogStatus::<(), &'static str>::NotConfigured;
        let recent = RecentFiles::default();
        let key = discovery_key(&paths);

        let unavailable =
            project_settings(&paths, &catalog, &DiscoveryStatus::Idle, &recent, false);
        assert!(matches!(
            unavailable.discovery,
            DiscoveryProjection::Unavailable { .. }
        ));
        assert!(!unavailable.auto_detect_enabled);

        let loading = DiscoveryStatus::Loading { key: key.clone() };
        assert_eq!(
            project_settings(&paths, &catalog, &loading, &recent, true).discovery,
            DiscoveryProjection::Loading
        );

        let ready = DiscoveryStatus::Ready {
            key: key.clone(),
            report: scan_steam_common_directories(&[]),
        };
        assert_eq!(
            project_settings(&paths, &catalog, &ready, &recent, true).discovery,
            DiscoveryProjection::Ready {
                request: key.request().get(),
                installation_count: 0,
                issue_count: 0,
            }
        );

        let failed = DiscoveryStatus::Failed {
            key: key.clone(),
            error: DiscoveryError::Unavailable,
        };
        assert_eq!(
            project_settings(&paths, &catalog, &failed, &recent, true).discovery,
            DiscoveryProjection::Failed {
                request: key.request().get(),
                error: "automatic Steam discovery is unavailable on this platform".to_owned(),
            }
        );
    }

    #[test]
    fn settings_view_actions_keep_stable_element_ids() {
        let projection = project_settings(
            &GamePaths::default(),
            &CatalogStatus::<(), &'static str>::NotConfigured,
            &DiscoveryStatus::Idle,
            &RecentFiles::default(),
            true,
        );

        assert_eq!(
            projection.installations[0].browse_id,
            "settings-browse-crusaders"
        );
        assert_eq!(
            projection.installations[0].clear_id,
            "settings-clear-crusaders"
        );
        assert_eq!(
            projection.installations[1].browse_id,
            "settings-browse-heroes"
        );
        assert_eq!(
            projection.installations[1].clear_id,
            "settings-clear-heroes"
        );
        assert_eq!(projection.auto_detect_id, "settings-auto-detect");
        assert_eq!(
            projection
                .recent_limits
                .iter()
                .map(|limit| limit.element_id)
                .collect::<Vec<_>>(),
            vec![
                "settings-recent-limit-5",
                "settings-recent-limit-10",
                "settings-recent-limit-15",
                "settings-recent-limit-20",
            ]
        );
        assert_eq!(projection.clear_recents_id, "settings-clear-recents");
    }

    #[test]
    fn settings_view_every_supported_recent_limit_is_the_unique_selected_choice() {
        for selected in RECENT_FILE_LIMITS {
            let recent = RecentFiles::new(selected);
            let projection = project_settings(
                &GamePaths::default(),
                &CatalogStatus::<(), &'static str>::NotConfigured,
                &DiscoveryStatus::Idle,
                &recent,
                true,
            );
            let selected_limits = projection
                .recent_limits
                .iter()
                .filter(|limit| limit.selected)
                .map(|limit| limit.limit)
                .collect::<Vec<_>>();

            assert_eq!(selected_limits, vec![selected]);
        }
    }

    #[test]
    fn settings_view_content_requests_vertical_scrolling_and_bounded_width() {
        let projection = project_settings(
            &GamePaths::default(),
            &CatalogStatus::<(), &'static str>::NotConfigured,
            &DiscoveryStatus::Idle,
            &RecentFiles::default(),
            true,
        );

        assert_eq!(
            projection.layout,
            SettingsLayoutProjection::BoundedVerticalScroll
        );

        let mut root = settings_scroll_root();
        assert_eq!(root.style().overflow.y, Some(Overflow::Scroll));

        let mut content = settings_bounded_content();
        assert_eq!(content.style().max_size.width, Some(px(860.0).into()));
    }
}
