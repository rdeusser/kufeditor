use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui::prelude::*;
use gpui::{
    Action, AnyElement, AnyWindowHandle, App, AsyncApp, BackgroundExecutor, Context, Div, Entity,
    FocusHandle, Focusable, KeyDownEvent, PathPromptOptions, PromptLevel, Stateful, Task,
    WeakEntity, Window, div, px,
};
use kufeditor_game::{Game, NameDictionary};
use kufeditor_workspace::{
    DiagnosticLocation, DocumentEdit, DocumentID, DocumentKind, LoadedDocument, SaveToken,
    SkillTextField, TroopField, TroopGroup, Workspace, WorkspaceError, load_path,
};

use crate::{
    actions::{OpenFile, Redo, Save, SaveAll, SaveAs, Undo},
    catalog_status::{CatalogRequestError, CatalogSession},
    components,
    notices::{Notice, NoticeCenter, NoticeLevel, NoticeSource},
    number_edit::{NumberCommand, NumberEdit, NumberOutcome},
    settings::{SettingsStartup, SettingsStartupWarning, SettingsWritePump},
    state::{Area, ClosePolicy, RecordSelections, RequestID, ShellState, navigation_projection},
    text_input::{TextInput, TextInputColors, TextInputEvent},
    theme::Theme,
    views,
};

mod catalog;
mod discovery;
#[path = "discovery_status.rs"]
pub(crate) mod discovery_status;
mod settings;
use self::discovery::{BrowsePromptLauncher, PlatformBrowsePromptLauncher};
use self::settings::protected_settings_notice;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TaskLaunchCounts {
    catalog: usize,
    discovery: usize,
    inspection: usize,
    settings: usize,
}

struct ActiveNumberEdit {
    target: NumberEditTarget,
    editor: NumberEdit,
}

impl ActiveNumberEdit {
    fn troop_field(document: DocumentID, record: usize, field: TroopField, value: i32) -> Self {
        Self {
            target: NumberEditTarget::TroopField {
                document,
                record,
                field,
            },
            editor: NumberEdit::new(i64::from(value), i64::from(i32::MIN), i64::from(i32::MAX)),
        }
    }

    fn skill_id(document: DocumentID, record: usize, value: i32) -> Self {
        Self {
            target: NumberEditTarget::SkillID { document, record },
            editor: NumberEdit::new(i64::from(value), i64::from(i32::MIN), i64::from(i32::MAX)),
        }
    }

