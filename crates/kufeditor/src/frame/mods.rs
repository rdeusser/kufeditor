use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    ops::ControlFlow,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use async_channel::{Receiver, Sender, TrySendError};
use gpui::{AppContext, Context, Entity, PathPromptOptions, Task};
use kufeditor_game::Game;
use kufeditor_mods::{
    ApplyModRequest, BackupID, BackupScan, CreateBackupRequest, CreateModRequest, GameRoot,
    ImportedModDisposition, InstallationID, InstallationScan, ModError, ModLibraryScan, ModLimits,
    ModMetadata, ModPackageID, ModProgress, ModProgressPhase, ModProgressReporter, ModService,
    ModStorePaths, RelativeGamePath, RestoreBackupRequest, UninstallModRequest,
};

use super::AppFrame;
use crate::{
    actions::{CancelModOperation, FocusNextModControl, FocusPreviousModControl},
    mod_status::{
        BackupSnapshot, InstalledModSnapshot, ModCollectionSnapshot, ModContextChange,
        ModCreateField, ModIssueScope, ModIssueSnapshot, ModLibraryState, ModOperationKind,
        ModOperationLaunch, ModOperationTarget, ModPackageSnapshot, ModPromptKind, ModRequestKey,
        ModRootCompletion, ModRootState, ModScanCompletion, ModScanScope, ModSection,
        progress_phase_allows_cancel,
    },
    notices::{Notice, NoticeSource},
    state::Area,
    text_input::{TextInput, TextInputColors, TextInputEvent},
    theme::Theme,
};

pub(crate) struct ModFormInputs {
    pub(crate) name: Entity<TextInput>,
    pub(crate) version: Entity<TextInput>,
    pub(crate) author: Entity<TextInput>,
    pub(crate) description: Entity<TextInput>,
    pub(crate) backup_label: Entity<TextInput>,
}

impl ModFormInputs {
    pub(crate) fn new(theme: &Theme, cx: &mut Context<AppFrame>) -> Self {
        let colors = TextInputColors {
            background: theme.raised,
            border: theme.border,
            text: theme.text,
            placeholder: theme.text_dim,
            selection: theme.accent_dim,
            cursor: theme.accent,
        };
        let name = mod_input("Package name", "mods-create-name-input", colors, cx);
        let version = mod_input("Package version", "mods-create-version-input", colors, cx);
        let author = mod_input("Optional author", "mods-create-author-input", colors, cx);
        let description = mod_input(
            "Optional description",
            "mods-create-description-input",
            colors,
            cx,
        );
        let backup_label = mod_input(
            "Optional backup label",
            "mods-backup-label-input",
            colors,
            cx,
        );
        subscribe_mod_input(&name, ModCreateField::Name, cx);
        subscribe_mod_input(&version, ModCreateField::Version, cx);
        subscribe_mod_input(&author, ModCreateField::Author, cx);
        subscribe_mod_input(&description, ModCreateField::Description, cx);
        subscribe_mod_input(&backup_label, ModCreateField::BackupLabel, cx);
        Self {
            name,
            version,
            author,
            description,
            backup_label,
        }
    }
}

fn mod_input(
    placeholder: &'static str,
    element_id: &'static str,
    colors: TextInputColors,
    cx: &mut Context<AppFrame>,
) -> Entity<TextInput> {
    cx.new(|cx| TextInput::new(String::new(), placeholder, element_id, colors, cx).with_tab_stop())
}

fn subscribe_mod_input(
    input: &Entity<TextInput>,
    field: ModCreateField,
    cx: &mut Context<AppFrame>,
) {
    cx.subscribe(input, move |frame, input, event, cx| {
        frame.handle_mod_input_event(field, &input, event, cx);
    })
    .detach();
}

pub(crate) enum ModPathsPromptResult {
    Selected(Vec<PathBuf>),
    Canceled,
    Failed(Notice),
}

pub(crate) enum ModPathPromptResult {
    Selected(PathBuf),
    Canceled,
    Failed(Notice),
}

pub(crate) trait ModPromptLauncher {
    fn launch_paths(
        &self,
        kind: crate::mod_status::ModPromptKind,
        initial_directory: Option<PathBuf>,
        options: PathPromptOptions,
        cx: &mut Context<AppFrame>,
    ) -> Task<ModPathsPromptResult>;

    fn launch_export(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
        cx: &mut Context<AppFrame>,
    ) -> Task<ModPathPromptResult>;
}

pub(crate) struct PlatformModPromptLauncher;

impl ModPromptLauncher for PlatformModPromptLauncher {
    fn launch_paths(
        &self,
        _: crate::mod_status::ModPromptKind,
        _: Option<PathBuf>,
        options: PathPromptOptions,
        cx: &mut Context<AppFrame>,
    ) -> Task<ModPathsPromptResult> {
        let prompt = cx.prompt_for_paths(options);
        cx.spawn(async move |_, _| match prompt.await {
            Ok(Ok(Some(paths))) => ModPathsPromptResult::Selected(paths),
            Ok(Ok(None)) => ModPathsPromptResult::Canceled,
            Ok(Err(error)) => ModPathsPromptResult::Failed(Notice::error(
                "Could not open the mod file picker",
                error.as_ref(),
            )),
            Err(error) => ModPathsPromptResult::Failed(Notice::error(
                "The mod file picker did not respond",
                &error,
            )),
        })
    }

    fn launch_export(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
        cx: &mut Context<AppFrame>,
    ) -> Task<ModPathPromptResult> {
        let prompt = cx.prompt_for_new_path(directory, suggested_name);
        cx.spawn(async move |_, _| match prompt.await {
            Ok(Ok(Some(path))) => ModPathPromptResult::Selected(path),
            Ok(Ok(None)) => ModPathPromptResult::Canceled,
            Ok(Err(error)) => ModPathPromptResult::Failed(Notice::error(
                "Could not open the package export picker",
                error.as_ref(),
            )),
            Err(error) => ModPathPromptResult::Failed(Notice::error(
                "The package export picker did not respond",
                &error,
            )),
        })
    }
}

fn import_prompt_options() -> PathPromptOptions {
    PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Select one mod ZIP package".into()),
    }
}

fn create_files_prompt_options(root: &Path) -> PathPromptOptions {
    PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some(format!("Select package files below {}", root.display()).into()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModRefreshScope {
    Library,
    Full,
}

struct ModOperationSuccess {
    message: String,
    refresh: ModRefreshScope,
}

#[derive(Debug)]
enum ModSelectionError {
    OutsideRoot { path: PathBuf, root: PathBuf },
    NonUnicode { path: PathBuf },
    InvalidRelative { path: PathBuf, source: ModError },
}

impl Display for ModSelectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideRoot { path, root } => write!(
                formatter,
                "the selected file {} is outside the selected game folder {}",
                path.display(),
                root.display()
            ),
            Self::NonUnicode { path } => write!(
                formatter,
                "the selected file has a non-Unicode game-relative path: {}",
                path.display()
            ),
            Self::InvalidRelative { path, .. } => write!(
                formatter,
                "the selected file path cannot be stored in a mod package: {}",
                path.display()
            ),
        }
    }
}

impl Error for ModSelectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRelative { source, .. } => Some(source),
            Self::OutsideRoot { .. } | Self::NonUnicode { .. } => None,
        }
    }
}

fn selected_relative_paths(
    root: &Path,
    paths: Vec<PathBuf>,
) -> Result<Vec<RelativeGamePath>, ModSelectionError> {
    let limits = ModLimits::default();
    let mut selected = paths
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ModSelectionError::OutsideRoot {
                    path: path.clone(),
                    root: root.to_path_buf(),
                })?;
            let value = relative
                .to_str()
                .ok_or_else(|| ModSelectionError::NonUnicode { path: path.clone() })?
                .replace(std::path::MAIN_SEPARATOR, "/");
            RelativeGamePath::parse(&value, &limits)
                .map_err(|source| ModSelectionError::InvalidRelative { path, source })
        })
        .collect::<Result<Vec<_>, _>>()?;
    selected.sort_by(|left, right| left.portable_key().cmp(right.portable_key()));
    selected.dedup_by(|left, right| left.portable_key() == right.portable_key());
    Ok(selected)
}

fn optional_mod_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn safe_package_filename(value: &str) -> String {
    let value = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches(['-', '.']);
    if value.is_empty() {
        "package".to_owned()
    } else {
        value.to_owned()
    }
}

const fn refresh_scope(kind: ModOperationKind) -> ModRefreshScope {
    match kind {
        ModOperationKind::Import | ModOperationKind::Create | ModOperationKind::RemovePackage => {
            ModRefreshScope::Library
        }
        ModOperationKind::Apply
        | ModOperationKind::Uninstall
        | ModOperationKind::CreateBackup
        | ModOperationKind::RestoreBackup
        | ModOperationKind::DeleteBackup => ModRefreshScope::Full,
    }
}

fn backup_subject(backup: &BackupSnapshot) -> String {
    backup
        .label
        .clone()
        .unwrap_or_else(|| format!("Backup {}", backup.backup_id))
}

