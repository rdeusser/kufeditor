use gpui::{Context, Div, FocusHandle, KeyDownEvent, SharedString, Stateful, div, prelude::*, px};
use kufeditor_game::Game;
use kufeditor_patches::{
    BackupStatus, FireRatePresetID, FireRateStatus, PatchID, PatchStatus, PatchTarget,
    fire_rate_presets, patch_definitions,
};

use crate::{
    components,
    frame::AppFrame,
    patch_status::{PatchInspectionPhase, PatchOperation, PatchPresentationState},
    theme::Theme,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PatchContentState {
    Unsupported,
    MissingRoot,
    Idle,
    Loading,
    Ready,
    Failed { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchCardModel {
    pub(crate) id: PatchID,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) experimental: bool,
    pub(crate) status: &'static str,
    pub(crate) action_label: &'static str,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
    operation: Option<PatchOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FireRateModel {
    pub(crate) id: FireRatePresetID,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) selected: bool,
    pub(crate) enabled: bool,
    pub(crate) reason: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchConfirmationModel {
    pub(crate) title: String,
    pub(crate) consequence: String,
    pub(crate) action_label: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchViewModel {
    pub(crate) game: &'static str,
    pub(crate) configured_root: Option<String>,
    pub(crate) content: PatchContentState,
    pub(crate) busy: bool,
    pub(crate) refresh_enabled: bool,
    pub(crate) executable: Option<String>,
    pub(crate) backup_status: Option<&'static str>,
    pub(crate) fire_rate_status: Option<String>,
    pub(crate) patches: Vec<PatchCardModel>,
    pub(crate) fire_rates: Vec<FireRateModel>,
    pub(crate) confirmation: Option<PatchConfirmationModel>,
}

pub(crate) fn project_patches(state: &PatchPresentationState) -> PatchViewModel {
    let busy = state.operation_in_progress();
    let pending = state.pending_confirmation().is_some();
    let blocked = busy || pending;
    let configured_root = state.root().map(|root| root.display().to_string());
    let content = if state.game() == Game::Heroes {
        PatchContentState::Unsupported
    } else if state.root().is_none() {
        PatchContentState::MissingRoot
    } else {
        match state.phase() {
            PatchInspectionPhase::Idle => PatchContentState::Idle,
            PatchInspectionPhase::Loading { .. } => PatchContentState::Loading,
            PatchInspectionPhase::Ready { .. } => PatchContentState::Ready,
            PatchInspectionPhase::Failed { message, .. } => PatchContentState::Failed {
                message: message.clone(),
            },
        }
    };
    let refresh_enabled = !blocked
        && matches!(
            content,
            PatchContentState::Idle | PatchContentState::Ready | PatchContentState::Failed { .. }
        );

    let mut model = PatchViewModel {
        game: state.game().label(),
        configured_root,
        content,
        busy,
        refresh_enabled,
        executable: None,
        backup_status: None,
        fire_rate_status: None,
        patches: Vec::new(),
        fire_rates: Vec::new(),
        confirmation: state.pending_confirmation().map(|confirmation| {
            let operation = confirmation.operation();
            PatchConfirmationModel {
                title: confirmation_title(operation),
                consequence: format!(
                    "KufEditor will update {}. Backup: {}.",
                    confirmation.executable().display(),
                    confirmation.backup().display(),
                ),
                action_label: confirmation_action_label(operation),
            }
        }),
    };

    let PatchInspectionPhase::Ready { snapshot, .. } = state.phase() else {
        return model;
    };
    model.executable = Some(snapshot.executable().display().to_string());
    model.backup_status = Some(match snapshot.backup_status() {
        BackupStatus::Missing => "Not created",
        BackupStatus::Present => "Available",
    });
    model.fire_rate_status = Some(fire_rate_status_label(snapshot.fire_rate()));
    model.patches = patch_definitions()
        .iter()
        .map(|definition| {
            patch_card(
                definition.id(),
                definition.name(),
                definition.description(),
                definition.experimental(),
                snapshot.patch_status(definition.id()),
                blocked,
            )
        })
        .collect();
    model.fire_rates = fire_rate_presets()
        .iter()
        .map(|preset| {
            let selected = snapshot.fire_rate() == FireRateStatus::Preset(preset.id());
            let unknown = snapshot.fire_rate() == FireRateStatus::Unknown;
            FireRateModel {
                id: preset.id(),
                name: preset.name(),
                description: preset.description(),
                selected,
                enabled: !blocked && !selected && !unknown,
                reason: action_reason(blocked, unknown, selected),
            }
        })
        .collect();
    model
}

fn patch_card(
    id: PatchID,
    name: &'static str,
    description: &'static str,
    experimental: bool,
    status: PatchStatus,
    blocked: bool,
) -> PatchCardModel {
    let (status_label, action_label, operation, unknown) = match status {
        PatchStatus::Applied => (
            "Applied",
            "Revert",
            Some(PatchOperation::SetPatch {
                id,
                target: PatchTarget::NotApplied,
            }),
            false,
        ),
        PatchStatus::NotApplied => (
            "Not applied",
            "Apply",
            Some(PatchOperation::SetPatch {
                id,
                target: PatchTarget::Applied,
            }),
            false,
        ),
        PatchStatus::Unknown => ("Unknown", "Cannot change", None, true),
    };
    PatchCardModel {
        id,
        name,
        description,
        experimental,
        status: status_label,
        action_label,
        enabled: !blocked && !unknown,
        reason: action_reason(blocked, unknown, false),
        operation,
    }
}

const fn action_reason(blocked: bool, unknown: bool, selected: bool) -> Option<&'static str> {
    if blocked {
        Some("Finish the current confirmation or file update first.")
    } else if unknown {
        Some("Executable bytes are not recognized.")
    } else if selected {
        Some("This preset is selected.")
    } else {
        None
    }
}

fn fire_rate_status_label(status: FireRateStatus) -> String {
    match status {
        FireRateStatus::Preset(id) => format!("{} preset", fire_rate_name(id)),
        FireRateStatus::Custom(values) => format!(
            "Custom: delay {}, multiplier {}, factor {}",
            values.base_delay(),
            values.multiplier(),
            values.distance_factor(),
        ),
        FireRateStatus::Unknown => "Unknown executable bytes".to_owned(),
    }
}

const fn fire_rate_name(id: FireRatePresetID) -> &'static str {
    match id {
        FireRatePresetID::Original => "Original",
        FireRatePresetID::Fast => "Fast",
        FireRatePresetID::Rapid => "Rapid",
        FireRatePresetID::Turbo => "Turbo",
    }
}

fn confirmation_title(operation: PatchOperation) -> String {
    match operation {
        PatchOperation::SetPatch {
            id,
            target: PatchTarget::Applied,
        } => format!("Apply {}?", patch_name(id)),
        PatchOperation::SetPatch {
            id,
            target: PatchTarget::NotApplied,
        } => format!("Revert {}?", patch_name(id)),
        PatchOperation::SetFireRate { id } => {
            format!("Use the {} fire-rate preset?", fire_rate_name(id))
        }
    }
}

const fn confirmation_action_label(operation: PatchOperation) -> &'static str {
    match operation {
        PatchOperation::SetPatch {
            target: PatchTarget::Applied,
            ..
        } => "Apply patch",
        PatchOperation::SetPatch {
            target: PatchTarget::NotApplied,
            ..
        } => "Revert patch",
        PatchOperation::SetFireRate { .. } => "Use preset",
    }
}

const fn patch_name(id: PatchID) -> &'static str {
    match id {
        PatchID::DebugMenu => "Debug Menu",
        PatchID::TerrainBounds => "Terrain Bounds Check",
    }
}