    fn skill_max_level(document: DocumentID, record: usize, value: u32) -> Self {
        Self {
            target: NumberEditTarget::SkillMaxLevel { document, record },
            editor: NumberEdit::new(i64::from(value), 1, 65_535),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumberEditTarget {
    TroopField {
        document: DocumentID,
        record: usize,
        field: TroopField,
    },
    SkillID {
        document: DocumentID,
        record: usize,
    },
    SkillMaxLevel {
        document: DocumentID,
        record: usize,
    },
}

impl NumberEditTarget {
    fn document(&self) -> DocumentID {
        match self {
            Self::TroopField { document, .. }
            | Self::SkillID { document, .. }
            | Self::SkillMaxLevel { document, .. } => *document,
        }
    }

    const fn format_name(self) -> &'static str {
        match self {
            Self::TroopField { .. } => "TroopInfo",
            Self::SkillID { .. } | Self::SkillMaxLevel { .. } => "SkillInfo",
        }
    }

    fn is_troop_field(&self, document: DocumentID, record: usize, field: TroopField) -> bool {
        matches!(
            self,
            Self::TroopField {
                document: target_document,
                record: target_record,
                field: target_field,
            } if *target_document == document && *target_record == record && *target_field == field
        )
    }

    fn is_skill_id(&self, document: DocumentID, record: usize) -> bool {
        matches!(
            self,
            Self::SkillID {
                document: target_document,
                record: target_record,
            } if *target_document == document && *target_record == record
        )
    }

    fn is_skill_max_level(&self, document: DocumentID, record: usize) -> bool {
        matches!(
            self,
            Self::SkillMaxLevel {
                document: target_document,
                record: target_record,
            } if *target_document == document && *target_record == record
        )
    }

    fn document_edit(
        &self,
        value: i64,
    ) -> Result<(DocumentID, DocumentEdit), std::num::TryFromIntError> {
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
            Self::SkillID { document, record } => Ok((
                document,
                DocumentEdit::SetSkillID {
                    record,
                    value: i32::try_from(value)?,
                },
            )),
            Self::SkillMaxLevel { document, record } => Ok((
                document,
                DocumentEdit::SetSkillMaxLevel {
                    record,
                    value: u32::try_from(value)?,
                },
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SkillTypeChoice {
    Combat,
    Magic,
}

impl SkillTypeChoice {
    const ALL: [Self; 2] = [Self::Combat, Self::Magic];

    const fn label(self) -> &'static str {
        match self {
            Self::Combat => "Combat",
            Self::Magic => "Magic",
        }
    }

    const fn value(self) -> u32 {
        match self {
            Self::Combat => 1,
            Self::Magic => 2,
        }
    }

    const fn document_edit(self, record: usize) -> DocumentEdit {
        DocumentEdit::SetSkillType {
            record,
            value: self.value(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextEditTarget {
    Skill {
        document: DocumentID,
        record: usize,
        field: SkillTextField,
    },
    TextSOX {
        document: DocumentID,
        record: usize,
    },
}

impl TextEditTarget {
    const fn skill(document: DocumentID, record: usize, field: SkillTextField) -> Self {
        Self::Skill {
            document,
            record,
            field,
        }
    }

    const fn text_sox(document: DocumentID, record: usize) -> Self {
        Self::TextSOX { document, record }
    }

    const fn document(self) -> DocumentID {
        match self {
            Self::Skill { document, .. } | Self::TextSOX { document, .. } => document,
        }
    }

    const fn format_name(self) -> &'static str {
        match self {
            Self::Skill { .. } => "SkillInfo",
            Self::TextSOX { .. } => "text SOX",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Skill { field, .. } => field.label(),
            Self::TextSOX { .. } => "Text",
        }
    }

    fn document_edit(self, value: String) -> (DocumentID, DocumentEdit) {
        match self {
            Self::Skill {
                document,
                record,
                field,
            } => (
                document,
                DocumentEdit::SetSkillText {
                    record,
                    field,
                    value,
                },
            ),
            Self::TextSOX { document, record } => {
                (document, DocumentEdit::SetTextSOXText { record, value })
            }
        }
    }
}

struct ActiveTextEdit {
    target: TextEditTarget,
    input: Entity<TextInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorRoute {
    Troop,
    Skill,
    TextSOX,
}

const fn editor_route(kind: DocumentKind) -> EditorRoute {
    match kind {
        DocumentKind::TroopInfo => EditorRoute::Troop,
        DocumentKind::SkillInfo => EditorRoute::Skill,
        DocumentKind::TextSOX => EditorRoute::TextSOX,
    }
}

#[derive(Debug, Eq, PartialEq)]
enum SkillTextProjection {
    Editable(String),
    Invalid {
        value: &'static str,
        diagnostic: String,
    },
}

fn skill_text_projection(
    workspace: &Workspace,
    document: DocumentID,
    record: usize,
    field: SkillTextField,
) -> SkillTextProjection {
    match workspace.skill_text(document, record, field) {
        Ok(value) => SkillTextProjection::Editable(value.to_owned()),
        Err(error) => SkillTextProjection::Invalid {
            value: "Invalid UTF-8",
            diagnostic: error.to_string(),
        },
    }
}

fn troop_diagnostic_title(location: DiagnosticLocation) -> String {
    let label = location.label();
    location.record().map_or_else(
        || label.to_owned(),
        |record| format!("{} · {label}", views::troop::troop_name(record)),
    )
}

fn invalid_number_notice() -> Notice {
    Notice::editor_info("Enter a whole number within the allowed range")
}

fn clear_editor_notice(notices: &mut NoticeCenter) {
    notices.clear(NoticeSource::Editor);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CloseDocuments {
    #[default]
    Save,
    Discard,
}

enum SaveAsPromptResult {
    Selected(PathBuf),
    Canceled,
    Failed(Notice),
}

enum OpenPromptResult {
    Selected(Vec<PathBuf>),
    Canceled,
    Failed(Notice),
}

trait OpenPromptLauncher {
    fn launch(
        &self,
        frame: &AppFrame,
        request: RequestID,
        options: PathPromptOptions,
        cx: &mut Context<AppFrame>,
    ) -> Task<OpenPromptResult>;
}

struct PlatformOpenPromptLauncher;

impl OpenPromptLauncher for PlatformOpenPromptLauncher {
    fn launch(
        &self,
        _: &AppFrame,
        _: RequestID,
        options: PathPromptOptions,
        cx: &mut Context<AppFrame>,
    ) -> Task<OpenPromptResult> {
        let prompt = cx.prompt_for_paths(options);
        cx.spawn(async move |_, _| match prompt.await {
            Ok(Ok(Some(paths))) => OpenPromptResult::Selected(paths),
            Ok(Ok(None)) => OpenPromptResult::Canceled,
            Ok(Err(error)) => OpenPromptResult::Failed(Notice::error(
                "Could not open the file picker",
                error.as_ref(),
            )),
            Err(error) => {
                OpenPromptResult::Failed(Notice::error("The file picker did not respond", &error))
            }
        })
    }
}

trait OpenPathLoader {
    fn start(
        &self,
        path: PathBuf,
        executor: &BackgroundExecutor,
    ) -> Task<(PathBuf, Result<LoadedDocument, WorkspaceError>)>;
}

struct FileSystemOpenPathLoader;

impl OpenPathLoader for FileSystemOpenPathLoader {
    fn start(
        &self,
        path: PathBuf,
        executor: &BackgroundExecutor,
    ) -> Task<(PathBuf, Result<LoadedDocument, WorkspaceError>)> {
        executor.spawn(async move {
            let loaded = load_path(path.clone());
            (path, loaded)
        })
    }
}

pub struct AppFrame {
    workspace: Workspace,
    pub(crate) shell: ShellState,
    theme: Theme,
    focus: FocusHandle,
    active_document: Option<DocumentID>,
    selections: RecordSelections,
    number_edit: Option<ActiveNumberEdit>,
    text_edit: Option<ActiveTextEdit>,
    game_paths: kufeditor_game::GamePaths,
    root_revisions: discovery_status::RootRevisions,
    recent_files: kufeditor_workspace::RecentFiles,
    catalog: CatalogSession<NameDictionary, CatalogRequestError>,
    discovery: discovery_status::DiscoveryStatus,
    settings: SettingsWritePump,
    notices: NoticeCenter,
    next_workspace_notice: u64,
    window_handle: Option<AnyWindowHandle>,
    close_armed: bool,
    close_pending: bool,
    close_documents: CloseDocuments,
    close_prompt_open: bool,
    open_prompt_launcher: Rc<dyn OpenPromptLauncher>,
    open_path_loader: Rc<dyn OpenPathLoader>,
    browse_prompt_launcher: Rc<dyn BrowsePromptLauncher>,
    #[cfg(test)]
    task_launches: TaskLaunchCounts,
}

impl AppFrame {
    pub fn new(startup: SettingsStartup, cx: &mut Context<Self>) -> Self {
        let SettingsStartup {
            path,
            active_game,
            game_paths,
            recent_files,
            persistence,
            warning,
        } = startup;
        let settings = SettingsWritePump::new(path, persistence);
        let mut notices = NoticeCenter::default();
        if let Some(warning) = warning {
            match warning {
                SettingsStartupWarning::Load(error) => notices.replace(
                    NoticeSource::Startup,
                    Notice::error("Could not load application settings", &error),
                ),
                SettingsStartupWarning::UnsupportedVersion { found } => notices.replace(
                    NoticeSource::SettingsWrite,
                    protected_settings_notice(found),
                ),
            }
        }
        Self {
            workspace: Workspace::new(),
            shell: ShellState::with_game(active_game),
            theme: Theme::forged_steel(),
            focus: cx.focus_handle(),
            active_document: None,
            selections: RecordSelections::default(),
            number_edit: None,
            text_edit: None,
            game_paths,
            root_revisions: discovery_status::RootRevisions::default(),
            recent_files,
            catalog: CatalogSession::default(),
            discovery: discovery_status::DiscoveryStatus::default(),
            settings,
            notices,
            next_workspace_notice: 1,
            window_handle: None,
            close_armed: false,
            close_pending: false,
            close_documents: CloseDocuments::Save,
            close_prompt_open: false,
            open_prompt_launcher: Rc::new(PlatformOpenPromptLauncher),
            open_path_loader: Rc::new(FileSystemOpenPathLoader),
            browse_prompt_launcher: Rc::new(PlatformBrowsePromptLauncher),
            #[cfg(test)]
            task_launches: TaskLaunchCounts::default(),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn cancel_property_edit(&mut self) {
        self.number_edit = None;
        self.text_edit = None;
        clear_editor_notice(&mut self.notices);
    }

    fn begin_number_edit(&mut self, edit: ActiveNumberEdit) {
        self.cancel_property_edit();
        self.number_edit = Some(edit);
    }

    fn activate_document(&mut self, document: DocumentID) {
        self.cancel_property_edit();
        self.active_document = Some(document);
    }

    fn select_record(&mut self, document: DocumentID, record: usize) {
        self.cancel_property_edit();
        self.active_document = Some(document);
        self.selections.select(document, record);
    }

    fn text_input_colors(&self) -> TextInputColors {
        TextInputColors {
            background: self.theme.raised,
            border: self.theme.accent,
            text: self.theme.text,
            placeholder: self.theme.text_dim,
            selection: self.theme.accent_dim,
            cursor: self.theme.accent,
        }
    }

    fn start_text_edit(
        &mut self,
        target: TextEditTarget,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_property_edit();
        let colors = self.text_input_colors();
        let input = cx.new(|cx| TextInput::new(value, target.label(), colors, cx));
        cx.subscribe(&input, |frame, input, event, cx| {
            frame.handle_text_input_event(&input, event, cx);
        })
        .detach();
        window.focus(&input.read(cx).focus_handle());
        self.text_edit = Some(ActiveTextEdit { target, input });
        cx.notify();
    }

    fn handle_text_input_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &TextInputEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self
            .text_edit
            .as_ref()
            .filter(|edit| edit.input == *input)
            .map(|edit| edit.target)
        else {
            return;
        };
        if self.active_document != Some(target.document()) {
            self.cancel_property_edit();
            self.notices.replace(
                NoticeSource::Workspace,
                Notice::info("The active document changed; edit canceled"),
            );
            cx.notify();
            return;
        }

        match event {
            TextInputEvent::Cancel => self.cancel_property_edit(),
            TextInputEvent::Commit(value) => {
                let (document, edit) = target.document_edit(value.clone());
                match self.workspace.apply(document, edit) {
                    Ok(()) => {
                        self.document_did_mutate();
                        self.cancel_property_edit();
                        self.notices.clear(NoticeSource::Editor);
                    }
                    Err(error) => {
                        let summary = match target {
                            TextEditTarget::TextSOX { document, record } => self
                                .workspace
                                .text_sox_max_length(document, record)
                                .map_or_else(
                                    |_| format!("Could not update {}", target.format_name()),
                                    |maximum| {
                                        format!(
                                            "Could not update {} · enter 1..={maximum} bytes",
                                            target.format_name()
                                        )
                                    },
                                ),
                            TextEditTarget::Skill { .. } => {
                                format!("Could not update {}", target.format_name())
                            }
                        };
                        self.notices
                            .replace(NoticeSource::Editor, Notice::editor_error(summary, &error));
                    }
                }
            }
        }
        cx.notify();
    }

    fn set_skill_type(&mut self, document: DocumentID, record: usize, choice: SkillTypeChoice) {
        self.cancel_property_edit();
        if self.active_document != Some(document) {
            self.notices.replace(
                NoticeSource::Workspace,
                Notice::info("The active document changed; edit canceled"),
            );
            return;
        }
        match self.workspace.apply(document, choice.document_edit(record)) {
            Ok(()) => {
                self.document_did_mutate();
                self.notices.clear(NoticeSource::Editor);
            }
            Err(error) => {
                self.notices.replace(
                    NoticeSource::Editor,
                    Notice::editor_error("Could not update SkillInfo", &error),
                );
            }
        }
    }

    pub fn window_should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.cancel_property_edit();
        cx.notify();
        self.window_handle = Some(window.window_handle());
        if std::mem::take(&mut self.close_armed) {
            return true;
        }
        if self.close_prompt_open || self.close_pending {
            return false;
        }
        if self.close_documents == CloseDocuments::Discard {
            return self.begin_settings_close(window, cx);
        }

        match ClosePolicy::from_dirty_count(self.dirty_count()) {
            ClosePolicy::Allow => self.begin_settings_close(window, cx),
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

    fn begin_settings_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.settings.has_failed() {
            self.close_prompt_open = true;
            let answer = window.prompt(
                PromptLevel::Warning,
                "Settings could not be saved. Retry before closing?",
                None,
                &["Retry", "Close Without Saving", "Cancel"],
                cx,
            );
            cx.spawn_in(window, async move |entity, cx| {
                let answer = answer.await.ok();
                let _ = entity.update_in(cx, move |frame, window, cx| {
                    frame.finish_settings_close_prompt(answer, window, cx);
                });
            })
            .detach();
            return false;
        }
        if !self.settings.is_settled() {
            self.close_pending = true;
            return false;
        }
        true
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_prompt_open = false;
        match answer {
            Some(0) => {
                self.close_pending = true;
                self.close_documents = CloseDocuments::Save;
                self.continue_close(cx);
            }
            Some(1) => {
                self.close_pending = true;
                self.close_documents = CloseDocuments::Discard;
                self.continue_close(cx);
            }
            Some(_) | None => {
                self.close_pending = false;
                self.close_documents = CloseDocuments::Save;
            }
        }
        cx.notify();
    }

    fn finish_settings_close_prompt(
        &mut self,
        answer: Option<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_prompt_open = false;
        match answer {
            Some(0) => {
                if let Some(revision) = self.settings.retry_failed() {
                    self.notices.begin(
                        NoticeSource::SettingsWrite,
                        revision.get(),
                        Notice::info("Saving application settings"),
                    );
                    self.close_pending = true;
                    self.start_next_settings_write(cx);
                }
            }
            Some(1) => {
                self.settings.discard_failed();
                self.close_pending = true;
                self.continue_close(cx);
            }
            Some(_) | None => {
                self.close_pending = false;
                self.close_documents = CloseDocuments::Save;
            }
        }
        cx.notify();
    }

    fn continue_close(&mut self, cx: &mut Context<Self>) {
        if !self.close_pending {
            return;
        }

        if self.close_documents == CloseDocuments::Save {
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
        }
        if self.settings.has_failed() {
            self.close_pending = false;
            return;
        }
        if !self.settings.is_settled() {
            return;
        }
        self.close_pending = false;
        self.close_documents = CloseDocuments::Save;
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
            NumberOutcome::Continue => {
                if self
                    .number_edit
                    .as_ref()
                    .is_some_and(|edit| edit.editor.is_valid())
                {
                    clear_editor_notice(&mut self.notices);
                }
            }
            NumberOutcome::Invalid => self
                .notices
                .replace(NoticeSource::Editor, invalid_number_notice()),
            NumberOutcome::Cancel => self.cancel_property_edit(),
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
            self.cancel_property_edit();
            self.notices.replace(
                NoticeSource::Workspace,
                Notice::info("The active document changed; edit canceled"),
            );
            return;
        }

        let Some(edit) = self.number_edit.as_ref() else {
            return;
        };
        let target = edit.target;
        let (document, document_edit) = match target.document_edit(value) {
            Ok(edit) => edit,
            Err(error) => {
                self.notices.replace(
                    NoticeSource::Editor,
                    Notice::editor_error(
                        format!("Could not update {}", target.format_name()),
                        &error,
                    ),
                );
                return;
            }
        };
        match self.workspace.apply(document, document_edit) {
            Ok(()) => {
                self.document_did_mutate();
                self.cancel_property_edit();
                self.notices.clear(NoticeSource::Editor);
            }
            Err(error) => {
                self.notices.replace(
                    NoticeSource::Editor,
                    Notice::editor_error(
                        format!("Could not update {}", target.format_name()),
                        &error,
                    ),
                );
            }
        }
    }

    fn open_action(&mut self, _: &OpenFile, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_property_edit();
        let request = self.begin_open_prompt(cx);
        let prompt = Rc::clone(&self.open_prompt_launcher).launch(
            self,
            request,
            PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("Open".into()),
            },
            cx,
        );

        cx.spawn(async move |entity, cx| {
            let paths = match prompt.await {
                OpenPromptResult::Selected(paths) => paths,
                OpenPromptResult::Canceled => {
                    set_open_notice(&entity, cx, request, None);
                    return;
                }
                OpenPromptResult::Failed(notice) => {
                    set_open_notice(&entity, cx, request, Some(notice));
                    return;
                }
            };
            let _ = entity.update(cx, move |frame, cx| {
                frame.open_paths(request, paths, cx);
            });
        })
        .detach();
    }

    fn begin_open_prompt(&mut self, cx: &mut Context<Self>) -> RequestID {
        let request = self.shell.begin_open();
        self.notices.begin(
            NoticeSource::Open,
            request.get(),
            Notice::info("Choose one or more .sox files"),
        );
        cx.notify();
        request
    }

    fn open_recent_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.cancel_property_edit();
        let request = self.shell.begin_open();
        self.notices.begin(
            NoticeSource::Open,
            request.get(),
            Notice::info(format!("Opening {}", display_name(&path))),
        );
        cx.notify();
        self.open_paths(request, vec![path], cx);
    }

    fn open_paths(&mut self, request: RequestID, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !self.shell.accepts_open(request) {
            return;
        }
        let loader = Rc::clone(&self.open_path_loader);
        let tasks = paths
            .into_iter()
            .map(|path| loader.start(path, cx.background_executor()))
            .collect::<Vec<_>>();

        cx.spawn(async move |entity, cx| {
            let mut batch = Vec::with_capacity(tasks.len());
            for task in tasks {
                batch.push(task.await);
            }
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_open_paths(request, batch, cx);
            });
        })
        .detach();
    }

    fn finish_open_paths(
        &mut self,
        request: RequestID,
        batch: Vec<(PathBuf, Result<LoadedDocument, WorkspaceError>)>,
        cx: &mut Context<Self>,
    ) {
        if !self.shell.accepts_open(request) {
            return;
        }

        let mut accepted = Vec::new();
        let mut failures = Vec::new();
        for (path, result) in batch {
            match result {
                Ok(loaded) => accepted.push((path, loaded)),
                Err(error) => failures.push((path, error)),
            }
        }

        let unicode_paths = accepted
            .iter()
            .filter(|(path, _)| path.to_str().is_some())
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        let accepted_count = accepted.len();
        let omitted_count = accepted.len() - unicode_paths.len();
        if !unicode_paths.is_empty() {
            let previous = self.recent_files.clone();
            self.recent_files.add_batch(unicode_paths);
            if !self.schedule_settings_write(self.shell.game(), cx) {
                self.recent_files = previous;
            }
        }

        for (index, (path, loaded)) in accepted.into_iter().enumerate() {
            let (_, loaded_document) = loaded.into_parts();
            let document = self.workspace.open_loaded(path, loaded_document);
            if index == 0 {
                self.activate_document(document);
                self.shell.select_area(Area::Files);
            }
        }

        let failed_count = failures.len();
        let notice = if failed_count > 0 {
            Notice::error_lines(
                format!(
                    "Opened {}; {} failed",
                    file_count(accepted_count),
                    failed_count
                ),
                failures
                    .into_iter()
                    .map(|(path, error)| (path.display().to_string(), error)),
            )
        } else if omitted_count > 0 {
            Notice::plain(
                NoticeLevel::Warning,
                format!(
                    "Opened {}; {} omitted from recent files",
                    file_count(accepted_count),
                    path_count(omitted_count)
                ),
            )
        } else {
            Notice::success(format!("Opened {}", file_count(accepted_count)))
        };
        self.notices
            .complete(NoticeSource::Open, request.get(), Some(notice));
        cx.notify();
    }

    fn save_action(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_property_edit();
        if let Some(document_id) = self.active_document {
            self.start_save(document_id, None, cx);
        }
    }

    fn save_all_action(&mut self, _: &SaveAll, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_property_edit();
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
            self.notices.replace(
                NoticeSource::Workspace,
                Notice::info("All documents are already saved"),
            );
            cx.notify();
        }
    }

    fn save_as_action(&mut self, _: &SaveAs, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_property_edit();
        cx.notify();
        let Some(document_id) = self.active_document else {
            return;
        };
        let current_path = match self.workspace.path(document_id) {
            Ok(path) => path.to_path_buf(),
            Err(error) => {
                self.notices.replace(
                    NoticeSource::Workspace,
                    Notice::error("Could not determine the save path", &error),
                );
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
        let notice_identity = self.allocate_workspace_notice_identity();
        self.notices
            .begin_pending(NoticeSource::Workspace, notice_identity);
        let prompt = cx.prompt_for_new_path(&parent, suggested.as_deref());

        cx.spawn(async move |entity, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(path))) => SaveAsPromptResult::Selected(path),
                Ok(Ok(None)) => SaveAsPromptResult::Canceled,
                Ok(Err(error)) => SaveAsPromptResult::Failed(Notice::error(
                    "Could not open Save As",
                    error.as_ref(),
                )),
                Err(error) => {
                    SaveAsPromptResult::Failed(Notice::error("Save As did not respond", &error))
                }
            };
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_save_as_prompt(notice_identity, document_id, result, cx);
            });
        })
        .detach();
    }

    fn finish_save_as_prompt(
        &mut self,
        notice_identity: u64,
        document_id: DocumentID,
        result: SaveAsPromptResult,
        cx: &mut Context<Self>,
    ) {
        match result {
            SaveAsPromptResult::Selected(path) => {
                if self.notices.complete(
                    NoticeSource::Workspace,
                    notice_identity,
                    Some(Notice::info("Saving document")),
                ) {
                    self.start_save_request(document_id, Some(path), notice_identity, cx);
                }
            }
            SaveAsPromptResult::Canceled => {
                self.notices
                    .cancel(NoticeSource::Workspace, notice_identity);
            }
            SaveAsPromptResult::Failed(notice) => {
                if self
                    .notices
                    .complete(NoticeSource::Workspace, notice_identity, Some(notice))
                {
                    cx.notify();
                }
            }
        }
    }

    fn undo_action(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.move_history(false, cx);
    }

    fn redo_action(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.move_history(true, cx);
    }

    fn move_history(&mut self, redo: bool, cx: &mut Context<Self>) {
        self.cancel_property_edit();
        let Some(document_id) = self.active_document else {
            return;
        };
        let result = if redo {
            self.workspace.redo(document_id)
        } else {
            self.workspace.undo(document_id)
        };
        match result {
            Ok(true) => {
                self.document_did_mutate();
                self.notices.clear(NoticeSource::Workspace);
            }
            Ok(false) => {}
            Err(error) => {
                self.notices.replace(
                    NoticeSource::Workspace,
                    Notice::error("Could not change document history", &error),
                );
            }
        }
        cx.notify();
    }

    fn start_save(
        &mut self,
        document_id: DocumentID,
        target: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> bool {
        let notice_identity = self.allocate_workspace_notice_identity();
        self.notices.begin(
            NoticeSource::Workspace,
            notice_identity,
            Notice::info("Saving document"),
        );
        self.start_save_request(document_id, target, notice_identity, cx)
    }

    fn start_save_request(
        &mut self,
        document_id: DocumentID,
        target: Option<PathBuf>,
        notice_identity: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let request = match self.workspace.prepare_save(document_id, target) {
            Ok(request) => request,
            Err(error) => {
                self.notices.complete(
                    NoticeSource::Workspace,
                    notice_identity,
                    Some(Notice::error("Could not start save", &error)),
                );
                self.close_pending = false;
                self.close_documents = CloseDocuments::Save;
                self.close_armed = false;
                cx.notify();
                return false;
            }
        };
        let request_document = request.document_id();
        let token = request.token();
        let task = cx.background_executor().spawn(async move { request.run() });
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, move |frame, cx| {
                frame.finish_save_result(request_document, token, notice_identity, result);
                frame.continue_close(cx);
                cx.notify();
            });
        })
        .detach();
        true
    }

    fn finish_save_result(
        &mut self,
        document_id: DocumentID,
        token: SaveToken,
        notice_identity: u64,
        result: Result<kufeditor_workspace::SavedDocument, kufeditor_workspace::WorkspaceError>,
    ) {
        match result {
            Ok(saved) => match self.workspace.finish_save(saved) {
                Ok(()) => {
                    let name = self
                        .workspace
                        .path(document_id)
                        .map_or_else(|_| "document".to_owned(), display_name);
                    self.notices.complete(
                        NoticeSource::Workspace,
                        notice_identity,
                        Some(Notice::success(format!("Saved {name}"))),
                    );
                }
                Err(error) => {
                    self.cancel_close_after_document_save_failure();
                    self.notices.complete(
                        NoticeSource::Workspace,
                        notice_identity,
                        Some(Notice::error("Could not finish save", &error)),
                    );
                }
            },
            Err(error) => match self.workspace.finish_save_failure(document_id, token) {
                Ok(()) => {
                    self.cancel_close_after_document_save_failure();
                    self.notices.complete(
                        NoticeSource::Workspace,
                        notice_identity,
                        Some(Notice::error("Could not save document", &error)),
                    );
                }
                Err(cleanup_error) => {
                    self.cancel_close_after_document_save_failure();
                    self.notices.complete(
                        NoticeSource::Workspace,
                        notice_identity,
                        Some(Notice::error(
                            "Could not reconcile failed save",
                            &cleanup_error,
                        )),
                    );
                }
            },
        }
    }

    fn allocate_workspace_notice_identity(&mut self) -> u64 {
        let identity = self.next_workspace_notice;
        self.next_workspace_notice += 1;
        identity
    }

    fn cancel_close_after_document_save_failure(&mut self) {
        if self.close_documents == CloseDocuments::Save {
            self.close_pending = false;
            self.close_armed = false;
        }
    }

    fn document_did_mutate(&mut self) {
        if self.close_documents == CloseDocuments::Discard {
            self.close_documents = CloseDocuments::Save;
            self.close_pending = false;
            self.close_armed = false;
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
                        frame.select_game(game, cx);
                    }))
            }))
    }

    fn select_game(&mut self, game: Game, cx: &mut Context<Self>) {
        if self.shell.game() == game {
            return;
        }
        if !self.schedule_settings_write(game, cx) {
            return;
        }
        self.shell.select_game(game);
        self.cancel_property_edit();
        self.start_catalog_load(cx);
        cx.notify();
    }

    fn select_area(&mut self, area: Area, cx: &mut Context<Self>) {
        if self.shell.area() == area {
            return;
        }
        self.shell.select_area(area);
        self.cancel_property_edit();
        cx.notify();
    }

    pub(crate) fn set_recent_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        let previous = self.recent_files.clone();
        if !self.recent_files.set_limit(limit) {
            return;
        }
        if !self.schedule_settings_write(self.shell.game(), cx) {
            self.recent_files = previous;
            return;
        }
        #[cfg(test)]
        {
            self.task_launches.settings += 1;
        }
        cx.notify();
    }

    pub(crate) fn clear_recent_files(&mut self, cx: &mut Context<Self>) {
        let previous = self.recent_files.clone();
        if !self.recent_files.clear() {
            return;
        }
        if !self.schedule_settings_write(self.shell.game(), cx) {
            self.recent_files = previous;
            return;
        }
        #[cfg(test)]
        {
            self.task_launches.settings += 1;
        }
        cx.notify();
    }

    fn navigation(&self, cx: &mut Context<Self>) -> Div {
        let projection = navigation_projection();
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
            .children(projection.primary.iter().copied().map(|area| {
                components::rail_item(
                    &self.theme,
                    area.element_id(),
                    area.label(),
                    self.shell.area() == area,
                )
                .on_click(cx.listener(move |frame, _, _, cx| {
                    frame.select_area(area, cx);
                }))
            }))
            .child(div().flex_1())
            .child(
                components::rail_item(
                    &self.theme,
                    projection.bottom.element_id(),
                    projection.bottom.label(),
                    self.shell.area() == projection.bottom,
                )
                .on_click(cx.listener(move |frame, _, _, cx| {
                    frame.select_area(projection.bottom, cx);
                })),
            )
    }

    fn home_recent_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match views::home::project_recent_files(self.recent_files.paths()) {
            views::home::RecentFilesProjection::Empty(message) => vec![
                div()
                    .id("home-recent-empty")
                    .py(px(12.0))
                    .text_color(self.theme.text_dim)
                    .child(message)
                    .into_any_element(),
            ],
            views::home::RecentFilesProjection::Rows(rows) => rows
                .into_iter()
                .enumerate()
                .map(|(index, row)| {
                    let path = row.path;
                    let hover = self.theme.raised;
                    div()
                        .id(("home-recent-row", index))
                        .w_full()
                        .px(px(12.0))
                        .py(px(10.0))
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .border_b_1()
                        .border_color(self.theme.border)
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click(cx.listener(move |frame, _, _, cx| {
                            frame.open_recent_path(path.clone(), cx);
                        }))
                        .child(div().text_color(self.theme.text).child(row.label))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(self.theme.text_dim)
                                .child(row.secondary),
                        )
                        .into_any_element()
                })
                .collect(),
        }
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.shell.area() {
            Area::Home => {
                views::home::render(&self.theme, self.shell.game(), self.home_recent_rows(cx))
                    .into_any_element()
            }
            Area::Files => {
                let editor = self
                    .active_document
                    .map(|document_id| self.document_editor(document_id, cx));
                views::files::render(&self.theme, self.document_tabs(cx), editor).into_any_element()
            }
            Area::Mods => views::mods::render(&self.theme).into_any_element(),
            Area::Patches => views::patches::render(&self.theme).into_any_element(),
            Area::Settings => {
                let projection = views::settings::project_settings(
                    &self.game_paths,
                    self.catalog.status(),
                    self.discovery.status(),
                    &self.recent_files,
                    kufeditor_game::steam_discovery_available(),
                );
                views::settings::render(&self.theme, projection, cx).into_any_element()
            }
        }
    }

    fn document_editor(&self, document_id: DocumentID, cx: &mut Context<Self>) -> Div {
        match self.workspace.document_kind(document_id).map(editor_route) {
            Ok(EditorRoute::Troop) => self.troop_editor(document_id, cx),
            Ok(EditorRoute::Skill) => self.skill_editor(document_id, cx),
            Ok(EditorRoute::TextSOX) => self.text_sox_editor(document_id, cx),
            Err(error) => div()
                .size_full()
                .p(px(28.0))
                .text_color(self.theme.text_dim)
                .child(format!("Could not open the document editor: {error}")),
        }
    }

    fn skill_editor(&self, document_id: DocumentID, cx: &mut Context<Self>) -> Div {
        let record_count = match self.workspace.record_count(document_id) {
            Ok(count) => count,
            Err(error) => {
                return div()
                    .size_full()
                    .p(px(28.0))
                    .text_color(self.theme.text_dim)
                    .child(format!("Could not read SkillInfo: {error}"));
            }
        };
        let selected = self
            .selections
            .selected(document_id)
            .min(record_count.saturating_sub(1));
        let records = self.skill_records(document_id, record_count, selected, cx);
        let details = if record_count == 0 {
            vec![
                div()
                    .text_color(self.theme.text_dim)
                    .child("This file has no skill records.")
                    .into_any_element(),
            ]
        } else {
            vec![
                self.skill_group(document_id, selected, cx)
                    .into_any_element(),
            ]
        };
        let diagnostics = self.skill_diagnostics(document_id);
        views::skill::render(&self.theme, records, details, diagnostics)
    }

    fn skill_records(
        &self,
        document_id: DocumentID,
        record_count: usize,
        selected: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        (0..record_count)
            .map(|record| {
                views::skill::record_row(
                    &self.theme,
                    ("skill-record", record),
                    record,
                    record == selected,
                )
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_record(document_id, record);
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect()
    }

    fn skill_group(&self, document_id: DocumentID, record: usize, cx: &mut Context<Self>) -> Div {
        let fields = vec![
            self.skill_id_field(document_id, record, cx),
            self.skill_text_field(document_id, record, SkillTextField::LocalizationKey, 1, cx),
            self.skill_text_field(document_id, record, SkillTextField::IconPath, 2, cx),
            self.skill_type_field(document_id, record, cx),
            self.skill_max_level_field(document_id, record, cx),
        ];
        views::skill::group(
            &self.theme,
            views::skill::skill_name(record).into_owned(),
            fields,
        )
    }

    fn skill_id_field(
        &self,
        document_id: DocumentID,
        record: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = self.workspace.skill_id(document_id, record);
        let active_edit = self
            .number_edit
            .as_ref()
            .filter(|edit| edit.target.is_skill_id(document_id, record));
        let display = active_edit.map_or_else(
            || {
                value
                    .as_ref()
                    .map_or_else(|_| "—".to_owned(), i32::to_string)
            },
            |edit| edit.editor.draft().to_owned(),
        );
        let row = views::skill::number_field_row(
            &self.theme,
            ("skill-field", 0_usize),
            "Skill ID",
            display,
            active_edit.is_some(),
            active_edit.is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
        );
        match value {
            Ok(value) => row
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.begin_number_edit(ActiveNumberEdit::skill_id(document_id, record, value));
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element(),
            Err(_) => row.into_any_element(),
        }
    }

    fn skill_text_field(
        &self,
        document_id: DocumentID,
        record: usize,
        field: SkillTextField,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = TextEditTarget::skill(document_id, record, field);
        if let Some(edit) = self.text_edit.as_ref().filter(|edit| edit.target == target) {
            return views::skill::text_editor_row(
                &self.theme,
                field.label(),
                edit.input.clone().into_any_element(),
            )
            .into_any_element();
        }

        match skill_text_projection(&self.workspace, document_id, record, field) {
            SkillTextProjection::Editable(value) => views::skill::text_field_row(
                &self.theme,
                ("skill-field", index),
                field.label(),
                value.clone(),
            )
            .on_click(cx.listener(move |frame, _, window, cx| {
                frame.start_text_edit(target, value.clone(), window, cx);
            }))
            .into_any_element(),
            SkillTextProjection::Invalid { value, diagnostic } => {
                views::skill::invalid_text_field(&self.theme, field.label(), value, diagnostic)
                    .into_any_element()
            }
        }
    }

    fn skill_type_field(
        &self,
        document_id: DocumentID,
        record: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = self.workspace.skill_type(document_id, record).ok();
        let choices = SkillTypeChoice::ALL
            .into_iter()
            .enumerate()
            .map(|(index, choice)| {
                components::choice_button(
                    &self.theme,
                    ("skill-type", index),
                    choice.label(),
                    value == Some(choice.value()),
                )
                .on_click(cx.listener(move |frame, _, _, cx| {
                    frame.set_skill_type(document_id, record, choice);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect();
        views::skill::choice_row(&self.theme, choices).into_any_element()
    }

    fn skill_max_level_field(
        &self,
        document_id: DocumentID,
        record: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = self.workspace.skill_max_level(document_id, record);
        let active_edit = self
            .number_edit
            .as_ref()
            .filter(|edit| edit.target.is_skill_max_level(document_id, record));
        let display = active_edit.map_or_else(
            || {
                value
                    .as_ref()
                    .map_or_else(|_| "—".to_owned(), u32::to_string)
            },
            |edit| edit.editor.draft().to_owned(),
        );
        let row = views::skill::number_field_row(
            &self.theme,
            ("skill-field", 4_usize),
            "Maximum Level",
            display,
            active_edit.is_some(),
            active_edit.is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
        );
        match value {
            Ok(value) => row
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.begin_number_edit(ActiveNumberEdit::skill_max_level(
                        document_id,
                        record,
                        value,
                    ));
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element(),
            Err(_) => row.into_any_element(),
        }
    }

    fn skill_diagnostics(&self, document_id: DocumentID) -> Vec<AnyElement> {
        let diagnostics = self.workspace.diagnostics(document_id).unwrap_or_default();
        if diagnostics.is_empty() {
            return vec![views::skill::no_diagnostics(&self.theme).into_any_element()];
        }

        diagnostics
            .into_iter()
            .map(|diagnostic| {
                let item = views::skill::diagnostic_item(diagnostic);
                views::skill::diagnostic_row(&self.theme, &item).into_any_element()
            })
            .collect()
    }

    fn text_sox_editor(&self, document_id: DocumentID, cx: &mut Context<Self>) -> Div {
        let record_count = match self.workspace.record_count(document_id) {
            Ok(count) => count,
            Err(error) => {
                return div()
                    .size_full()
                    .p(px(28.0))
                    .text_color(self.theme.text_dim)
                    .child(format!("Could not read text SOX: {error}"));
            }
        };
        let selected = self
            .selections
            .selected(document_id)
            .min(record_count.saturating_sub(1));
        let records = self.text_sox_records(document_id, record_count, selected, cx);
        let details = if record_count == 0 {
            vec![views::text::empty_properties(&self.theme).into_any_element()]
        } else {
            vec![
                self.text_sox_property_group(document_id, selected, cx)
                    .into_any_element(),
            ]
        };
        let diagnostics = self.text_sox_diagnostics(document_id);
        views::text::render(&self.theme, records, details, diagnostics)
    }

    fn text_sox_records(
        &self,
        document_id: DocumentID,
        record_count: usize,
        selected: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        (0..record_count)
            .map(|record| {
                let wire_index = self.workspace.text_sox_index(document_id, record);
                let maximum = self.workspace.text_sox_max_length(document_id, record);
                let text = self.workspace.text_sox_text(document_id, record);
                let (wire_index, maximum, used, text_preview) = match (wire_index, maximum, text) {
                    (Ok(wire_index), Ok(maximum), Ok(text)) => {
                        (wire_index, maximum, text.len(), views::text::preview(text))
                    }
                    _ => (0, 0, 0, "Unavailable".to_owned()),
                };
                views::text::record_row(
                    &self.theme,
                    ("text-record", record),
                    views::text::entry_metadata(record, wire_index, used, maximum),
                    text_preview,
                    record == selected,
                )
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.select_record(document_id, record);
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect()
    }

    fn text_sox_property_group(
        &self,
        document_id: DocumentID,
        record: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let wire_index = self.workspace.text_sox_index(document_id, record);
        let maximum = self.workspace.text_sox_max_length(document_id, record);
        let text = self.workspace.text_sox_text(document_id, record);
        let (Ok(wire_index), Ok(maximum), Ok(text)) = (wire_index, maximum, text) else {
            return div()
                .text_color(self.theme.text_dim)
                .child("Could not read the selected text entry.");
        };
        let target = TextEditTarget::text_sox(document_id, record);
        let text_field =
            if let Some(edit) = self.text_edit.as_ref().filter(|edit| edit.target == target) {
                let current = edit.input.read(cx).content().len();
                views::text::text_editor_row(
                    &self.theme,
                    edit.input.clone().into_any_element(),
                    current,
                    maximum,
                )
                .into_any_element()
            } else {
                let value = text.to_owned();
                views::text::text_field_row(
                    &self.theme,
                    ("text-field", record),
                    views::text::preview(&value),
                )
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.start_text_edit(target, value.clone(), window, cx);
                }))
                .into_any_element()
            };
        views::text::property_group(&self.theme, wire_index, maximum, text_field)
    }

    fn text_sox_diagnostics(&self, document_id: DocumentID) -> Vec<AnyElement> {
        let diagnostics = self.workspace.diagnostics(document_id).unwrap_or_default();
        if diagnostics.is_empty() {
            return vec![views::text::no_diagnostics(&self.theme).into_any_element()];
        }

        diagnostics
            .into_iter()
            .map(|diagnostic| {
                let wire_index = diagnostic
                    .location
                    .record()
                    .and_then(|record| self.workspace.text_sox_index(document_id, record).ok());
                let item = views::text::diagnostic_item(diagnostic, wire_index);
                views::text::diagnostic_row(&self.theme, &item).into_any_element()
            })
            .collect()
    }

    fn troop_editor(&self, document_id: DocumentID, cx: &mut Context<Self>) -> Div {
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
        let selected = self
            .selections
            .selected(document_id)
            .min(record_count.saturating_sub(1));
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
        document_id: DocumentID,
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
                    frame.select_record(document_id, record);
                    window.focus(&frame.focus);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect()
    }

    fn troop_groups(
        &self,
        document_id: DocumentID,
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
        document_id: DocumentID,
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
            active_edit.is_some_and(|edit| edit.editor.invalid() || !edit.editor.is_valid()),
        );

        match value {
            Ok(value) => row
                .on_click(cx.listener(move |frame, _, window, cx| {
                    frame.begin_number_edit(ActiveNumberEdit::troop_field(
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

    fn troop_diagnostics(&self, document_id: DocumentID) -> Vec<AnyElement> {
        let diagnostics = self.workspace.diagnostics(document_id).unwrap_or_default();
        if diagnostics.is_empty() {
            return vec![views::troop::no_diagnostics(&self.theme).into_any_element()];
        }

        diagnostics
            .into_iter()
            .map(|diagnostic| {
                let title = troop_diagnostic_title(diagnostic.location);
                views::troop::diagnostic_row(
                    &self.theme,
                    diagnostic.severity,
                    title,
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
                    frame.activate_document(document_id);
                    cx.notify();
                }))
                .into_any_element()
            })
            .collect()
    }

    fn notice_bar(&self) -> Option<AnyElement> {
        self.notices.current().map(|notice| {
            let label = match notice.level() {
                NoticeLevel::Info => "INFO",
                NoticeLevel::Success => "SAVED",
                NoticeLevel::Warning => "WARNING",
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
    request: RequestID,
    notice: Option<Notice>,
) {
    let _ = entity.update(cx, move |frame, cx| {
        if frame.shell.accepts_open(request) {
            frame
                .notices
                .complete(NoticeSource::Open, request.get(), notice);
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

fn file_count(count: usize) -> String {
    let label = if count == 1 { "file" } else { "files" };
    format!("{count} {label}")
}

fn path_count(count: usize) -> String {
    let label = if count == 1 { "path" } else { "paths" };
    format!("{count} {label}")
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

    use std::{
        cell::Cell,
        fs,
        future::Future,
        path::PathBuf,
        pin::Pin,
        rc::Rc,
        sync::{Arc, Mutex},
        task::{Context as TaskContext, Poll, Waker},
    };

    use gpui::{
        AppContext, BackgroundExecutor, Context, EntityInputHandler, PathPromptOptions, Render,
        Task, TestAppContext, WindowOptions,
    };
    use kufeditor_game::Game;
    use kufeditor_workspace::{
        DiagnosticLocation, Document, DocumentEdit, DocumentID, DocumentKind, LoadedDocument,
        SaveNumberTarget, SkillDocument, SkillTextField, TextSOXDocument, TroopDocument,
        TroopField, Workspace, WorkspaceError, load_path,
    };

    use super::{
        ActiveNumberEdit, AppFrame, CloseDocuments, EditorRoute, OpenPathLoader,
        OpenPromptLauncher, OpenPromptResult, SkillTextProjection, SkillTypeChoice, TextEditTarget,
        editor_route, invalid_number_notice, skill_text_projection, troop_diagnostic_title,
    };
    use crate::{
        actions::{OpenFile, SaveAs},
        catalog_status::CatalogStatus,
        frame::discovery_status::DiscoveryStatus,
        notices::{Notice, NoticeLevel, NoticeSource},
        settings::{SettingsQueueResult, SettingsStartup, image_from_runtime},
        state::{Area, RecordSelections, RequestID, navigation_projection},
        text_input::{TextInputEvent, bind as bind_text_input},
    };

    fn test_startup() -> SettingsStartup {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        drop(file);
        SettingsStartup::load(path)
    }

    #[test]
    fn troop_diagnostic_title_uses_document_location_without_record_prefix() {
        assert_eq!(
            troop_diagnostic_title(DiagnosticLocation::Save(SaveNumberTarget::CampaignIndex)),
            "Campaign"
        );
    }

    fn write_valid_sox(path: &std::path::Path) {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(0..8)
            .unwrap()
            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
        fs::write(path, bytes).unwrap();
    }

    fn write_invalid_sox(path: &std::path::Path) {
        fs::write(path, b"not a valid SOX document").unwrap();
    }

    fn begin_open_paths(
        frame: &mut AppFrame,
        paths: Vec<PathBuf>,
        cx: &mut gpui::Context<AppFrame>,
    ) {
        let request = frame.shell.begin_open();
        frame.notices.begin(
            NoticeSource::Open,
            request.get(),
            Notice::info("Opening files"),
        );
        frame.open_paths(request, paths, cx);
    }

    fn open_paths_in_workspace(frame: &AppFrame) -> Vec<PathBuf> {
        frame
            .workspace
            .document_ids()
            .iter()
            .map(|document| frame.workspace.path(*document).unwrap().to_path_buf())
            .collect()
    }

    struct PromptLaunchProbe {
        launched: Rc<Cell<bool>>,
        request_ready: Rc<Cell<bool>>,
        notice_ready: Rc<Cell<bool>>,
    }

    impl OpenPromptLauncher for PromptLaunchProbe {
        fn launch(
            &self,
            frame: &AppFrame,
            request: RequestID,
            _: PathPromptOptions,
            _: &mut Context<AppFrame>,
        ) -> Task<OpenPromptResult> {
            self.launched.set(true);
            self.request_ready.set(frame.shell.accepts_open(request));
            self.notice_ready.set(
                frame.notices.current().map(Notice::summary)
                    == Some("Choose one or more .sox files"),
            );
            Task::ready(OpenPromptResult::Canceled)
        }
    }

    type LoadResult = Result<LoadedDocument, WorkspaceError>;

    #[derive(Default)]
    struct ManualLoadState {
        result: Option<LoadResult>,
        waker: Option<Waker>,
    }

    struct ManualLoadFuture {
        state: Arc<Mutex<ManualLoadState>>,
    }

    impl Future for ManualLoadFuture {
        type Output = LoadResult;

        fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
            let mut state = self.state.lock().unwrap();
            if let Some(result) = state.result.take() {
                Poll::Ready(result)
            } else {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    #[derive(Default)]
    struct ControlledOpenPathLoader {
        started: Arc<Mutex<Vec<PathBuf>>>,
        pending: Mutex<Vec<Arc<Mutex<ManualLoadState>>>>,
    }

    impl ControlledOpenPathLoader {
        fn started_paths(&self) -> Vec<PathBuf> {
            self.started.lock().unwrap().clone()
        }

        fn release(&self, index: usize, result: LoadResult) {
            let state = Arc::clone(self.pending.lock().unwrap().get(index).unwrap());
            let waker = {
                let mut state = state.lock().unwrap();
                state.result = Some(result);
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    impl OpenPathLoader for ControlledOpenPathLoader {
        fn start(
            &self,
            path: PathBuf,
            executor: &BackgroundExecutor,
        ) -> Task<(PathBuf, LoadResult)> {
            let state = Arc::new(Mutex::new(ManualLoadState::default()));
            self.pending.lock().unwrap().push(Arc::clone(&state));
            let started = Arc::clone(&self.started);
            executor.spawn(async move {
                started.lock().unwrap().push(path.clone());
                let result = ManualLoadFuture { state }.await;
                (path, result)
            })
        }
    }

    #[test]
    fn invalid_number_notice_explains_the_allowed_range() {
        let notice = invalid_number_notice();

        assert_eq!(notice.level(), NoticeLevel::Info);
        assert_eq!(
            notice.summary(),
            "Enter a whole number within the allowed range"
        );
    }

    fn open_troop(frame: &mut AppFrame, path: &str, move_speed: i32) -> DocumentID {
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

    fn skill_document(
        record_count: usize,
        localization_key: &[u8],
        icon_path: &[u8],
    ) -> SkillDocument {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100_u32.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(record_count).unwrap().to_le_bytes());
        for record in 0..record_count {
            bytes.extend_from_slice(&i32::try_from(record).unwrap().to_le_bytes());
            bytes.extend_from_slice(&u16::try_from(localization_key.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(localization_key);
            bytes.extend_from_slice(&u16::try_from(icon_path.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(icon_path);
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&50_u32.to_le_bytes());
        }
        bytes.resize(bytes.len() + 64, 0);
        SkillDocument::parse(bytes).unwrap()
    }

    fn open_skill(frame: &mut AppFrame, path: &str, record_count: usize) -> DocumentID {
        frame.workspace.open_loaded(
            PathBuf::from(path),
            Document::Skill(skill_document(
                record_count,
                b"@(S_Melee)",
                b"IL_SKL_Melee.tga",
            )),
        )
    }

    fn text_sox_document(records: &[(u32, &str)]) -> TextSOXDocument {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100_u32.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(records.len()).unwrap().to_le_bytes());
        for (wire_index, text) in records {
            bytes.extend_from_slice(&wire_index.to_le_bytes());
            bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        TextSOXDocument::parse(bytes).unwrap()
    }

    fn open_text_sox(frame: &mut AppFrame, path: &str, records: &[(u32, &str)]) -> DocumentID {
        frame.workspace.open_loaded(
            PathBuf::from(path),
            Document::TextSOX(text_sox_document(records)),
        )
    }

    #[test]
    fn text_sox_routing_keeps_all_document_routes_distinct() {
        assert_eq!(editor_route(DocumentKind::SkillInfo), EditorRoute::Skill);
        assert_eq!(editor_route(DocumentKind::TroopInfo), EditorRoute::Troop);
        assert_eq!(editor_route(DocumentKind::TextSOX), EditorRoute::TextSOX);
    }

    #[gpui::test]
    fn open_batch_starts_all_loads_then_applies_once_in_input_order(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let paths = ["A.sox", "B.sox", "C.sox"].map(|name| directory.path().join(name));
        for path in &paths {
            write_valid_sox(path);
        }
        let [first_loaded, second_loaded, third_loaded] =
            paths.clone().map(|path| load_path(path).unwrap());
        let loader = Rc::new(ControlledOpenPathLoader::default());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.open_path_loader = Rc::<ControlledOpenPathLoader>::clone(&loader);
                begin_open_paths(frame, paths.to_vec(), cx);
            })
            .unwrap();
        cx.run_until_parked();
        let started = loader.started_paths();
        assert_eq!(started.len(), paths.len());
        for path in &paths {
            assert!(started.contains(path));
        }

        window
            .update(cx, |frame, _, _| {
                assert!(frame.workspace.document_ids().is_empty());
                assert!(frame.recent_files.paths().is_empty());
            })
            .unwrap();

        loader.release(0, Ok(first_loaded));
        loader.release(1, Ok(second_loaded));
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert!(frame.workspace.document_ids().is_empty());
                assert!(frame.recent_files.paths().is_empty());
            })
            .unwrap();

        loader.release(2, Ok(third_loaded));
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(open_paths_in_workspace(frame), paths);
                assert_eq!(frame.recent_files.paths(), paths);
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Success);
                assert_eq!(notice.summary(), "Opened 3 files");
            })
            .unwrap();
    }

    #[gpui::test]
    fn open_action_creates_request_and_notice_before_launching_picker(cx: &mut TestAppContext) {
        let launched = Rc::new(Cell::new(false));
        let request_ready = Rc::new(Cell::new(false));
        let notice_ready = Rc::new(Cell::new(false));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                frame.open_prompt_launcher = Rc::new(PromptLaunchProbe {
                    launched: Rc::clone(&launched),
                    request_ready: Rc::clone(&request_ready),
                    notice_ready: Rc::clone(&notice_ready),
                });
                frame.open_action(&OpenFile, window, cx);
            })
            .unwrap();

        assert!(launched.get());
        assert!(request_ready.get());
        assert!(notice_ready.get());
    }

    #[gpui::test]
    fn open_batch_keeps_successes_and_reports_each_load_failure(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("A.sox");
        let invalid = directory.path().join("B.sox");
        let third = directory.path().join("C.sox");
        write_valid_sox(&first);
        write_invalid_sox(&invalid);
        write_valid_sox(&third);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                begin_open_paths(
                    frame,
                    vec![first.clone(), invalid.clone(), third.clone()],
                    cx,
                );
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    open_paths_in_workspace(frame),
                    [first.clone(), third.clone()]
                );
                assert_eq!(frame.recent_files.paths(), [first.clone(), third.clone()]);
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Error);
                assert_eq!(notice.summary(), "Opened 2 files; 1 failed");
                assert!(
                    notice
                        .detail()
                        .starts_with(&format!("{}: ", invalid.display()))
                );
                assert!(notice.detail().contains("failed to parse"));
                assert!(notice.detail().contains("Caused by:"));
            })
            .unwrap();
    }

    #[gpui::test]
    fn all_failed_open_paths_add_no_recent_file_or_settings_revision(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("A.sox");
        let second = directory.path().join("B.sox");
        write_invalid_sox(&first);
        write_invalid_sox(&second);
        let blocker = directory.path().join("settings-blocker");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                begin_open_paths(frame, vec![first, second], cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, cx| {
                assert!(frame.workspace.document_ids().is_empty());
                assert!(frame.recent_files.paths().is_empty());
                assert!(!frame.settings.has_failed());
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Error);
                assert_eq!(notice.summary(), "Opened 0 files; 2 failed");
                assert!(frame.schedule_settings_write(frame.shell.game(), cx));
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.settings.retry_failed().unwrap().get(), 2);
            })
            .unwrap();
    }

    #[gpui::test]
    fn one_accepted_open_batch_schedules_exactly_one_settings_revision(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("A.sox");
        let second = directory.path().join("B.sox");
        write_valid_sox(&first);
        write_valid_sox(&second);
        let blocker = directory.path().join("settings-blocker");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                begin_open_paths(frame, vec![first, second], cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.settings.retry_failed().unwrap().get(), 2);
            })
            .unwrap();
    }

    #[gpui::test]
    fn stale_open_completion_cannot_apply_after_a_new_request(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let stale = directory.path().join("stale.sox");
        let current = directory.path().join("current.sox");
        write_valid_sox(&stale);
        write_valid_sox(&current);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                begin_open_paths(frame, vec![stale], cx);
                begin_open_paths(frame, vec![current.clone()], cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    open_paths_in_workspace(frame),
                    std::slice::from_ref(&current)
                );
                assert_eq!(frame.recent_files.paths(), std::slice::from_ref(&current));
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Opened 1 file")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn late_picker_paths_cannot_supersede_a_newer_recent_file_open(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let picker_path = directory.path().join("picker.sox");
        let recent_path = directory.path().join("recent.sox");
        write_valid_sox(&recent_path);
        let recent_document = load_path(recent_path.clone()).unwrap();
        let loader = Rc::new(ControlledOpenPathLoader::default());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.open_path_loader = Rc::<ControlledOpenPathLoader>::clone(&loader);
                let picker_request = frame.shell.begin_open();
                frame.notices.begin(
                    NoticeSource::Open,
                    picker_request.get(),
                    Notice::info("Choose one or more .sox files"),
                );
                frame.open_recent_path(recent_path.clone(), cx);

                frame.open_paths(picker_request, vec![picker_path], cx);
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(loader.started_paths(), std::slice::from_ref(&recent_path));
        loader.release(0, Ok(recent_document));
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    open_paths_in_workspace(frame),
                    std::slice::from_ref(&recent_path)
                );
                assert_eq!(
                    frame.recent_files.paths(),
                    std::slice::from_ref(&recent_path)
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn duplicate_paths_open_twice_but_appear_once_in_recent_files(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("duplicate.sox");
        write_valid_sox(&path);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                begin_open_paths(frame, vec![path.clone(), path.clone()], cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(open_paths_in_workspace(frame), [path.clone(), path.clone()]);
                assert_eq!(frame.recent_files.paths(), [path]);
            })
            .unwrap();
    }

    #[gpui::test]
    fn opening_an_existing_recent_path_moves_it_to_the_front(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.sox");
        let existing = directory.path().join("existing.sox");
        write_valid_sox(&existing);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame
                    .recent_files
                    .add_batch(vec![first.clone(), existing.clone()]);
                begin_open_paths(frame, vec![existing.clone()], cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.recent_files.paths(), [existing, first]);
            })
            .unwrap();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[gpui::test]
    fn filesystem_non_unicode_success_uses_shared_open_pipeline(cx: &mut TestAppContext) {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(OsString::from_vec(vec![
            b'n', b'o', b'n', b'-', 0xff, b'.', b's', b'o', b'x',
        ]));
        write_valid_sox(&path);
        let settings_path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(settings_path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                begin_open_paths(frame, vec![path.clone()], cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(open_paths_in_workspace(frame), [path]);
                assert!(frame.recent_files.paths().is_empty());
                assert!(!frame.settings.has_failed());
                assert!(!settings_path.exists());
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert_eq!(
                    notice.summary(),
                    "Opened 1 file; 1 path omitted from recent files"
                );
            })
            .unwrap();
    }

    #[cfg(target_os = "macos")]
    #[gpui::test]
    fn injected_non_unicode_success_uses_shared_open_pipeline(cx: &mut TestAppContext) {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let directory = tempfile::tempdir().unwrap();
        let fixture = directory.path().join("fixture.sox");
        write_valid_sox(&fixture);
        let loaded_document = load_path(fixture).unwrap();
        let path = PathBuf::from(OsString::from_vec(vec![
            b'n', b'o', b'n', b'-', 0xff, b'.', b's', b'o', b'x',
        ]));
        let settings_path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(settings_path.clone());
        let loader = Rc::new(ControlledOpenPathLoader::default());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.open_path_loader = Rc::<ControlledOpenPathLoader>::clone(&loader);
                begin_open_paths(frame, vec![path.clone()], cx);
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(loader.started_paths(), std::slice::from_ref(&path));
        loader.release(0, Ok(loaded_document));
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(open_paths_in_workspace(frame), [path]);
                assert!(frame.recent_files.paths().is_empty());
                assert!(!frame.settings.has_failed());
                assert!(!settings_path.exists());
                let notice = frame.notices.current().unwrap();
                assert_eq!(notice.level(), NoticeLevel::Warning);
                assert_eq!(
                    notice.summary(),
                    "Opened 1 file; 1 path omitted from recent files"
                );
            })
            .unwrap();
    }

    #[test]
    fn skill_number_targets_build_checked_typed_edits() {
        let mut workspace = Workspace::new();
        let document = workspace.open_loaded(
            PathBuf::from("SkillInfo.sox"),
            Document::Skill(skill_document(1, b"@(S_Melee)", b"IL_SKL_Melee.tga")),
        );

        let skill_id = ActiveNumberEdit::skill_id(document, 0, 0);
        assert_eq!(
            skill_id.target.document_edit(i64::from(i32::MIN)).unwrap(),
            (
                document,
                DocumentEdit::SetSkillID {
                    record: 0,
                    value: i32::MIN,
                },
            )
        );

        let maximum_level = ActiveNumberEdit::skill_max_level(document, 0, 50);
        assert_eq!(
            maximum_level.target.document_edit(65_535).unwrap(),
            (
                document,
                DocumentEdit::SetSkillMaxLevel {
                    record: 0,
                    value: 65_535,
                },
            )
        );
        assert!(maximum_level.target.document_edit(-1).is_err());
    }

    #[test]
    fn skill_type_choices_build_wire_values_one_and_two() {
        assert_eq!(
            SkillTypeChoice::Combat.document_edit(4),
            DocumentEdit::SetSkillType {
                record: 4,
                value: 1,
            }
        );
        assert_eq!(
            SkillTypeChoice::Magic.document_edit(4),
            DocumentEdit::SetSkillType {
                record: 4,
                value: 2,
            }
        );
    }

    #[test]
    fn text_sox_generalization_keeps_skill_text_targets_field_specific() {
        let mut workspace = Workspace::new();
        let document = workspace.open_loaded(
            PathBuf::from("SkillInfo.sox"),
            Document::Skill(skill_document(1, b"@(S_Melee)", b"IL_SKL_Melee.tga")),
        );

        for (field, value) in [
            (SkillTextField::LocalizationKey, "@(S_Changed)"),
            (SkillTextField::IconPath, "changed.tga"),
        ] {
            let target = TextEditTarget::skill(document, 0, field);
            assert_eq!(
                target.document_edit(value.to_owned()),
                (
                    document,
                    DocumentEdit::SetSkillText {
                        record: 0,
                        field,
                        value: value.to_owned(),
                    },
                )
            );
        }
    }

    #[test]
    fn text_sox_targets_build_record_specific_edits() {
        let mut workspace = Workspace::new();
        let document = workspace.open_loaded(
            PathBuf::from("StringTable.sox"),
            Document::TextSOX(text_sox_document(&[(9001, "Alpha")])),
        );
        let target = TextEditTarget::text_sox(document, 3);

        assert_eq!(
            target.document_edit("Omega".to_owned()),
            (
                document,
                DocumentEdit::SetTextSOXText {
                    record: 3,
                    value: "Omega".to_owned(),
                },
            )
        );
    }

    #[test]
    fn text_sox_and_skill_targets_report_document_format_and_label() {
        let mut workspace = Workspace::new();
        let skill_document = workspace.open_loaded(
            PathBuf::from("SkillInfo.sox"),
            Document::Skill(skill_document(1, b"@(S_Melee)", b"IL_SKL_Melee.tga")),
        );
        let text_document = workspace.open_loaded(
            PathBuf::from("StringTable.sox"),
            Document::TextSOX(text_sox_document(&[(9001, "Alpha")])),
        );
        let skill = TextEditTarget::skill(skill_document, 1, SkillTextField::LocalizationKey);
        let text = TextEditTarget::text_sox(text_document, 2);

        assert_eq!(skill.document(), skill_document);
        assert_eq!(skill.format_name(), "SkillInfo");
        assert_eq!(skill.label(), "Localization Key");
        assert_eq!(text.document(), text_document);
        assert_eq!(text.format_name(), "text SOX");
        assert_eq!(text.label(), "Text");
    }

    #[test]
    fn invalid_skill_utf8_projects_as_a_disabled_diagnostic() {
        let mut workspace = Workspace::new();
        let document = workspace.open_loaded(
            PathBuf::from("SkillInfo.sox"),
            Document::Skill(skill_document(1, &[0xff, 0xfe], b"IL_SKL_Melee.tga")),
        );

        let projection =
            skill_text_projection(&workspace, document, 0, SkillTextField::LocalizationKey);

        let SkillTextProjection::Invalid { value, diagnostic } = projection else {
            panic!("invalid UTF-8 must not become an editable string");
        };
        assert_eq!(value, "Invalid UTF-8");
        assert!(diagnostic.contains("Localization Key is not valid UTF-8"));
    }

    #[test]
    fn skill_record_selection_is_kept_per_document() {
        let mut workspace = Workspace::new();
        let first = workspace.open_loaded(
            PathBuf::from("first.sox"),
            Document::Skill(skill_document(10, b"first", b"first.tga")),
        );
        let second = workspace.open_loaded(
            PathBuf::from("second.sox"),
            Document::Skill(skill_document(6, b"second", b"second.tga")),
        );
        let mut selections = RecordSelections::default();

        selections.select(first, 9);
        selections.select(second, 5);

        assert_eq!(selections.selected(first), 9);
        assert_eq!(selections.selected(second), 5);
    }

    #[test]
    fn text_sox_record_selection_is_kept_per_document() {
        let mut workspace = Workspace::new();
        let first = workspace.open_loaded(
            PathBuf::from("first.sox"),
            Document::TextSOX(text_sox_document(&[(1, "one"), (2, "two"), (3, "three")])),
        );
        let second = workspace.open_loaded(
            PathBuf::from("second.sox"),
            Document::TextSOX(text_sox_document(&[(4, "four"), (5, "five")])),
        );
        let mut selections = RecordSelections::default();

        selections.select(first, 2);
        selections.select(second, 1);

        assert_eq!(selections.selected(first), 2);
        assert_eq!(selections.selected(second), 1);
    }

    #[gpui::test]
    fn text_sox_native_input_commit_creates_one_edit_and_clears_the_draft(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            bind_text_input(cx);
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let document = window
            .update(cx, |frame, window, cx| {
                let document = open_text_sox(frame, "StringTable.sox", &[(9001, "Alpha")]);
                frame.activate_document(document);
                frame.shell.select_area(Area::Files);
                frame.start_text_edit(
                    TextEditTarget::text_sox(document, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );
                let input = frame.text_edit.as_ref().unwrap().input.clone();
                assert!(input.read(cx).focus_handle().is_focused(window));
                document
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "B r a v o enter");
        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Bravo");
                assert!(frame.text_edit.is_none());
                assert!(frame.workspace.undo(document).unwrap());
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Alpha");
                assert!(!frame.workspace.undo(document).unwrap());
                assert!(frame.workspace.redo(document).unwrap());
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Bravo");
                assert!(!frame.workspace.redo(document).unwrap());
            })
            .unwrap();
    }

    #[gpui::test]
    fn text_sox_canceled_save_as_notifies_after_canceling_the_draft(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        window
            .update(cx, |frame, window, cx| {
                let document = open_text_sox(frame, "StringTable.sox", &[(9001, "Alpha")]);
                frame.activate_document(document);
                frame.shell.select_area(Area::Files);
                frame.start_text_edit(
                    TextEditTarget::text_sox(document, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );
            })
            .unwrap();
        let frame_entity = window.root(cx).unwrap();
        let notification_count = Rc::new(Cell::new(0_usize));
        cx.update(|cx| {
            let notification_count = Rc::clone(&notification_count);
            cx.observe(&frame_entity, move |_, _| {
                notification_count.set(notification_count.get() + 1);
            })
            .detach();
        });

        window
            .update(cx, |frame, window, cx| {
                frame.save_as_action(&SaveAs, window, cx);
                assert!(frame.text_edit.is_none());
            })
            .unwrap();
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();

        assert_eq!(notification_count.get(), 1);
    }

    #[gpui::test]
    fn stale_save_as_selection_cannot_start_work_or_replace_a_newer_notice(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let selected_path = directory.path().join("stale-selection.sox");
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, "original.sox", 100);
                frame.activate_document(document);
                frame.save_as_action(&SaveAs, window, cx);
                frame.notices.replace(
                    NoticeSource::Workspace,
                    Notice::plain(NoticeLevel::Error, "newer workspace failure"),
                );
            })
            .unwrap();

        cx.simulate_new_path_selection(|_| Some(selected_path.clone()));
        cx.run_until_parked();

        assert!(!selected_path.exists());
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("newer workspace failure")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn stale_document_save_completion_cannot_replace_a_newer_notice(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.sox");
        let second_path = directory.path().join("second.sox");
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, _| {
                let first = open_troop(frame, &first_path.to_string_lossy(), 100);
                let _second = open_troop(frame, &second_path.to_string_lossy(), 200);
                let first_request = frame.workspace.prepare_save(first, None).unwrap();
                let first_token = first_request.token();
                let first_result = first_request.run();
                assert!(first_result.is_ok());

                frame.notices.begin(
                    NoticeSource::Workspace,
                    10,
                    Notice::info("Saving first document"),
                );
                frame.notices.begin(
                    NoticeSource::Workspace,
                    11,
                    Notice::info("Saving second document"),
                );
                frame.finish_save_result(first, first_token, 10, first_result);

                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Saving second document")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn canceled_save_as_restores_an_in_flight_save_notice_identity(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let document_path = directory.path().join("document.sox");
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (document, token, result, notice_identity) = window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame.activate_document(document);
                let request = frame.workspace.prepare_save(document, None).unwrap();
                let token = request.token();
                let result = request.run();
                assert!(result.is_ok());
                let notice_identity = frame.allocate_workspace_notice_identity();
                frame.notices.begin(
                    NoticeSource::Workspace,
                    notice_identity,
                    Notice::info("Saving document"),
                );

                frame.save_as_action(&SaveAs, window, cx);
                (document, token, result, notice_identity)
            })
            .unwrap();
        assert!(cx.did_prompt_for_new_path());

        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                frame.finish_save_result(document, token, notice_identity, result);

                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Saved document.sox")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn save_completion_while_save_as_is_pending_survives_picker_cancellation(
        cx: &mut TestAppContext,
    ) {
        let cases = [
            (false, "Saved document.sox", NoticeLevel::Success),
            (true, "Could not save document", NoticeLevel::Error),
        ];

        for (must_fail, expected_summary, expected_level) in cases {
            let directory = tempfile::tempdir().unwrap();
            let document_path = if must_fail {
                let blocker = directory.path().join("not-a-directory");
                fs::write(&blocker, b"blocker").unwrap();
                blocker.join("document.sox")
            } else {
                directory.path().join("document.sox")
            };
            let window = cx.update(|cx| {
                cx.open_window(WindowOptions::default(), |_, cx| {
                    cx.new(|cx| AppFrame::new(test_startup(), cx))
                })
                .unwrap()
            });
            let (document, token, result, notice_identity) = window
                .update(cx, |frame, window, cx| {
                    let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                    frame.activate_document(document);
                    let request = frame.workspace.prepare_save(document, None).unwrap();
                    let token = request.token();
                    let result = request.run();
                    assert_eq!(result.is_err(), must_fail);
                    let notice_identity = frame.allocate_workspace_notice_identity();
                    frame.notices.begin(
                        NoticeSource::Workspace,
                        notice_identity,
                        Notice::info("Saving document"),
                    );

                    frame.save_as_action(&SaveAs, window, cx);
                    (document, token, result, notice_identity)
                })
                .unwrap();
            assert!(cx.did_prompt_for_new_path());

            window
                .update(cx, |frame, _, _| {
                    frame.finish_save_result(document, token, notice_identity, result);
                })
                .unwrap();
            cx.simulate_new_path_selection(|_| None);
            cx.run_until_parked();

            window
                .update(cx, |frame, _, _| {
                    let notice = frame.notices.current().unwrap();
                    assert_eq!(notice.summary(), expected_summary);
                    assert_eq!(notice.level(), expected_level);
                })
                .unwrap();
        }
    }

    #[gpui::test]
    fn empty_text_sox_commit_keeps_the_same_draft_and_history(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            bind_text_input(cx);
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (document, state, input) = window
            .update(cx, |frame, window, cx| {
                let document = open_text_sox(frame, "StringTable.sox", &[(9001, "Alpha")]);
                frame.activate_document(document);
                frame.shell.select_area(Area::Files);
                let state = frame.workspace.state_id(document).unwrap();
                frame.start_text_edit(
                    TextEditTarget::text_sox(document, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );
                let input = frame.text_edit.as_ref().unwrap().input.clone();
                (document, state, input)
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "backspace enter");
        window
            .update(cx, |frame, window, cx| {
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Alpha");
                assert_eq!(frame.workspace.state_id(document).unwrap(), state);
                assert!(!frame.workspace.can_undo(document).unwrap());
                assert_eq!(frame.text_edit.as_ref().unwrap().input, input);
                assert_eq!(input.read(cx).content(), "");
                assert!(input.read(cx).focus_handle().is_focused(window));
                let notice = frame.notices.current().unwrap();
                assert!(notice.is_editor_feedback());
                assert!(notice.summary().contains("text SOX"));
                assert!(notice.summary().contains("1..=5 bytes"));
            })
            .unwrap();
    }

    #[gpui::test]
    fn oversized_text_sox_commit_keeps_the_same_draft_and_history(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            bind_text_input(cx);
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (document, state, input) = window
            .update(cx, |frame, window, cx| {
                let document = open_text_sox(frame, "StringTable.sox", &[(9001, "Alpha")]);
                frame.activate_document(document);
                frame.shell.select_area(Area::Files);
                let state = frame.workspace.state_id(document).unwrap();
                frame.start_text_edit(
                    TextEditTarget::text_sox(document, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );
                let input = frame.text_edit.as_ref().unwrap().input.clone();
                (document, state, input)
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "L o n g e r enter");
        window
            .update(cx, |frame, window, cx| {
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Alpha");
                assert_eq!(frame.workspace.state_id(document).unwrap(), state);
                assert!(!frame.workspace.can_undo(document).unwrap());
                assert_eq!(frame.text_edit.as_ref().unwrap().input, input);
                assert_eq!(input.read(cx).content(), "Longer");
                assert!(input.read(cx).focus_handle().is_focused(window));
                let notice = frame.notices.current().unwrap();
                assert!(notice.is_editor_feedback());
                assert!(notice.summary().contains("text SOX"));
                assert!(notice.summary().contains("1..=5 bytes"));
            })
            .unwrap();
    }

    #[gpui::test]
    fn non_ascii_text_sox_commit_is_state_neutral_and_keeps_the_draft(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (document, state, input) = window
            .update(cx, |frame, window, cx| {
                let document = open_text_sox(frame, "StringTable.sox", &[(9001, "Alpha")]);
                frame.activate_document(document);
                frame.shell.select_area(Area::Files);
                let state = frame.workspace.state_id(document).unwrap();
                frame.start_text_edit(
                    TextEditTarget::text_sox(document, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );
                let input = frame.text_edit.as_ref().unwrap().input.clone();
                input.update(cx, |input, input_cx| {
                    input.replace_text_in_range(None, "Café", window, input_cx);
                });
                (document, state, input)
            })
            .unwrap();

        input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("Café".to_owned()));
        });
        cx.run_until_parked();

        window
            .update(cx, |frame, window, cx| {
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Alpha");
                assert_eq!(frame.workspace.state_id(document).unwrap(), state);
                assert!(!frame.workspace.can_undo(document).unwrap());
                assert_eq!(frame.text_edit.as_ref().unwrap().input, input);
                assert_eq!(input.read(cx).content(), "Café");
                assert!(input.read(cx).focus_handle().is_focused(window));
                let notice = frame.notices.current().unwrap();
                assert!(notice.is_editor_feedback());
                assert!(notice.summary().contains("text SOX"));
                assert!(notice.summary().contains("1..=5 bytes"));
            })
            .unwrap();
    }

    #[gpui::test]
    fn stale_text_sox_event_cannot_mutate_a_hidden_document(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (first, first_state, second, second_state, stale_input) = window
            .update(cx, |frame, window, cx| {
                let first = open_text_sox(frame, "first.sox", &[(1, "Alpha")]);
                let second = open_text_sox(frame, "second.sox", &[(2, "Bravo")]);
                let first_state = frame.workspace.state_id(first).unwrap();
                let second_state = frame.workspace.state_id(second).unwrap();
                frame.activate_document(first);
                frame.start_text_edit(
                    TextEditTarget::text_sox(first, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );
                let input = frame.text_edit.as_ref().unwrap().input.clone();

                frame.active_document = Some(second);
                assert_eq!(frame.text_edit.as_ref().unwrap().input, input);
                (first, first_state, second, second_state, input)
            })
            .unwrap();

        stale_input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("Wrong".to_owned()));
        });
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.active_document, Some(second));
                assert!(frame.text_edit.is_none());
                assert_eq!(frame.workspace.text_sox_text(first, 0).unwrap(), "Alpha");
                assert_eq!(frame.workspace.text_sox_text(second, 0).unwrap(), "Bravo");
                assert_eq!(frame.workspace.state_id(first).unwrap(), first_state);
                assert_eq!(frame.workspace.state_id(second).unwrap(), second_state);
                assert!(!frame.workspace.can_undo(first).unwrap());
                assert!(!frame.workspace.can_redo(first).unwrap());
                assert!(!frame.workspace.can_undo(second).unwrap());
                assert!(!frame.workspace.can_redo(second).unwrap());
                assert!(!frame.workspace.is_dirty(first).unwrap());
                assert!(!frame.workspace.is_dirty(second).unwrap());
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("The active document changed; edit canceled")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn selecting_another_text_sox_record_cancels_the_active_draft(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                let document =
                    open_text_sox(frame, "StringTable.sox", &[(1, "Alpha"), (2, "Bravo")]);
                frame.activate_document(document);
                frame.start_text_edit(
                    TextEditTarget::text_sox(document, 0),
                    "Alpha".to_owned(),
                    window,
                    cx,
                );

                frame.select_record(document, 1);

                assert_eq!(frame.selections.selected(document), 1);
                assert!(frame.text_edit.is_none());
                assert_eq!(frame.workspace.text_sox_text(document, 0).unwrap(), "Alpha");
                assert!(!frame.workspace.is_dirty(document).unwrap());
            })
            .unwrap();
    }

    #[gpui::test]
    fn skill_maximum_level_feedback_clears_only_after_valid_recovery(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        window
            .update(cx, |frame, window, _| {
                let document = open_skill(frame, "SkillInfo.sox", 1);
                frame.activate_document(document);
                frame.begin_number_edit(ActiveNumberEdit::skill_max_level(document, 0, 50));
                window.focus(&frame.focus);
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "7 0 0 0 0 enter");
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame.number_edit.as_ref().map(|edit| edit.editor.draft()),
                    Some("70000")
                );
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Enter a whole number within the allowed range")
                );
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "0");
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame.number_edit.as_ref().map(|edit| edit.editor.draft()),
                    Some("700000")
                );
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Enter a whole number within the allowed range")
                );
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "backspace");
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame.number_edit.as_ref().map(|edit| edit.editor.draft()),
                    Some("70000")
                );
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Enter a whole number within the allowed range")
                );
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), "backspace");
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame.number_edit.as_ref().map(|edit| edit.editor.draft()),
                    Some("7000")
                );
                assert!(frame.notices.current().is_none());
            })
            .unwrap();
    }

    #[gpui::test]
    fn inactive_number_edit_cannot_survive_activation_or_commit(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, _| {
                let first = open_troop(frame, "first.sox", 100);
                let second = open_troop(frame, "second.sox", 200);
                frame.activate_document(first);
                frame.number_edit = Some(ActiveNumberEdit::troop_field(
                    first,
                    0,
                    TroopField::MoveSpeed,
                    100,
                ));

                frame.activate_document(second);

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
    fn stale_skill_text_event_cannot_mutate_a_hidden_document(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (first, second, stale_input) = window
            .update(cx, |frame, window, cx| {
                let first = open_skill(frame, "first.sox", 1);
                let second = open_skill(frame, "second.sox", 1);
                frame.activate_document(first);
                frame.start_text_edit(
                    TextEditTarget::skill(first, 0, SkillTextField::LocalizationKey),
                    "@(S_Melee)".to_owned(),
                    window,
                    cx,
                );
                let input = frame.text_edit.as_ref().unwrap().input.clone();
                assert!(input.read(cx).focus_handle().is_focused(window));

                frame.activate_document(second);
                assert!(frame.text_edit.is_none());
                (first, second, input)
            })
            .unwrap();

        stale_input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("hidden mutation".to_owned()));
        });
        cx.run_until_parked();

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.active_document, Some(second));
                assert_eq!(
                    frame
                        .workspace
                        .skill_text(first, 0, SkillTextField::LocalizationKey)
                        .unwrap(),
                    "@(S_Melee)"
                );
                assert!(!frame.workspace.is_dirty(first).unwrap());
            })
            .unwrap();
    }

    #[gpui::test]
    fn skill_property_switches_cancel_old_drafts_and_only_active_text_commits(
        cx: &mut TestAppContext,
    ) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (document, stale_input, active_input) = window
            .update(cx, |frame, window, cx| {
                let document = open_skill(frame, "SkillInfo.sox", 1);
                frame.activate_document(document);
                frame.begin_number_edit(ActiveNumberEdit::skill_id(document, 0, 0));
                assert!(frame.number_edit.is_some());

                frame.start_text_edit(
                    TextEditTarget::skill(document, 0, SkillTextField::LocalizationKey),
                    "@(S_Melee)".to_owned(),
                    window,
                    cx,
                );
                let stale_input = frame.text_edit.as_ref().unwrap().input.clone();
                assert!(frame.number_edit.is_none());

                frame.start_text_edit(
                    TextEditTarget::skill(document, 0, SkillTextField::IconPath),
                    "IL_SKL_Melee.tga".to_owned(),
                    window,
                    cx,
                );
                let active_input = frame.text_edit.as_ref().unwrap().input.clone();
                assert_ne!(stale_input, active_input);
                (document, stale_input, active_input)
            })
            .unwrap();

        stale_input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("wrong field".to_owned()));
        });
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame
                        .workspace
                        .skill_text(document, 0, SkillTextField::LocalizationKey)
                        .unwrap(),
                    "@(S_Melee)"
                );
                assert_eq!(frame.text_edit.as_ref().unwrap().input, active_input);
            })
            .unwrap();

        active_input.update(cx, |_, cx| {
            cx.emit(TextInputEvent::Commit("changed.tga".to_owned()));
        });
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert_eq!(
                    frame
                        .workspace
                        .skill_text(document, 0, SkillTextField::IconPath)
                        .unwrap(),
                    "changed.tga"
                );
                assert!(frame.text_edit.is_none());
                assert!(frame.workspace.is_dirty(document).unwrap());
            })
            .unwrap();
    }

    #[gpui::test]
    fn skill_record_switch_and_cancel_event_clear_the_active_text_draft(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let (document, canceled_input) = window
            .update(cx, |frame, window, cx| {
                let document = open_skill(frame, "SkillInfo.sox", 2);
                frame.activate_document(document);
                frame.start_text_edit(
                    TextEditTarget::skill(document, 0, SkillTextField::LocalizationKey),
                    "@(S_Melee)".to_owned(),
                    window,
                    cx,
                );
                frame.select_record(document, 1);
                assert_eq!(frame.selections.selected(document), 1);
                assert!(frame.text_edit.is_none());

                frame.start_text_edit(
                    TextEditTarget::skill(document, 1, SkillTextField::LocalizationKey),
                    "@(S_Melee)".to_owned(),
                    window,
                    cx,
                );
                frame
                    .notices
                    .replace(NoticeSource::Editor, invalid_number_notice());
                let input = frame.text_edit.as_ref().unwrap().input.clone();
                (document, input)
            })
            .unwrap();

        canceled_input.update(cx, |_, cx| cx.emit(TextInputEvent::Cancel));
        cx.run_until_parked();
        window
            .update(cx, |frame, _, _| {
                assert!(frame.text_edit.is_none());
                assert!(frame.notices.current().is_none());
                assert_eq!(
                    frame
                        .workspace
                        .skill_text(document, 1, SkillTextField::LocalizationKey)
                        .unwrap(),
                    "@(S_Melee)"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn app_frame_opens_at_home(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.shell.area(), Area::Home);
            })
            .unwrap();
    }

    #[gpui::test]
    fn startup_bootstrap_initializes_the_canonical_runtime_owners(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(
            &path,
            br#"{
                "version": 1,
                "active_game": "heroes",
                "crusaders_path": "/games/Crusaders",
                "heroes_path": "/games/Heroes",
                "max_recent_files": 5,
                "recent_files": ["/files/recent.sox"]
            }"#,
        )
        .unwrap();
        let startup = SettingsStartup::load(path);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, _| {
                assert_eq!(frame.shell.game(), Game::Heroes);
                assert_eq!(
                    frame.game_paths.root(Game::Crusaders),
                    Some(std::path::Path::new("/games/Crusaders"))
                );
                assert_eq!(
                    frame.game_paths.root(Game::Heroes),
                    Some(std::path::Path::new("/games/Heroes"))
                );
                assert_eq!(frame.recent_files.limit(), 5);
                assert_eq!(
                    frame.recent_files.paths(),
                    [PathBuf::from("/files/recent.sox")]
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn changing_the_game_persists_once_and_an_identical_selection_is_skipped(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| frame.select_game(Game::Heroes, cx))
            .unwrap();
        cx.run_until_parked();
        let saved = serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved.get("active_game").and_then(serde_json::Value::as_str),
            Some("heroes")
        );

        fs::remove_file(&path).unwrap();
        window
            .update(cx, |frame, _, cx| frame.select_game(Game::Heroes, cx))
            .unwrap();
        cx.run_until_parked();
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[gpui::test]
    fn invalid_settings_snapshot_keeps_the_game_and_discards_a_stale_failure(
        cx: &mut TestAppContext,
    ) {
        use std::os::unix::ffi::OsStringExt;

        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| frame.select_game(Game::Heroes, cx))
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |frame, _, cx| {
                assert!(frame.settings.has_failed());
                frame.game_paths.set_root(
                    Game::Crusaders,
                    Some(PathBuf::from(std::ffi::OsString::from_vec(vec![
                        b'/', 0xff, b'x',
                    ]))),
                );

                frame.select_game(Game::Crusaders, cx);

                assert_eq!(frame.shell.game(), Game::Heroes);
                assert!(!frame.settings.has_failed());
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Could not prepare application settings")
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn protected_settings_changes_warn_without_starting_a_write(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original = br#"{"version":2,"future":true}"#;
        fs::write(&path, original).unwrap();
        let startup = SettingsStartup::load(path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.select_game(Game::Heroes, cx);
                assert!(frame.settings.is_settled());
                assert_eq!(
                    frame.notices.current().map(Notice::level),
                    Some(NoticeLevel::Warning)
                );
            })
            .unwrap();
        cx.run_until_parked();

        assert_eq!(fs::read(path).unwrap(), original);
    }

    #[gpui::test]
    fn first_successful_coalesced_settings_replacement_clears_the_startup_error(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, b"{").unwrap();
        let startup = SettingsStartup::load(path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Could not load application settings")
                );

                let first_image =
                    image_from_runtime(Game::Heroes, &frame.game_paths, &frame.recent_files)
                        .unwrap();
                let SettingsQueueResult::Queued(first_revision) = frame.settings.queue(first_image)
                else {
                    panic!("enabled persistence must queue revision 1");
                };
                frame.notices.begin(
                    NoticeSource::SettingsWrite,
                    first_revision.get(),
                    Notice::info("Saving application settings"),
                );
                let first_request = frame.settings.take_ready().unwrap();

                let latest_image =
                    image_from_runtime(Game::Crusaders, &frame.game_paths, &frame.recent_files)
                        .unwrap();
                let SettingsQueueResult::Queued(latest_revision) =
                    frame.settings.queue(latest_image)
                else {
                    panic!("enabled persistence must queue revision 2");
                };
                frame.notices.begin(
                    NoticeSource::SettingsWrite,
                    latest_revision.get(),
                    Notice::info("Saving application settings"),
                );
                assert!(frame.settings.take_ready().is_none());

                let first_completion = first_request.run();
                let saved =
                    serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap();
                assert_eq!(
                    saved.get("active_game").and_then(serde_json::Value::as_str),
                    Some("heroes")
                );
                frame.finish_settings_write(first_completion, cx);

                assert_eq!(
                    frame.settings.latest_revision_for_test(),
                    Some(latest_revision)
                );
                assert_eq!(
                    frame.notices.current().map(Notice::level),
                    Some(NoticeLevel::Info)
                );
                assert_eq!(
                    frame.notices.current().map(Notice::summary),
                    Some("Saving application settings")
                );
            })
            .unwrap();
        cx.run_until_parked();
    }

    #[gpui::test]
    fn close_waits_for_the_newest_coalesced_settings_write(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                frame.select_game(Game::Heroes, cx);
                frame.select_game(Game::Crusaders, cx);
                assert!(!frame.window_should_close(window, cx));
                assert!(frame.close_pending);
            })
            .unwrap();

        let executor = cx.executor();
        let first_write_finished = (0..100).any(|_| {
            executor.tick();
            fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|value| {
                    value
                        .get("active_game")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .as_deref()
                == Some("heroes")
        });
        assert!(first_write_finished);
        assert_eq!(cx.read(|app| app.windows().len()), 1);
        window
            .update(cx, |frame, _, _| {
                assert!(frame.close_pending);
                assert!(!frame.settings.is_settled());
            })
            .unwrap();

        let newest_write_finished = (0..100).any(|_| {
            executor.tick();
            fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|value| {
                    value
                        .get("active_game")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
                .as_deref()
                == Some("crusaders")
        });
        assert!(newest_write_finished);
        assert_eq!(cx.read(|app| app.windows().len()), 1);
        window
            .update(cx, |frame, _, _| {
                assert!(frame.close_pending);
                assert!(!frame.settings.is_settled());
            })
            .unwrap();
        cx.run_until_parked();

        assert!(cx.read(|app| app.windows().is_empty()));
        let saved = serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap();
        assert_eq!(
            saved.get("active_game").and_then(serde_json::Value::as_str),
            Some("crusaders")
        );
    }

    #[gpui::test]
    fn failed_settings_retry_resumes_close_and_removes_the_exact_window(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let path = blocker.join("settings.json");
        let startup = SettingsStartup::load(path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.run_until_parked();
        assert!(!cx.read(|app| app.windows().is_empty()));

        window
            .update(cx, |frame, window, cx| {
                assert!(frame.settings.has_failed());
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        assert_eq!(
            cx.pending_prompt().map(|prompt| prompt.0),
            Some("Settings could not be saved. Retry before closing?".to_owned())
        );

        fs::remove_file(&blocker).unwrap();
        fs::create_dir(&blocker).unwrap();
        cx.simulate_prompt_answer("Retry");
        cx.run_until_parked();

        assert!(path.exists());
        assert!(cx.read(|app| app.windows().is_empty()));
    }

    #[gpui::test]
    fn failed_settings_retry_removes_only_the_target_window(cx: &mut TestAppContext) {
        let sentinel = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let path = blocker.join("settings.json");
        let startup = SettingsStartup::load(path.clone());
        let target = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        target
            .update(cx, |frame, window, cx| {
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.run_until_parked();
        target
            .update(cx, |frame, window, cx| {
                assert!(frame.settings.has_failed());
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();

        fs::remove_file(&blocker).unwrap();
        fs::create_dir(&blocker).unwrap();
        cx.simulate_prompt_answer("Retry");
        cx.run_until_parked();

        assert!(path.exists());
        assert_eq!(cx.read(|app| app.windows().len()), 1);
        sentinel
            .update(cx, |frame, _, _| {
                assert_eq!(frame.shell.area(), Area::Home);
            })
            .unwrap();
    }

    #[gpui::test]
    fn close_without_saving_discards_a_failed_settings_snapshot(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let path = blocker.join("settings.json");
        let startup = SettingsStartup::load(path);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });
        window
            .update(cx, |frame, _, cx| frame.select_game(Game::Heroes, cx))
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, window, cx| {
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.simulate_prompt_answer("Close Without Saving");
        cx.run_until_parked();

        assert!(cx.read(|app| app.windows().is_empty()));
    }

    #[gpui::test]
    fn cancel_after_a_failed_settings_write_keeps_the_window_and_failure(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });
        window
            .update(cx, |frame, _, cx| frame.select_game(Game::Heroes, cx))
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |frame, window, cx| {
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();

        assert!(!cx.read(|app| app.windows().is_empty()));
        window
            .update(cx, |frame, _, _| {
                assert!(frame.settings.has_failed());
                assert!(!frame.close_pending);
            })
            .unwrap();
    }

    #[gpui::test]
    fn protected_settings_mode_bypasses_close_drain(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, br#"{"version":2}"#).unwrap();
        let startup = SettingsStartup::load(path);
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                frame.select_game(Game::Heroes, cx);
                assert!(frame.window_should_close(window, cx));
                assert_eq!(
                    frame.notices.current().map(Notice::level),
                    Some(NoticeLevel::Warning)
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn dirty_documents_save_before_pending_settings_allow_close(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let document_path = directory.path().join("document.sox");
        let settings_path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(settings_path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetTroopField {
                            record: 0,
                            field: TroopField::MoveSpeed,
                            value: 101,
                        },
                    )
                    .unwrap();
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Save All");
        cx.run_until_parked();

        assert!(document_path.exists());
        assert!(settings_path.exists());
        assert!(cx.read(|app| app.windows().is_empty()));
    }

    #[gpui::test]
    fn discarded_documents_still_wait_for_pending_settings_before_close(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let document_path = directory.path().join("discarded.sox");
        let settings_path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(settings_path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetTroopField {
                            record: 0,
                            field: TroopField::MoveSpeed,
                            value: 101,
                        },
                    )
                    .unwrap();
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.simulate_prompt_answer("Discard Changes");
        cx.run_until_parked();

        assert!(!document_path.exists());
        assert!(settings_path.exists());
        assert!(cx.read(|app| app.windows().is_empty()));
    }

    #[gpui::test]
    fn discarded_documents_reach_the_settings_failure_prompt(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let document_path = directory.path().join("discarded.sox");
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetTroopField {
                            record: 0,
                            field: TroopField::MoveSpeed,
                            value: 101,
                        },
                    )
                    .unwrap();
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.simulate_prompt_answer("Discard Changes");
        cx.run_until_parked();

        window
            .update(cx, |frame, window, cx| {
                assert_eq!(frame.close_documents, CloseDocuments::Discard);
                assert!(frame.settings.has_failed());
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        assert_eq!(
            cx.pending_prompt().map(|prompt| prompt.0),
            Some("Settings could not be saved. Retry before closing?".to_owned())
        );
    }

    #[gpui::test]
    fn canceling_settings_close_revokes_the_document_discard_decision(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let document_path = directory.path().join("discarded.sox");
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetTroopField {
                            record: 0,
                            field: TroopField::MoveSpeed,
                            value: 101,
                        },
                    )
                    .unwrap();
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.simulate_prompt_answer("Discard Changes");
        cx.run_until_parked();
        window
            .update(cx, |frame, window, cx| {
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();

        window
            .update(cx, |frame, window, cx| {
                assert_eq!(frame.close_documents, CloseDocuments::Save);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        assert_eq!(
            cx.pending_prompt().map(|prompt| prompt.0),
            Some("1 unsaved document. Save before closing?".to_owned())
        );
    }

    #[gpui::test]
    fn editing_after_a_settings_failure_revokes_the_document_discard_decision(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let document_path = directory.path().join("discarded.sox");
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let startup = SettingsStartup::load(blocker.join("settings.json"));
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        let document = window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetTroopField {
                            record: 0,
                            field: TroopField::MoveSpeed,
                            value: 101,
                        },
                    )
                    .unwrap();
                frame.select_game(Game::Heroes, cx);
                assert!(!frame.window_should_close(window, cx));
                document
            })
            .unwrap();
        cx.simulate_prompt_answer("Discard Changes");
        cx.run_until_parked();

        window
            .update(cx, |frame, window, cx| {
                frame.activate_document(document);
                frame.begin_number_edit(ActiveNumberEdit::troop_field(
                    document,
                    0,
                    TroopField::MoveSpeed,
                    101,
                ));
                frame.commit_number_edit(102);
                assert!(!frame.window_should_close(window, cx));
            })
            .unwrap();
        assert_eq!(
            cx.pending_prompt().map(|prompt| prompt.0),
            Some("1 unsaved document. Save before closing?".to_owned())
        );
    }

    #[gpui::test]
    fn discard_close_survives_a_document_save_failure_before_settings_finish(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        fs::write(&blocker, b"blocker").unwrap();
        let document_path = blocker.join("document.sox");
        let settings_path = directory.path().join("settings.json");
        let startup = SettingsStartup::load(settings_path.clone());
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(startup, cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                let document = open_troop(frame, &document_path.to_string_lossy(), 100);
                frame
                    .workspace
                    .apply(
                        document,
                        DocumentEdit::SetTroopField {
                            record: 0,
                            field: TroopField::MoveSpeed,
                            value: 101,
                        },
                    )
                    .unwrap();
                let request = frame.workspace.prepare_save(document, None).unwrap();
                let token = request.token();
                let result = request.run();
                assert!(result.is_err());

                frame.select_game(Game::Heroes, cx);
                frame.window_handle = Some(window.window_handle());
                frame.close_pending = true;
                frame.close_documents = CloseDocuments::Discard;
                let notice_identity = frame.allocate_workspace_notice_identity();
                frame.notices.begin(
                    NoticeSource::Workspace,
                    notice_identity,
                    Notice::info("Saving document"),
                );
                frame.finish_save_result(document, token, notice_identity, result);
                frame.continue_close(cx);

                assert!(frame.close_pending);
                assert_eq!(frame.close_documents, CloseDocuments::Discard);
            })
            .unwrap();
        cx.run_until_parked();

        assert!(settings_path.exists());
        assert!(cx.read(|app| app.windows().is_empty()));
    }

    #[gpui::test]
    fn troop_editor_builds_for_a_loaded_document(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
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

    #[gpui::test]
    fn navigation_selecting_settings_updates_the_shell_route(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.select_area(Area::Settings, cx);
                assert_eq!(frame.shell.area(), Area::Settings);
                let navigation = navigation_projection();
                assert_eq!(navigation.primary, Area::PRIMARY);
                assert_eq!(navigation.bottom, Area::Settings);
            })
            .unwrap();
    }

    #[gpui::test]
    fn settings_view_recent_limit_changes_schedule_one_revision(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.set_recent_limit(5, cx);
                let revision = frame.settings.latest_revision_for_test().unwrap();
                assert_eq!(frame.recent_files.limit(), 5);
                assert_eq!(revision.get(), 1);
                assert_eq!(frame.task_launches.settings, 1);

                frame.set_recent_limit(5, cx);
                assert_eq!(frame.settings.latest_revision_for_test(), Some(revision));
                assert_eq!(frame.task_launches.settings, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn settings_view_clear_recents_schedules_only_a_real_change(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, _, cx| {
                frame.recent_files.add(PathBuf::from("/files/recent.sox"));
                frame.clear_recent_files(cx);
                let revision = frame.settings.latest_revision_for_test().unwrap();
                assert!(frame.recent_files.paths().is_empty());
                assert_eq!(revision.get(), 1);
                assert_eq!(frame.task_launches.settings, 1);

                frame.clear_recent_files(cx);
                assert_eq!(frame.settings.latest_revision_for_test(), Some(revision));
                assert_eq!(frame.task_launches.settings, 1);
            })
            .unwrap();
    }

    #[gpui::test]
    fn settings_view_render_is_state_and_task_launch_pure(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| AppFrame::new(test_startup(), cx))
            })
            .unwrap()
        });

        window
            .update(cx, |frame, window, cx| {
                frame.shell.select_area(Area::Settings);
                let shell = (frame.shell.area(), frame.shell.game());
                let paths = frame.game_paths.clone();
                let recent = frame.recent_files.clone();
                let settings_revision = frame.settings.latest_revision_for_test();
                let settings_settled = frame.settings.is_settled();
                let notice = frame.notices.current().map(|notice| {
                    (
                        notice.level(),
                        notice.summary().to_owned(),
                        notice.detail().to_owned(),
                    )
                });
                let task_launches = frame.task_launches;
                assert!(frame.window_handle.is_none());
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::NotConfigured
                ));
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));

                drop(frame.render(window, cx));

                assert!(frame.window_handle.is_none());
                assert_eq!((frame.shell.area(), frame.shell.game()), shell);
                assert_eq!(frame.game_paths, paths);
                assert_eq!(frame.recent_files, recent);
                assert!(matches!(
                    frame.catalog.status(),
                    CatalogStatus::NotConfigured
                ));
                assert!(matches!(frame.discovery.status(), DiscoveryStatus::Idle));
                assert_eq!(
                    frame.notices.current().map(|notice| (
                        notice.level(),
                        notice.summary().to_owned(),
                        notice.detail().to_owned(),
                    )),
                    notice
                );
                assert_eq!(frame.settings.latest_revision_for_test(), settings_revision);
                assert_eq!(frame.settings.is_settled(), settings_settled);
                assert_eq!(frame.task_launches, task_launches);
            })
            .unwrap();
    }
}