struct ModProgressBridge;

struct ModProgressUpdate {
    sequence: u64,
    phase_epoch: u64,
    progress: ModProgress,
}

struct ModProgressBridgeReporter {
    latest: Arc<Mutex<Option<ModProgressUpdate>>>,
    cancellation: Arc<AtomicBool>,
    wake: Sender<()>,
    sequence: u64,
    phase_epoch: u64,
    last_phase: Option<ModProgressPhase>,
}

struct ModProgressBridgeReader {
    latest: Arc<Mutex<Option<ModProgressUpdate>>>,
    cancellation: Arc<AtomicBool>,
    wake: Receiver<()>,
}

impl ModProgressBridge {
    fn channel() -> (ModProgressBridgeReporter, ModProgressBridgeReader) {
        let latest = Arc::new(Mutex::new(None));
        let cancellation = Arc::new(AtomicBool::new(false));
        let (wake_sender, wake_receiver) = async_channel::bounded(1);
        (
            ModProgressBridgeReporter {
                latest: Arc::clone(&latest),
                cancellation: Arc::clone(&cancellation),
                wake: wake_sender,
                sequence: 0,
                phase_epoch: 0,
                last_phase: None,
            },
            ModProgressBridgeReader {
                latest,
                cancellation,
                wake: wake_receiver,
            },
        )
    }
}

impl ModProgressBridgeReporter {
    fn lock_latest(&self) -> MutexGuard<'_, Option<ModProgressUpdate>> {
        self.latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl ModProgressReporter for ModProgressBridgeReporter {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
        self.sequence = self.sequence.saturating_add(1);
        if self.last_phase != Some(progress.phase) {
            self.phase_epoch = self.phase_epoch.saturating_add(1);
            self.last_phase = Some(progress.phase);
        }
        *self.lock_latest() = Some(ModProgressUpdate {
            sequence: self.sequence,
            phase_epoch: self.phase_epoch,
            progress: progress.clone(),
        });
        match self.wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(()) | TrySendError::Closed(())) => {}
        }
        if self.cancellation.load(Ordering::Acquire) && progress_phase_allows_cancel(progress.phase)
        {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

impl ModProgressBridgeReader {
    fn lock_latest(&self) -> MutexGuard<'_, Option<ModProgressUpdate>> {
        self.latest
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn next(&self) -> Option<ModProgressUpdate> {
        self.wake.recv().await.ok()?;
        self.lock_latest().take()
    }

    #[cfg(test)]
    fn request_cancellation(&self) {
        self.cancellation.store(true, Ordering::Release);
    }

    fn cancellation_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancellation)
    }

    #[cfg(test)]
    fn queued_wakes(&self) -> usize {
        self.wake.len()
    }

    #[cfg(test)]
    fn take_pending(&self) -> Option<ModProgressUpdate> {
        let _ = self.wake.try_recv();
        self.lock_latest().take()
    }
}