#[derive(Clone, Copy)]
enum PatchControlAction {
    Refresh,
    Operation(PatchOperation),
    DismissConfirmation,
    ConfirmOperation,
}

impl PatchControlAction {
    fn activate(self, frame: &mut AppFrame, cx: &mut Context<AppFrame>) {
        match self {
            Self::Refresh => frame.start_patch_inspection(cx),
            Self::Operation(PatchOperation::SetPatch { id, target }) => {
                frame.request_patch_change(id, target, cx);
            }
            Self::Operation(PatchOperation::SetFireRate { id }) => {
                frame.request_fire_rate_change(id, cx);
            }
            Self::DismissConfirmation => frame.dismiss_patch_confirmation(cx),
            Self::ConfirmOperation => frame.confirm_patch_operation(cx),
        }
    }
}

pub(crate) fn render(
    theme: &Theme,
    model: &PatchViewModel,
    initial_focus: &FocusHandle,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let refresh = patch_action_button(
        theme,
        "patch-refresh",
        if matches!(model.content, PatchContentState::Loading) {
            "Checking…"
        } else {
            "Refresh"
        },
        model.refresh_enabled,
        PatchControlAction::Refresh,
        Some(initial_focus),
        cx,
    );
    div()
        .id("patches-route")
        .debug_selector(|| "patches-route".to_owned())
        .size_full()
        .overflow_y_scroll()
        .bg(theme.background)
        .p(px(24.0))
        .child(
            div()
                .w_full()
                .max_w(px(980.0))
                .mx_auto()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(22.0))
                                        .text_color(theme.text)
                                        .child("Executable Patches"),
                                )
                                .child(
                                    div()
                                        .text_color(theme.text_dim)
                                        .child(format!("{} · Kuf2Main.exe patches", model.game)),
                                ),
                        )
                        .child(refresh),
                )
                .children(
                    model
                        .confirmation
                        .as_ref()
                        .map(|confirmation| render_confirmation(theme, confirmation, cx)),
                )
                .child(render_content(theme, model, cx)),
        )
}

