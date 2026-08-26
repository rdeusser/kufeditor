use std::ops::ControlFlow;

use crate::RelativeGamePath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModProgressPhase {
    InspectingPackage,
    CopyingPackage,
    CreatingPackage,
    PublishingPackage,
    PlanningApply,
    StagingFiles,
    CreatingRecovery,
    CommittingFiles,
    PublishingInstallation,
    PlanningUninstall,
    StagingUninstall,
    RestoringFiles,
    PublishingUninstall,
    RollingBack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModProgress {
    pub phase: ModProgressPhase,
    pub completed: u64,
    pub total: u64,
    pub path: Option<RelativeGamePath>,
}

pub trait ModProgressReporter {
    fn report(&mut self, progress: &ModProgress) -> ControlFlow<()>;
}

pub(crate) struct ContinueProgress;

impl ModProgressReporter for ContinueProgress {
    fn report(&mut self, _: &ModProgress) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}
