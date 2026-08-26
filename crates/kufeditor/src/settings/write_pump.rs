use std::path::PathBuf;

use super::{
    PersistenceMode,
    store::{SettingsImageV1, SettingsSaveError, save_image},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SettingsRevision(u64);

impl SettingsRevision {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

pub(crate) struct SettingsWriteRequest {
    revision: SettingsRevision,
    path: PathBuf,
    image: SettingsImageV1,
}

impl SettingsWriteRequest {
    #[cfg(test)]
    pub(crate) const fn revision(&self) -> SettingsRevision {
        self.revision
    }

    pub(crate) fn run(self) -> SettingsWriteCompletion {
        let result = save_image(&self.path, &self.image);
        SettingsWriteCompletion {
            revision: self.revision,
            image: self.image,
            result,
        }
    }
}

pub(crate) struct SettingsWriteCompletion {
    revision: SettingsRevision,
    image: SettingsImageV1,
    result: Result<(), SettingsSaveError>,
}

pub(crate) struct SettingsWritePump {
    path: PathBuf,
    mode: PersistenceMode,
    next_revision: u64,
    latest_revision: Option<SettingsRevision>,
    pending: Option<(SettingsRevision, SettingsImageV1)>,
    in_flight: Option<SettingsRevision>,
    failed: Option<(SettingsRevision, SettingsImageV1)>,
}

pub(crate) enum SettingsQueueResult {
    Queued(SettingsRevision),
    Protected(SettingsRevision),
}

pub(crate) struct SettingsWriteFinish {
    pub(crate) revision: SettingsRevision,
    pub(crate) is_latest: bool,
    pub(crate) result: Result<(), SettingsSaveError>,
}

impl SettingsWritePump {
    pub(crate) fn new(path: PathBuf, mode: PersistenceMode) -> Self {
        Self {
            path,
            mode,
            next_revision: 1,
            latest_revision: None,
            pending: None,
            in_flight: None,
            failed: None,
        }
    }

    pub(crate) fn queue(&mut self, image: SettingsImageV1) -> SettingsQueueResult {
        self.failed = None;
        let revision = self.allocate_revision();
        self.latest_revision = Some(revision);
        if matches!(
            self.mode,
            PersistenceMode::ProtectedUnsupportedVersion { .. }
        ) {
            self.pending = None;
            return SettingsQueueResult::Protected(revision);
        }
        self.pending = Some((revision, image));
        SettingsQueueResult::Queued(revision)
    }

    pub(crate) fn take_ready(&mut self) -> Option<SettingsWriteRequest> {
        if self.in_flight.is_some() {
            return None;
        }
        let (revision, image) = self.pending.take()?;
        self.in_flight = Some(revision);
        Some(SettingsWriteRequest {
            revision,
            path: self.path.clone(),
            image,
        })
    }

    pub(crate) fn finish(&mut self, completion: SettingsWriteCompletion) -> SettingsWriteFinish {
        debug_assert_eq!(self.in_flight, Some(completion.revision));
        if self.in_flight == Some(completion.revision) {
            self.in_flight = None;
        }
        let is_latest = self.latest_revision == Some(completion.revision);
        if is_latest && completion.result.is_err() {
            self.failed = Some((completion.revision, completion.image));
        }
        SettingsWriteFinish {
            revision: completion.revision,
            is_latest,
            result: completion.result,
        }
    }

    pub(crate) fn retry_failed(&mut self) -> Option<SettingsRevision> {
        let (_, image) = self.failed.take()?;
        let revision = self.allocate_revision();
        self.latest_revision = Some(revision);
        self.pending = Some((revision, image));
        Some(revision)
    }

    pub(crate) fn discard_failed(&mut self) -> bool {
        self.failed.take().is_some()
    }

    pub(crate) fn discard_obsolete(&mut self) {
        self.pending = None;
        self.failed = None;
        let revision = self.allocate_revision();
        self.latest_revision = Some(revision);
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.pending.is_none() && self.in_flight.is_none() && self.failed.is_none()
    }

    pub(crate) fn has_failed(&self) -> bool {
        self.failed.is_some()
    }

    pub(crate) const fn mode(&self) -> &PersistenceMode {
        &self.mode
    }

    #[cfg(test)]
    pub(crate) const fn latest_revision_for_test(&self) -> Option<SettingsRevision> {
        self.latest_revision
    }

    fn allocate_revision(&mut self) -> SettingsRevision {
        let revision = SettingsRevision(self.next_revision);
        self.next_revision += 1;
        revision
    }
}

#[cfg(test)]
mod tests {
    use std::{io, path::PathBuf};

    use kufeditor_game::{Game, GamePaths};
    use kufeditor_workspace::RecentFiles;

    use super::{
        SettingsQueueResult, SettingsRevision, SettingsWriteCompletion, SettingsWritePump,
    };
    use crate::settings::{
        PersistenceMode,
        store::{SettingsImageV1, SettingsSaveError, image_from_runtime},
    };

    fn image(game: Game) -> SettingsImageV1 {
        image_from_runtime(game, &GamePaths::default(), &RecentFiles::default()).unwrap()
    }

    fn successful(request: super::SettingsWriteRequest) -> SettingsWriteCompletion {
        SettingsWriteCompletion {
            revision: request.revision,
            image: request.image,
            result: Ok(()),
        }
    }

    fn failed(request: super::SettingsWriteRequest) -> SettingsWriteCompletion {
        SettingsWriteCompletion {
            revision: request.revision,
            image: request.image,
            result: Err(SettingsSaveError::Write {
                path: PathBuf::from("settings.json"),
                source: io::Error::other("fixture failure"),
            }),
        }
    }

    #[test]
    fn pending_images_coalesce_while_one_write_is_in_flight() {
        let mut pump =
            SettingsWritePump::new(PathBuf::from("settings.json"), PersistenceMode::Enabled);
        assert!(matches!(
            pump.queue(image(Game::Crusaders)),
            SettingsQueueResult::Queued(SettingsRevision(1))
        ));
        let first = pump.take_ready().unwrap();

        assert!(matches!(
            pump.queue(image(Game::Heroes)),
            SettingsQueueResult::Queued(SettingsRevision(2))
        ));
        assert!(matches!(
            pump.queue(image(Game::Crusaders)),
            SettingsQueueResult::Queued(SettingsRevision(3))
        ));
        assert!(pump.take_ready().is_none());

        let finish = pump.finish(successful(first));
        assert_eq!(finish.revision, SettingsRevision(1));
        assert!(!finish.is_latest);
        assert!(finish.result.is_ok());
        assert_eq!(pump.take_ready().unwrap().revision(), SettingsRevision(3));
    }

    #[test]
    fn a_latest_failure_is_retained_and_retried_as_a_newer_revision() {
        let mut pump =
            SettingsWritePump::new(PathBuf::from("settings.json"), PersistenceMode::Enabled);
        pump.queue(image(Game::Crusaders));
        let first = pump.take_ready().unwrap();

        let finish = pump.finish(failed(first));
        assert!(finish.is_latest);
        assert!(finish.result.is_err());
        assert!(pump.has_failed());
        assert_eq!(pump.retry_failed(), Some(SettingsRevision(2)));
        assert!(!pump.has_failed());
        assert_eq!(pump.take_ready().unwrap().revision(), SettingsRevision(2));
    }

    #[test]
    fn a_new_image_discards_an_obsolete_retained_failure() {
        let mut pump =
            SettingsWritePump::new(PathBuf::from("settings.json"), PersistenceMode::Enabled);
        pump.queue(image(Game::Crusaders));
        let first = pump.take_ready().unwrap();
        pump.finish(failed(first));
        assert!(pump.has_failed());

        assert!(matches!(
            pump.queue(image(Game::Heroes)),
            SettingsQueueResult::Queued(SettingsRevision(2))
        ));
        assert!(!pump.has_failed());
        assert_eq!(pump.take_ready().unwrap().revision(), SettingsRevision(2));
    }

    #[test]
    fn invalid_runtime_state_discards_obsolete_in_flight_and_retained_images() {
        let mut pump =
            SettingsWritePump::new(PathBuf::from("settings.json"), PersistenceMode::Enabled);
        pump.queue(image(Game::Crusaders));
        let in_flight = pump.take_ready().unwrap();

        pump.discard_obsolete();
        let finish = pump.finish(failed(in_flight));

        assert!(!finish.is_latest);
        assert!(!pump.has_failed());
        assert!(pump.is_settled());

        pump.queue(image(Game::Heroes));
        let latest = pump.take_ready().unwrap();
        pump.finish(failed(latest));
        assert!(pump.has_failed());

        pump.discard_obsolete();

        assert!(!pump.has_failed());
        assert!(pump.is_settled());
    }

    #[test]
    fn protected_mode_never_creates_a_write_request_and_is_settled() {
        let mode = PersistenceMode::ProtectedUnsupportedVersion { found: 2 };
        let mut pump = SettingsWritePump::new(PathBuf::from("settings.json"), mode.clone());

        assert!(matches!(
            pump.queue(image(Game::Crusaders)),
            SettingsQueueResult::Protected(SettingsRevision(1))
        ));
        assert!(pump.take_ready().is_none());
        assert!(pump.is_settled());
        assert_eq!(pump.mode(), &mode);
    }

    #[test]
    fn only_the_latest_completion_is_notice_relevant() {
        let mut pump =
            SettingsWritePump::new(PathBuf::from("settings.json"), PersistenceMode::Enabled);
        pump.queue(image(Game::Crusaders));
        let first = pump.take_ready().unwrap();
        pump.queue(image(Game::Heroes));

        assert!(!pump.finish(successful(first)).is_latest);

        let latest = pump.take_ready().unwrap();
        assert!(pump.finish(successful(latest)).is_latest);
        assert!(pump.is_settled());
    }

    #[test]
    fn close_settlement_tracks_pending_in_flight_failure_retry_and_discard() {
        let mut pump =
            SettingsWritePump::new(PathBuf::from("settings.json"), PersistenceMode::Enabled);
        pump.queue(image(Game::Crusaders));
        assert!(!pump.is_settled());

        let first = pump.take_ready().unwrap();
        assert!(!pump.is_settled());
        pump.queue(image(Game::Heroes));
        assert!(!pump.is_settled());
        pump.finish(successful(first));
        assert!(!pump.is_settled());

        let latest = pump.take_ready().unwrap();
        pump.finish(failed(latest));
        assert!(pump.has_failed());
        assert!(!pump.is_settled());

        assert_eq!(pump.retry_failed(), Some(SettingsRevision(3)));
        assert!(!pump.is_settled());
        let retry = pump.take_ready().unwrap();
        pump.finish(failed(retry));
        assert!(pump.has_failed());

        assert!(pump.discard_failed());
        assert!(pump.is_settled());
        assert!(!pump.discard_failed());
    }
}