impl AppFrame {
    fn handle_mod_input_event(
        &mut self,
        field: ModCreateField,
        input: &Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextInputEvent::ContentChanged => {
                self.mods
                    .set_create_field(field, input.read(cx).content().to_owned());
            }
            TextInputEvent::Commit(value) => {
                self.mods.set_create_field(field, value.clone());
            }
            TextInputEvent::Cancel => self.dismiss_or_cancel_mod_operation(cx),
        }
        cx.notify();
    }

    pub(super) fn focus_next_mod_control(
        &mut self,
        _: &FocusNextModControl,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.area() == Area::Mods {
            window.focus_next();
            cx.notify();
        }
    }

    pub(super) fn focus_previous_mod_control(
        &mut self,
        _: &FocusPreviousModControl,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.area() == Area::Mods {
            window.focus_prev();
            cx.notify();
        }
    }

    pub(super) fn cancel_mod_operation(
        &mut self,
        _: &CancelModOperation,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.shell.area() == Area::Mods {
            self.dismiss_or_cancel_mod_operation(cx);
        }
    }

    pub(crate) fn import_mod_package(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.mods.begin_prompt(ModPromptKind::Import) else {
            return;
        };
        self.notices.begin(
            NoticeSource::Mods,
            key.request().get(),
            Notice::info("Choose one mod ZIP package"),
        );
        let launcher = Rc::clone(&self.mod_prompt_launcher);
        let prompt =
            launcher.launch_paths(ModPromptKind::Import, None, import_prompt_options(), cx);
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = prompt.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_mod_import_prompt(key, result, cx);
            });
        })
        .detach();
    }

    fn finish_mod_import_prompt(
        &mut self,
        key: ModRequestKey,
        result: ModPathsPromptResult,
        cx: &mut Context<Self>,
    ) {
        if !self.mods.finish_prompt(ModPromptKind::Import, key) {
            return;
        }
        match result {
            ModPathsPromptResult::Selected(paths) => {
                let Some(source) = paths.into_iter().next() else {
                    let _ = self
                        .notices
                        .complete(NoticeSource::Mods, key.request().get(), None);
                    self.resume_interrupted_mod_scan(cx);
                    cx.notify();
                    return;
                };
                let _ = self
                    .notices
                    .complete(NoticeSource::Mods, key.request().get(), None);
                self.start_mod_import(source, cx);
            }
            ModPathsPromptResult::Canceled => {
                let _ = self
                    .notices
                    .complete(NoticeSource::Mods, key.request().get(), None);
                self.resume_interrupted_mod_scan(cx);
                cx.notify();
            }
            ModPathsPromptResult::Failed(notice) => {
                let _ =
                    self.notices
                        .complete(NoticeSource::Mods, key.request().get(), Some(notice));
                self.resume_interrupted_mod_scan(cx);
                cx.notify();
            }
        }
    }

    fn start_mod_import(&mut self, source: PathBuf, cx: &mut Context<Self>) {
        let service = self.mod_service.clone();
        self.begin_mod_operation(ModOperationKind::Import, cx, move |progress| {
            let imported = service.import_package(&source, progress)?;
            let metadata = imported.package().manifest().metadata();
            let disposition = match imported.disposition() {
                ImportedModDisposition::Added => "Imported",
                ImportedModDisposition::AlreadyPresent => "Already in the library",
            };
            Ok(ModOperationSuccess {
                message: format!("{disposition} {} {}", metadata.name(), metadata.version()),
                refresh: ModRefreshScope::Library,
            })
        });
    }

    pub(crate) fn select_mod_create_files(&mut self, cx: &mut Context<Self>) {
        let game = self.shell.game();
        let Some(root) = self.game_paths.root(game).map(ToOwned::to_owned) else {
            self.notices.replace(
                NoticeSource::Mods,
                Notice::info(format!(
                    "Configure the {} game folder before selecting package files",
                    game.label()
                )),
            );
            cx.notify();
            return;
        };
        let Some(key) = self.mods.begin_prompt(ModPromptKind::SelectFiles) else {
            return;
        };
        self.notices.begin(
            NoticeSource::Mods,
            key.request().get(),
            Notice::info("Choose files below the configured game folder"),
        );
        let launcher = Rc::clone(&self.mod_prompt_launcher);
        let prompt = launcher.launch_paths(
            ModPromptKind::SelectFiles,
            Some(root.clone()),
            create_files_prompt_options(&root),
            cx,
        );
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = prompt.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_mod_files_prompt(key, &root, result, cx);
            });
        })
        .detach();
    }

    fn finish_mod_files_prompt(
        &mut self,
        key: ModRequestKey,
        root: &Path,
        result: ModPathsPromptResult,
        cx: &mut Context<Self>,
    ) {
        if !self.mods.finish_prompt(ModPromptKind::SelectFiles, key) {
            return;
        }
        let notice = match result {
            ModPathsPromptResult::Selected(paths) if paths.is_empty() => None,
            ModPathsPromptResult::Selected(paths) => match selected_relative_paths(root, paths) {
                Ok(paths) => {
                    let count = paths.len();
                    self.mods.set_create_files(paths);
                    Some(Notice::success(format!(
                        "Selected {count} package {}",
                        if count == 1 { "file" } else { "files" }
                    )))
                }
                Err(error) => Some(Notice::error("Could not use the selected files", &error)),
            },
            ModPathsPromptResult::Canceled => None,
            ModPathsPromptResult::Failed(notice) => Some(notice),
        };
        let completed = self
            .notices
            .complete(NoticeSource::Mods, key.request().get(), notice);
        if completed {
            self.schedule_success_notice_dismissal(NoticeSource::Mods, cx);
        }
        self.resume_interrupted_mod_scan(cx);
        cx.notify();
    }

    pub(crate) fn export_mod_package(&mut self, cx: &mut Context<Self>) {
        let game = self.shell.game();
        let Some(root) = self.game_paths.root(game).map(ToOwned::to_owned) else {
            self.notices.replace(
                NoticeSource::Mods,
                Notice::info(format!(
                    "Configure the {} game folder before creating a package",
                    game.label()
                )),
            );
            cx.notify();
            return;
        };
        let draft = self.mods.create_draft().clone();
        let metadata = match ModMetadata::new(
            draft.name.trim(),
            draft.version.trim(),
            optional_mod_text(&draft.author),
            optional_mod_text(&draft.description),
            None,
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.notices.replace(
                    NoticeSource::Mods,
                    Notice::error("Could not create the package metadata", &error),
                );
                cx.notify();
                return;
            }
        };
        if draft.files.is_empty() {
            self.notices.replace(
                NoticeSource::Mods,
                Notice::info("Select at least one game file before exporting a package"),
            );
            cx.notify();
            return;
        }
        let Some(key) = self.mods.begin_prompt(ModPromptKind::Export) else {
            return;
        };
        let suggested_name = format!(
            "{}-{}.zip",
            safe_package_filename(metadata.name()),
            safe_package_filename(metadata.version())
        );
        self.notices.begin(
            NoticeSource::Mods,
            key.request().get(),
            Notice::info("Choose where to export the mod package"),
        );
        let launcher = Rc::clone(&self.mod_prompt_launcher);
        let prompt = launcher.launch_export(&root, Some(&suggested_name), cx);
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = prompt.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_mod_export_prompt(key, root, metadata, draft.files, result, cx);
            });
        })
        .detach();
    }

    fn finish_mod_export_prompt(
        &mut self,
        key: ModRequestKey,
        root: PathBuf,
        metadata: ModMetadata,
        files: Vec<RelativeGamePath>,
        result: ModPathPromptResult,
        cx: &mut Context<Self>,
    ) {
        if !self.mods.finish_prompt(ModPromptKind::Export, key) {
            return;
        }
        match result {
            ModPathPromptResult::Selected(output) => {
                let _ = self
                    .notices
                    .complete(NoticeSource::Mods, key.request().get(), None);
                self.start_mod_creation(root, metadata, files, output, cx);
            }
            ModPathPromptResult::Canceled => {
                let _ = self
                    .notices
                    .complete(NoticeSource::Mods, key.request().get(), None);
                self.resume_interrupted_mod_scan(cx);
                cx.notify();
            }
            ModPathPromptResult::Failed(notice) => {
                let _ =
                    self.notices
                        .complete(NoticeSource::Mods, key.request().get(), Some(notice));
                self.resume_interrupted_mod_scan(cx);
                cx.notify();
            }
        }
    }

    fn resume_interrupted_mod_scan(&mut self, cx: &mut Context<Self>) {
        let library_loading = matches!(self.mods.library_state(), ModLibraryState::Loading);
        let root_loading = matches!(self.mods.root_state(), ModRootState::Loading);
        if !library_loading && !root_loading {
            return;
        }
        if self.shell.area() == Area::Mods {
            self.start_mod_scan(cx);
        } else {
            self.start_mod_library_scan(cx);
        }
    }

    fn start_mod_creation(
        &mut self,
        configured_root: PathBuf,
        metadata: ModMetadata,
        files: Vec<RelativeGamePath>,
        output: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let game = self.shell.game();
        let stores = self.mod_stores.clone();
        let service = self.mod_service.clone();
        self.begin_mod_operation(ModOperationKind::Create, cx, move |progress| {
            let root = GameRoot::inspect(game, configured_root, &stores)?;
            let request = CreateModRequest::new(metadata, &root, files, &output)?;
            let created = service.create_package(request, progress)?;
            let imported = service.import_package(created.output_path(), progress)?;
            let metadata = imported.package().manifest().metadata();
            Ok(ModOperationSuccess {
                message: format!(
                    "Exported and imported {} {}",
                    metadata.name(),
                    metadata.version()
                ),
                refresh: ModRefreshScope::Library,
            })
        });
    }

    fn begin_mod_operation<F>(&mut self, kind: ModOperationKind, cx: &mut Context<Self>, work: F)
    where
        F: FnOnce(&mut ModProgressBridgeReporter) -> Result<ModOperationSuccess, ModError>
            + Send
            + 'static,
    {
        let Some(key) = self.mods.begin_operation(kind) else {
            return;
        };
        self.launch_mod_operation(
            ModOperationLaunch {
                kind,
                target: None,
                key,
            },
            cx,
            work,
        );
    }

    fn launch_mod_operation<F>(
        &mut self,
        launch: ModOperationLaunch,
        cx: &mut Context<Self>,
        work: F,
    ) where
        F: FnOnce(&mut ModProgressBridgeReporter) -> Result<ModOperationSuccess, ModError>
            + Send
            + 'static,
    {
        let (mut reporter, reader) = ModProgressBridge::channel();
        self.mod_cancellation = Some(reader.cancellation_token());
        self.notices.begin(
            NoticeSource::Mods,
            launch.key.request().get(),
            Notice::info(format!("{} in progress", launch.kind.label())),
        );
        #[cfg(test)]
        {
            self.task_launches.mods += 1;
        }
        cx.notify();

        let progress_key = launch.key;
        cx.spawn(async move |entity, cx| {
            while let Some(update) = reader.next().await {
                let _ = entity.update(cx, |frame, cx| {
                    if frame.mods.update_progress(
                        progress_key,
                        update.sequence,
                        update.phase_epoch,
                        &update.progress,
                    ) {
                        cx.notify();
                    }
                });
            }
        })
        .detach();

        let task = cx
            .background_executor()
            .spawn(async move { work(&mut reporter) });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_mod_operation(launch, result, cx);
            });
        })
        .detach();
    }

    fn finish_mod_operation(
        &mut self,
        launch: ModOperationLaunch,
        result: Result<ModOperationSuccess, ModError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(success) => {
                if !self.mods.finish_operation(launch.key) {
                    return;
                }
                self.mod_cancellation = None;
                let completed = self.notices.complete(
                    NoticeSource::Mods,
                    launch.key.request().get(),
                    Some(Notice::success(success.message)),
                );
                if completed {
                    self.schedule_success_notice_dismissal(NoticeSource::Mods, cx);
                }
                self.refresh_after_mod_operation(success.refresh, cx);
            }
            Err(error) => {
                let title = format!("{} failed", launch.kind.label());
                let notice = Notice::error(title.clone(), &error);
                let issue = ModIssueSnapshot::from_error(
                    ModIssueScope::Operation,
                    format!("operation-{}", launch.key.request().get()),
                    title,
                    &error,
                );
                if !self.mods.fail_operation(launch.key, issue) {
                    return;
                }
                self.mod_cancellation = None;
                let _ = self.notices.complete(
                    NoticeSource::Mods,
                    launch.key.request().get(),
                    Some(notice),
                );
                self.refresh_after_mod_operation(refresh_scope(launch.kind), cx);
            }
        }
        cx.notify();
    }

    fn refresh_after_mod_operation(&mut self, scope: ModRefreshScope, cx: &mut Context<Self>) {
        match scope {
            ModRefreshScope::Library => {
                let _ = self.mods.library_changed();
                if self.shell.area() == Area::Mods {
                    self.start_mod_scan(cx);
                } else {
                    self.start_mod_library_scan(cx);
                }
            }
            ModRefreshScope::Full => self.start_mod_scan(cx),
        }
    }

    pub(crate) fn request_mod_apply(&mut self, package_id: ModPackageID, cx: &mut Context<Self>) {
        let Some(subject) = self
            .mods
            .package(package_id)
            .map(|package| format!("{} {}", package.name, package.version))
        else {
            return;
        };
        let _ = self.mods.begin_confirmation(
            ModOperationKind::Apply,
            ModOperationTarget::Package(package_id),
            subject,
            "Replace files in the selected game folder with files from this package.",
        );
        cx.notify();
    }

    pub(crate) fn request_mod_package_removal(
        &mut self,
        package_id: ModPackageID,
        cx: &mut Context<Self>,
    ) {
        let Some(subject) = self
            .mods
            .package(package_id)
            .map(|package| format!("{} {}", package.name, package.version))
        else {
            return;
        };
        let _ = self.mods.begin_confirmation(
            ModOperationKind::RemovePackage,
            ModOperationTarget::Package(package_id),
            subject,
            "Delete this package from the library. Installed packages must be uninstalled first.",
        );
        cx.notify();
    }

    pub(crate) fn request_mod_uninstall(
        &mut self,
        installation_id: InstallationID,
        cx: &mut Context<Self>,
    ) {
        let Some(subject) = self
            .mods
            .installation(installation_id)
            .map(|installed| format!("{} {}", installed.name, installed.version))
        else {
            return;
        };
        let _ = self.mods.begin_confirmation(
            ModOperationKind::Uninstall,
            ModOperationTarget::Installation(installation_id),
            subject,
            "Restore the original files, delete files added by the mod, and remove the mod from Installed.",
        );
        cx.notify();
    }

    pub(crate) fn request_mod_backup_restore(
        &mut self,
        backup_id: BackupID,
        cx: &mut Context<Self>,
    ) {
        let Some(subject) = self.mods.backup(backup_id).map(backup_subject) else {
            return;
        };
        let _ = self.mods.begin_confirmation(
            ModOperationKind::RestoreBackup,
            ModOperationTarget::Backup(backup_id),
            subject,
            "Replace matching game files with files from this backup. Other files will stay in place.",
        );
        cx.notify();
    }

    pub(crate) fn request_mod_backup_deletion(
        &mut self,
        backup_id: BackupID,
        cx: &mut Context<Self>,
    ) {
        let Some(subject) = self.mods.backup(backup_id).map(backup_subject) else {
            return;
        };
        let _ = self.mods.begin_confirmation(
            ModOperationKind::DeleteBackup,
            ModOperationTarget::Backup(backup_id),
            subject,
            "Permanently delete this backup.",
        );
        cx.notify();
    }

    pub(crate) fn create_mod_backup(&mut self, cx: &mut Context<Self>) {
        let game = self.shell.game();
        let Some(configured_root) = self.game_paths.root(game).map(ToOwned::to_owned) else {
            self.notices.replace(
                NoticeSource::Mods,
                Notice::info(format!(
                    "Configure the {} game folder before creating a backup",
                    game.label()
                )),
            );
            cx.notify();
            return;
        };
        let label = optional_mod_text(&self.mods.create_draft().backup_label);
        let stores = self.mod_stores.clone();
        let service = self.mod_service.clone();
        self.begin_mod_operation(ModOperationKind::CreateBackup, cx, move |progress| {
            let root = GameRoot::inspect(game, configured_root, &stores)?;
            let request = CreateBackupRequest::new(&root, label)?;
            let backup = service.create_backup(request, progress)?;
            Ok(ModOperationSuccess {
                message: format!(
                    "Created backup {}",
                    backup_subject(&BackupSnapshot::from(&backup))
                ),
                refresh: ModRefreshScope::Full,
            })
        });
    }

    pub(crate) fn dismiss_or_cancel_mod_operation(&mut self, cx: &mut Context<Self>) {
        if self.mods.dismiss_confirmation() {
            cx.notify();
            return;
        }
        if self.mods.request_cancellation() {
            if let Some(cancellation) = &self.mod_cancellation {
                cancellation.store(true, Ordering::Release);
            }
            cx.notify();
        }
    }

    pub(crate) fn confirm_mod_operation(&mut self, cx: &mut Context<Self>) {
        let Some(launch) = self.mods.confirm_operation() else {
            return;
        };
        match (launch.kind, launch.target) {
            (ModOperationKind::Apply, Some(ModOperationTarget::Package(package_id))) => {
                self.launch_confirmed_mod_apply(launch, package_id, cx);
            }
            (
                ModOperationKind::Uninstall,
                Some(ModOperationTarget::Installation(installation_id)),
            ) => {
                self.launch_confirmed_mod_uninstall(launch, installation_id, cx);
            }
            (ModOperationKind::RestoreBackup, Some(ModOperationTarget::Backup(backup_id))) => {
                self.launch_confirmed_mod_backup_restore(launch, backup_id, cx);
            }
            (ModOperationKind::DeleteBackup, Some(ModOperationTarget::Backup(backup_id))) => {
                self.launch_confirmed_mod_backup_deletion(launch, backup_id, cx);
            }
            (ModOperationKind::RemovePackage, Some(ModOperationTarget::Package(package_id))) => {
                self.launch_confirmed_mod_package_removal(launch, package_id, cx);
            }
            _ => self.abort_confirmed_mod_operation(launch, cx),
        }
    }

    fn launch_confirmed_mod_apply(
        &mut self,
        launch: ModOperationLaunch,
        package_id: ModPackageID,
        cx: &mut Context<Self>,
    ) {
        let Some((game, configured_root)) = self.mod_root_context() else {
            self.abort_confirmed_mod_operation(launch, cx);
            return;
        };
        let stores = self.mod_stores.clone();
        let service = self.mod_service.clone();
        self.launch_mod_operation(launch, cx, move |progress| {
            let root = GameRoot::inspect(game, configured_root, &stores)?;
            let report = service.apply(ApplyModRequest::new(&root, package_id), progress)?;
            Ok(ModOperationSuccess {
                message: format!(
                    "Applied {} {} · updated {} files",
                    report.installation().metadata().name(),
                    report.installation().metadata().version(),
                    report.committed_paths().len()
                ),
                refresh: ModRefreshScope::Full,
            })
        });
    }

    fn launch_confirmed_mod_uninstall(
        &mut self,
        launch: ModOperationLaunch,
        installation_id: InstallationID,
        cx: &mut Context<Self>,
    ) {
        let Some((game, configured_root)) = self.mod_root_context() else {
            self.abort_confirmed_mod_operation(launch, cx);
            return;
        };
        let stores = self.mod_stores.clone();
        let service = self.mod_service.clone();
        self.launch_mod_operation(launch, cx, move |progress| {
            let root = GameRoot::inspect(game, configured_root, &stores)?;
            let report =
                service.uninstall(UninstallModRequest::new(&root, installation_id), progress)?;
            Ok(ModOperationSuccess {
                message: format!(
                    "Uninstalled mod · {} restored, {} removed",
                    report.restored_paths().len(),
                    report.removed_paths().len()
                ),
                refresh: ModRefreshScope::Full,
            })
        });
    }

    fn launch_confirmed_mod_backup_restore(
        &mut self,
        launch: ModOperationLaunch,
        backup_id: BackupID,
        cx: &mut Context<Self>,
    ) {
        let Some((game, configured_root)) = self.mod_root_context() else {
            self.abort_confirmed_mod_operation(launch, cx);
            return;
        };
        let stores = self.mod_stores.clone();
        let service = self.mod_service.clone();
        self.launch_mod_operation(launch, cx, move |progress| {
            let root = GameRoot::inspect(game, configured_root, &stores)?;
            let report =
                service.restore_backup(RestoreBackupRequest::new(&root, backup_id), progress)?;
            Ok(ModOperationSuccess {
                message: format!(
                    "Restored backup · replaced {} files",
                    report.committed_paths().len()
                ),
                refresh: ModRefreshScope::Full,
            })
        });
    }

    fn launch_confirmed_mod_backup_deletion(
        &mut self,
        launch: ModOperationLaunch,
        backup_id: BackupID,
        cx: &mut Context<Self>,
    ) {
        let Some((game, configured_root)) = self.mod_root_context() else {
            self.abort_confirmed_mod_operation(launch, cx);
            return;
        };
        let stores = self.mod_stores.clone();
        let service = self.mod_service.clone();
        self.launch_mod_operation(launch, cx, move |_| {
            let root = GameRoot::inspect(game, configured_root, &stores)?;
            service.delete_backup(&root, backup_id)?;
            Ok(ModOperationSuccess {
                message: "Deleted backup".to_owned(),
                refresh: ModRefreshScope::Full,
            })
        });
    }

    fn launch_confirmed_mod_package_removal(
        &mut self,
        launch: ModOperationLaunch,
        package_id: ModPackageID,
        cx: &mut Context<Self>,
    ) {
        let service = self.mod_service.clone();
        self.launch_mod_operation(launch, cx, move |_| {
            service.remove_package(package_id)?;
            Ok(ModOperationSuccess {
                message: "Removed package from the library".to_owned(),
                refresh: ModRefreshScope::Library,
            })
        });
    }

    fn mod_root_context(&self) -> Option<(Game, PathBuf)> {
        let game = self.shell.game();
        self.game_paths
            .root(game)
            .map(|root| (game, root.to_path_buf()))
    }

    fn abort_confirmed_mod_operation(
        &mut self,
        launch: ModOperationLaunch,
        cx: &mut Context<Self>,
    ) {
        if self.mods.finish_operation(launch.key) {
            self.notices.replace(
                NoticeSource::Mods,
                Notice::info("That mod action is no longer available"),
            );
            cx.notify();
        }
    }

    pub(crate) fn start_mod_library_scan(&mut self, cx: &mut Context<Self>) {
        self.start_mod_scan_scope(ModScanScope::LibraryOnly, cx);
    }

    pub(crate) fn start_mod_scan(&mut self, cx: &mut Context<Self>) {
        self.start_mod_scan_scope(ModScanScope::Full, cx);
    }

    fn start_mod_scan_scope(&mut self, scope: ModScanScope, cx: &mut Context<Self>) {
        if self.mods.active_operation().is_some() {
            return;
        }
        let _ = self.sync_mod_context();
        let game = self.shell.game();
        let configured_root = self.game_paths.root(game).map(ToOwned::to_owned);
        let key = self
            .mods
            .begin_scan(scope, configured_root.as_ref().is_some());
        #[cfg(test)]
        {
            self.task_launches.mods += 1;
        }
        cx.notify();

        let service = self.mod_service.clone();
        let stores = self.mod_stores.clone();
        let task = cx
            .background_executor()
            .spawn(async move { scan_mods(&service, &stores, game, configured_root, scope) });
        cx.spawn(async move |entity, cx| {
            let completion = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_mod_scan(key, completion, cx);
            });
        })
        .detach();
    }

    pub(crate) fn finish_mod_scan(
        &mut self,
        key: ModRequestKey,
        completion: ModScanCompletion,
        cx: &mut Context<Self>,
    ) {
        if self.mods.finish_scan(key, completion) {
            cx.notify();
        }
    }

    pub(crate) fn select_mod_section(&mut self, section: ModSection, cx: &mut Context<Self>) {
        if self.mods.section() == section {
            return;
        }
        self.mods.select_section(section);
        cx.notify();
    }

    pub(crate) fn select_mod_package(&mut self, package_id: ModPackageID, cx: &mut Context<Self>) {
        if self.mods.package(package_id).is_some() {
            self.mods.select_package(Some(package_id));
            cx.notify();
        }
    }

    pub(crate) fn select_mod_installation(
        &mut self,
        installation_id: InstallationID,
        cx: &mut Context<Self>,
    ) {
        if self.mods.installation(installation_id).is_some() {
            self.mods.select_installation(Some(installation_id));
            cx.notify();
        }
    }

    pub(crate) fn select_mod_backup(&mut self, backup_id: BackupID, cx: &mut Context<Self>) {
        if self.mods.backup(backup_id).is_some() {
            self.mods.select_backup(Some(backup_id));
            cx.notify();
        }
    }

    pub(super) fn active_mod_context_changed(&mut self, cx: &mut Context<Self>) {
        let change = self.sync_mod_context();
        if change == ModContextChange::Changed {
            if let Some(cancellation) = self.mod_cancellation.take() {
                cancellation.store(true, Ordering::Release);
            }
            self.notices.clear(NoticeSource::Mods);
            if self.shell.area() == Area::Mods {
                self.start_mod_scan(cx);
            }
        }
        self.active_patch_context_changed(cx);
    }

    fn sync_mod_context(&mut self) -> ModContextChange {
        self.mods.set_context(
            self.shell.game(),
            self.root_revisions.revision(self.shell.game()),
        )
    }
}

