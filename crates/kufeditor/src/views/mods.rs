use gpui::{AnyElement, Context, Div, FocusHandle, SharedString, Stateful, div, prelude::*, px};
use kufeditor_mods::{
    BackupID, InstallationID, InstalledModStatus, ModPackageID, ModProgressPhase,
};

use crate::{
    components,
    frame::{AppFrame, mods::ModFormInputs},
    mod_status::{
        BackupSnapshot, InstalledModSnapshot, ModIssueSnapshot, ModLibraryState,
        ModPackageSnapshot, ModPresentationState, ModRootState, ModSection,
    },
    theme::Theme,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModSectionModel {
    pub(crate) section: ModSection,
    pub(crate) element_id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModActionAvailability {
    pub(crate) enabled: bool,
    pub(crate) reason: Option<String>,
}

impl ModActionAvailability {
    fn enabled() -> Self {
        Self {
            enabled: true,
            reason: None,
        }
    }

    fn disabled(reason: impl Into<String>) -> Self {
        Self {
            enabled: false,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModContentState {
    Idle {
        message: &'static str,
    },
    Loading {
        message: &'static str,
    },
    MissingRoot {
        requirement: String,
    },
    Empty {
        title: &'static str,
        next_action: &'static str,
    },
    Ready,
    Failed {
        title: String,
        detail: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstalledRowModel {
    pub(crate) installation_id: InstallationID,
    pub(crate) element_id: String,
    pub(crate) selected: bool,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) health: &'static str,
    pub(crate) secondary: String,
    pub(crate) paths: Vec<String>,
    pub(crate) action: ModActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LibraryRowModel {
    pub(crate) package_id: ModPackageID,
    pub(crate) element_id: String,
    pub(crate) selected: bool,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) game: String,
    pub(crate) author: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) secondary: String,
    pub(crate) apply: ModActionAvailability,
    pub(crate) remove: ModActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackupRowModel {
    pub(crate) backup_id: BackupID,
    pub(crate) element_id: String,
    pub(crate) selected: bool,
    pub(crate) label: String,
    pub(crate) game: String,
    pub(crate) secondary: String,
    pub(crate) restore: ModActionAvailability,
    pub(crate) delete: ModActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModRowModel {
    Installed(InstalledRowModel),
    Library(LibraryRowModel),
    Backup(BackupRowModel),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModIssueRowModel {
    pub(crate) element_id: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) recovery_paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModCreateModel {
    pub(crate) selected_file_count: usize,
    pub(crate) select_files: ModActionAvailability,
    pub(crate) export: ModActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModConfirmationModel {
    pub(crate) title: String,
    pub(crate) consequence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModViewModel {
    pub(crate) game: String,
    pub(crate) configured_root: Option<String>,
    pub(crate) section: ModSection,
    pub(crate) sections: Vec<ModSectionModel>,
    pub(crate) content: ModContentState,
    pub(crate) rows: Vec<ModRowModel>,
    pub(crate) issues: Vec<ModIssueRowModel>,
    pub(crate) import_action: ModActionAvailability,
    pub(crate) create_backup_action: ModActionAvailability,
    pub(crate) create: ModCreateModel,
    pub(crate) active_operation: Option<&'static str>,
    pub(crate) pending_confirmation: Option<ModConfirmationModel>,
    pub(crate) progress: Option<ModProgressModel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModProgressModel {
    pub(crate) operation: &'static str,
    pub(crate) phase: &'static str,
    pub(crate) completed: u64,
    pub(crate) total: u64,
    pub(crate) path: Option<String>,
    pub(crate) can_cancel: bool,
    pub(crate) cancel_requested: bool,
}

pub(crate) fn project_mods(state: &ModPresentationState) -> ModViewModel {
    let section = state.section();
    let rows = match section {
        ModSection::Installed => installed_rows(state),
        ModSection::Library => library_rows(state),
        ModSection::Backups => backup_rows(state),
        ModSection::Create => Vec::new(),
    };
    let content = content_state(state, rows.is_empty());
    let operation_reason = state
        .active_operation()
        .map(|operation| format!("{} is already running.", operation.label()));
    let import_action = operation_reason
        .as_ref()
        .map_or_else(ModActionAvailability::enabled, |reason| {
            ModActionAvailability::disabled(reason.clone())
        });
    let create_draft = state.create_draft();
    let root_action = match (&state.root_state(), operation_reason.as_ref()) {
        (_, Some(reason)) => ModActionAvailability::disabled(reason.clone()),
        (ModRootState::Ready { .. }, None) => ModActionAvailability::enabled(),
        (_, None) => ModActionAvailability::disabled(root_requirement(state, ModSection::Create)),
    };
    let export = if !root_action.enabled {
        root_action.clone()
    } else if create_draft.name.trim().is_empty() {
        ModActionAvailability::disabled("Enter a package name.")
    } else if create_draft.version.trim().is_empty() {
        ModActionAvailability::disabled("Enter a package version.")
    } else if create_draft.files.is_empty() {
        ModActionAvailability::disabled("Select at least one game file.")
    } else {
        ModActionAvailability::enabled()
    };
    ModViewModel {
        game: state.game().label().to_owned(),
        configured_root: match state.root_state() {
            ModRootState::Ready {
                configured_root, ..
            } => Some(configured_root.clone()),
            ModRootState::Idle
            | ModRootState::Loading
            | ModRootState::MissingRoot
            | ModRootState::Failed(_) => None,
        },
        section,
        sections: ModSection::ALL
            .into_iter()
            .map(|candidate| ModSectionModel {
                section: candidate,
                element_id: candidate.element_id(),
                label: candidate.label(),
                selected: candidate == section,
            })
            .collect(),
        content,
        rows,
        issues: issue_rows(state),
        import_action,
        create_backup_action: root_action.clone(),
        create: ModCreateModel {
            selected_file_count: create_draft.files.len(),
            select_files: root_action,
            export,
        },
        active_operation: state
            .active_operation()
            .map(crate::mod_status::ModOperationKind::label),
        pending_confirmation: state.pending_confirmation().map(|confirmation| {
            ModConfirmationModel {
                title: format!(
                    "Confirm {} for {}",
                    confirmation.operation.label(),
                    confirmation.subject
                ),
                consequence: confirmation.consequence.clone(),
            }
        }),
        progress: state.progress().map(|progress| ModProgressModel {
            operation: progress.operation.label(),
            phase: progress_phase_label(progress.phase),
            completed: progress.completed,
            total: progress.total,
            path: progress.path.as_ref().map(|path| path.as_str().to_owned()),
            can_cancel: progress.can_cancel,
            cancel_requested: progress.cancel_requested,
        }),
    }
}

fn content_state(state: &ModPresentationState, rows_empty: bool) -> ModContentState {
    match state.section() {
        ModSection::Library => match state.library_state() {
            ModLibraryState::Idle => ModContentState::Idle {
                message: "The package library has not been scanned yet.",
            },
            ModLibraryState::Loading => ModContentState::Loading {
                message: "Scanning the package library…",
            },
            ModLibraryState::Ready(_) if rows_empty => ModContentState::Empty {
                title: "No packages in the library",
                next_action: "Import a ZIP package to add it to your library.",
            },
            ModLibraryState::Ready(_) => ModContentState::Ready,
            ModLibraryState::Failed(issue) => ModContentState::Failed {
                title: issue.title.clone(),
                detail: issue.detail.clone(),
            },
        },
        ModSection::Installed => root_content(
            state,
            rows_empty,
            "No mods installed for this game",
            "Import a package in Library, then apply it to this game.",
        ),
        ModSection::Backups => root_content(
            state,
            rows_empty,
            "No backups for this game",
            "Create a full backup before changing game files.",
        ),
        ModSection::Create => match state.root_state() {
            ModRootState::Idle => ModContentState::Idle {
                message: "Open Mods to inspect the configured game root.",
            },
            ModRootState::Loading => ModContentState::Loading {
                message: "Inspecting the configured game root…",
            },
            ModRootState::MissingRoot => ModContentState::MissingRoot {
                requirement: root_requirement(state, ModSection::Create),
            },
            ModRootState::Ready { .. } => ModContentState::Ready,
            ModRootState::Failed(issue) => ModContentState::Failed {
                title: issue.title.clone(),
                detail: issue.detail.clone(),
            },
        },
    }
}

fn root_content(
    state: &ModPresentationState,
    rows_empty: bool,
    empty_title: &'static str,
    next_action: &'static str,
) -> ModContentState {
    match state.root_state() {
        ModRootState::Idle => ModContentState::Idle {
            message: "The game root has not been scanned yet.",
        },
        ModRootState::Loading => ModContentState::Loading {
            message: "Scanning the configured game root…",
        },
        ModRootState::MissingRoot => ModContentState::MissingRoot {
            requirement: root_requirement(state, state.section()),
        },
        ModRootState::Ready { .. } if rows_empty => ModContentState::Empty {
            title: empty_title,
            next_action,
        },
        ModRootState::Ready { .. } => ModContentState::Ready,
        ModRootState::Failed(issue) => ModContentState::Failed {
            title: issue.title.clone(),
            detail: issue.detail.clone(),
        },
    }
}

fn installed_rows(state: &ModPresentationState) -> Vec<ModRowModel> {
    let ModRootState::Ready { installations, .. } = state.root_state() else {
        return Vec::new();
    };
    installations
        .rows
        .iter()
        .map(|installed| {
            ModRowModel::Installed(InstalledRowModel {
                installation_id: installed.installation_id,
                element_id: format!("mod-installed-{}", installed.installation_id),
                selected: state.selected_installation() == Some(installed.installation_id),
                name: installed.name.clone(),
                version: installed.version.clone(),
                health: installed_health(installed.status),
                secondary: format!(
                    "Installed {} · {}",
                    installed.installed_at,
                    count_label(
                        u64::try_from(installed.files.len()).unwrap_or(u64::MAX),
                        "file",
                        "files",
                    )
                ),
                paths: installed
                    .files
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect(),
                action: installed_action(state, installed),
            })
        })
        .collect()
}

fn installed_action(
    state: &ModPresentationState,
    installed: &InstalledModSnapshot,
) -> ModActionAvailability {
    if let Some(operation) = state.active_operation() {
        return ModActionAvailability::disabled(format!(
            "{} is already running.",
            operation.label()
        ));
    }
    match installed.status {
        Some(InstalledModStatus::Clean) => ModActionAvailability::enabled(),
        Some(InstalledModStatus::Modified) => {
            ModActionAvailability::disabled("Installed files changed.")
        }
        Some(InstalledModStatus::Missing) => {
            ModActionAvailability::disabled("Installed files are missing.")
        }
        None => ModActionAvailability::disabled("Health check failed."),
    }
}

fn library_rows(state: &ModPresentationState) -> Vec<ModRowModel> {
    let ModLibraryState::Ready(library) = state.library_state() else {
        return Vec::new();
    };
    library
        .rows
        .iter()
        .map(|package| {
            ModRowModel::Library(LibraryRowModel {
                package_id: package.package_id,
                element_id: format!("mod-library-{}", package.package_id),
                selected: state.selected_package() == Some(package.package_id),
                name: package.name.clone(),
                version: package.version.clone(),
                game: package.game.label().to_owned(),
                author: package.author.clone(),
                description: package.description.clone(),
                secondary: format!(
                    "{} · {} package · {} unpacked",
                    count_label(package.file_count, "file", "files"),
                    byte_count(package.compressed_bytes),
                    byte_count(package.uncompressed_bytes)
                ),
                apply: package_action(state, package),
                remove: package_removal_action(state, package),
            })
        })
        .collect()
}

fn package_action(
    state: &ModPresentationState,
    package: &ModPackageSnapshot,
) -> ModActionAvailability {
    if let Some(operation) = state.active_operation() {
        return ModActionAvailability::disabled(format!(
            "{} is already running.",
            operation.label()
        ));
    }
    let ModRootState::Ready { installations, .. } = state.root_state() else {
        return ModActionAvailability::disabled(root_requirement(state, ModSection::Library));
    };
    if package.game != state.game() {
        return ModActionAvailability::disabled(format!(
            "This package is for {}, not {}.",
            package.game.label(),
            state.game().label()
        ));
    }
    if installations
        .rows
        .iter()
        .any(|installed| installed.name.eq_ignore_ascii_case(&package.name))
    {
        return ModActionAvailability::disabled("An installed mod already uses this package name.");
    }
    for package_path in &package.files {
        if installations.rows.iter().any(|installed| {
            installed
                .files
                .iter()
                .any(|path| path.portable_key() == package_path.portable_key())
        }) {
            return ModActionAvailability::disabled(format!(
                "An installed mod already owns {}.",
                package_path.as_str()
            ));
        }
    }
    ModActionAvailability::enabled()
}

fn package_removal_action(
    state: &ModPresentationState,
    package: &ModPackageSnapshot,
) -> ModActionAvailability {
    if let Some(operation) = state.active_operation() {
        return ModActionAvailability::disabled(format!(
            "{} is already running.",
            operation.label()
        ));
    }
    if let ModRootState::Ready { installations, .. } = state.root_state()
        && installations
            .rows
            .iter()
            .any(|installed| installed.package_id == package.package_id)
    {
        return ModActionAvailability::disabled(
            "Uninstall this package before removing it from the library.",
        );
    }
    ModActionAvailability::enabled()
}

fn backup_rows(state: &ModPresentationState) -> Vec<ModRowModel> {
    let ModRootState::Ready { backups, .. } = state.root_state() else {
        return Vec::new();
    };
    backups
        .rows
        .iter()
        .map(|backup| ModRowModel::Backup(backup_row(state, backup)))
        .collect()
}

fn backup_row(state: &ModPresentationState, backup: &BackupSnapshot) -> BackupRowModel {
    let action =
        state
            .active_operation()
            .map_or_else(ModActionAvailability::enabled, |operation| {
                ModActionAvailability::disabled(format!(
                    "{} is already running.",
                    operation.label()
                ))
            });
    BackupRowModel {
        backup_id: backup.backup_id,
        element_id: format!("mod-backup-{}", backup.backup_id),
        selected: state.selected_backup() == Some(backup.backup_id),
        label: backup
            .label
            .clone()
            .unwrap_or_else(|| format!("Backup {}", short_id(&backup.backup_id.to_string()))),
        game: backup.game.label().to_owned(),
        secondary: format!(
            "{} · {} · {}",
            backup.created_at,
            count_label(backup.file_count, "file", "files"),
            byte_count(backup.total_bytes)
        ),
        restore: action.clone(),
        delete: action,
    }
}

fn issue_rows(state: &ModPresentationState) -> Vec<ModIssueRowModel> {
    let section = state.section();
    let mut issues = Vec::new();
    match state.library_state() {
        ModLibraryState::Ready(library) => issues.extend(library.issues.iter()),
        ModLibraryState::Failed(issue) => issues.push(issue),
        ModLibraryState::Idle | ModLibraryState::Loading => {}
    }
    match state.root_state() {
        ModRootState::Ready {
            installations,
            backups,
            ..
        } => match section {
            ModSection::Installed => issues.extend(installations.issues.iter()),
            ModSection::Backups => issues.extend(backups.issues.iter()),
            ModSection::Library | ModSection::Create => {}
        },
        ModRootState::Failed(issue) => issues.push(issue),
        ModRootState::Idle | ModRootState::Loading | ModRootState::MissingRoot => {}
    }
    issues.extend(state.operation_issues());
    issues
        .into_iter()
        .filter(|issue| issue.scope.belongs_to(section))
        .map(issue_row)
        .collect()
}

fn issue_row(issue: &ModIssueSnapshot) -> ModIssueRowModel {
    ModIssueRowModel {
        element_id: format!("mod-issue-{}", issue.identity),
        title: issue.title.clone(),
        detail: issue.detail.clone(),
        recovery_paths: issue.recovery_paths.clone(),
    }
}

fn root_requirement(state: &ModPresentationState, section: ModSection) -> String {
    let action = match section {
        ModSection::Installed => "view installed mods",
        ModSection::Library => "apply this package",
        ModSection::Backups => "manage backups",
        ModSection::Create => "create a package",
    };
    format!(
        "Configure the {} game folder in Settings to {action}.",
        state.game().label()
    )
}

const fn installed_health(status: Option<InstalledModStatus>) -> &'static str {
    match status {
        Some(InstalledModStatus::Clean) => "Clean",
        Some(InstalledModStatus::Modified) => "Modified",
        Some(InstalledModStatus::Missing) => "Missing files",
        None => "Health unavailable",
    }
}

fn count_label(count: u64, singular: &str, plural: &str) -> String {
    let label = if count == 1 { singular } else { plural };
    format!("{count} {label}")
}

fn byte_count(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        format!("{} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

const fn progress_phase_label(phase: ModProgressPhase) -> &'static str {
    match phase {
        ModProgressPhase::InspectingPackage => "Inspecting package",
        ModProgressPhase::CopyingPackage => "Copying package",
        ModProgressPhase::CreatingPackage => "Creating package",
        ModProgressPhase::PublishingPackage => "Publishing package",
        ModProgressPhase::PlanningApply => "Planning apply",
        ModProgressPhase::StagingFiles => "Staging files",
        ModProgressPhase::CreatingRecovery => "Creating recovery",
        ModProgressPhase::CommittingFiles => "Committing files",
        ModProgressPhase::PublishingInstallation => "Publishing installation",
        ModProgressPhase::PlanningUninstall => "Planning uninstall",
        ModProgressPhase::StagingUninstall => "Staging uninstall",
        ModProgressPhase::RestoringFiles => "Restoring files",
        ModProgressPhase::PublishingUninstall => "Publishing uninstall",
        ModProgressPhase::ScanningBackup => "Scanning backup",
        ModProgressPhase::CopyingBackup => "Copying backup",
        ModProgressPhase::PublishingBackup => "Publishing backup",
        ModProgressPhase::StagingBackupRestore => "Staging backup restore",
        ModProgressPhase::CreatingRestoreRecovery => "Creating restore recovery",
        ModProgressPhase::RestoringBackup => "Restoring backup",
        ModProgressPhase::RollingBack => "Rolling back",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModControlAction {
    SelectSection(ModSection),
    Refresh,
    ImportPackage,
    SelectInstallation(InstallationID),
    Uninstall(InstallationID),
    SelectPackage(ModPackageID),
    Apply(ModPackageID),
    RemovePackage(ModPackageID),
    SelectBackup(BackupID),
    RestoreBackup(BackupID),
    DeleteBackup(BackupID),
    CreateBackup,
    SelectCreateFiles,
    ExportPackage,
    DismissOrCancel,
    ConfirmOperation,
}

impl ModControlAction {
    fn activate(self, frame: &mut AppFrame, cx: &mut Context<AppFrame>) {
        match self {
            Self::SelectSection(section) => frame.select_mod_section(section, cx),
            Self::Refresh => frame.start_mod_scan(cx),
            Self::ImportPackage => frame.import_mod_package(cx),
            Self::SelectInstallation(installation_id) => {
                frame.select_mod_installation(installation_id, cx);
            }
            Self::Uninstall(installation_id) => {
                frame.request_mod_uninstall(installation_id, cx);
            }
            Self::SelectPackage(package_id) => frame.select_mod_package(package_id, cx),
            Self::Apply(package_id) => frame.request_mod_apply(package_id, cx),
            Self::RemovePackage(package_id) => {
                frame.request_mod_package_removal(package_id, cx);
            }
            Self::SelectBackup(backup_id) => frame.select_mod_backup(backup_id, cx),
            Self::RestoreBackup(backup_id) => {
                frame.request_mod_backup_restore(backup_id, cx);
            }
            Self::DeleteBackup(backup_id) => {
                frame.request_mod_backup_deletion(backup_id, cx);
            }
            Self::CreateBackup => frame.create_mod_backup(cx),
            Self::SelectCreateFiles => frame.select_mod_create_files(cx),
            Self::ExportPackage => frame.export_mod_package(cx),
            Self::DismissOrCancel => frame.dismiss_or_cancel_mod_operation(cx),
            Self::ConfirmOperation => frame.confirm_mod_operation(cx),
        }
    }
}

pub(crate) fn render(
    theme: &Theme,
    model: &ModViewModel,
    initial_focus: &FocusHandle,
    inputs: &ModFormInputs,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    div()
        .id("mods-route")
        .debug_selector(|| "mods-route".to_owned())
        .size_full()
        .flex()
        .bg(theme.background)
        .child(section_rail(theme, model, initial_focus, cx))
        .child(route_body(theme, model, inputs, cx))
}

fn section_rail(
    theme: &Theme,
    model: &ModViewModel,
    initial_focus: &FocusHandle,
    cx: &mut Context<AppFrame>,
) -> Div {
    let section_items = model
        .sections
        .iter()
        .map(|section| {
            let target = section.section;
            let selector = section.element_id.to_owned();
            let item =
                components::rail_item(theme, section.element_id, section.label, section.selected)
                    .debug_selector(move || selector)
                    .tab_index(0)
                    .focus(move |style| {
                        style
                            .border_color(theme.accent)
                            .bg(theme.accent_dim)
                            .text_color(theme.accent)
                    })
                    .when(target == ModSection::Installed, |item| {
                        item.track_focus(initial_focus)
                    });
            bind_mod_control(item, ModControlAction::SelectSection(target), cx)
        })
        .collect::<Vec<_>>();
    let root = model
        .configured_root
        .as_deref()
        .unwrap_or("Game root not configured");
    div()
        .w(px(182.0))
        .flex_none()
        .p(px(14.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .bg(theme.surface)
        .border_r_1()
        .border_color(theme.border)
        .child(
            div()
                .px(px(12.0))
                .pt(px(5.0))
                .pb(px(9.0))
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child("MOD WORKSHOP"),
        )
        .children(section_items)
        .child(div().flex_1())
        .child(
            div()
                .px(px(12.0))
                .py(px(9.0))
                .border_t_1()
                .border_color(theme.border)
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.text)
                        .child(model.game.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.text_dim)
                        .text_ellipsis()
                        .child(root.to_owned()),
                ),
        )
}

fn route_body(
    theme: &Theme,
    model: &ModViewModel,
    inputs: &ModFormInputs,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let rows = model
        .rows
        .iter()
        .map(|row| render_row(theme, row, cx))
        .collect::<Vec<_>>();
    let issues = model
        .issues
        .iter()
        .map(|issue| render_issue(theme, issue))
        .collect::<Vec<_>>();
    div()
        .id("mods-scroll")
        .debug_selector(|| "mods-scroll".to_owned())
        .flex_1()
        .min_w_0()
        .min_h_0()
        .overflow_y_scroll()
        .p(px(28.0))
        .child(
            div()
                .w_full()
                .max_w(px(920.0))
                .mx_auto()
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(route_header(theme, model, cx))
                .children(
                    model
                        .pending_confirmation
                        .as_ref()
                        .map(|confirmation| confirmation_panel(theme, confirmation, cx)),
                )
                .children(
                    model
                        .progress
                        .as_ref()
                        .map(|progress| progress_panel(theme, progress, cx)),
                )
                .children(
                    (model.section == ModSection::Backups)
                        .then(|| backup_creation_panel(theme, model, inputs, cx)),
                )
                .child(render_content(theme, model, rows, inputs, cx))
                .children((!issues.is_empty()).then(|| {
                    components::surface(theme)
                        .w_full()
                        .p(px(18.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.accent)
                                .child("SCAN ISSUES"),
                        )
                        .children(issues)
                })),
        )
}

fn route_header(theme: &Theme, model: &ModViewModel, cx: &mut Context<AppFrame>) -> Div {
    let import = mod_action_button(
        theme,
        "mods-import-package".to_owned(),
        "Import ZIP",
        &model.import_action,
        ModControlAction::ImportPackage,
        cx,
    );
    let refresh = mod_action_button(
        theme,
        "mods-refresh".to_owned(),
        "Refresh",
        &ModActionAvailability::enabled(),
        ModControlAction::Refresh,
        cx,
    );
    div()
        .w_full()
        .flex()
        .items_end()
        .justify_between()
        .gap(px(18.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(
                    div()
                        .text_size(px(26.0))
                        .text_color(theme.text)
                        .child(model.section.label()),
                )
                .child(
                    div()
                        .text_color(theme.text_dim)
                        .child(section_description(model.section)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .children(model.active_operation.map(|operation| {
                    div()
                        .px(px(9.0))
                        .py(px(5.0))
                        .rounded_md()
                        .bg(theme.accent_dim)
                        .text_size(px(12.0))
                        .text_color(theme.accent)
                        .child(operation)
                }))
                .child(refresh)
                .child(import),
        )
}

fn confirmation_panel(
    theme: &Theme,
    confirmation: &ModConfirmationModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    components::surface(theme)
        .id("mods-confirmation")
        .debug_selector(|| "mods-confirmation".to_owned())
        .w_full()
        .p(px(16.0))
        .border_color(theme.accent)
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_color(theme.text)
                .text_size(px(16.0))
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
                .child(mod_action_button(
                    theme,
                    "mods-confirmation-dismiss".to_owned(),
                    "Cancel",
                    &ModActionAvailability::enabled(),
                    ModControlAction::DismissOrCancel,
                    cx,
                ))
                .child(mod_action_button(
                    theme,
                    "mods-confirmation-accept".to_owned(),
                    "Confirm",
                    &ModActionAvailability::enabled(),
                    ModControlAction::ConfirmOperation,
                    cx,
                )),
        )
}

fn progress_panel(
    theme: &Theme,
    progress: &ModProgressModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let cancel = if progress.can_cancel {
        ModActionAvailability::enabled()
    } else if progress.cancel_requested {
        ModActionAvailability::disabled("Cancellation was requested.")
    } else {
        ModActionAvailability::disabled("This phase must finish without interruption.")
    };
    components::surface(theme)
        .id("mods-progress")
        .debug_selector(|| "mods-progress".to_owned())
        .w_full()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .justify_between()
                .child(div().text_color(theme.text).child(progress.operation))
                .child(
                    div()
                        .text_color(theme.accent)
                        .child(format!("{} / {}", progress.completed, progress.total)),
                ),
        )
        .child(div().text_color(theme.text_dim).child(progress.phase))
        .children(progress.path.as_ref().map(|path| {
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(path.clone())
        }))
        .children(progress.cancel_requested.then(|| {
            div()
                .text_size(px(12.0))
                .text_color(theme.accent)
                .child("Cancellation requested")
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(mod_action_button(
                    theme,
                    "mods-progress-cancel".to_owned(),
                    "Cancel operation",
                    &cancel,
                    ModControlAction::DismissOrCancel,
                    cx,
                ))
                .children(cancel.reason.map(|reason| {
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.text_dim)
                        .child(reason)
                })),
        )
}

fn backup_creation_panel(
    theme: &Theme,
    model: &ModViewModel,
    inputs: &ModFormInputs,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    components::surface(theme)
        .id("mods-backup-create")
        .debug_selector(|| "mods-backup-create".to_owned())
        .w_full()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(status_title(theme, "Create a full backup"))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child("Optional label"),
        )
        .child(inputs.backup_label.clone())
        .child(mod_action_line(
            theme,
            "mods-backup-create-action".to_owned(),
            "Create backup",
            &model.create_backup_action,
            ModControlAction::CreateBackup,
            cx,
        ))
}

fn render_content(
    theme: &Theme,
    model: &ModViewModel,
    rows: Vec<AnyElement>,
    inputs: &ModFormInputs,
    cx: &mut Context<AppFrame>,
) -> Div {
    let surface = components::surface(theme)
        .w_full()
        .p(px(20.0))
        .flex()
        .flex_col()
        .gap(px(12.0));
    match &model.content {
        ModContentState::Idle { message } | ModContentState::Loading { message } => surface.child(
            div()
                .py(px(24.0))
                .text_color(theme.text_dim)
                .child(*message),
        ),
        ModContentState::MissingRoot { requirement } => surface
            .child(status_title(theme, "Game root required"))
            .child(div().text_color(theme.text_dim).child(requirement.clone())),
        ModContentState::Empty { title, next_action } => surface
            .child(status_title(theme, title))
            .child(div().text_color(theme.text_dim).child(*next_action)),
        ModContentState::Failed { title, detail } => surface
            .border_color(theme.accent)
            .child(status_title(theme, title))
            .child(div().text_color(theme.text_dim).child(detail.clone())),
        ModContentState::Ready if model.section == ModSection::Create => {
            render_create(theme, &model.create, inputs, cx)
        }
        ModContentState::Ready => surface.children(rows),
    }
}

fn render_row(theme: &Theme, row: &ModRowModel, cx: &mut Context<AppFrame>) -> AnyElement {
    match row {
        ModRowModel::Installed(row) => render_installed_row(theme, row, cx).into_any_element(),
        ModRowModel::Library(row) => render_library_row(theme, row, cx).into_any_element(),
        ModRowModel::Backup(row) => render_backup_row(theme, row, cx).into_any_element(),
    }
}

fn render_installed_row(
    theme: &Theme,
    row: &InstalledRowModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let paths = row.paths.iter().take(3).cloned().collect::<Vec<_>>();
    bind_mod_control(
        row_shell(theme, &row.element_id, row.selected),
        ModControlAction::SelectInstallation(row.installation_id),
        cx,
    )
    .child(row_heading(theme, &row.name, &row.version, row.health))
    .child(
        div()
            .text_color(theme.text_dim)
            .child(row.secondary.clone()),
    )
    .children(paths.into_iter().map(|path| {
        div()
            .text_size(px(12.0))
            .text_color(theme.text_dim)
            .child(path)
    }))
    .child(mod_action_line(
        theme,
        format!("{}-uninstall", row.element_id),
        "Uninstall",
        &row.action,
        ModControlAction::Uninstall(row.installation_id),
        cx,
    ))
}

fn render_library_row(
    theme: &Theme,
    row: &LibraryRowModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    bind_mod_control(
        row_shell(theme, &row.element_id, row.selected),
        ModControlAction::SelectPackage(row.package_id),
        cx,
    )
    .child(row_heading(theme, &row.name, &row.version, &row.game))
    .child(
        div()
            .text_color(theme.text_dim)
            .child(row.secondary.clone()),
    )
    .children(row.author.as_ref().map(|author| {
        div()
            .text_size(px(12.0))
            .text_color(theme.text_dim)
            .child(format!("By {author}"))
    }))
    .children(row.description.as_ref().map(|description| {
        div()
            .text_size(px(12.0))
            .text_color(theme.text_dim)
            .child(description.clone())
    }))
    .child(
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(mod_action_line(
                theme,
                format!("{}-apply", row.element_id),
                "Apply",
                &row.apply,
                ModControlAction::Apply(row.package_id),
                cx,
            ))
            .child(mod_action_line(
                theme,
                format!("{}-remove", row.element_id),
                "Remove from library",
                &row.remove,
                ModControlAction::RemovePackage(row.package_id),
                cx,
            )),
    )
}

fn render_backup_row(
    theme: &Theme,
    row: &BackupRowModel,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    bind_mod_control(
        row_shell(theme, &row.element_id, row.selected),
        ModControlAction::SelectBackup(row.backup_id),
        cx,
    )
    .child(row_heading(theme, &row.label, "", &row.game))
    .child(
        div()
            .text_color(theme.text_dim)
            .child(row.secondary.clone()),
    )
    .child(
        div()
            .flex()
            .gap(px(8.0))
            .child(mod_action_button(
                theme,
                format!("{}-restore", row.element_id),
                "Restore",
                &row.restore,
                ModControlAction::RestoreBackup(row.backup_id),
                cx,
            ))
            .child(mod_action_button(
                theme,
                format!("{}-delete", row.element_id),
                "Delete",
                &row.delete,
                ModControlAction::DeleteBackup(row.backup_id),
                cx,
            )),
    )
}

fn row_shell(theme: &Theme, id: &str, selected: bool) -> Stateful<Div> {
    let selector = id.to_owned();
    let hover = theme.surface;
    div()
        .id(SharedString::from(id.to_owned()))
        .debug_selector(move || selector)
        .tab_index(0)
        .w_full()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(7.0))
        .rounded_md()
        .border_1()
        .border_color(if selected { theme.accent } else { theme.border })
        .bg(if selected {
            theme.accent_dim
        } else {
            theme.raised
        })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .focus(move |style| style.border_color(theme.accent).bg(theme.accent_dim))
}

fn row_heading(theme: &Theme, name: &str, version: &str, badge: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().text_color(theme.text).child(if version.is_empty() {
            name.to_owned()
        } else {
            format!("{name} {version}")
        }))
        .child(
            div()
                .px(px(7.0))
                .py(px(2.0))
                .rounded_md()
                .bg(theme.surface)
                .text_size(px(11.0))
                .text_color(theme.accent)
                .child(badge.to_owned()),
        )
}

fn mod_action_line(
    theme: &Theme,
    id: String,
    label: &'static str,
    action: &ModActionAvailability,
    command: ModControlAction,
    cx: &mut Context<AppFrame>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(mod_action_button(theme, id, label, action, command, cx))
        .children(action.reason.as_ref().map(|reason| {
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(reason.clone())
        }))
}

fn availability_button(
    theme: &Theme,
    id: String,
    label: &'static str,
    action: &ModActionAvailability,
) -> Stateful<Div> {
    let hover = theme.raised;
    let selector = id.clone();
    div()
        .id(SharedString::from(id))
        .debug_selector(move || selector)
        .h(px(32.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .text_color(if action.enabled {
            theme.text
        } else {
            theme.text_dim
        })
        .when(action.enabled, move |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .focus(move |style| style.border_color(theme.accent).bg(theme.accent_dim))
        })
        .when(!action.enabled, |button| button.opacity(0.45))
        .child(label)
}

fn mod_action_button(
    theme: &Theme,
    id: String,
    label: &'static str,
    availability: &ModActionAvailability,
    command: ModControlAction,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    let button = availability_button(theme, id, label, availability);
    if availability.enabled {
        bind_mod_control(button, command, cx)
    } else {
        button
    }
}

fn bind_mod_control(
    control: Stateful<Div>,
    command: ModControlAction,
    cx: &mut Context<AppFrame>,
) -> Stateful<Div> {
    control.on_click(cx.listener(move |frame, _, _, cx| command.activate(frame, cx)))
}

fn render_create(
    theme: &Theme,
    model: &ModCreateModel,
    inputs: &ModFormInputs,
    cx: &mut Context<AppFrame>,
) -> Div {
    let fields = [
        ("Name", inputs.name.clone()),
        ("Version", inputs.version.clone()),
        ("Author", inputs.author.clone()),
        ("Description", inputs.description.clone()),
    ];
    components::surface(theme)
        .w_full()
        .p(px(20.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .children(fields.into_iter().map(|(label, input)| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.text_dim)
                        .child(label),
                )
                .child(input)
        }))
        .child(
            div()
                .text_color(theme.text_dim)
                .child(format!("{} selected files", model.selected_file_count)),
        )
        .child(mod_action_line(
            theme,
            "mods-create-select-files".to_owned(),
            "Select game files",
            &model.select_files,
            ModControlAction::SelectCreateFiles,
            cx,
        ))
        .child(mod_action_line(
            theme,
            "mods-create-export".to_owned(),
            "Export package",
            &model.export,
            ModControlAction::ExportPackage,
            cx,
        ))
}

fn render_issue(theme: &Theme, issue: &ModIssueRowModel) -> Stateful<Div> {
    div()
        .id(SharedString::from(issue.element_id.clone()))
        .w_full()
        .p(px(12.0))
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised)
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(div().text_color(theme.text).child(issue.title.clone()))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(theme.text_dim)
                .child(issue.detail.clone()),
        )
        .children(issue.recovery_paths.iter().map(|path| {
            div()
                .text_size(px(12.0))
                .text_color(theme.accent)
                .child(path.clone())
        }))
}

fn status_title(theme: &Theme, title: &str) -> Div {
    div()
        .text_size(px(17.0))
        .text_color(theme.text)
        .child(title.to_owned())
}

const fn section_description(section: ModSection) -> &'static str {
    match section {
        ModSection::Installed => "Installed packages and current file health.",
        ModSection::Library => "Validated packages available to either game.",
        ModSection::Backups => "Full snapshots for the active game root.",
        ModSection::Create => "Build a portable package from selected game files.",
    }
}

#[cfg(test)]
mod tests {
    use kufeditor_game::Game;
    use kufeditor_mods::{
        BackupID, InstallationID, InstalledModStatus, ModLimits, ModPackageID, OperationID,
        RelativeGamePath,
    };

    use super::{ModContentState, ModRowModel, project_mods};
    use crate::mod_status::{
        BackupSnapshot, InstalledModSnapshot, ModCollectionSnapshot, ModIssueScope,
        ModIssueSnapshot, ModPackageSnapshot, ModPresentationState, ModRootCompletion,
        ModScanCompletion, ModScanScope, ModSection,
    };

    #[test]
    fn mods_projection_names_loading_empty_missing_root_failed_and_create_states() {
        let mut state = ModPresentationState::default();
        assert!(matches!(
            project_mods(&state).content,
            ModContentState::Idle { .. }
        ));

        let missing_key = state.begin_scan(ModScanScope::Full, false);
        assert!(matches!(
            project_mods(&state).content,
            ModContentState::MissingRoot { .. }
        ));
        state.select_section(ModSection::Library);
        let loading = project_mods(&state);
        assert!(matches!(loading.content, ModContentState::Loading { .. }));
        assert!(loading.import_action.enabled);
        assert!(state.finish_scan(
            missing_key,
            ModScanCompletion::new(
                Ok(ModCollectionSnapshot::new(Vec::new(), Vec::new())),
                ModRootCompletion::MissingRoot,
            ),
        ));
        let empty_library = project_mods(&state);
        assert!(matches!(
            empty_library.content,
            ModContentState::Empty {
                next_action: "Import a ZIP package to add it to your library.",
                ..
            }
        ));

        let failed_key = state.begin_scan(ModScanScope::LibraryOnly, false);
        assert!(state.finish_scan(
            failed_key,
            ModScanCompletion::new(
                Err(issue(
                    ModIssueScope::Library,
                    "library",
                    "Could not scan the mod library",
                )),
                ModRootCompletion::NotRequested,
            ),
        ));
        assert!(matches!(
            project_mods(&state).content,
            ModContentState::Failed { .. }
        ));

        let ready_key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(
            ready_key,
            ready_completion(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        ));
        state.select_section(ModSection::Installed);
        assert!(matches!(
            project_mods(&state).content,
            ModContentState::Empty {
                next_action: "Import a package in Library, then apply it to this game.",
                ..
            }
        ));
        state.select_section(ModSection::Backups);
        assert!(matches!(
            project_mods(&state).content,
            ModContentState::Empty {
                next_action: "Create a full backup before changing game files.",
                ..
            }
        ));
        state.select_section(ModSection::Create);
        assert!(matches!(
            project_mods(&state).content,
            ModContentState::Ready
        ));
    }

    #[test]
    fn mods_projection_keeps_installed_ids_health_and_action_reasons() {
        let state = action_state();
        let installed_model = project_mods(&state);
        let installed = installed_model
            .rows
            .iter()
            .filter_map(|row| match row {
                ModRowModel::Installed(row) => Some(row),
                ModRowModel::Library(_) | ModRowModel::Backup(_) => None,
            })
            .collect::<Vec<_>>();
        let [clean, modified, missing, unavailable] = installed.as_slice() else {
            panic!("expected four installed rows");
        };
        assert_eq!(clean.element_id, format!("mod-installed-{}", id('1')));
        assert!(clean.action.enabled);
        assert_eq!(
            modified.action.reason.as_deref(),
            Some("Installed files changed.")
        );
        assert_eq!(
            missing.action.reason.as_deref(),
            Some("Installed files are missing.")
        );
        assert_eq!(
            unavailable.action.reason.as_deref(),
            Some("Health check failed.")
        );
    }

    #[test]
    fn mods_projection_explains_library_conflicts_and_keeps_backup_ids() {
        let mut state = action_state();
        state.select_section(ModSection::Library);
        let library_model = project_mods(&state);
        let library = library_model
            .rows
            .iter()
            .filter_map(|row| match row {
                ModRowModel::Library(row) => Some(row),
                ModRowModel::Installed(_) | ModRowModel::Backup(_) => None,
            })
            .collect::<Vec<_>>();
        let [wrong_game, duplicate_name, path_clash, ready] = library.as_slice() else {
            panic!("expected four library rows");
        };
        assert!(action_reason(wrong_game).contains("Heroes"));
        assert!(action_reason(duplicate_name).contains("name"));
        assert!(
            action_reason(path_clash)
                .to_ascii_lowercase()
                .contains("shared.sox")
        );
        assert!(ready.apply.enabled);
        assert!(ready.remove.enabled);
        assert_eq!(ready.author.as_deref(), Some("Forgeworks"));
        assert_eq!(
            ready.description.as_deref(),
            Some("Built for projection tests.")
        );
        assert!(ready.secondary.contains("8 B package"));
        assert!(ready.secondary.contains("16 B unpacked"));

        state.select_section(ModSection::Backups);
        let backup_model = project_mods(&state);
        let Some(ModRowModel::Backup(backup)) = backup_model.rows.first() else {
            panic!("expected one backup row");
        };
        assert_eq!(backup.element_id, format!("mod-backup-{}", id('9')));
        assert_eq!(backup.game, "Crusaders");
        assert!(backup.restore.enabled);
        assert!(backup.delete.enabled);
    }

    #[test]
    fn mods_projection_keeps_scan_issues_and_recovery_paths() {
        let mut state = ModPresentationState::default();
        let mut recovery_issue = issue(
            ModIssueScope::Installed,
            "operation",
            "Apply failed and was rolled back",
        );
        recovery_issue.recovery_paths = vec!["Data/a.sox".to_owned(), "Data/b.sox".to_owned()];
        let key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(
            key,
            ready_completion(Vec::new(), Vec::new(), Vec::new(), vec![recovery_issue]),
        ));

        let model = project_mods(&state);
        assert_eq!(model.issues.len(), 1);
        let issue = model.issues.first().expect("expected the recovery issue");
        assert_eq!(issue.element_id, "mod-issue-operation");
        assert_eq!(issue.recovery_paths, ["Data/a.sox", "Data/b.sox"]);
        assert!(issue.detail.contains("rolled back"));
    }

    fn action_state() -> ModPresentationState {
        let mut state = ModPresentationState::default();
        let installed_package = package('a', "Alpha", Game::Crusaders, &["shared.sox"]);
        let installed_rows = vec![
            installed('1', &installed_package, Some(InstalledModStatus::Clean)),
            installed(
                '2',
                &package('b', "Modified", Game::Crusaders, &["modified.sox"]),
                Some(InstalledModStatus::Modified),
            ),
            installed(
                '3',
                &package('c', "Missing", Game::Crusaders, &["missing.sox"]),
                Some(InstalledModStatus::Missing),
            ),
            installed(
                '4',
                &package('d', "Unknown", Game::Crusaders, &["unknown.sox"]),
                None,
            ),
        ];
        let packages = vec![
            package('5', "Wrong game", Game::Heroes, &["wrong.sox"]),
            package('6', "alpha", Game::Crusaders, &["duplicate.sox"]),
            package('7', "Path clash", Game::Crusaders, &["SHARED.SOX"]),
            package('8', "Ready", Game::Crusaders, &["ready.sox"]),
        ];
        let key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(
            key,
            ready_completion(
                packages,
                installed_rows,
                vec![backup('9', "Before overhaul")],
                Vec::new(),
            ),
        ));
        state
    }

    fn action_reason(row: &super::LibraryRowModel) -> &str {
        row.apply
            .reason
            .as_deref()
            .expect("expected a disabled-action reason")
    }

    fn ready_completion(
        packages: Vec<ModPackageSnapshot>,
        installations: Vec<InstalledModSnapshot>,
        backups: Vec<BackupSnapshot>,
        installation_issues: Vec<ModIssueSnapshot>,
    ) -> ModScanCompletion {
        ModScanCompletion::new(
            Ok(ModCollectionSnapshot::new(packages, Vec::new())),
            ModRootCompletion::Ready {
                configured_root: "/game".into(),
                installations: ModCollectionSnapshot::new(installations, installation_issues),
                backups: ModCollectionSnapshot::new(backups, Vec::new()),
            },
        )
    }

    fn package(digit: char, name: &str, game: Game, files: &[&str]) -> ModPackageSnapshot {
        ModPackageSnapshot {
            package_id: ModPackageID::parse(&id(digit)).unwrap(),
            name: name.to_owned(),
            version: "1.0".to_owned(),
            author: Some("Forgeworks".to_owned()),
            description: Some("Built for projection tests.".to_owned()),
            game,
            files: files
                .iter()
                .map(|path| RelativeGamePath::parse(path, &ModLimits::default()).unwrap())
                .collect(),
            compressed_bytes: 8,
            uncompressed_bytes: 16,
            file_count: u64::try_from(files.len()).unwrap(),
        }
    }

    fn installed(
        digit: char,
        package: &ModPackageSnapshot,
        status: Option<InstalledModStatus>,
    ) -> InstalledModSnapshot {
        InstalledModSnapshot {
            installation_id: InstallationID::parse(&id(digit)).unwrap(),
            package_id: package.package_id,
            operation_id: OperationID::parse(&id('e')).unwrap(),
            name: package.name.clone(),
            version: package.version.clone(),
            game: package.game,
            installed_at: "2026-08-26T12:00:00Z".to_owned(),
            status,
            files: package.files.clone(),
        }
    }

    fn backup(digit: char, label: &str) -> BackupSnapshot {
        BackupSnapshot {
            backup_id: BackupID::parse(&id(digit)).unwrap(),
            label: Some(label.to_owned()),
            game: Game::Crusaders,
            created_at: "2026-08-26T12:00:00Z".to_owned(),
            file_count: 2,
            total_bytes: 1_536,
        }
    }

    fn issue(scope: ModIssueScope, identity: &str, detail: &str) -> ModIssueSnapshot {
        ModIssueSnapshot {
            scope,
            identity: identity.to_owned(),
            title: "Scan issue".to_owned(),
            detail: detail.to_owned(),
            recovery_paths: Vec::new(),
        }
    }

    fn id(digit: char) -> String {
        digit.to_string().repeat(64)
    }
}
