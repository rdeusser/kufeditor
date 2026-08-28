//! Executable patch definitions, detection, backup, apply, and revert operations.

mod definitions;
mod error;
mod inspection;
mod transaction;

pub use definitions::{
    ByteEdit, ContextImage, FireRatePreset, FireRatePresetID, FireRateValues, PatchDefinition,
    PatchID, fire_rate_presets, patch_definitions,
};
pub use error::{PatchError, RecoveryStatus, RollbackFailure, RollbackStage};
pub use inspection::{
    BackupStatus, ExecutableInspection, FireRateStatus, MAX_EXECUTABLE_BYTES, PatchState,
    PatchStatus, inspect,
};
pub use transaction::{PatchChange, PatchOperationResult, PatchTarget, set_fire_rate, set_patch};
