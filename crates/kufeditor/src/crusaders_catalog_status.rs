use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use kufeditor_game::NameDictionary;

use crate::{catalog_status::CatalogRequestError, state::CrusadersCatalogRequestID};

#[derive(Debug)]
pub(crate) enum CrusadersCatalogStatus {
    NotConfigured,
    Dormant,
    Loading {
        key: CrusadersCatalogKey,
    },
    Ready {
        key: CrusadersCatalogKey,
        dictionary: Arc<NameDictionary>,
        issue_count: usize,
    },
    Failed {
        key: CrusadersCatalogKey,
        error: CatalogRequestError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CrusadersCatalogKey {
    request: CrusadersCatalogRequestID,
    root: PathBuf,
}

pub(crate) struct CrusadersCatalogSession {
    status: CrusadersCatalogStatus,
    retained: Option<CrusadersCatalogStatus>,
    root: Option<PathBuf>,
}

impl CrusadersCatalogKey {
    pub(crate) fn new(request: CrusadersCatalogRequestID, root: impl Into<PathBuf>) -> Self {
        Self {
            request,
            root: root.into(),
        }
    }

    pub(crate) const fn request(&self) -> CrusadersCatalogRequestID {
        self.request
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

impl CrusadersCatalogSession {
    pub(crate) fn begin(&mut self, key: CrusadersCatalogKey) {
        self.root = Some(key.root.clone());
        self.retained = None;
        self.status = CrusadersCatalogStatus::Loading { key };
    }

    pub(crate) fn activate(&mut self, root: &Path) -> bool {
        if self.root.as_deref() != Some(root) {
            return false;
        }

        match &self.status {
            CrusadersCatalogStatus::Loading { key }
            | CrusadersCatalogStatus::Ready { key, .. }
            | CrusadersCatalogStatus::Failed { key, .. } => key.root() == root,
            CrusadersCatalogStatus::Dormant => {
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
            CrusadersCatalogStatus::NotConfigured => false,
        }
    }

    pub(crate) fn dormant(&mut self, root: Option<&Path>) -> bool {
        let root_changed = self.root.as_deref() != root;
        if root_changed {
            self.root = root.map(ToOwned::to_owned);
            self.retained = None;
            self.status = CrusadersCatalogStatus::Dormant;
            return true;
        }

        let status = std::mem::replace(&mut self.status, CrusadersCatalogStatus::Dormant);
        match status {
            CrusadersCatalogStatus::Loading { .. }
            | CrusadersCatalogStatus::Ready { .. }
            | CrusadersCatalogStatus::Failed { .. } => self.retained = Some(status),
            CrusadersCatalogStatus::NotConfigured | CrusadersCatalogStatus::Dormant => {}
        }
        false
    }

    pub(crate) fn not_configured(&mut self) {
        self.root = None;
        self.retained = None;
        self.status = CrusadersCatalogStatus::NotConfigured;
    }

    pub(crate) fn finish_ready(
        &mut self,
        key: CrusadersCatalogKey,
        dictionary: Arc<NameDictionary>,
        issue_count: usize,
    ) -> bool {
        if self.root.as_deref() != Some(key.root()) {
            return false;
        }
        if matches!(
            &self.status,
            CrusadersCatalogStatus::Loading { key: current } if current == &key
        ) {
            self.status = CrusadersCatalogStatus::Ready {
                key,
                dictionary,
                issue_count,
            };
            return true;
        }
        if matches!(
            self.retained.as_ref(),
            Some(CrusadersCatalogStatus::Loading { key: current }) if current == &key
        ) {
            self.retained = Some(CrusadersCatalogStatus::Ready {
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
        key: CrusadersCatalogKey,
        error: CatalogRequestError,
    ) -> bool {
        if self.root.as_deref() != Some(key.root()) {
            return false;
        }
        if matches!(
            &self.status,
            CrusadersCatalogStatus::Loading { key: current } if current == &key
        ) {
            self.status = CrusadersCatalogStatus::Failed { key, error };
            return true;
        }
        if matches!(
            self.retained.as_ref(),
            Some(CrusadersCatalogStatus::Loading { key: current }) if current == &key
        ) {
            self.retained = Some(CrusadersCatalogStatus::Failed { key, error });
            return true;
        }
        false
    }

    pub(crate) const fn status(&self) -> &CrusadersCatalogStatus {
        &self.status
    }
}

impl Default for CrusadersCatalogSession {
    fn default() -> Self {
        Self {
            status: CrusadersCatalogStatus::Dormant,
            retained: None,
            root: None,
        }
    }
}

fn status_has_root(status: &CrusadersCatalogStatus, root: &Path) -> bool {
    match status {
        CrusadersCatalogStatus::Loading { key }
        | CrusadersCatalogStatus::Ready { key, .. }
        | CrusadersCatalogStatus::Failed { key, .. } => key.root() == root,
        CrusadersCatalogStatus::NotConfigured | CrusadersCatalogStatus::Dormant => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::CrusadersCatalogKey;
    use crate::state::ShellState;

    #[test]
    fn crusaders_catalog_keys_include_the_request_and_exact_root() {
        let mut shell = ShellState::default();
        let request = shell.begin_crusaders_catalog();
        let key = CrusadersCatalogKey::new(request, "/games/crusaders");

        assert_eq!(key.request(), request);
        assert_eq!(key.root(), Path::new("/games/crusaders"));
        assert_ne!(
            key,
            CrusadersCatalogKey::new(request, "/games/crusaders/../crusaders")
        );
    }
}
