use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use kufeditor_game::NameDictionary;

use crate::{catalog_status::CatalogRequestError, state::SaveCatalogRequestID};

#[derive(Debug)]
pub(crate) enum SaveCatalogStatus {
    NotConfigured,
    Dormant,
    Loading {
        key: SaveCatalogKey,
    },
    Ready {
        key: SaveCatalogKey,
        dictionary: Arc<NameDictionary>,
        issue_count: usize,
    },
    Failed {
        key: SaveCatalogKey,
        error: CatalogRequestError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SaveCatalogKey {
    request: SaveCatalogRequestID,
    root: PathBuf,
}

pub(crate) struct SaveCatalogSession {
    status: SaveCatalogStatus,
    retained: Option<SaveCatalogStatus>,
    root: Option<PathBuf>,
}

impl SaveCatalogKey {
    pub(crate) fn new(request: SaveCatalogRequestID, root: impl Into<PathBuf>) -> Self {
        Self {
            request,
            root: root.into(),
        }
    }

    pub(crate) const fn request(&self) -> SaveCatalogRequestID {
        self.request
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

impl SaveCatalogSession {
    pub(crate) fn begin(&mut self, key: SaveCatalogKey) {
        self.root = Some(key.root.clone());
        self.retained = None;
        self.status = SaveCatalogStatus::Loading { key };
    }

    pub(crate) fn activate(&mut self, root: &Path) -> bool {
        if self.root.as_deref() != Some(root) {
            return false;
        }

        match &self.status {
            SaveCatalogStatus::Loading { key }
            | SaveCatalogStatus::Ready { key, .. }
            | SaveCatalogStatus::Failed { key, .. } => key.root() == root,
            SaveCatalogStatus::Dormant => {
                if !self
                    .retained
                    .as_ref()
                    .is_some_and(|status| status_has_root(status, root))
                {
                    return false;
                }
                if let Some(status) = self.retained.take() {
                    self.status = status;
                }
                true
            }
            SaveCatalogStatus::NotConfigured => false,
        }
    }

    pub(crate) fn dormant(&mut self, root: Option<&Path>) -> bool {
        let root_changed = self.root.as_deref() != root;
        if root_changed {
            self.root = root.map(ToOwned::to_owned);
            self.retained = None;
            self.status = SaveCatalogStatus::Dormant;
            return true;
        }

        let status = std::mem::replace(&mut self.status, SaveCatalogStatus::Dormant);
        match status {
            SaveCatalogStatus::Loading { .. }
            | SaveCatalogStatus::Ready { .. }
            | SaveCatalogStatus::Failed { .. } => self.retained = Some(status),
            SaveCatalogStatus::NotConfigured | SaveCatalogStatus::Dormant => {}
        }
        false
    }

    pub(crate) fn not_configured(&mut self) {
        self.root = None;
        self.retained = None;
        self.status = SaveCatalogStatus::NotConfigured;
    }

    pub(crate) fn finish_ready(
        &mut self,
        key: SaveCatalogKey,
        dictionary: Arc<NameDictionary>,
        issue_count: usize,
    ) -> bool {
        if self.root.as_deref() != Some(key.root()) {
            return false;
        }
        if matches!(
            &self.status,
            SaveCatalogStatus::Loading { key: current } if current == &key
        ) {
            self.status = SaveCatalogStatus::Ready {
                key,
                dictionary,
                issue_count,
            };
            return true;
        }
        if matches!(
            self.retained.as_ref(),
            Some(SaveCatalogStatus::Loading { key: current }) if current == &key
        ) {
            self.retained = Some(SaveCatalogStatus::Ready {
                key,
                dictionary,
                issue_count,
            });
            return true;
        }
        false
    }

    pub(crate) fn finish_failed(
        &mut self,
        key: SaveCatalogKey,
        error: CatalogRequestError,
    ) -> bool {
        if self.root.as_deref() != Some(key.root()) {
            return false;
        }
        if matches!(
            &self.status,
            SaveCatalogStatus::Loading { key: current } if current == &key
        ) {
            self.status = SaveCatalogStatus::Failed { key, error };
            return true;
        }
        if matches!(
            self.retained.as_ref(),
            Some(SaveCatalogStatus::Loading { key: current }) if current == &key
        ) {
            self.retained = Some(SaveCatalogStatus::Failed { key, error });
            return true;
        }
        false
    }

    pub(crate) const fn status(&self) -> &SaveCatalogStatus {
        &self.status
    }
}

impl Default for SaveCatalogSession {
    fn default() -> Self {
        Self {
            status: SaveCatalogStatus::Dormant,
            retained: None,
            root: None,
        }
    }
}

fn status_has_root(status: &SaveCatalogStatus, root: &Path) -> bool {
    match status {
        SaveCatalogStatus::Loading { key }
        | SaveCatalogStatus::Ready { key, .. }
        | SaveCatalogStatus::Failed { key, .. } => key.root() == root,
        SaveCatalogStatus::NotConfigured | SaveCatalogStatus::Dormant => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::SaveCatalogKey;
    use crate::state::ShellState;

    #[test]
    fn save_catalog_keys_include_the_request_and_exact_root() {
        let mut shell = ShellState::default();
        let request = shell.begin_save_catalog();
        let key = SaveCatalogKey::new(request, "/games/crusaders");

        assert_eq!(key.request(), request);
        assert_eq!(key.root(), Path::new("/games/crusaders"));
        assert_ne!(
            key,
            SaveCatalogKey::new(request, "/games/crusaders/../crusaders")
        );
    }
}