fn scan_mods(
    service: &ModService,
    stores: &ModStorePaths,
    game: Game,
    configured_root: Option<PathBuf>,
    scope: ModScanScope,
) -> ModScanCompletion {
    let library = scan_library(service);
    let root = match scope {
        ModScanScope::LibraryOnly => ModRootCompletion::NotRequested,
        ModScanScope::Full => configured_root.map_or(ModRootCompletion::MissingRoot, |root| {
            scan_root(service, stores, game, root)
        }),
    };
    ModScanCompletion::new(library, root)
}

fn scan_library(
    service: &ModService,
) -> Result<ModCollectionSnapshot<ModPackageSnapshot>, ModIssueSnapshot> {
    service.scan_library().map_or_else(
        |error| {
            Err(ModIssueSnapshot::from_error(
                ModIssueScope::Library,
                "library-scan",
                "Could not check the mod library",
                &error,
            ))
        },
        |scan| Ok(library_snapshot(&scan)),
    )
}

fn library_snapshot(scan: &ModLibraryScan) -> ModCollectionSnapshot<ModPackageSnapshot> {
    let rows = scan.packages().iter().map(Into::into).collect();
    let issues = scan
        .issues()
        .iter()
        .enumerate()
        .map(|(index, issue)| {
            ModIssueSnapshot::from_error(
                ModIssueScope::Library,
                format!("library-{index}"),
                format!("Could not use {}", issue.path().display()),
                issue.error(),
            )
        })
        .collect();
    ModCollectionSnapshot::new(rows, issues)
}