fn render_content(theme: &Theme, model: &PatchViewModel, cx: &mut Context<AppFrame>) -> Div {
    match &model.content {
        PatchContentState::Unsupported => status_panel(
            theme,
            "No patches for Heroes",
            "Executable patches are available only for Crusaders.",
        ),
        PatchContentState::MissingRoot => status_panel(
            theme,
            "Crusaders is not configured",
            "Choose the Crusaders game folder in Settings before editing Kuf2Main.exe.",
        ),
        PatchContentState::Idle => status_panel(
            theme,
            "Kuf2Main.exe has not been checked",
            "Select Refresh to check patch and backup status.",
        ),
        PatchContentState::Loading => status_panel(
            theme,
            "Checking Kuf2Main.exe",
            "Reading patch bytes, fire-rate settings, and backup status.",
        ),
        PatchContentState::Failed { message } => {
            status_panel(theme, "Could not check Kuf2Main.exe", message)
        }
        PatchContentState::Ready => render_ready(theme, model, cx),
    }
}

fn status_panel(theme: &Theme, title: &str, detail: &str) -> Div {
    components::surface(theme)
        .w_full()
        .p(px(22.0))
        .flex()
        .flex_col()
        .gap(px(7.0))
        .child(
            div()
                .text_size(px(17.0))
                .text_color(theme.text)
                .child(title.to_owned()),
        )
        .child(div().text_color(theme.text_dim).child(detail.to_owned()))
}

fn render_ready(theme: &Theme, model: &PatchViewModel, cx: &mut Context<AppFrame>) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .children(model.busy.then(|| {
            status_panel(
                theme,
                "Writing Kuf2Main.exe",
                "Wait for the write and follow-up check to finish.",
            )
        }))
        .child(executable_panel(theme, model))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(theme.accent)
                .child("EXECUTABLE PATCHES"),
        )
        .children(
            model
                .patches
                .iter()
                .map(|patch| render_patch_card(theme, patch, cx)),
        )
        .child(
            div()
                .pt(px(4.0))
                .text_size(px(13.0))
                .text_color(theme.accent)
                .child("FIRE RATE"),
        )
        .child(render_fire_rates(theme, model, cx))
}

fn executable_panel(theme: &Theme, model: &PatchViewModel) -> Div {
    components::surface(theme)
        .w_full()
        .p(px(18.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_color(theme.text)
                .child(model.executable.clone().unwrap_or_default()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(format!(
                    "Backup: {}",
                    model.backup_status.unwrap_or("Unknown")
                )),
        )
        .children(model.configured_root.as_ref().map(|root| {
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(format!("Game folder: {root}"))
        }))
}

fn render_patch_card(
    theme: &Theme,
    patch: &PatchCardModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let id = patch_element_id(patch.id);
    let status = patch.status;
    let button = patch_action_button(
        theme,
        patch_action_element_id(patch.id),
        patch.action_label,
        patch.enabled,
        PatchControlAction::Operation(patch.operation.unwrap_or(PatchOperation::SetPatch {
            id: patch.id,
            target: PatchTarget::Applied,
        })),
        None,
        cx,
    );
    components::surface(theme)
        .id(id)
        .debug_selector(move || id.to_owned())
        .w_full()
        .p(px(18.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(17.0))
                        .text_color(theme.text)
                        .child(patch.name),
                )
                .child(status_badge(theme, status))
                .children(patch.experimental.then(|| {
                    div()
                        .px(px(7.0))
                        .py(px(2.0))
                        .rounded_md()
                        .bg(theme.accent_dim)
                        .text_size(px(11.0))
                        .text_color(theme.accent)
                        .child("EXPERIMENTAL")
                })),
        )
        .child(div().text_color(theme.text_dim).child(patch.description))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(9.0))
                .child(button)
                .children(patch.reason.map(|reason| {
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.text_dim)
                        .child(reason)
                })),
        )
}

