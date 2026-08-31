use std::path::{Path, PathBuf};

use kufeditor_game::Game;
use kufeditor_patches::{
    BackupStatus, ExecutableInspection, FireRatePresetID, FireRateStatus, PatchID, PatchStatus,
    PatchTarget,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchContextChange {
    Changed,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PatchRequestID(u64);

impl PatchRequestID {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PatchContext {
    game: Game,
    root: Option<PathBuf>,
    root_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchKey {
    request: PatchRequestID,
    context: PatchContext,
}

impl PatchKey {
    #[cfg(test)]
    pub(crate) const fn request(&self) -> PatchRequestID {
        self.request
    }

    pub(crate) const fn game(&self) -> Game {
        self.context.game
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.context.root.as_deref()
    }

    #[cfg(test)]
    pub(crate) const fn root_revision(&self) -> u64 {
        self.context.root_revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutablePatchState {
    id: PatchID,
    status: PatchStatus,
}

impl ExecutablePatchState {
    pub(crate) const fn new(id: PatchID, status: PatchStatus) -> Self {
        Self { id, status }
    }

    pub(crate) const fn id(self) -> PatchID {
        self.id
    }

    pub(crate) const fn status(self) -> PatchStatus {
        self.status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchSnapshot {
    executable: PathBuf,
    backup: PathBuf,
    backup_status: BackupStatus,
    patches: [ExecutablePatchState; 2],
    fire_rate: FireRateStatus,
}

impl PatchSnapshot {
    pub(crate) const fn new(
        executable: PathBuf,
        backup: PathBuf,
        backup_status: BackupStatus,
        patches: [ExecutablePatchState; 2],
        fire_rate: FireRateStatus,
    ) -> Self {
        Self {
            executable,
            backup,
            backup_status,
            patches,
            fire_rate,
        }
    }

    pub(crate) fn from_inspection(inspection: &ExecutableInspection) -> Self {
        Self::new(
            inspection.path().to_path_buf(),
            inspection.backup_path().to_path_buf(),
            inspection.backup_status(),
            (*inspection.patches())
                .map(|state| ExecutablePatchState::new(state.id(), state.status())),
            inspection.fire_rate(),
        )
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    pub(crate) fn backup(&self) -> &Path {
        &self.backup
    }

    pub(crate) const fn backup_status(&self) -> BackupStatus {
        self.backup_status
    }

    pub(crate) fn patch_status(&self, id: PatchID) -> PatchStatus {
        self.patches
            .iter()
            .find(|state| state.id() == id)
            .map_or(PatchStatus::Unknown, |state| state.status())
    }

    pub(crate) const fn fire_rate(&self) -> FireRateStatus {
        self.fire_rate
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PatchInspectionPhase {
    Idle,
    Loading {
        key: PatchKey,
    },
    Ready {
        key: PatchKey,
        snapshot: PatchSnapshot,
    },
    Failed {
        key: PatchKey,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchOperation {
    SetPatch { id: PatchID, target: PatchTarget },
    SetFireRate { id: FireRatePresetID },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchPendingConfirmation {
    key: PatchKey,
    operation: PatchOperation,
    executable: PathBuf,
    backup: PathBuf,
}

impl PatchPendingConfirmation {
    pub(crate) const fn operation(&self) -> PatchOperation {
        self.operation
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    pub(crate) fn backup(&self) -> &Path {
        &self.backup
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchOperationKey {
    request: PatchRequestID,
    context: PatchContext,
    operation: PatchOperation,
    executable: PathBuf,
    backup: PathBuf,
}

impl PatchOperationKey {
    pub(crate) const fn request(&self) -> PatchRequestID {
        self.request
    }

    pub(crate) const fn operation(&self) -> PatchOperation {
        self.operation
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.context.root.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchOperationLaunch {
    key: PatchOperationKey,
}

impl PatchOperationLaunch {
    pub(crate) const fn key(&self) -> &PatchOperationKey {
        &self.key
    }

    pub(crate) const fn operation(&self) -> PatchOperation {
        self.key.operation()
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.key.root()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchFinish {
    Current,
    ContextChanged,
    Ignored,
}

#[derive(Debug)]
pub(crate) struct PatchPresentationState {
    context: PatchContext,
    next_request: u64,
    phase: PatchInspectionPhase,
    pending_confirmation: Option<PatchPendingConfirmation>,
    active_operation: Option<PatchOperationKey>,
}

impl PatchPresentationState {
    pub(crate) fn new(game: Game, root: Option<PathBuf>, root_revision: u64) -> Self {
        Self {
            context: PatchContext {
                game,
                root,
                root_revision,
            },
            next_request: 0,
            phase: PatchInspectionPhase::Idle,
            pending_confirmation: None,
            active_operation: None,
        }
    }

    pub(crate) const fn game(&self) -> Game {
        self.context.game
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.context.root.as_deref()
    }

    #[cfg(test)]
    pub(crate) const fn root_revision(&self) -> u64 {
        self.context.root_revision
    }

    pub(crate) const fn phase(&self) -> &PatchInspectionPhase {
        &self.phase
    }

    pub(crate) const fn pending_confirmation(&self) -> Option<&PatchPendingConfirmation> {
        self.pending_confirmation.as_ref()
    }

    pub(crate) const fn operation_in_progress(&self) -> bool {
        self.active_operation.is_some()
    }

    pub(crate) fn active_backup(&self) -> Option<&Path> {
        self.active_operation
            .as_ref()
            .map(|operation| operation.backup.as_path())
    }

    pub(crate) fn set_context(
        &mut self,
        game: Game,
        root: Option<PathBuf>,
        root_revision: u64,
    ) -> PatchContextChange {
        let context = PatchContext {
            game,
            root,
            root_revision,
        };
        if context == self.context {
            return PatchContextChange::Unchanged;
        }
        self.context = context;
        self.phase = PatchInspectionPhase::Idle;
        self.pending_confirmation = None;
        PatchContextChange::Changed
    }

    pub(crate) fn begin_inspection(&mut self) -> Option<PatchKey> {
        if self.active_operation.is_some()
            || self.context.game != Game::Crusaders
            || self.context.root.is_none()
        {
            self.phase = PatchInspectionPhase::Idle;
            self.pending_confirmation = None;
            return None;
        }
        let request = self.next_request();
        let key = PatchKey {
            request,
            context: self.context.clone(),
        };
        self.phase = PatchInspectionPhase::Loading { key: key.clone() };
        self.pending_confirmation = None;
        Some(key)
    }

    pub(crate) fn finish_inspection(
        &mut self,
        key: PatchKey,
        result: Result<PatchSnapshot, String>,
    ) -> bool {
        let is_current = key.context == self.context
            && matches!(
                &self.phase,
                PatchInspectionPhase::Loading { key: active } if active == &key
            );
        if !is_current {
            return false;
        }
        self.phase = match result {
            Ok(snapshot) => PatchInspectionPhase::Ready { key, snapshot },
            Err(message) => PatchInspectionPhase::Failed { key, message },
        };
        true
    }

    pub(crate) fn request_operation(&mut self, operation: PatchOperation) -> bool {
        if self.active_operation.is_some() {
            return false;
        }
        let PatchInspectionPhase::Ready { key, snapshot } = &self.phase else {
            return false;
        };
        if !operation_available(snapshot, operation) {
            return false;
        }
        self.pending_confirmation = Some(PatchPendingConfirmation {
            key: key.clone(),
            operation,
            executable: snapshot.executable().to_path_buf(),
            backup: snapshot.backup().to_path_buf(),
        });
        true
    }

    pub(crate) fn dismiss_confirmation(&mut self) {
        self.pending_confirmation = None;
    }

    pub(crate) fn confirm_operation(&mut self) -> Option<PatchOperationLaunch> {
        if self.active_operation.is_some() {
            self.pending_confirmation = None;
            return None;
        }
        let pending = self.pending_confirmation.take()?;
        let ready_is_current = pending.key.context == self.context
            && matches!(
                &self.phase,
                PatchInspectionPhase::Ready { key, .. } if key == &pending.key
            );
        if !ready_is_current {
            return None;
        }
        let key = PatchOperationKey {
            request: self.next_request(),
            context: self.context.clone(),
            operation: pending.operation,
            executable: pending.executable,
            backup: pending.backup,
        };
        self.active_operation = Some(key.clone());
        Some(PatchOperationLaunch { key })
    }

    pub(crate) fn finish_operation(&mut self, key: &PatchOperationKey) -> PatchFinish {
        if self.active_operation.as_ref() != Some(key) {
            return PatchFinish::Ignored;
        }
        self.active_operation = None;
        if key.context == self.context {
            PatchFinish::Current
        } else {
            PatchFinish::ContextChanged
        }
    }

    fn next_request(&mut self) -> PatchRequestID {
        self.next_request += 1;
        PatchRequestID(self.next_request)
    }
}

fn operation_available(snapshot: &PatchSnapshot, operation: PatchOperation) -> bool {
    match operation {
        PatchOperation::SetPatch { id, target } => matches!(
            (snapshot.patch_status(id), target),
            (PatchStatus::NotApplied, PatchTarget::Applied)
                | (PatchStatus::Applied, PatchTarget::NotApplied)
        ),
        PatchOperation::SetFireRate { id } => {
            snapshot.fire_rate() != FireRateStatus::Unknown
                && snapshot.fire_rate() != FireRateStatus::Preset(id)
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "state tests use fixed configured contexts and must fail at the setup boundary"
)]
mod tests {
    use std::path::{Path, PathBuf};

    use kufeditor_game::Game;
    use kufeditor_patches::{
        BackupStatus, FireRatePresetID, FireRateStatus, PatchID, PatchStatus, PatchTarget,
    };

    use super::{
        ExecutablePatchState, PatchContextChange, PatchFinish, PatchInspectionPhase,
        PatchOperation, PatchPresentationState, PatchSnapshot,
    };

    #[test]
    fn inspection_keys_capture_the_exact_context_and_reject_late_results() {
        let mut state = PatchPresentationState::new(
            Game::Crusaders,
            Some(PathBuf::from("/games/crusaders")),
            3,
        );
        let key = state.begin_inspection().expect("configured Crusaders root");
        assert_eq!(key.request().get(), 1);
        assert_eq!(key.root(), Some(Path::new("/games/crusaders")));
        assert_eq!(key.root_revision(), 3);
        assert_eq!(state.root_revision(), 3);
        assert!(state.finish_inspection(key.clone(), Ok(snapshot())));
        assert!(matches!(state.phase(), PatchInspectionPhase::Ready { .. }));

        assert_eq!(
            state.set_context(
                Game::Crusaders,
                Some(PathBuf::from("/games/crusaders-new")),
                4,
            ),
            PatchContextChange::Changed,
        );
        assert!(matches!(state.phase(), PatchInspectionPhase::Idle));
        assert!(!state.finish_inspection(key, Ok(snapshot())));
        assert_eq!(
            state.set_context(
                Game::Crusaders,
                Some(PathBuf::from("/games/crusaders-new")),
                4,
            ),
            PatchContextChange::Unchanged,
        );

        state.set_context(Game::Heroes, Some(PathBuf::from("/games/heroes")), 1);
        assert!(state.begin_inspection().is_none());
        state.set_context(Game::Crusaders, None, 5);
        assert!(state.begin_inspection().is_none());
    }

    #[test]
    fn confirmations_hold_stable_targets_and_one_operation_owns_all_contexts() {
        let mut state = ready_state();
        let operation = PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            target: PatchTarget::Applied,
        };
        assert!(state.request_operation(operation));
        let confirmation = state.pending_confirmation().expect("pending confirmation");
        assert_eq!(
            confirmation.executable(),
            Path::new("/games/crusaders/Kuf2Main.exe")
        );
        assert_eq!(
            confirmation.backup(),
            Path::new("/games/crusaders/Kuf2Main.exe.bak")
        );

        let launch = state.confirm_operation().expect("current confirmation");
        assert_eq!(launch.operation(), operation);
        assert!(state.operation_in_progress());
        assert_eq!(
            state.active_backup(),
            Some(Path::new("/games/crusaders/Kuf2Main.exe.bak"))
        );
        assert!(!state.request_operation(PatchOperation::SetFireRate {
            id: FireRatePresetID::Turbo,
        }));

        state.set_context(Game::Crusaders, Some(PathBuf::from("/games/other")), 2);
        assert!(state.operation_in_progress());
        assert!(state.begin_inspection().is_none());
        assert_eq!(
            state.finish_operation(launch.key()),
            PatchFinish::ContextChanged
        );
        assert!(!state.operation_in_progress());
        assert!(state.begin_inspection().is_some());
    }

    #[test]
    fn operation_admission_matches_known_patch_and_fire_rate_states() {
        let mut state = ready_state();
        assert!(!state.request_operation(PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            target: PatchTarget::NotApplied,
        }));
        assert!(!state.request_operation(PatchOperation::SetPatch {
            id: PatchID::TerrainBounds,
            target: PatchTarget::Applied,
        }));
        assert!(!state.request_operation(PatchOperation::SetFireRate {
            id: FireRatePresetID::Original,
        }));
        assert!(state.request_operation(PatchOperation::SetFireRate {
            id: FireRatePresetID::Turbo,
        }));
        state.dismiss_confirmation();

        let custom = PatchSnapshot::new(
            "/games/crusaders/Kuf2Main.exe".into(),
            "/games/crusaders/Kuf2Main.exe.bak".into(),
            BackupStatus::Present,
            [
                ExecutablePatchState::new(PatchID::DebugMenu, PatchStatus::Applied),
                ExecutablePatchState::new(PatchID::TerrainBounds, PatchStatus::NotApplied),
            ],
            FireRateStatus::Custom(kufeditor_patches::FireRateValues::new(4, 2, -0.008)),
        );
        let key = state
            .begin_inspection()
            .expect("begin replacement inspection");
        assert!(state.finish_inspection(key, Ok(custom)));
        assert!(state.request_operation(PatchOperation::SetPatch {
            id: PatchID::DebugMenu,
            target: PatchTarget::NotApplied,
        }));
        state.dismiss_confirmation();
        assert!(state.request_operation(PatchOperation::SetFireRate {
            id: FireRatePresetID::Fast,
        }));
    }

    #[test]
    fn failed_inspection_is_bound_to_its_key() {
        let mut state = PatchPresentationState::new(
            Game::Crusaders,
            Some(PathBuf::from("/games/crusaders")),
            1,
        );
        let key = state.begin_inspection().expect("configured root");
        assert!(state.finish_inspection(key, Err("unsupported executable".to_owned())));
        let PatchInspectionPhase::Failed { message, .. } = state.phase() else {
            panic!("expected failed inspection");
        };
        assert_eq!(message, "unsupported executable");
    }

    fn ready_state() -> PatchPresentationState {
        let mut state = PatchPresentationState::new(
            Game::Crusaders,
            Some(PathBuf::from("/games/crusaders")),
            1,
        );
        let key = state.begin_inspection().expect("configured root");
        assert!(state.finish_inspection(key, Ok(snapshot())));
        state
    }

    fn snapshot() -> PatchSnapshot {
        PatchSnapshot::new(
            "/games/crusaders/Kuf2Main.exe".into(),
            "/games/crusaders/Kuf2Main.exe.bak".into(),
            BackupStatus::Missing,
            [
                ExecutablePatchState::new(PatchID::DebugMenu, PatchStatus::NotApplied),
                ExecutablePatchState::new(PatchID::TerrainBounds, PatchStatus::Unknown),
            ],
            FireRateStatus::Preset(FireRatePresetID::Original),
        )
    }
}