fn scan_root(
    service: &ModService,
    stores: &ModStorePaths,
    game: Game,
    configured_root: PathBuf,
) -> ModRootCompletion {
    let root = match GameRoot::inspect(game, configured_root, stores) {
        Ok(root) => root,
        Err(error) => {
            return ModRootCompletion::Failed(ModIssueSnapshot::from_error(
                ModIssueScope::Root,
                "game-root",
                "Could not check the selected game folder",
                &error,
            ));
        }
    };
    let installations = service.scan_installations(&root).map_or_else(
        |error| {
            ModCollectionSnapshot::new(
                Vec::new(),
                vec![ModIssueSnapshot::from_error(
                    ModIssueScope::Installed,
                    "installation-scan",
                    "Could not check installed mods",
                    &error,
                )],
            )
        },
        |scan| installation_snapshot(&scan),
    );
    let backups = service.scan_backups(&root).map_or_else(
        |error| {
            ModCollectionSnapshot::new(
                Vec::new(),
                vec![ModIssueSnapshot::from_error(
                    ModIssueScope::Backups,
                    "backup-scan",
                    "Could not check backups",
                    &error,
                )],
            )
        },
        |scan| backup_snapshot(&scan),
    );
    ModRootCompletion::Ready {
        configured_root: root.configured_path().display().to_string(),
        installations,
        backups,
    }
}

fn installation_snapshot(scan: &InstallationScan) -> ModCollectionSnapshot<InstalledModSnapshot> {
    let rows = scan.installations().iter().map(Into::into).collect();
    let issues = scan
        .issues()
        .iter()
        .map(|issue| {
            let identity = issue.installation_id().map_or_else(
                || format!("installation-record-{}", issue.record_index()),
                |installation| format!("installation-{installation}"),
            );
            ModIssueSnapshot::from_error(
                ModIssueScope::Installed,
                identity,
                "Could not check installed mod files",
                issue.error(),
            )
        })
        .collect();
    ModCollectionSnapshot::new(rows, issues)
}