fn render_fire_rates(theme: &Theme, model: &PatchViewModel, cx: &mut Context<AppFrame>) -> Div {
    components::surface(theme)
        .w_full()
        .p(px(18.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .text_color(theme.text)
                .child(model.fire_rate_status.clone().unwrap_or_default()),
        )
        .children(model.fire_rates.iter().map(|preset| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .py(px(5.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_color(if preset.selected {
                                    theme.accent
                                } else {
                                    theme.text
                                })
                                .child(preset.name),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.text_dim)
                                .child(preset.description),
                        )
                        .children(preset.reason.map(|reason| {
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.text_dim)
                                .child(reason)
                        })),
                )
                .child(patch_action_button(
                    theme,
                    fire_rate_element_id(preset.id),
                    if preset.selected {
                        "Selected"
                    } else {
                        "Use preset"
                    },
                    preset.enabled,
                    PatchControlAction::Operation(PatchOperation::SetFireRate { id: preset.id }),
                    None,
                    cx,
                ))
        }))
}

fn render_confirmation(
    theme: &Theme,
    confirmation: &PatchConfirmationModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    components::surface(theme)
        .id("patch-confirmation")
        .debug_selector(|| "patch-confirmation".to_owned())
        .w_full()
        .p(px(18.0))
        .border_color(theme.accent)
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(17.0))
                .text_color(theme.text)
                .child(confirmation.title.clone()),
        )
        .child(
            div()
                .text_color(theme.text_dim)
                .child(confirmation.consequence.clone()),
        )
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .child(patch_action_button(
                    theme,
                    "patch-confirmation-dismiss",
                    "Cancel",
                    true,
                    PatchControlAction::DismissConfirmation,
                    None,
                    cx,
                ))
                .child(patch_action_button(
                    theme,
                    "patch-confirmation-accept",
                    confirmation.action_label,
                    true,
                    PatchControlAction::ConfirmOperation,
                    None,
                    cx,
                )),
        )
}

fn status_badge(theme: &Theme, label: &'static str) -> Div {
    div()
        .px(px(7.0))
        .py(px(2.0))
        .rounded_md()
        .bg(theme.raised)
        .text_size(px(11.0))
        .text_color(theme.accent)
        .child(label)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the control factory keeps stable identity, focus, availability, and one action binding together"
)]
fn patch_action_button(
    theme: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
    action: PatchControlAction,
    initial_focus: Option<&FocusHandle>,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let hover = theme.raised;
    let selector = id.to_owned();
    let button = div()
        .id(SharedString::from(id))
        .debug_selector(move || selector)
        .h(px(34.0))
        .px(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .text_color(if enabled { theme.text } else { theme.text_dim })
        .when(enabled, move |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .focus(move |style| style.border_color(theme.accent).bg(theme.accent_dim))
        })
        .when(!enabled, |button| button.opacity(0.45))
        .when_some(initial_focus, Stateful::track_focus)
        .child(label);
    if enabled {
        bind_patch_control(button, action, cx)
    } else {
        button
    }
}

fn bind_patch_control(
    control: Stateful<Div>,
    action: PatchControlAction,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let keyboard_action = action;
    control
        .on_click(cx.listener(move |frame, _, _, cx| action.activate(frame, cx)))
        .on_key_down(cx.listener(move |frame, event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                keyboard_action.activate(frame, cx);
            }
        }))
}

const fn patch_element_id(id: PatchID) -> &'static str {
    match id {
        PatchID::DebugMenu => "patch-card-debug-menu",
        PatchID::TerrainBounds => "patch-card-terrain-bounds",
    }
}

const fn patch_action_element_id(id: PatchID) -> &'static str {
    match id {
        PatchID::DebugMenu => "patch-debug-menu-action",
        PatchID::TerrainBounds => "patch-terrain-bounds-action",
    }
}

