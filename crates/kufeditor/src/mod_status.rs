use std::error::Error;

use kufeditor_game::Game;
use kufeditor_mods::{
    BackupID, BackupInfo, InstallationID, InstalledMod, InstalledModStatus, ModError, ModPackageID,
    ModPackageInfo, ModProgress, ModProgressPhase, OperationID, RelativeGamePath,
};

use crate::state::RequestID;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum ModSection {
    #[default]
    Installed,
    Library,
    Backups,
    Create,
}

impl ModSection {
    pub(crate) const ALL: [Self; 4] = [Self::Installed, Self::Library, Self::Backups, Self::Create];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Installed => "Installed",
            Self::Library => "Library",
            Self::Backups => "Backups",
            Self::Create => "Create",
        }
    }

    pub(crate) const fn element_id(self) -> &'static str {
        match self {
            Self::Installed => "mods-section-installed",
            Self::Library => "mods-section-library",
            Self::Backups => "mods-section-backups",
            Self::Create => "mods-section-create",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModRequestKey {
    request: RequestID,
    game: Game,
    root_revision: u64,
    library_revision: u64,
}

impl ModRequestKey {
    pub(crate) const fn request(self) -> RequestID {
        self.request
    }

    #[cfg(test)]
    pub(crate) const fn with_request_offset(self, offset: u64) -> Self {
        Self {
            request: RequestID::from_value(self.request.get().wrapping_add(offset)),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModScanScope {
    LibraryOnly,
    Full,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModPackageSnapshot {
    pub(crate) package_id: ModPackageID,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) author: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) game: Game,
    pub(crate) files: Vec<RelativeGamePath>,
    pub(crate) compressed_bytes: u64,
    pub(crate) uncompressed_bytes: u64,
    pub(crate) file_count: u64,
}

impl From<&ModPackageInfo> for ModPackageSnapshot {
    fn from(package: &ModPackageInfo) -> Self {
        let manifest = package.manifest();
        let metadata = manifest.metadata();
        Self {
            package_id: package.package_id(),
            name: metadata.name().to_owned(),
            version: metadata.version().to_owned(),
            author: metadata.author().map(ToOwned::to_owned),
            description: metadata.description().map(ToOwned::to_owned),
            game: manifest.game(),
            files: manifest.files().to_vec(),
            compressed_bytes: package.compressed_bytes(),
            uncompressed_bytes: package.uncompressed_bytes(),
            file_count: package.file_count(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstalledModSnapshot {
    pub(crate) installation_id: InstallationID,
    pub(crate) package_id: ModPackageID,
    pub(crate) operation_id: OperationID,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) game: Game,
    pub(crate) installed_at: String,
    pub(crate) status: Option<InstalledModStatus>,
    pub(crate) files: Vec<RelativeGamePath>,
}

impl From<&InstalledMod> for InstalledModSnapshot {
    fn from(installed: &InstalledMod) -> Self {
        Self {
            installation_id: installed.installation_id(),
            package_id: installed.package_id(),
            operation_id: installed.operation_id(),
            name: installed.metadata().name().to_owned(),
            version: installed.metadata().version().to_owned(),
            game: installed.game(),
            installed_at: installed.installed_at().as_str().to_owned(),
            status: installed.status(),
            files: installed
                .files()
                .iter()
                .map(|file| file.path().clone())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BackupSnapshot {
    pub(crate) backup_id: BackupID,
    pub(crate) label: Option<String>,
    pub(crate) game: Game,
    pub(crate) created_at: String,
    pub(crate) file_count: u64,
    pub(crate) total_bytes: u64,
}

impl From<&BackupInfo> for BackupSnapshot {
    fn from(backup: &BackupInfo) -> Self {
        Self {
            backup_id: backup.backup_id(),
            label: backup.label().map(ToOwned::to_owned),
            game: backup.game(),
            created_at: backup.created_at().as_str().to_owned(),
            file_count: backup.file_count(),
            total_bytes: backup.total_bytes(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "create and operation issues are populated by the Task 9 operation layer"
    )
)]
pub(crate) enum ModIssueScope {
    Installed,
    Library,
    Backups,
    Create,
    Root,
    Operation,
}

impl ModIssueScope {
    pub(crate) const fn belongs_to(self, section: ModSection) -> bool {
        matches!(self, Self::Operation)
            || matches!(
                (self, section),
                (
                    Self::Root,
                    ModSection::Installed | ModSection::Backups | ModSection::Create
                )
            )
            || matches!(
                (self, section),
                (Self::Installed, ModSection::Installed)
                    | (Self::Library, ModSection::Library)
                    | (Self::Backups, ModSection::Backups)
                    | (Self::Create, ModSection::Create)
            )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModIssueSnapshot {
    pub(crate) scope: ModIssueScope,
    pub(crate) identity: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) recovery_paths: Vec<String>,
}

impl ModIssueSnapshot {
    pub(crate) fn from_error(
        scope: ModIssueScope,
        identity: impl Into<String>,
        title: impl Into<String>,
        error: &ModError,
    ) -> Self {
        Self {
            scope,
            identity: identity.into(),
            title: title.into(),
            detail: format_error(error),
            recovery_paths: recovery_paths(error),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModCollectionSnapshot<T> {
    pub(crate) rows: Vec<T>,
    pub(crate) issues: Vec<ModIssueSnapshot>,
}

impl<T> ModCollectionSnapshot<T> {
    pub(crate) const fn new(rows: Vec<T>, issues: Vec<ModIssueSnapshot>) -> Self {
        Self { rows, issues }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModRootCompletion {
    NotRequested,
    MissingRoot,
    Failed(ModIssueSnapshot),
    Ready {
        configured_root: String,
        installations: ModCollectionSnapshot<InstalledModSnapshot>,
        backups: ModCollectionSnapshot<BackupSnapshot>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModScanCompletion {
    library: Result<ModCollectionSnapshot<ModPackageSnapshot>, ModIssueSnapshot>,
    root: ModRootCompletion,
}

impl ModScanCompletion {
    pub(crate) const fn new(
        library: Result<ModCollectionSnapshot<ModPackageSnapshot>, ModIssueSnapshot>,
        root: ModRootCompletion,
    ) -> Self {
        Self { library, root }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModLibraryState {
    Idle,
    Loading,
    Ready(ModCollectionSnapshot<ModPackageSnapshot>),
    Failed(ModIssueSnapshot),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModRootState {
    Idle,
    Loading,
    MissingRoot,
    Ready {
        configured_root: String,
        installations: ModCollectionSnapshot<InstalledModSnapshot>,
        backups: ModCollectionSnapshot<BackupSnapshot>,
    },
    Failed(ModIssueSnapshot),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ModCreateDraft {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) author: String,
    pub(crate) description: String,
    pub(crate) backup_label: String,
    pub(crate) files: Vec<RelativeGamePath>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModCreateField {
    Name,
    Version,
    Author,
    Description,
    BackupLabel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModOperationKind {
    Import,
    Create,
    Apply,
    Uninstall,
    CreateBackup,
    RestoreBackup,
    DeleteBackup,
    RemovePackage,
}

impl ModOperationKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Import => "Import package",
            Self::Create => "Create package",
            Self::Apply => "Apply package",
            Self::Uninstall => "Uninstall mod",
            Self::CreateBackup => "Create backup",
            Self::RestoreBackup => "Restore backup",
            Self::DeleteBackup => "Delete backup",
            Self::RemovePackage => "Remove package",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModPromptKind {
    Import,
    SelectFiles,
    Export,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModOperationTarget {
    Package(ModPackageID),
    Installation(InstallationID),
    Backup(BackupID),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModPendingConfirmation {
    pub(crate) operation: ModOperationKind,
    pub(crate) target: ModOperationTarget,
    pub(crate) subject: String,
    pub(crate) consequence: String,
    pub(crate) key: ModRequestKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModOperationLaunch {
    pub(crate) kind: ModOperationKind,
    pub(crate) target: Option<ModOperationTarget>,
    pub(crate) key: ModRequestKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModProgressSnapshot {
    sequence: u64,
    phase_epoch: u64,
    pub(crate) operation: ModOperationKind,
    pub(crate) phase: ModProgressPhase,
    pub(crate) completed: u64,
    pub(crate) total: u64,
    pub(crate) path: Option<RelativeGamePath>,
    pub(crate) can_cancel: bool,
    pub(crate) cancel_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveModOperation {
    kind: ModOperationKind,
    key: ModRequestKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModContextChange {
    Unchanged,
    Changed,
}

#[derive(Debug)]
pub(crate) struct ModPresentationState {
    section: ModSection,
    game: Game,
    root_revision: u64,
    library_revision: u64,
    next_request_id: u64,
    active_scan: Option<(ModRequestKey, ModScanScope)>,
    active_prompt: Option<(ModPromptKind, ModRequestKey)>,
    library_state: ModLibraryState,
    root_state: ModRootState,
    selected_package: Option<ModPackageID>,
    selected_installation: Option<InstallationID>,
    selected_backup: Option<BackupID>,
    create_draft: ModCreateDraft,
    pending_confirmation: Option<ModPendingConfirmation>,
    progress: Option<ModProgressSnapshot>,
    active_operation: Option<ActiveModOperation>,
    operation_issues: Vec<ModIssueSnapshot>,
}

impl Default for ModPresentationState {
    fn default() -> Self {
        Self {
            section: ModSection::Installed,
            game: Game::default(),
            root_revision: 0,
            library_revision: 0,
            next_request_id: 0,
            active_scan: None,
            active_prompt: None,
            library_state: ModLibraryState::Idle,
            root_state: ModRootState::Idle,
            selected_package: None,
            selected_installation: None,
            selected_backup: None,
            create_draft: ModCreateDraft::default(),
            pending_confirmation: None,
            progress: None,
            active_operation: None,
            operation_issues: Vec::new(),
        }
    }
}

impl ModPresentationState {
    pub(crate) const fn section(&self) -> ModSection {
        self.section
    }

    pub(crate) fn select_section(&mut self, section: ModSection) {
        self.section = section;
    }

    pub(crate) const fn game(&self) -> Game {
        self.game
    }

    #[cfg(test)]
    pub(crate) const fn root_revision(&self) -> u64 {
        self.root_revision
    }

    #[cfg(test)]
    pub(crate) const fn library_revision(&self) -> u64 {
        self.library_revision
    }

    pub(crate) const fn library_state(&self) -> &ModLibraryState {
        &self.library_state
    }

    pub(crate) const fn root_state(&self) -> &ModRootState {
        &self.root_state
    }

    pub(crate) const fn selected_package(&self) -> Option<ModPackageID> {
        self.selected_package
    }

    pub(crate) const fn selected_installation(&self) -> Option<InstallationID> {
        self.selected_installation
    }

    pub(crate) const fn selected_backup(&self) -> Option<BackupID> {
        self.selected_backup
    }

    pub(crate) fn package(&self, package_id: ModPackageID) -> Option<&ModPackageSnapshot> {
        let ModLibraryState::Ready(library) = &self.library_state else {
            return None;
        };
        library
            .rows
            .iter()
            .find(|package| package.package_id == package_id)
    }

    pub(crate) fn installation(
        &self,
        installation_id: InstallationID,
    ) -> Option<&InstalledModSnapshot> {
        let ModRootState::Ready { installations, .. } = &self.root_state else {
            return None;
        };
        installations
            .rows
            .iter()
            .find(|installed| installed.installation_id == installation_id)
    }

    pub(crate) fn backup(&self, backup_id: BackupID) -> Option<&BackupSnapshot> {
        let ModRootState::Ready { backups, .. } = &self.root_state else {
            return None;
        };
        backups
            .rows
            .iter()
            .find(|backup| backup.backup_id == backup_id)
    }

    pub(crate) fn select_package(&mut self, package: Option<ModPackageID>) {
        self.selected_package = package;
    }

    pub(crate) fn select_installation(&mut self, installation: Option<InstallationID>) {
        self.selected_installation = installation;
    }

    pub(crate) fn select_backup(&mut self, backup: Option<BackupID>) {
        self.selected_backup = backup;
    }

    pub(crate) const fn create_draft(&self) -> &ModCreateDraft {
        &self.create_draft
    }

    pub(crate) fn set_create_field(&mut self, field: ModCreateField, value: String) {
        match field {
            ModCreateField::Name => self.create_draft.name = value,
            ModCreateField::Version => self.create_draft.version = value,
            ModCreateField::Author => self.create_draft.author = value,
            ModCreateField::Description => self.create_draft.description = value,
            ModCreateField::BackupLabel => self.create_draft.backup_label = value,
        }
    }

    pub(crate) fn set_create_files(&mut self, files: Vec<RelativeGamePath>) {
        self.create_draft.files = files;
    }

    pub(crate) const fn pending_confirmation(&self) -> Option<&ModPendingConfirmation> {
        self.pending_confirmation.as_ref()
    }

    pub(crate) const fn progress(&self) -> Option<&ModProgressSnapshot> {
        self.progress.as_ref()
    }

    pub(crate) const fn active_operation(&self) -> Option<ModOperationKind> {
        match self.active_operation {
            Some(operation) => Some(operation.kind),
            None => None,
        }
    }

    pub(crate) fn operation_issues(&self) -> &[ModIssueSnapshot] {
        &self.operation_issues
    }

    pub(crate) fn begin_scan(
        &mut self,
        scope: ModScanScope,
        root_configured: bool,
    ) -> ModRequestKey {
        let key = self.next_key();
        self.active_scan = Some((key, scope));
        self.active_prompt = None;
        self.pending_confirmation = None;
        self.library_state = ModLibraryState::Loading;
        if scope == ModScanScope::Full {
            self.root_state = if root_configured {
                ModRootState::Loading
            } else {
                ModRootState::MissingRoot
            };
        }
        key
    }

    pub(crate) fn finish_scan(
        &mut self,
        key: ModRequestKey,
        completion: ModScanCompletion,
    ) -> bool {
        let Some((active, scope)) = self.active_scan else {
            return false;
        };
        if active != key || !self.key_is_current(key) {
            return false;
        }
        self.active_scan = None;
        self.library_state = match completion.library {
            Ok(library) => ModLibraryState::Ready(library),
            Err(issue) => ModLibraryState::Failed(issue),
        };
        if scope == ModScanScope::Full {
            self.root_state = match completion.root {
                ModRootCompletion::NotRequested => ModRootState::Idle,
                ModRootCompletion::MissingRoot => ModRootState::MissingRoot,
                ModRootCompletion::Failed(issue) => ModRootState::Failed(issue),
                ModRootCompletion::Ready {
                    configured_root,
                    installations,
                    backups,
                } => ModRootState::Ready {
                    configured_root,
                    installations,
                    backups,
                },
            };
        }
        self.reconcile_scan_selections();
        true
    }

    pub(crate) fn set_context(&mut self, game: Game, root_revision: u64) -> ModContextChange {
        if self.game == game && self.root_revision == root_revision {
            return ModContextChange::Unchanged;
        }
        self.game = game;
        self.root_revision = root_revision;
        self.active_scan = None;
        self.active_prompt = None;
        self.library_state = ModLibraryState::Idle;
        self.root_state = ModRootState::Idle;
        self.selected_installation = None;
        self.selected_backup = None;
        self.pending_confirmation = None;
        self.progress = None;
        self.active_operation = None;
        self.operation_issues.clear();
        ModContextChange::Changed
    }

    pub(crate) fn library_changed(&mut self) -> u64 {
        self.library_revision = self.library_revision.wrapping_add(1);
        self.active_scan = None;
        self.active_prompt = None;
        self.library_state = ModLibraryState::Idle;
        self.selected_package = None;
        self.pending_confirmation = None;
        self.progress = None;
        self.active_operation = None;
        self.library_revision
    }

    pub(crate) fn begin_prompt(&mut self, kind: ModPromptKind) -> Option<ModRequestKey> {
        if self.active_operation.is_some() {
            return None;
        }
        let key = self.next_key();
        self.active_scan = None;
        self.active_prompt = Some((kind, key));
        self.pending_confirmation = None;
        Some(key)
    }

    pub(crate) fn finish_prompt(&mut self, kind: ModPromptKind, key: ModRequestKey) -> bool {
        if self.active_prompt != Some((kind, key)) || !self.key_is_current(key) {
            return false;
        }
        self.active_prompt = None;
        true
    }

    pub(crate) fn begin_confirmation(
        &mut self,
        operation: ModOperationKind,
        target: ModOperationTarget,
        subject: impl Into<String>,
        consequence: impl Into<String>,
    ) -> Option<ModRequestKey> {
        if self.active_operation.is_some() {
            return None;
        }
        let key = self.next_key();
        self.active_scan = None;
        self.active_prompt = None;
        self.pending_confirmation = Some(ModPendingConfirmation {
            operation,
            target,
            subject: subject.into(),
            consequence: consequence.into(),
            key,
        });
        Some(key)
    }

    pub(crate) fn dismiss_confirmation(&mut self) -> bool {
        self.pending_confirmation.take().is_some()
    }

    pub(crate) fn confirm_operation(&mut self) -> Option<ModOperationLaunch> {
        let pending = self.pending_confirmation.as_ref()?;
        if self.active_operation.is_some() || !self.key_is_current(pending.key) {
            self.pending_confirmation = None;
            return None;
        }
        let launch = ModOperationLaunch {
            kind: pending.operation,
            target: Some(pending.target),
            key: pending.key,
        };
        self.pending_confirmation = None;
        self.active_operation = Some(ActiveModOperation {
            kind: launch.kind,
            key: launch.key,
        });
        self.progress = None;
        self.operation_issues.clear();
        Some(launch)
    }

    pub(crate) fn begin_operation(&mut self, kind: ModOperationKind) -> Option<ModRequestKey> {
        if self.active_operation.is_some() {
            return None;
        }
        let key = self.next_key();
        self.active_scan = None;
        self.active_prompt = None;
        self.pending_confirmation = None;
        self.active_operation = Some(ActiveModOperation { kind, key });
        self.progress = None;
        self.operation_issues.clear();
        Some(key)
    }

    pub(crate) fn update_progress(
        &mut self,
        key: ModRequestKey,
        sequence: u64,
        phase_epoch: u64,
        progress: &ModProgress,
    ) -> bool {
        let Some(operation) = self.active_operation else {
            return false;
        };
        if operation.key != key || !self.key_is_current(key) {
            return false;
        }
        if self.progress.as_ref().is_some_and(|current| {
            sequence <= current.sequence
                || phase_epoch < current.phase_epoch
                || (phase_epoch == current.phase_epoch
                    && (current.phase != progress.phase || progress.completed < current.completed))
        }) {
            return false;
        }
        let cancel_requested = self
            .progress
            .as_ref()
            .is_some_and(|current| current.cancel_requested);
        self.progress = Some(ModProgressSnapshot {
            sequence,
            phase_epoch,
            operation: operation.kind,
            phase: progress.phase,
            completed: progress.completed,
            total: progress.total,
            path: progress.path.clone(),
            can_cancel: progress_phase_allows_cancel(progress.phase) && !cancel_requested,
            cancel_requested,
        });
        true
    }

    pub(crate) fn request_cancellation(&mut self) -> bool {
        let Some(progress) = self.progress.as_mut() else {
            return false;
        };
        if !progress.can_cancel || progress.cancel_requested {
            return false;
        }
        progress.cancel_requested = true;
        progress.can_cancel = false;
        true
    }

    pub(crate) fn finish_operation(&mut self, key: ModRequestKey) -> bool {
        if !matches!(self.active_operation, Some(operation) if operation.key == key)
            || !self.key_is_current(key)
        {
            return false;
        }
        self.active_operation = None;
        self.progress = None;
        true
    }

    pub(crate) fn fail_operation(&mut self, key: ModRequestKey, issue: ModIssueSnapshot) -> bool {
        if !self.finish_operation(key) {
            return false;
        }
        self.operation_issues.push(issue);
        true
    }

    fn next_key(&mut self) -> ModRequestKey {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        ModRequestKey {
            request: RequestID::from_value(self.next_request_id),
            game: self.game,
            root_revision: self.root_revision,
            library_revision: self.library_revision,
        }
    }

    fn key_is_current(&self, key: ModRequestKey) -> bool {
        key.request.get() == self.next_request_id
            && key.game == self.game
            && key.root_revision == self.root_revision
            && key.library_revision == self.library_revision
    }

    fn reconcile_scan_selections(&mut self) {
        if let ModLibraryState::Ready(library) = &self.library_state {
            self.selected_package = reconcile_id(
                self.selected_package,
                library.rows.iter().map(|package| package.package_id),
            );
        } else if !matches!(self.library_state, ModLibraryState::Loading) {
            self.selected_package = None;
        }

        if let ModRootState::Ready {
            installations,
            backups,
            ..
        } = &self.root_state
        {
            self.selected_installation = reconcile_id(
                self.selected_installation,
                installations
                    .rows
                    .iter()
                    .map(|installed| installed.installation_id),
            );
            self.selected_backup = reconcile_id(
                self.selected_backup,
                backups.rows.iter().map(|backup| backup.backup_id),
            );
        } else if !matches!(self.root_state, ModRootState::Loading) {
            self.selected_installation = None;
            self.selected_backup = None;
        }
    }
}

fn reconcile_id<T: Copy + Eq>(selected: Option<T>, values: impl Iterator<Item = T>) -> Option<T> {
    let values = values.collect::<Vec<_>>();
    selected
        .filter(|selected| values.contains(selected))
        .or_else(|| values.first().copied())
}

fn format_error(error: &dyn Error) -> String {
    let mut detail = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        detail.push_str("\nCaused by: ");
        detail.push_str(&cause.to_string());
        source = cause.source();
    }
    detail
}

fn recovery_paths(error: &ModError) -> Vec<String> {
    let Some(recovery) = error.recovery_report() else {
        return Vec::new();
    };
    let groups = [
        ("Committed", recovery.committed()),
        ("Rolled back", recovery.rolled_back()),
        ("Rollback failed", recovery.rollback_failed()),
        ("Unchanged", recovery.unchanged()),
    ];
    groups
        .into_iter()
        .flat_map(|(label, paths)| {
            paths
                .iter()
                .map(move |path| format!("{label}: {}", path.as_str()))
        })
        .collect()
}

pub(crate) const fn progress_phase_allows_cancel(phase: ModProgressPhase) -> bool {
    !matches!(
        phase,
        ModProgressPhase::PublishingPackage
            | ModProgressPhase::PublishingInstallation
            | ModProgressPhase::PublishingUninstall
            | ModProgressPhase::PublishingBackup
            | ModProgressPhase::RollingBack
    )
}

#[cfg(test)]
mod tests {
    use kufeditor_game::Game;
    use kufeditor_mods::{
        BackupID, InstallationID, InstalledModStatus, ModLimits, ModPackageID, ModProgress,
        ModProgressPhase, OperationID, RelativeGamePath,
    };

    use super::{
        BackupSnapshot, InstalledModSnapshot, ModCollectionSnapshot, ModContextChange,
        ModIssueScope, ModLibraryState, ModOperationKind, ModOperationTarget, ModPackageSnapshot,
        ModPresentationState, ModPromptKind, ModRootCompletion, ModRootState, ModScanCompletion,
        ModScanScope, ModSection,
    };

    #[test]
    fn defaults_to_installed_and_preserves_the_section_across_scans() {
        let mut state = ModPresentationState::default();
        assert_eq!(state.section(), ModSection::Installed);

        state.select_section(ModSection::Backups);
        let key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(key, ready_completion(Vec::new(), Vec::new(), Vec::new())));

        assert_eq!(state.section(), ModSection::Backups);
        assert!(matches!(state.library_state(), ModLibraryState::Ready(_)));
        assert!(matches!(state.root_state(), ModRootState::Ready { .. }));
    }

    #[test]
    fn stable_id_reconciliation_keeps_a_selection_then_chooses_the_first_remaining_row() {
        let mut state = ModPresentationState::default();
        let package_a = package('a', "Alpha", Game::Crusaders, &["alpha.sox"]);
        let package_b = package('b', "Bravo", Game::Crusaders, &["bravo.sox"]);
        let installed_a = installed('1', &package_a, InstalledModStatus::Clean);
        let installed_b = installed('2', &package_b, InstalledModStatus::Clean);
        let backup_a = backup('3', "Alpha backup");
        let backup_b = backup('4', "Bravo backup");

        let key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(
            key,
            ready_completion(
                vec![package_a.clone(), package_b.clone()],
                vec![installed_a.clone(), installed_b.clone()],
                vec![backup_a.clone(), backup_b.clone()],
            ),
        ));
        state.select_package(Some(package_b.package_id));
        state.select_installation(Some(installed_b.installation_id));
        state.select_backup(Some(backup_b.backup_id));

        let key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(
            key,
            ready_completion(
                vec![package_b.clone()],
                vec![installed_b.clone()],
                vec![backup_b.clone()],
            ),
        ));
        assert_eq!(state.selected_package(), Some(package_b.package_id));
        assert_eq!(
            state.selected_installation(),
            Some(installed_b.installation_id)
        );
        assert_eq!(state.selected_backup(), Some(backup_b.backup_id));

        let key = state.begin_scan(ModScanScope::Full, true);
        assert!(state.finish_scan(
            key,
            ready_completion(
                vec![package_a.clone()],
                vec![installed_a.clone()],
                vec![backup_a.clone()],
            ),
        ));
        assert_eq!(state.selected_package(), Some(package_a.package_id));
        assert_eq!(
            state.selected_installation(),
            Some(installed_a.installation_id)
        );
        assert_eq!(state.selected_backup(), Some(backup_a.backup_id));
    }

    #[test]
    fn every_context_revision_rejects_stale_scan_results() {
        let mut state = ModPresentationState::default();
        let first = state.begin_scan(ModScanScope::Full, true);

        assert_eq!(
            state.set_context(Game::Crusaders, 1),
            ModContextChange::Changed
        );
        assert!(!state.finish_scan(first, ready_completion(Vec::new(), Vec::new(), Vec::new())));

        let second = state.begin_scan(ModScanScope::Full, true);
        assert_eq!(state.library_changed(), 1);
        assert!(!state.finish_scan(second, ready_completion(Vec::new(), Vec::new(), Vec::new())));

        let third = state.begin_scan(ModScanScope::Full, true);
        assert_eq!(
            state.set_context(Game::Heroes, 7),
            ModContextChange::Changed
        );
        assert!(!state.finish_scan(third, ready_completion(Vec::new(), Vec::new(), Vec::new())));
        assert_eq!(state.game(), Game::Heroes);
        assert_eq!(state.root_revision(), 7);
        assert_eq!(state.library_revision(), 1);
        assert!(matches!(state.root_state(), ModRootState::Idle));
    }

    #[test]
    fn only_one_destructive_operation_can_own_the_context_until_it_changes() {
        let mut state = ModPresentationState::default();
        let operation = state
            .begin_operation(ModOperationKind::Apply)
            .expect("the first operation should start");

        assert!(state.begin_operation(ModOperationKind::Uninstall).is_none());
        assert_eq!(
            state.set_context(Game::Heroes, 1),
            ModContextChange::Changed
        );
        assert!(!state.finish_operation(operation.with_request_offset(1)));
        assert!(!state.finish_operation(operation));
        assert!(state.begin_operation(ModOperationKind::Uninstall).is_some());
    }

    #[test]
    fn operation_and_issue_kinds_have_stable_labels_and_scopes() {
        let operations = [
            ModOperationKind::Import,
            ModOperationKind::Create,
            ModOperationKind::Apply,
            ModOperationKind::Uninstall,
            ModOperationKind::CreateBackup,
            ModOperationKind::RestoreBackup,
            ModOperationKind::DeleteBackup,
            ModOperationKind::RemovePackage,
        ];
        assert!(
            operations
                .into_iter()
                .all(|operation| !operation.label().is_empty())
        );

        assert!(ModIssueScope::Installed.belongs_to(ModSection::Installed));
        assert!(ModIssueScope::Library.belongs_to(ModSection::Library));
        assert!(ModIssueScope::Backups.belongs_to(ModSection::Backups));
        assert!(ModIssueScope::Create.belongs_to(ModSection::Create));
        assert!(ModIssueScope::Root.belongs_to(ModSection::Installed));
        assert!(ModIssueScope::Root.belongs_to(ModSection::Backups));
        assert!(ModIssueScope::Operation.belongs_to(ModSection::Create));
    }

    #[test]
    fn mods_stale_prompts_and_confirmations_require_the_exact_current_key() {
        let mut state = ModPresentationState::default();
        let first = state
            .begin_prompt(ModPromptKind::Import)
            .expect("the first prompt should start");
        let second = state
            .begin_prompt(ModPromptKind::Import)
            .expect("a newer prompt should supersede the first");
        assert!(!state.finish_prompt(ModPromptKind::Import, first));
        assert!(state.finish_prompt(ModPromptKind::Import, second));

        let package_id = ModPackageID::parse(&"a".repeat(64)).unwrap();
        let confirmation = state
            .begin_confirmation(
                ModOperationKind::Apply,
                ModOperationTarget::Package(package_id),
                "Alpha 1.0",
                "Replace its owned game files.",
            )
            .expect("the confirmation should open");
        assert_eq!(
            state.pending_confirmation().map(|pending| pending.key),
            Some(confirmation)
        );
        assert_eq!(state.library_changed(), 1);
        assert!(state.confirm_operation().is_none());

        let prompt = state
            .begin_prompt(ModPromptKind::SelectFiles)
            .expect("the file prompt should start");
        assert_eq!(
            state.set_context(Game::Heroes, 3),
            ModContextChange::Changed
        );
        assert!(!state.finish_prompt(ModPromptKind::SelectFiles, prompt));
    }

    #[test]
    fn mods_actions_confirm_stable_targets_and_track_bounded_progress() {
        let mut state = ModPresentationState::default();
        let installation_id = InstallationID::parse(&"b".repeat(64)).unwrap();
        state
            .begin_confirmation(
                ModOperationKind::Uninstall,
                ModOperationTarget::Installation(installation_id),
                "Bravo 1.0",
                "Restore its before-images and remove its installation record.",
            )
            .expect("the confirmation should open");
        let launch = state
            .confirm_operation()
            .expect("the exact confirmation should launch");
        assert_eq!(launch.kind, ModOperationKind::Uninstall);
        assert_eq!(
            launch.target,
            Some(ModOperationTarget::Installation(installation_id))
        );

        assert!(state.update_progress(
            launch.key,
            1,
            1,
            &ModProgress {
                phase: ModProgressPhase::StagingUninstall,
                completed: 5,
                total: 10,
                path: None,
            },
        ));
        assert!(!state.update_progress(
            launch.key,
            2,
            1,
            &ModProgress {
                phase: ModProgressPhase::StagingUninstall,
                completed: 4,
                total: 10,
                path: None,
            },
        ));
        assert!(state.progress().is_some_and(|progress| progress.can_cancel));
        assert!(state.request_cancellation());
        assert!(
            state
                .progress()
                .is_some_and(|progress| progress.cancel_requested)
        );

        assert!(state.update_progress(
            launch.key,
            3,
            2,
            &ModProgress {
                phase: ModProgressPhase::PublishingUninstall,
                completed: 0,
                total: 1,
                path: None,
            },
        ));
        assert!(
            state
                .progress()
                .is_some_and(|progress| !progress.can_cancel)
        );
        assert!(!state.request_cancellation());
        assert!(state.finish_operation(launch.key));
    }

    #[test]
    fn mods_progress_accepts_valid_phase_changes_that_reuse_package_phases() {
        let mut state = ModPresentationState::default();
        let operation = state
            .begin_operation(ModOperationKind::Create)
            .expect("the create operation should start");

        for (sequence, phase_epoch, progress) in [
            (
                1,
                1,
                ModProgress {
                    phase: ModProgressPhase::CreatingPackage,
                    completed: 1,
                    total: 1,
                    path: None,
                },
            ),
            (
                2,
                2,
                ModProgress {
                    phase: ModProgressPhase::InspectingPackage,
                    completed: 0,
                    total: 2,
                    path: None,
                },
            ),
            (
                3,
                3,
                ModProgress {
                    phase: ModProgressPhase::PublishingPackage,
                    completed: 0,
                    total: 1,
                    path: None,
                },
            ),
            (
                4,
                4,
                ModProgress {
                    phase: ModProgressPhase::InspectingPackage,
                    completed: 0,
                    total: 2,
                    path: None,
                },
            ),
        ] {
            assert!(
                state.update_progress(operation, sequence, phase_epoch, &progress),
                "valid phase change to {:?} was discarded",
                progress.phase
            );
        }
        assert!(state.progress().is_some_and(|progress| {
            progress.phase == ModProgressPhase::InspectingPackage && progress.can_cancel
        }));
    }

    fn ready_completion(
        packages: Vec<ModPackageSnapshot>,
        installations: Vec<InstalledModSnapshot>,
        backups: Vec<BackupSnapshot>,
    ) -> ModScanCompletion {
        ModScanCompletion::new(
            Ok(ModCollectionSnapshot::new(packages, Vec::new())),
            ModRootCompletion::Ready {
                configured_root: "/game".into(),
                installations: ModCollectionSnapshot::new(installations, Vec::new()),
                backups: ModCollectionSnapshot::new(backups, Vec::new()),
            },
        )
    }

    fn package(digit: char, name: &str, game: Game, files: &[&str]) -> ModPackageSnapshot {
        ModPackageSnapshot {
            package_id: ModPackageID::parse(&digit.to_string().repeat(64)).unwrap(),
            name: name.to_owned(),
            version: "1.0".to_owned(),
            author: None,
            description: None,
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
        status: InstalledModStatus,
    ) -> InstalledModSnapshot {
        InstalledModSnapshot {
            installation_id: InstallationID::parse(&digit.to_string().repeat(64)).unwrap(),
            package_id: package.package_id,
            operation_id: OperationID::parse(&"d".repeat(64)).unwrap(),
            name: package.name.clone(),
            version: package.version.clone(),
            game: package.game,
            installed_at: "2026-08-26T12:00:00Z".to_owned(),
            status: Some(status),
            files: package.files.clone(),
        }
    }

    fn backup(digit: char, label: &str) -> BackupSnapshot {
        BackupSnapshot {
            backup_id: BackupID::parse(&digit.to_string().repeat(64)).unwrap(),
            label: Some(label.to_owned()),
            game: Game::Crusaders,
            created_at: "2026-08-26T12:00:00Z".to_owned(),
            file_count: 2,
            total_bytes: 32,
        }
    }
}