fn backup_snapshot(scan: &BackupScan) -> ModCollectionSnapshot<BackupSnapshot> {
    let rows = scan.backups().iter().map(Into::into).collect();
    let issues = scan
        .issues()
        .iter()
        .enumerate()
        .map(|(index, issue)| {
            ModIssueSnapshot::from_error(
                ModIssueScope::Backups,
                format!("backup-{index}"),
                format!("Could not use {}", issue.path().display()),
                issue.error(),
            )
        })
        .collect();
    ModCollectionSnapshot::new(rows, issues)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "the GPUI tests use controlled temporary settings and game roots"
    )]

    use std::{
        fs,
        ops::ControlFlow,
        path::Path,
        rc::Rc,
        sync::{Arc, atomic::Ordering},
    };

    use gpui::{
        AppContext, Entity, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, TestAppContext,
        VisualTestContext, WindowOptions, point, px, size,
    };
    use kufeditor_game::Game;
    use kufeditor_mods::{
        CreateModRequest, GameRoot, InstallationID, ModLimits, ModMetadata, ModPackageID,
        ModProgress, ModProgressPhase, ModProgressReporter, ModService, ModStorePaths,
        RelativeGamePath,
    };

    use super::{AppFrame, ModPathPromptResult, ModPathsPromptResult, ModProgressBridge};
    use crate::{
        mod_status::{
            ModCreateField, ModLibraryState, ModOperationKind, ModOperationTarget, ModPromptKind,
            ModRootState, ModSection,
        },
        notices::{Notice, NoticeLevel},
        settings::SettingsStartup,
        state::Area,
        test_support::ControlledModPromptLauncher,
    };

    #[gpui::test]
    fn settings_parent_remains_the_mod_store_when_settings_are_protected(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let settings_path = directory.path().join("settings.json");
        fs::write(&settings_path, br#"{"version":2,"future":true}"#).unwrap();
        let window = test_window(cx, SettingsStartup::load(settings_path));

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.mod_stores.application_data(), directory.path());
            })
            .unwrap();
    }

    #[gpui::test]
    fn library_starts_independently_and_the_mods_route_adds_the_root_scan(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let game_root = directory.path().join("game");
        fs::create_dir(&game_root).unwrap();
        let mut startup = SettingsStartup::load(directory.path().join("settings.json"));
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(game_root));
        let window = test_window(cx, startup);

        window
            .update(cx, |frame, _, cx| {
                frame.start_mod_library_scan(cx);
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Loading
                ));
                assert!(matches!(frame.mods.root_state(), ModRootState::Idle));
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Ready(_)
                ));
                frame.select_area(Area::Mods, cx);
                assert!(matches!(frame.mods.root_state(), ModRootState::Loading));
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Ready(_)
                ));
                assert!(matches!(
                    frame.mods.root_state(),
                    ModRootState::Ready { .. }
                ));
                assert_eq!(frame.task_launches.mods, 2);
            })
            .unwrap();
    }

    #[gpui::test]
    fn a_game_change_discards_the_pending_library_completion(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let window = test_window(
            cx,
            SettingsStartup::load(directory.path().join("settings.json")),
        );

        window
            .update(cx, |frame, _, cx| {
                frame.start_mod_library_scan(cx);
                frame.shell.select_game(Game::Heroes);
                frame.active_mod_context_changed(cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.mods.game(), Game::Heroes);
                assert!(matches!(frame.mods.library_state(), ModLibraryState::Idle));
                assert_eq!(frame.task_launches.mods, 1);
            })
            .unwrap();
    }

    #[test]
    fn mods_actions_progress_bridge_has_one_wake_and_keeps_the_latest_value() {
        let (mut reporter, reader) = ModProgressBridge::channel();
        for completed in 0..100 {
            assert_eq!(
                reporter.report(&ModProgress {
                    phase: ModProgressPhase::StagingFiles,
                    completed,
                    total: 100,
                    path: None,
                }),
                ControlFlow::Continue(())
            );
        }

        assert_eq!(reader.queued_wakes(), 1);
        assert_eq!(
            reader
                .take_pending()
                .expect("the newest progress value should remain")
                .progress
                .completed,
            99
        );
        assert_eq!(reader.queued_wakes(), 0);

        reader.request_cancellation();
        assert_eq!(
            reporter.report(&ModProgress {
                phase: ModProgressPhase::PublishingPackage,
                completed: 0,
                total: 1,
                path: None,
            }),
            ControlFlow::Continue(())
        );
        assert_eq!(
            reporter.report(&ModProgress {
                phase: ModProgressPhase::CopyingPackage,
                completed: 1,
                total: 2,
                path: None,
            }),
            ControlFlow::Break(())
        );
    }

    #[test]
    fn mods_actions_progress_bridge_marks_a_repeated_phase_after_coalescing() {
        let (mut reporter, reader) = ModProgressBridge::channel();
        assert_eq!(
            reporter.report(&ModProgress {
                phase: ModProgressPhase::InspectingPackage,
                completed: 4,
                total: 4,
                path: None,
            }),
            ControlFlow::Continue(())
        );
        let first = reader
            .take_pending()
            .expect("the first inspection update should be queued");
        assert_eq!(first.sequence, 1);
        assert_eq!(first.phase_epoch, 1);

        for progress in [
            ModProgress {
                phase: ModProgressPhase::PublishingPackage,
                completed: 0,
                total: 1,
                path: None,
            },
            ModProgress {
                phase: ModProgressPhase::InspectingPackage,
                completed: 0,
                total: 4,
                path: None,
            },
        ] {
            assert_eq!(reporter.report(&progress), ControlFlow::Continue(()));
        }
        let latest = reader
            .take_pending()
            .expect("the repeated inspection update should remain queued");
        assert_eq!(latest.sequence, 3);
        assert_eq!(latest.phase_epoch, 3);
        assert_eq!(latest.progress.phase, ModProgressPhase::InspectingPackage);
        assert_eq!(latest.progress.completed, 0);
    }

    #[gpui::test]
    fn mods_actions_prompts_keep_platform_choices_behind_the_launcher(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let game_root = directory.path().join("game");
        let selected = game_root.join("Data/alpha.sox");
        fs::create_dir_all(selected.parent().unwrap()).unwrap();
        fs::write(&selected, b"alpha").unwrap();
        let mut startup = SettingsStartup::load(directory.path().join("app/settings.json"));
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(game_root.clone()));
        let launcher = Rc::new(ControlledModPromptLauncher::default());
        launcher.queue_paths(ModPathsPromptResult::Selected(vec![selected]));
        launcher.queue_export(ModPathPromptResult::Canceled);
        launcher.queue_paths(ModPathsPromptResult::Canceled);
        launcher.queue_paths(ModPathsPromptResult::Failed(Notice::plain(
            NoticeLevel::Error,
            "Picker unavailable",
        )));
        let window = test_window(cx, startup);

        window
            .update(cx, |frame, _, cx| {
                frame.mod_prompt_launcher =
                    Rc::clone(&launcher) as Rc<dyn super::ModPromptLauncher>;
                frame.select_mod_create_files(cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                assert_eq!(
                    frame
                        .mods
                        .create_draft()
                        .files
                        .iter()
                        .map(RelativeGamePath::as_str)
                        .collect::<Vec<_>>(),
                    ["Data/alpha.sox"]
                );
                frame
                    .mods
                    .set_create_field(ModCreateField::Name, "Forged Pack".to_owned());
                frame
                    .mods
                    .set_create_field(ModCreateField::Version, "1.0".to_owned());
                let operation_launches_before = frame.task_launches.mods;
                frame.export_mod_package(cx);
                assert_eq!(frame.task_launches.mods, operation_launches_before);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                frame.import_mod_package(cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| frame.import_mod_package(cx))
            .unwrap();
        cx.run_until_parked();

        let requests = launcher.paths_requests();
        let [
            select_files_request,
            canceled_import_request,
            failed_import_request,
        ] = requests.as_slice()
        else {
            panic!("expected the select-files and two import prompt requests");
        };
        assert_eq!(select_files_request.kind, ModPromptKind::SelectFiles);
        assert_eq!(
            select_files_request.initial_directory.as_deref(),
            Some(game_root.as_path())
        );
        assert!(select_files_request.files);
        assert!(select_files_request.multiple);
        assert_eq!(canceled_import_request.kind, ModPromptKind::Import);
        assert_eq!(canceled_import_request.initial_directory, None);
        assert!(!canceled_import_request.multiple);
        assert_eq!(failed_import_request.kind, ModPromptKind::Import);
        let exports = launcher.export_requests();
        let [export_request] = exports.as_slice() else {
            panic!("expected one export prompt request");
        };
        assert_eq!(export_request.directory, game_root);
        assert_eq!(
            export_request.suggested_name.as_deref(),
            Some("Forged-Pack-1.0.zip")
        );
        window
            .update(cx, |frame, _, _| {
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Error);
                assert_eq!(notice.summary(), "Picker unavailable");
            })
            .unwrap();
    }

    #[gpui::test]
    fn mods_actions_canceled_prompt_resumes_the_scan_it_superseded(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let launcher = Rc::new(ControlledModPromptLauncher::default());
        launcher.queue_paths(ModPathsPromptResult::Canceled);
        let window = test_window(
            cx,
            SettingsStartup::load(directory.path().join("app/settings.json")),
        );

        window
            .update(cx, |frame, _, cx| {
                frame.mod_prompt_launcher = launcher;
                frame.start_mod_library_scan(cx);
                frame.import_mod_package(cx);
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Loading
                ));
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert!(matches!(
                    frame.mods.library_state(),
                    ModLibraryState::Ready(_)
                ));
                assert_eq!(frame.task_launches.mods, 2);
            })
            .unwrap();
    }

    #[gpui::test]
    fn mods_actions_import_a_selected_package_and_retain_failure_detail(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let package = create_package_fixture(directory.path());
        let invalid = directory.path().join("invalid.zip");
        fs::write(&invalid, b"not a ZIP package").unwrap();
        let startup = SettingsStartup::load(directory.path().join("app/settings.json"));
        let launcher = Rc::new(ControlledModPromptLauncher::default());
        launcher.queue_paths(ModPathsPromptResult::Selected(vec![package]));
        launcher.queue_paths(ModPathsPromptResult::Selected(vec![invalid]));
        let window = test_window(cx, startup);

        window
            .update(cx, |frame, _, cx| {
                frame.mod_prompt_launcher =
                    Rc::clone(&launcher) as Rc<dyn super::ModPromptLauncher>;
                frame.import_mod_package(cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                let ModLibraryState::Ready(library) = frame.mods.library_state() else {
                    panic!("the imported package must be visible");
                };
                assert_eq!(library.rows.len(), 1);
                assert_eq!(
                    library.rows.first().map(|package| package.name.as_str()),
                    Some("Forged Pack")
                );
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Success);
                assert!(notice.summary().contains("Imported Forged Pack 1.0"));
                frame.import_mod_package(cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.mods.operation_issues().len(), 1);
                assert!(
                    frame
                        .mods
                        .operation_issues()
                        .first()
                        .is_some_and(|issue| issue.detail.contains("ZIP"))
                );
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Error);
                assert_eq!(notice.summary(), "Import package failed");
            })
            .unwrap();
    }

    #[gpui::test]
    fn mods_stale_prompt_and_operation_completions_leave_the_new_context_untouched(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("game");
        fs::create_dir_all(&root).unwrap();
        let window = test_window(
            cx,
            SettingsStartup::load(directory.path().join("app/settings.json")),
        );

        window
            .update(cx, |frame, _, cx| {
                let original =
                    RelativeGamePath::parse("Data/original.sox", &ModLimits::default()).unwrap();
                frame.mods.set_create_files(vec![original.clone()]);
                let prompt = frame.mods.begin_prompt(ModPromptKind::SelectFiles).unwrap();
                frame.notices.begin(
                    crate::notices::NoticeSource::Mods,
                    prompt.request().get(),
                    Notice::info("Waiting for a stale picker"),
                );
                frame.shell.select_game(Game::Heroes);
                frame.active_mod_context_changed(cx);
                frame.finish_mod_files_prompt(
                    prompt,
                    &root,
                    ModPathsPromptResult::Selected(vec![root.join("replacement.sox")]),
                    cx,
                );
                assert_eq!(frame.mods.game(), Game::Heroes);
                assert_eq!(frame.mods.create_draft().files, [original]);
                assert!(frame.notices.current().is_none());

                let operation = frame
                    .mods
                    .begin_operation(crate::mod_status::ModOperationKind::Import)
                    .unwrap();
                assert!(frame.mods.update_progress(
                    operation,
                    1,
                    1,
                    &ModProgress {
                        phase: ModProgressPhase::CopyingPackage,
                        completed: 1,
                        total: 2,
                        path: None,
                    }
                ));
                frame.shell.select_game(Game::Crusaders);
                frame.active_mod_context_changed(cx);
                assert!(!frame.mods.finish_operation(operation));
                assert_eq!(frame.mods.game(), Game::Crusaders);
                assert!(frame.mods.progress().is_none());
                assert!(frame.mods.active_operation().is_none());
                assert!(frame.notices.current().is_none());
            })
            .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn mods_stale_non_unicode_game_relative_selection_is_rejected() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(OsString::from_vec(vec![b'D', b'a', b't', b'a', b'/', 0xff]));

        assert!(matches!(
            super::selected_relative_paths(directory.path(), vec![path]),
            Err(super::ModSelectionError::NonUnicode { .. })
        ));
    }

    #[gpui::test]
    fn mods_actions_rendered_controls_complete_the_recoverable_lifecycle(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app");
        let game_root = directory.path().join("active-game");
        fs::create_dir_all(&game_root).unwrap();
        let package_id = import_package_fixture(directory.path(), &app_data);
        let mut startup = SettingsStartup::load(app_data.join("settings.json"));
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(game_root.clone()));
        let (frame, cx) = cx.add_window_view(move |_, cx| AppFrame::new(startup, cx));
        frame.update(cx, |frame, cx| frame.select_area(Area::Mods, cx));
        cx.run_until_parked();
        draw_mod_frame(cx, &frame);

        let installation_id = apply_rendered_package(&frame, cx, &game_root, package_id);
        restore_and_delete_rendered_backup(&frame, cx, &game_root);
        uninstall_and_remove_rendered_package(&frame, cx, &game_root, package_id, installation_id);
    }

    fn apply_rendered_package(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        game_root: &Path,
        package_id: ModPackageID,
    ) -> InstallationID {
        click(cx, "mods-section-library");
        draw_mod_frame(cx, frame);
        let package_row = test_selector(format!("mod-library-{package_id}"));
        let apply = test_selector(format!("{package_row}-apply"));
        let remove = test_selector(format!("{package_row}-remove"));
        assert!(cx.debug_bounds(package_row).is_some());
        click(cx, package_row);
        click(cx, apply);
        frame.update(cx, |frame, _| {
            let confirmation = frame.mods.pending_confirmation().unwrap();
            assert_eq!(confirmation.operation, ModOperationKind::Apply);
            assert_eq!(confirmation.target, ModOperationTarget::Package(package_id));
            assert!(confirmation.subject.contains("Forged Pack 1.0"));
            assert!(confirmation.consequence.contains("selected game folder"));
            let status = frame.status_bar_projection();
            assert!(status.detail.contains("Before-images are created"));
            assert!(status.detail.contains("rolls back committed paths"));
        });
        draw_mod_frame(cx, frame);
        frame.update_in(cx, |frame, window, _| window.focus(&frame.mods_focus));
        cx.simulate_keystrokes("escape");
        frame.update(cx, |frame, _| {
            assert!(frame.mods.pending_confirmation().is_none());
        });
        draw_mod_frame(cx, frame);
        click(cx, apply);
        draw_mod_frame(cx, frame);
        frame.update_in(cx, |frame, window, _| window.focus(&frame.mods_focus));
        press_tabs(cx, 7);
        key_cycle(cx, "enter");
        cx.run_until_parked();
        assert_eq!(
            fs::read(game_root.join("Data/alpha.sox")).unwrap(),
            b"alpha"
        );
        let installation_id = frame.update(cx, |frame, _| {
            let ModRootState::Ready { installations, .. } = frame.mods.root_state() else {
                panic!("the active root must refresh after apply");
            };
            let [installation] = installations.rows.as_slice() else {
                panic!("expected one installed mod after apply");
            };
            installation.installation_id
        });

        draw_mod_frame(cx, frame);
        click(cx, remove);
        frame.update(cx, |frame, _| {
            assert!(
                frame.mods.pending_confirmation().is_none(),
                "a referenced package removal must remain disabled"
            );
        });
        installation_id
    }

    fn restore_and_delete_rendered_backup(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        game_root: &Path,
    ) {
        click(cx, "mods-section-backups");
        draw_mod_frame(cx, frame);
        click(cx, "mods-backup-create-action");
        cx.run_until_parked();
        let backup_id = frame.update(cx, |frame, _| {
            let ModRootState::Ready { backups, .. } = frame.mods.root_state() else {
                panic!("the active root must refresh after backup creation");
            };
            let [backup] = backups.rows.as_slice() else {
                panic!("expected one backup after creation");
            };
            backup.backup_id
        });
        fs::write(game_root.join("Data/alpha.sox"), b"changed").unwrap();
        draw_mod_frame(cx, frame);
        let backup_row = test_selector(format!("mod-backup-{backup_id}"));
        let restore = test_selector(format!("{backup_row}-restore"));
        let delete = test_selector(format!("{backup_row}-delete"));
        click(cx, restore);
        frame.update(cx, |frame, _| {
            let confirmation = frame.mods.pending_confirmation().unwrap();
            assert_eq!(confirmation.operation, ModOperationKind::RestoreBackup);
            assert_eq!(confirmation.target, ModOperationTarget::Backup(backup_id));
            assert!(
                confirmation
                    .consequence
                    .contains("Other files will stay in place")
            );
        });
        draw_mod_frame(cx, frame);
        click(cx, "mods-confirmation-accept");
        cx.run_until_parked();
        assert_eq!(
            fs::read(game_root.join("Data/alpha.sox")).unwrap(),
            b"alpha"
        );

        draw_mod_frame(cx, frame);
        click(cx, delete);
        frame.update(cx, |frame, _| {
            let confirmation = frame.mods.pending_confirmation().unwrap();
            assert_eq!(confirmation.operation, ModOperationKind::DeleteBackup);
            assert!(confirmation.consequence.contains("Permanently delete"));
            let status = frame.status_bar_projection();
            assert!(status.detail.contains("permanently deleted"));
            assert!(status.detail.contains("game files are unchanged"));
        });
        draw_mod_frame(cx, frame);
        click(cx, "mods-confirmation-accept");
        cx.run_until_parked();
        frame.update(cx, |frame, _| {
            let ModRootState::Ready { backups, .. } = frame.mods.root_state() else {
                panic!("the active root must refresh after backup deletion");
            };
            assert!(backups.rows.is_empty());
        });
    }

    fn uninstall_and_remove_rendered_package(
        frame: &Entity<AppFrame>,
        cx: &mut VisualTestContext,
        game_root: &Path,
        package_id: ModPackageID,
        installation_id: InstallationID,
    ) {
        draw_mod_frame(cx, frame);
        click(cx, "mods-section-installed");
        draw_mod_frame(cx, frame);
        let installation_row = test_selector(format!("mod-installed-{installation_id}"));
        let uninstall = test_selector(format!("{installation_row}-uninstall"));
        click(cx, uninstall);
        frame.update(cx, |frame, _| {
            let confirmation = frame.mods.pending_confirmation().unwrap();
            assert_eq!(confirmation.operation, ModOperationKind::Uninstall);
            assert_eq!(
                confirmation.target,
                ModOperationTarget::Installation(installation_id)
            );
            assert!(
                confirmation
                    .consequence
                    .contains("Restore the original files")
            );
        });
        draw_mod_frame(cx, frame);
        click(cx, "mods-confirmation-accept");
        cx.run_until_parked();
        assert!(!game_root.join("Data/alpha.sox").exists());

        draw_mod_frame(cx, frame);
        click(cx, "mods-section-library");
        draw_mod_frame(cx, frame);
        let package_row = test_selector(format!("mod-library-{package_id}"));
        click(cx, test_selector(format!("{package_row}-remove")));
        frame.update(cx, |frame, _| {
            let confirmation = frame.mods.pending_confirmation().unwrap();
            assert_eq!(confirmation.operation, ModOperationKind::RemovePackage);
            assert!(
                confirmation
                    .consequence
                    .contains("Delete this package from the library")
            );
        });
        draw_mod_frame(cx, frame);
        click(cx, "mods-confirmation-accept");
        cx.run_until_parked();
        frame.update(cx, |frame, _| {
            let ModLibraryState::Ready(library) = frame.mods.library_state() else {
                panic!("the library must refresh after package removal");
            };
            assert!(library.rows.is_empty());
        });
    }

    #[gpui::test]
    fn mods_actions_creation_fields_export_and_import_the_package(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        cx.update(crate::text_input::bind);
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app");
        let game_root = directory.path().join("active-game");
        let selected = game_root.join("Data/create.sox");
        fs::create_dir_all(selected.parent().unwrap()).unwrap();
        fs::write(&selected, b"created through GPUI").unwrap();
        let output = directory.path().join("exported.zip");
        let launcher = Rc::new(ControlledModPromptLauncher::default());
        launcher.queue_paths(ModPathsPromptResult::Selected(vec![selected]));
        launcher.queue_export(ModPathPromptResult::Selected(output.clone()));
        let mut startup = SettingsStartup::load(app_data.join("settings.json"));
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(game_root));
        let (frame, cx) = cx.add_window_view(move |_, cx| AppFrame::new(startup, cx));
        frame.update(cx, |frame, cx| {
            frame.mod_prompt_launcher = Rc::clone(&launcher) as Rc<dyn super::ModPromptLauncher>;
            frame.select_area(Area::Mods, cx);
            frame.select_mod_section(ModSection::Create, cx);
        });
        cx.run_until_parked();
        draw_mod_frame(cx, &frame);
        click(cx, "mods-create-select-files");
        cx.run_until_parked();

        frame.update_in(cx, |frame, window, cx| {
            window.focus(&frame.mod_inputs.name.read(cx).focus_handle());
        });
        cx.simulate_input("Forged UI Pack");
        frame.update_in(cx, |frame, window, cx| {
            window.focus(&frame.mod_inputs.version.read(cx).focus_handle());
        });
        cx.simulate_input("2.0");
        frame.update(cx, |frame, _| {
            assert_eq!(frame.mods.create_draft().name, "Forged UI Pack");
            assert_eq!(frame.mods.create_draft().version, "2.0");
            assert_eq!(frame.mods.create_draft().files.len(), 1);
        });

        draw_mod_frame(cx, &frame);
        click(cx, "mods-create-export");
        cx.run_until_parked();
        assert!(output.is_file());
        frame.update(cx, |frame, _| {
            let ModLibraryState::Ready(library) = frame.mods.library_state() else {
                panic!("the created package must be imported into the library");
            };
            assert_eq!(library.rows.len(), 1);
            let package = library.rows.first().unwrap();
            assert_eq!(package.name, "Forged UI Pack");
            assert_eq!(package.version, "2.0");
            let notice = frame.notices.current().unwrap();
            assert_eq!(notice.level(), NoticeLevel::Success);
            assert!(notice.summary().contains("Exported and imported"));
        });
    }

    #[gpui::test]
    fn mods_keyboard_escape_requests_cancellation_only_in_a_safe_phase(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let directory = tempfile::tempdir().unwrap();
        let startup = SettingsStartup::load(directory.path().join("settings.json"));
        let (frame, cx) = cx.add_window_view(move |_, cx| AppFrame::new(startup, cx));
        let cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
        frame.update(cx, |frame, cx| {
            frame.shell.select_area(Area::Mods);
            let operation = frame
                .mods
                .begin_operation(ModOperationKind::Import)
                .unwrap();
            assert!(frame.mods.update_progress(
                operation,
                1,
                1,
                &ModProgress {
                    phase: ModProgressPhase::CopyingPackage,
                    completed: 3,
                    total: 9,
                    path: Some(
                        RelativeGamePath::parse("Data/current.sox", &ModLimits::default()).unwrap(),
                    ),
                }
            ));
            frame.mod_cancellation = Some(Arc::clone(&cancellation));
            cx.notify();
        });
        draw_mod_frame(cx, &frame);
        assert!(cx.debug_bounds("mods-progress").is_some());
        assert!(cx.debug_bounds("mods-progress-cancel").is_some());

        frame.update_in(cx, |frame, window, _| window.focus(&frame.mods_focus));
        cx.simulate_keystrokes("escape");
        assert!(cancellation.load(Ordering::Acquire));
        frame.update(cx, |frame, _| {
            let progress = frame.mods.progress().unwrap();
            assert!(progress.cancel_requested);
            assert!(!progress.can_cancel);
        });
    }

    #[gpui::test]
    fn mods_keyboard_tabs_through_creation_fields_in_visual_order(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        cx.update(crate::text_input::bind);
        let directory = tempfile::tempdir().unwrap();
        let game_root = directory.path().join("game");
        fs::create_dir_all(&game_root).unwrap();
        let mut startup = SettingsStartup::load(directory.path().join("app/settings.json"));
        startup
            .game_paths
            .set_root(Game::Crusaders, Some(game_root));
        let (frame, cx) = cx.add_window_view(move |_, cx| AppFrame::new(startup, cx));
        frame.update(cx, |frame, cx| {
            frame.select_area(Area::Mods, cx);
            frame.select_mod_section(ModSection::Create, cx);
        });
        cx.run_until_parked();
        draw_mod_frame(cx, &frame);

        frame.update_in(cx, |frame, window, _| window.focus(&frame.mods_focus));
        press_tabs(cx, 6);
        frame.update_in(cx, |frame, window, cx| {
            assert!(
                frame
                    .mod_inputs
                    .name
                    .read(cx)
                    .focus_handle()
                    .is_focused(window)
            );
        });
        cx.simulate_keystrokes("tab");
        frame.update_in(cx, |frame, window, cx| {
            assert!(
                frame
                    .mod_inputs
                    .version
                    .read(cx)
                    .focus_handle()
                    .is_focused(window)
            );
        });
        cx.simulate_keystrokes("shift-tab");
        frame.update_in(cx, |frame, window, cx| {
            assert!(
                frame
                    .mod_inputs
                    .name
                    .read(cx)
                    .focus_handle()
                    .is_focused(window)
            );
        });
    }

    #[gpui::test]
    fn mods_keyboard_and_pointer_activate_the_same_section_controls(cx: &mut TestAppContext) {
        cx.update(crate::actions::bind);
        let directory = tempfile::tempdir().unwrap();
        let startup = SettingsStartup::load(directory.path().join("settings.json"));
        let (frame, cx) = cx.add_window_view(move |_, cx| AppFrame::new(startup, cx));
        frame.update(cx, |frame, cx| {
            frame.shell.select_area(Area::Mods);
            cx.notify();
        });
        draw_mod_frame(cx, &frame);

        for selector in [
            "mods-section-installed",
            "mods-section-library",
            "mods-section-backups",
            "mods-section-create",
        ] {
            assert!(
                cx.debug_bounds(selector).is_some(),
                "missing Mods control {selector}"
            );
        }

        click(cx, "mods-section-library");
        frame.update(cx, |frame, _| {
            assert_eq!(frame.mods.section(), ModSection::Library);
        });

        frame.update_in(cx, |frame, window, _| window.focus(&frame.mods_focus));
        press_tabs(cx, 2);
        key_cycle(cx, "enter");
        frame.update(cx, |frame, _| {
            assert_eq!(frame.mods.section(), ModSection::Backups);
        });
    }

    fn draw_mod_frame(cx: &mut VisualTestContext, frame: &Entity<AppFrame>) {
        let frame = frame.clone();
        cx.draw(
            point(px(0.0), px(0.0)),
            size(px(1180.0), px(780.0)),
            move |_, _| frame,
        );
    }

    fn create_package_fixture(directory: &Path) -> std::path::PathBuf {
        let game_path = directory.join("builder-game");
        fs::create_dir_all(game_path.join("Data")).unwrap();
        fs::write(game_path.join("Data/alpha.sox"), b"alpha").unwrap();
        let stores = ModStorePaths::new(directory.join("builder-application-data"));
        let root = GameRoot::inspect(Game::Crusaders, game_path, &stores).unwrap();
        let output = directory.join("forged-pack.zip");
        let metadata = ModMetadata::new(
            "Forged Pack",
            "1.0",
            Some("Forgeworks".to_owned()),
            Some("A controlled GPUI import fixture.".to_owned()),
            None,
        )
        .unwrap();
        let path = RelativeGamePath::parse("Data/alpha.sox", &ModLimits::default()).unwrap();
        ModService::new(stores)
            .create_package(
                CreateModRequest::new(metadata, &root, vec![path], &output).unwrap(),
                &mut ContinueProgress,
            )
            .unwrap();
        output
    }

    fn import_package_fixture(directory: &Path, app_data: &Path) -> ModPackageID {
        let package = create_package_fixture(directory);
        ModService::new(ModStorePaths::new(app_data.to_path_buf()))
            .import_package(&package, &mut ContinueProgress)
            .unwrap()
            .package()
            .package_id()
    }

    struct ContinueProgress;

    impl ModProgressReporter for ContinueProgress {
        fn report(&mut self, _: &ModProgress) -> ControlFlow<()> {
            ControlFlow::Continue(())
        }
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing click target {selector}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
    }

    fn press_tabs(cx: &mut VisualTestContext, count: usize) {
        for _ in 0..count {
            cx.simulate_keystrokes("tab");
        }
    }

    fn key_cycle(cx: &mut VisualTestContext, key: &str) {
        cx.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
        });
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
    }

    fn test_selector(selector: String) -> &'static str {
        Box::leak(selector.into_boxed_str())
    }

    fn test_window(
        cx: &mut TestAppContext,
        startup: SettingsStartup,
    ) -> gpui::WindowHandle<AppFrame> {
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        })
    }
}