const fn fire_rate_element_id(id: FireRatePresetID) -> &'static str {
    match id {
        FireRatePresetID::Original => "patch-fire-original",
        FireRatePresetID::Fast => "patch-fire-fast",
        FireRatePresetID::Rapid => "patch-fire-rapid",
        FireRatePresetID::Turbo => "patch-fire-turbo",
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "projection tests use fixed configured contexts and must fail at setup"
    )]

    use std::path::PathBuf;

    use kufeditor_game::Game;
    use kufeditor_patches::{
        BackupStatus, FireRatePresetID, FireRateStatus, PatchID, PatchStatus, PatchTarget,
    };

    use super::{PatchContentState, project_patches};
    use crate::patch_status::{
        ExecutablePatchState, PatchOperation, PatchPresentationState, PatchSnapshot,
    };

    #[test]
    fn projection_names_unsupported_missing_idle_loading_and_failed_states() {
        let heroes =
            PatchPresentationState::new(Game::Heroes, Some(PathBuf::from("/games/heroes")), 1);
        assert!(matches!(
            project_patches(&heroes).content,
            PatchContentState::Unsupported
        ));

        let missing = PatchPresentationState::new(Game::Crusaders, None, 1);
        assert!(matches!(
            project_patches(&missing).content,
            PatchContentState::MissingRoot
        ));

        let mut configured = PatchPresentationState::new(
            Game::Crusaders,
            Some(PathBuf::from("/games/crusaders")),
            1,
        );
        assert!(matches!(
            project_patches(&configured).content,
            PatchContentState::Idle
        ));
        let key = configured.begin_inspection().expect("configured root");
        assert!(matches!(
            project_patches(&configured).content,
            PatchContentState::Loading
        ));
        assert!(configured.finish_inspection(key, Err("wrong executable version".to_owned())));
        assert!(matches!(
            project_patches(&configured).content,
            PatchContentState::Failed { message } if message == "wrong executable version"
        ));
    }

    #[test]
    fn ready_projection_keeps_patch_statuses_presets_and_action_reasons() {
        let state = ready_state();
        let model = project_patches(&state);
        assert!(matches!(model.content, PatchContentState::Ready));
        assert_eq!(
            model.executable.as_deref(),
            Some("/games/crusaders/Kuf2Main.exe")
        );
        assert_eq!(model.backup_status, Some("Not created"));
        let [debug, terrain] = model.patches.as_slice() else {
            panic!("expected two patch cards");
        };
        assert_eq!(debug.id, PatchID::DebugMenu);
        assert_eq!(debug.status, "Not applied");
        assert_eq!(debug.action_label, "Apply");
        assert!(debug.enabled);
        assert_eq!(terrain.status, "Unknown");
        assert!(!terrain.enabled);
        assert_eq!(terrain.reason, Some("Executable bytes are not recognized."));

        let original = model
            .fire_rates
            .iter()
            .find(|preset| preset.id == FireRatePresetID::Original)
            .expect("original preset");
        assert!(original.selected);
        assert!(!original.enabled);
        let turbo = model
            .fire_rates
            .iter()
            .find(|preset| preset.id == FireRatePresetID::Turbo)
            .expect("turbo preset");
        assert!(turbo.enabled);
    }

    #[test]
    fn confirmation_uses_stable_paths_and_busy_state_disables_every_action() {
        let mut state = ready_state();
        assert!(state.request_operation(PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            target: PatchTarget::Applied,
        }));
        let confirmation = project_patches(&state)
            .confirmation
            .expect("confirmation model");
        assert!(confirmation.title.contains("Debug Menu"));
        assert!(
            confirmation
                .consequence
                .contains("/games/crusaders/Kuf2Main.exe")
        );
        assert!(
            confirmation
                .consequence
                .contains("/games/crusaders/Kuf2Main.exe.bak")
        );

        assert!(state.confirm_operation().is_some());
        let busy = project_patches(&state);
        assert!(busy.busy);
        assert!(busy.patches.iter().all(|patch| !patch.enabled));
        assert!(busy.fire_rates.iter().all(|preset| !preset.enabled));
    }

    fn ready_state() -> PatchPresentationState {
        let mut state = PatchPresentationState::new(
            Game::Crusaders,
            Some(PathBuf::from("/games/crusaders")),
            1,
        );
        let key = state.begin_inspection().expect("configured root");
        assert!(state.finish_inspection(
            key,
            Ok(PatchSnapshot::new(
                "/games/crusaders/Kuf2Main.exe".into(),
                "/games/crusaders/Kuf2Main.exe.bak".into(),
                BackupStatus::Missing,
                [
                    ExecutablePatchState::new(PatchID::DebugMenu, PatchStatus::NotApplied),
                    ExecutablePatchState::new(PatchID::TerrainBounds, PatchStatus::Unknown),
                ],
                FireRateStatus::Preset(FireRatePresetID::Original),
            )),
        ));
        state
    }
}
