use std::io;

use crate::{
    GameRoot, InstallationConflictKind, InstallationID, InstalledMod, InstalledModStatus, ModError,
    ModPackageID, ModProgress, ModProgressPhase, ModProgressReporter, ModService, PackageErrorKind,
    RelativeGamePath, UninstallErrorKind,
    library::existing_package_directory,
    package::inspect_package,
    registry::{
        InstallationPlanConflict, InstallationRecord, installation_plan_conflict,
        load_installation_records, store_installations_with_hook,
    },
    transaction::{
        FileTransaction, TransactionFailpoint, UninstallTransaction, validate_game_root,
    },
};

#[derive(Clone, Copy, Debug)]
pub struct UninstallModRequest<'a> {
    root: &'a GameRoot,
    installation_id: InstallationID,
}

impl<'a> UninstallModRequest<'a> {
    pub const fn new(root: &'a GameRoot, installation_id: InstallationID) -> Self {
        Self {
            root,
            installation_id,
        }
    }

    pub const fn root(&self) -> &GameRoot {
        self.root
    }

    pub const fn installation_id(&self) -> InstallationID {
        self.installation_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstalledFileChangeKind {
    Modified,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangedInstalledFile {
    path: RelativeGamePath,
    kind: InstalledFileChangeKind,
}

impl ChangedInstalledFile {
    pub(crate) const fn new(path: RelativeGamePath, kind: InstalledFileChangeKind) -> Self {
        Self { path, kind }
    }

    pub const fn path(&self) -> &RelativeGamePath {
        &self.path
    }

    pub const fn kind(&self) -> InstalledFileChangeKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangedInstalledFiles {
    files: Vec<ChangedInstalledFile>,
}

impl ChangedInstalledFiles {
    pub(crate) const fn new(files: Vec<ChangedInstalledFile>) -> Self {
        Self { files }
    }

    pub fn files(&self) -> &[ChangedInstalledFile] {
        &self.files
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallModReport {
    installation_id: InstallationID,
    restored_paths: Vec<RelativeGamePath>,
    removed_paths: Vec<RelativeGamePath>,
}

impl UninstallModReport {
    pub const fn installation_id(&self) -> InstallationID {
        self.installation_id
    }

    pub fn restored_paths(&self) -> &[RelativeGamePath] {
        &self.restored_paths
    }

    pub fn removed_paths(&self) -> &[RelativeGamePath] {
        &self.removed_paths
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ApplyModRequest<'a> {
    root: &'a GameRoot,
    package_id: ModPackageID,
}

impl<'a> ApplyModRequest<'a> {
    pub const fn new(root: &'a GameRoot, package_id: ModPackageID) -> Self {
        Self { root, package_id }
    }

    pub const fn root(&self) -> &GameRoot {
        self.root
    }

    pub const fn package_id(&self) -> ModPackageID {
        self.package_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyModReport {
    installation: InstalledMod,
    committed_paths: Vec<RelativeGamePath>,
}

impl ApplyModReport {
    pub const fn installation(&self) -> &InstalledMod {
        &self.installation
    }

    pub fn committed_paths(&self) -> &[RelativeGamePath] {
        &self.committed_paths
    }
}

impl ModService {
    pub fn apply(
        &self,
        request: ApplyModRequest<'_>,
        progress: &mut impl ModProgressReporter,
    ) -> Result<ApplyModReport, ModError> {
        self.apply_with_failpoint(request, progress, &TransactionFailpoint::default())
    }

    pub fn uninstall(
        &self,
        request: UninstallModRequest<'_>,
        progress: &mut impl ModProgressReporter,
    ) -> Result<UninstallModReport, ModError> {
        self.uninstall_with_failpoint(request, progress, &TransactionFailpoint::default())
    }

    fn uninstall_with_failpoint(
        &self,
        request: UninstallModRequest<'_>,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<UninstallModReport, ModError> {
        validate_game_root(request.root)?;
        report_uninstall_planning(progress, 0)?;
        let records = load_installation_records(&self.paths, &self.limits)?;
        let Some(record) = records
            .iter()
            .find(|record| record.installation_id == request.installation_id)
            .cloned()
        else {
            return Err(ModError::uninstall(
                request.installation_id,
                None,
                UninstallErrorKind::MissingInstallation,
            ));
        };
        if record.root_key != request.root.key() {
            return Err(ModError::uninstall(
                request.installation_id,
                Some(request.root.canonical_path().to_path_buf()),
                UninstallErrorKind::WrongRoot,
            ));
        }
        let mut transaction =
            UninstallTransaction::load(&self.paths, request.root, &record, &self.limits)?;
        report_uninstall_planning(progress, 1)?;
        transaction.stage_installed(&self.limits, progress)?;

        let mut records = match load_installation_records(&self.paths, &self.limits) {
            Ok(records) => records,
            Err(error) => return Err(transaction.recover_error(error, progress, failpoint)),
        };
        let Some(record_index) = records
            .iter()
            .position(|candidate| candidate.installation_id == request.installation_id)
        else {
            let error = ModError::uninstall(
                request.installation_id,
                None,
                UninstallErrorKind::MissingInstallation,
            );
            return Err(transaction.recover_error(error, progress, failpoint));
        };
        if records.get(record_index) != Some(&record) {
            let error = ModError::uninstall(
                request.installation_id,
                Some(self.paths.installation_registry()),
                UninstallErrorKind::InvalidRecoveryImage,
            );
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        if let Err(error) = transaction.restore_originals(&self.limits, progress, failpoint) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        records.remove(record_index);

        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::PublishingUninstall,
                completed: 0,
                total: 1,
                path: None,
            })
            .is_break()
        {
            let error = ModError::Canceled {
                operation: "mod uninstall",
            };
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        if let Err(error) = store_installations_with_hook(&self.paths, &records, |path| {
            failpoint.check_registry_publication(path)
        }) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        let _ = progress.report(&ModProgress {
            phase: ModProgressPhase::PublishingUninstall,
            completed: 1,
            total: 1,
            path: None,
        });
        let restored_paths = transaction.restored_paths().to_vec();
        let removed_paths = transaction.removed_paths().to_vec();
        transaction.finish_success()?;
        Ok(UninstallModReport {
            installation_id: request.installation_id,
            restored_paths,
            removed_paths,
        })
    }

    fn apply_with_failpoint(
        &self,
        request: ApplyModRequest<'_>,
        progress: &mut impl ModProgressReporter,
        failpoint: &TransactionFailpoint,
    ) -> Result<ApplyModReport, ModError> {
        validate_game_root(request.root)?;
        report_planning(progress, 0)?;
        let package = self.inspect_apply_package(request, progress)?;

        let records = load_installation_records(&self.paths, &self.limits)?;
        reject_conflict(
            &records,
            request.root,
            package.manifest().metadata().name(),
            package.manifest().files(),
        )?;
        report_planning(progress, 1)?;

        let installed_at = crate::ModTimestamp::now()?;
        let mut transaction = FileTransaction::begin_apply(
            &self.paths,
            request.root,
            request.package_id,
            installed_at,
            package.manifest().files(),
        )?;
        if let Err(error) = transaction.stage_package(&package, &self.limits, progress, failpoint) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        if let Err(error) = transaction.create_recovery(&self.limits, progress, failpoint) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }

        let mut records = match load_installation_records(&self.paths, &self.limits) {
            Ok(records) => records,
            Err(error) => return Err(transaction.recover_error(error, progress, failpoint)),
        };
        if let Err(error) = reject_conflict(
            &records,
            request.root,
            package.manifest().metadata().name(),
            package.manifest().files(),
        ) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        if let Err(error) = transaction.commit_files(&self.limits, progress, failpoint) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }

        let installed_files = match transaction.installed_files() {
            Ok(files) => files,
            Err(error) => return Err(transaction.recover_error(error, progress, failpoint)),
        };
        let record = InstallationRecord {
            installation_id: transaction.installation_id(),
            package_id: request.package_id,
            metadata: package.manifest().metadata().clone(),
            game: request.root.game(),
            configured_root: request.root.configured_path().to_path_buf(),
            canonical_root: request.root.canonical_path().to_path_buf(),
            root_key: request.root.key(),
            installed_at: transaction.installed_at().clone(),
            operation_id: transaction.operation_id(),
            files: installed_files,
        };
        records.push(record.clone());

        if progress
            .report(&ModProgress {
                phase: ModProgressPhase::PublishingInstallation,
                completed: 0,
                total: 1,
                path: None,
            })
            .is_break()
        {
            let error = ModError::Canceled {
                operation: "mod apply",
            };
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        if let Err(error) = store_installations_with_hook(&self.paths, &records, |path| {
            failpoint.check_registry_publication(path)
        }) {
            return Err(transaction.recover_error(error, progress, failpoint));
        }
        let _ = progress.report(&ModProgress {
            phase: ModProgressPhase::PublishingInstallation,
            completed: 1,
            total: 1,
            path: None,
        });

        Ok(ApplyModReport {
            installation: InstalledMod::from_record(record, Some(InstalledModStatus::Clean)),
            committed_paths: transaction.committed_paths(),
        })
    }

    fn package_path(&self, package_id: ModPackageID) -> Result<std::path::PathBuf, ModError> {
        let path = self.paths.packages().join(format!("{package_id}.zip"));
        let Some(_) = existing_package_directory(&self.paths)? else {
            return Err(ModError::package(
                path,
                None,
                PackageErrorKind::MissingLibraryPackage,
            ));
        };
        match std::fs::symlink_metadata(&path) {
            Ok(_) => Ok(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(ModError::package(
                path,
                None,
                PackageErrorKind::MissingLibraryPackage,
            )),
            Err(error) => Err(ModError::io("inspect package for apply", path, error)),
        }
    }

    fn inspect_apply_package(
        &self,
        request: ApplyModRequest<'_>,
        progress: &mut impl ModProgressReporter,
    ) -> Result<crate::ModPackageInfo, ModError> {
        let package_path = self.package_path(request.package_id)?;
        let package = inspect_package(&package_path, &self.limits, progress)?;
        if package.package_id() != request.package_id {
            return Err(ModError::package(
                package_path,
                None,
                PackageErrorKind::DestinationCollision,
            ));
        }
        if package.manifest().game() != request.root.game() {
            return Err(ModError::PackageGameMismatch {
                package: package.manifest().game(),
                target: request.root.game(),
            });
        }
        Ok(package)
    }
}

fn report_planning(
    progress: &mut impl ModProgressReporter,
    completed: u64,
) -> Result<(), ModError> {
    if progress
        .report(&ModProgress {
            phase: ModProgressPhase::PlanningApply,
            completed,
            total: 1,
            path: None,
        })
        .is_break()
    {
        Err(ModError::Canceled {
            operation: "mod apply",
        })
    } else {
        Ok(())
    }
}

fn report_uninstall_planning(
    progress: &mut impl ModProgressReporter,
    completed: u64,
) -> Result<(), ModError> {
    if progress
        .report(&ModProgress {
            phase: ModProgressPhase::PlanningUninstall,
            completed,
            total: 1,
            path: None,
        })
        .is_break()
    {
        Err(ModError::Canceled {
            operation: "mod uninstall",
        })
    } else {
        Ok(())
    }
}

fn reject_conflict(
    records: &[InstallationRecord],
    root: &GameRoot,
    name: &str,
    files: &[RelativeGamePath],
) -> Result<(), ModError> {
    let Some(conflict) = installation_plan_conflict(records, root.key(), name, files) else {
        return Ok(());
    };
    let (kind, installation, path) = match conflict {
        InstallationPlanConflict::DuplicateName { installation_id } => (
            InstallationConflictKind::DuplicateName,
            installation_id,
            None,
        ),
        InstallationPlanConflict::PathOverlap {
            installation_id,
            path,
        } => (
            InstallationConflictKind::PathOverlap,
            installation_id,
            Some(path),
        ),
    };
    Err(ModError::InstallationConflict {
        kind,
        installation,
        path,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write as _, ops::ControlFlow, path::Path};

    use kufeditor_game::Game;
    use tempfile::{TempDir, tempdir};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::{ApplyModRequest, ModService};
    use crate::{
        ModError, ModProgress, ModProgressPhase, ModProgressReporter, ModStorePaths,
        OperationState, RecoveryReport, RelativeGamePath, registry::store_installations,
        transaction::TransactionFailpoint,
    };

    struct ContinueProgress;

    impl ModProgressReporter for ContinueProgress {
        fn report(&mut self, _: &ModProgress) -> ControlFlow<()> {
            ControlFlow::Continue(())
        }
    }

    #[test]
    fn failures_before_the_first_game_write_remove_the_operation_and_preserve_every_target()
    -> Result<(), Box<dyn std::error::Error>> {
        for state in [
            OperationState::Planned,
            OperationState::Staged,
            OperationState::Recoverable,
        ] {
            let fixture = ApplyFixture::new()?;
            let registry_before = fs::read(fixture.stores.installation_registry())?;

            let result = fixture.service.apply_with_failpoint(
                ApplyModRequest::new(&fixture.root, fixture.package),
                &mut ContinueProgress,
                &TransactionFailpoint::after_state(state),
            );

            assert!(result.is_err(), "{state:?} failpoint must stop apply");
            fixture.assert_original_game()?;
            assert_eq!(
                fs::read(fixture.stores.installation_registry())?,
                registry_before
            );
            assert_eq!(fs::read_dir(fixture.stores.operations())?.count(), 0);
        }
        Ok(())
    }

    #[test]
    fn every_commit_failure_restores_the_exact_game_in_reverse_order()
    -> Result<(), Box<dyn std::error::Error>> {
        for committed_count in 1..=3 {
            let fixture = ApplyFixture::new()?;
            let registry_before = fs::read(fixture.stores.installation_registry())?;

            let error = fixture
                .service
                .apply_with_failpoint(
                    ApplyModRequest::new(&fixture.root, fixture.package),
                    &mut ContinueProgress,
                    &TransactionFailpoint::after_committed_paths(committed_count),
                )
                .expect_err("the commit failpoint must stop apply");
            let recovery = recovery(&error)?;

            fixture.assert_original_game()?;
            assert_eq!(
                path_strings(recovery.committed()),
                ["a.sox", "b.sox", "c.sox"]
                    .get(..committed_count)
                    .ok_or("invalid committed-path fixture range")?
            );
            assert_eq!(
                path_strings(recovery.rolled_back()),
                ["c.sox", "b.sox", "a.sox"]
                    .get((3 - committed_count)..)
                    .ok_or("invalid rollback-path fixture range")?
            );
            assert!(recovery.rollback_failed().is_empty());
            assert_eq!(
                path_strings(recovery.unchanged()),
                ["a.sox", "b.sox", "c.sox"]
                    .get(committed_count..)
                    .ok_or("invalid unchanged-path fixture range")?
            );
            assert_eq!(
                fs::read(fixture.stores.installation_registry())?,
                registry_before
            );
            fixture.assert_one_retained_operation()?;
        }
        Ok(())
    }

    #[test]
    fn registry_failure_rolls_back_all_files_and_retains_the_operation()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = ApplyFixture::new()?;
        let registry_before = fs::read(fixture.stores.installation_registry())?;

        let error = fixture
            .service
            .apply_with_failpoint(
                ApplyModRequest::new(&fixture.root, fixture.package),
                &mut ContinueProgress,
                &TransactionFailpoint::at_registry_publication(),
            )
            .expect_err("the registry failpoint must stop apply");
        let recovery = recovery(&error)?;

        fixture.assert_original_game()?;
        assert_eq!(
            path_strings(recovery.committed()),
            ["a.sox", "b.sox", "c.sox"]
        );
        assert_eq!(
            path_strings(recovery.rolled_back()),
            ["c.sox", "b.sox", "a.sox"]
        );
        assert!(recovery.rollback_failed().is_empty());
        assert!(recovery.unchanged().is_empty());
        assert_eq!(
            fs::read(fixture.stores.installation_registry())?,
            registry_before
        );
        fixture.assert_one_retained_operation()?;
        Ok(())
    }

    #[test]
    fn rollback_failure_reports_the_failed_path_and_continues_restoring_other_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = ApplyFixture::new()?;
        let failpoint = TransactionFailpoint::after_committed_paths(3).with_rollback_attempt(2);

        let error = fixture
            .service
            .apply_with_failpoint(
                ApplyModRequest::new(&fixture.root, fixture.package),
                &mut ContinueProgress,
                &failpoint,
            )
            .expect_err("the commit failpoint must stop apply");
        let recovery = recovery(&error)?;

        assert_eq!(fs::read(fixture.root_path.join("a.sox"))?, b"old-a");
        assert_eq!(fs::read(fixture.root_path.join("b.sox"))?, b"new-b");
        assert_eq!(fs::read(fixture.root_path.join("c.sox"))?, b"old-c");
        assert_eq!(
            path_strings(recovery.committed()),
            ["a.sox", "b.sox", "c.sox"]
        );
        assert_eq!(path_strings(recovery.rolled_back()), ["c.sox", "a.sox"]);
        assert_eq!(path_strings(recovery.rollback_failed()), ["b.sox"]);
        assert!(recovery.unchanged().is_empty());
        fixture.assert_one_retained_operation()?;
        Ok(())
    }

    #[test]
    fn cancellation_before_and_after_game_writes_uses_the_matching_cleanup_policy()
    -> Result<(), Box<dyn std::error::Error>> {
        for (phase, completed, retains_operation) in [
            (ModProgressPhase::PlanningApply, 0, false),
            (ModProgressPhase::InspectingPackage, 1, false),
            (ModProgressPhase::StagingFiles, 1, false),
            (ModProgressPhase::CreatingRecovery, 1, false),
            (ModProgressPhase::CommittingFiles, 0, false),
            (ModProgressPhase::CommittingFiles, 1, true),
            (ModProgressPhase::PublishingInstallation, 0, true),
        ] {
            let fixture = ApplyFixture::new()?;
            let mut progress = CancelAt { phase, completed };

            let error = fixture
                .service
                .apply(
                    ApplyModRequest::new(&fixture.root, fixture.package),
                    &mut progress,
                )
                .expect_err("the selected progress point must cancel apply");

            fixture.assert_original_game()?;
            assert_eq!(error.recovery_report().is_some(), retains_operation);
            let operation_count = if fixture.stores.operations().exists() {
                fs::read_dir(fixture.stores.operations())?.count()
            } else {
                0
            };
            assert_eq!(operation_count, usize::from(retains_operation));
        }
        Ok(())
    }

    #[test]
    fn every_uninstall_restore_failure_returns_to_the_exact_installed_state_in_reverse_order()
    -> Result<(), Box<dyn std::error::Error>> {
        for restored_count in 1..=3 {
            let fixture = ApplyFixture::new()?;
            let installation_id = fixture.install()?;
            let registry_before = fs::read(fixture.stores.installation_registry())?;

            let error = fixture
                .service
                .uninstall_with_failpoint(
                    super::UninstallModRequest::new(&fixture.root, installation_id),
                    &mut ContinueProgress,
                    &TransactionFailpoint::after_committed_paths(restored_count),
                )
                .expect_err("the restore failpoint must stop uninstall");
            let recovery = recovery(&error)?;

            fixture.assert_installed_game()?;
            assert_eq!(
                path_strings(recovery.committed()),
                ["a.sox", "b.sox", "c.sox"]
                    .get(..restored_count)
                    .ok_or("invalid restored-path fixture range")?
            );
            assert_eq!(
                path_strings(recovery.rolled_back()),
                ["c.sox", "b.sox", "a.sox"]
                    .get((3 - restored_count)..)
                    .ok_or("invalid uninstall rollback fixture range")?
            );
            assert!(recovery.rollback_failed().is_empty());
            assert_eq!(
                path_strings(recovery.unchanged()),
                ["a.sox", "b.sox", "c.sox"]
                    .get(restored_count..)
                    .ok_or("invalid uninstall unchanged fixture range")?
            );
            assert_eq!(
                fs::read(fixture.stores.installation_registry())?,
                registry_before
            );
            fixture.assert_retained_uninstall_staging()?;
        }
        Ok(())
    }

    #[test]
    fn uninstall_registry_failure_restores_every_installed_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = ApplyFixture::new()?;
        let installation_id = fixture.install()?;
        let registry_before = fs::read(fixture.stores.installation_registry())?;

        let error = fixture
            .service
            .uninstall_with_failpoint(
                super::UninstallModRequest::new(&fixture.root, installation_id),
                &mut ContinueProgress,
                &TransactionFailpoint::at_registry_publication(),
            )
            .expect_err("the registry failpoint must stop uninstall");
        let recovery = recovery(&error)?;

        fixture.assert_installed_game()?;
        assert_eq!(
            path_strings(recovery.committed()),
            ["a.sox", "b.sox", "c.sox"]
        );
        assert_eq!(
            path_strings(recovery.rolled_back()),
            ["c.sox", "b.sox", "a.sox"]
        );
        assert!(recovery.rollback_failed().is_empty());
        assert!(recovery.unchanged().is_empty());
        assert_eq!(
            fs::read(fixture.stores.installation_registry())?,
            registry_before
        );
        fixture.assert_retained_uninstall_staging()?;
        Ok(())
    }

    #[test]
    fn uninstall_rollback_failure_is_reported_without_skipping_other_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = ApplyFixture::new()?;
        let installation_id = fixture.install()?;
        let failpoint = TransactionFailpoint::after_committed_paths(3).with_rollback_attempt(2);

        let error = fixture
            .service
            .uninstall_with_failpoint(
                super::UninstallModRequest::new(&fixture.root, installation_id),
                &mut ContinueProgress,
                &failpoint,
            )
            .expect_err("the restore failpoint must stop uninstall");
        let recovery = recovery(&error)?;

        assert_eq!(fs::read(fixture.root_path.join("a.sox"))?, b"new-a");
        assert!(!fixture.root_path.join("b.sox").exists());
        assert_eq!(fs::read(fixture.root_path.join("c.sox"))?, b"new-c");
        assert_eq!(path_strings(recovery.rolled_back()), ["c.sox", "a.sox"]);
        assert_eq!(path_strings(recovery.rollback_failed()), ["b.sox"]);
        fixture.assert_retained_uninstall_staging()?;
        Ok(())
    }

    #[test]
    fn uninstall_cancellation_cleans_before_writes_and_rolls_back_after_writes()
    -> Result<(), Box<dyn std::error::Error>> {
        for (phase, completed, retains_staging) in [
            (ModProgressPhase::PlanningUninstall, 0, false),
            (ModProgressPhase::StagingUninstall, 1, false),
            (ModProgressPhase::RestoringFiles, 0, false),
            (ModProgressPhase::RestoringFiles, 1, true),
            (ModProgressPhase::PublishingUninstall, 0, true),
        ] {
            let fixture = ApplyFixture::new()?;
            let installation_id = fixture.install()?;
            let mut progress = CancelAt { phase, completed };

            let error = fixture
                .service
                .uninstall(
                    super::UninstallModRequest::new(&fixture.root, installation_id),
                    &mut progress,
                )
                .expect_err("the selected progress point must cancel uninstall");

            fixture.assert_installed_game()?;
            assert_eq!(error.recovery_report().is_some(), retains_staging);
            assert_eq!(
                fixture.uninstall_staging_count()?,
                usize::from(retains_staging)
            );
        }
        Ok(())
    }

    fn recovery(error: &ModError) -> Result<&RecoveryReport, Box<dyn std::error::Error>> {
        error
            .recovery_report()
            .ok_or_else(|| "missing recovery report".into())
    }

    fn path_strings(paths: &[RelativeGamePath]) -> Vec<&str> {
        paths.iter().map(RelativeGamePath::as_str).collect()
    }

    struct CancelAt {
        phase: ModProgressPhase,
        completed: u64,
    }

    impl ModProgressReporter for CancelAt {
        fn report(&mut self, progress: &ModProgress) -> ControlFlow<()> {
            if progress.phase == self.phase && progress.completed == self.completed {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    struct ApplyFixture {
        _directory: TempDir,
        stores: ModStorePaths,
        service: ModService,
        root_path: std::path::PathBuf,
        root: crate::GameRoot,
        package: crate::ModPackageID,
    }

    impl ApplyFixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let directory = tempdir()?;
            let stores = ModStorePaths::new(directory.path().join("application-data"));
            let root_path = directory.path().join("game");
            fs::create_dir(&root_path)?;
            fs::write(root_path.join("a.sox"), b"old-a")?;
            fs::write(root_path.join("c.sox"), b"old-c")?;
            let root = crate::GameRoot::inspect(Game::Heroes, root_path.clone(), &stores)?;
            let service = ModService::new(stores.clone());
            let source = directory.path().join("fixture.zip");
            write_package(&source)?;
            let package = service
                .import_package(&source, &mut ContinueProgress)?
                .package()
                .package_id();
            store_installations(&stores, &[])?;
            Ok(Self {
                _directory: directory,
                stores,
                service,
                root_path,
                root,
                package,
            })
        }

        fn assert_original_game(&self) -> Result<(), Box<dyn std::error::Error>> {
            assert_eq!(fs::read(self.root_path.join("a.sox"))?, b"old-a");
            assert!(!self.root_path.join("b.sox").exists());
            assert_eq!(fs::read(self.root_path.join("c.sox"))?, b"old-c");
            Ok(())
        }

        fn install(&self) -> Result<crate::InstallationID, Box<dyn std::error::Error>> {
            Ok(self
                .service
                .apply(
                    ApplyModRequest::new(&self.root, self.package),
                    &mut ContinueProgress,
                )?
                .installation()
                .installation_id())
        }

        fn assert_installed_game(&self) -> Result<(), Box<dyn std::error::Error>> {
            assert_eq!(fs::read(self.root_path.join("a.sox"))?, b"new-a");
            assert_eq!(fs::read(self.root_path.join("b.sox"))?, b"new-b");
            assert_eq!(fs::read(self.root_path.join("c.sox"))?, b"new-c");
            Ok(())
        }

        fn assert_retained_uninstall_staging(&self) -> Result<(), Box<dyn std::error::Error>> {
            let operation = fs::read_dir(self.stores.operations())?
                .next()
                .transpose()?
                .ok_or("missing retained apply operation")?
                .path();
            let staging = fs::read_dir(operation)?
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("uninstall-")
                })
                .collect::<Vec<_>>();
            assert_eq!(staging.len(), 1);
            assert!(
                staging
                    .first()
                    .ok_or("missing retained uninstall staging")?
                    .path()
                    .join("uninstall-v1.json")
                    .is_file()
            );
            Ok(())
        }

        fn uninstall_staging_count(&self) -> Result<usize, Box<dyn std::error::Error>> {
            let operation = fs::read_dir(self.stores.operations())?
                .next()
                .transpose()?
                .ok_or("missing retained apply operation")?
                .path();
            Ok(fs::read_dir(operation)?
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("uninstall-")
                })
                .count())
        }

        fn assert_one_retained_operation(&self) -> Result<(), Box<dyn std::error::Error>> {
            let entries = fs::read_dir(self.stores.operations())?.collect::<Result<Vec<_>, _>>()?;
            assert_eq!(entries.len(), 1);
            let operation = entries
                .first()
                .ok_or("missing retained operation")?
                .path()
                .join("operation-v1.json");
            assert!(operation.is_file());
            let image: serde_json::Value = serde_json::from_slice(&fs::read(operation)?)?;
            assert_eq!(image.get("state"), Some(&serde_json::json!("recoverable")));
            assert!(
                self.stores
                    .operations()
                    .join(
                        entries
                            .first()
                            .ok_or("missing retained operation")?
                            .file_name()
                    )
                    .join("staged/a.sox")
                    .is_file()
            );
            Ok(())
        }
    }

    fn write_package(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = fs::File::create(path)?;
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        writer.start_file("mod.json", options)?;
        writer.write_all(
            br#"{"name":"Fixture","version":"1","game":"heroes","files":["a.sox","b.sox","c.sox"]}"#,
        )?;
        for (name, bytes) in [
            ("a.sox", b"new-a".as_slice()),
            ("b.sox", b"new-b".as_slice()),
            ("c.sox", b"new-c".as_slice()),
        ] {
            writer.start_file(name, options)?;
            writer.write_all(bytes)?;
        }
        writer.finish()?.sync_all()?;
        Ok(())
    }
}
