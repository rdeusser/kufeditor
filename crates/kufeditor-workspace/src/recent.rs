use std::path::{Path, PathBuf};

pub const RECENT_FILE_LIMITS: [usize; 4] = [5, 10, 15, 20];
pub const DEFAULT_RECENT_FILE_LIMIT: usize = 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentFiles {
    paths: Vec<PathBuf>,
    limit: usize,
}

pub const fn normalize_recent_limit(value: usize) -> usize {
    if value <= 7 {
        RECENT_FILE_LIMITS[0]
    } else if value <= 12 {
        RECENT_FILE_LIMITS[1]
    } else if value <= 17 {
        RECENT_FILE_LIMITS[2]
    } else {
        RECENT_FILE_LIMITS[3]
    }
}

impl RecentFiles {
    pub fn new(limit: usize) -> Self {
        Self {
            paths: Vec::new(),
            limit: normalize_recent_limit(limit),
        }
    }

    pub fn from_persisted(limit: usize, paths: Vec<PathBuf>) -> Self {
        let mut recent = Self::new(limit);
        recent.add_batch(paths);
        recent
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub const fn limit(&self) -> usize {
        self.limit
    }

    pub fn add(&mut self, path: PathBuf) -> bool {
        let previous = self.paths.clone();
        if let Some(index) = self.paths.iter().position(|existing| existing == &path) {
            self.paths.remove(index);
        }
        self.paths.insert(0, path);
        self.paths.truncate(self.limit);
        self.paths != previous
    }

    pub fn add_batch(&mut self, paths: Vec<PathBuf>) -> bool {
        let previous = self.paths.clone();
        let mut unique = Vec::with_capacity(paths.len());
        for path in paths {
            if !unique.contains(&path) {
                unique.push(path);
            }
        }

        for path in unique.into_iter().rev() {
            self.add(path);
        }

        self.paths != previous
    }

    pub fn set_limit(&mut self, limit: usize) -> bool {
        let limit = normalize_recent_limit(limit);
        let changed = self.limit != limit || self.paths.len() > limit;
        self.limit = limit;
        self.paths.truncate(limit);
        changed
    }

    pub fn remove(&mut self, path: &Path) -> bool {
        let Some(index) = self.paths.iter().position(|existing| existing == path) else {
            return false;
        };
        self.paths.remove(index);
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        self.paths.clear();
        true
    }
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new(DEFAULT_RECENT_FILE_LIMIT)
    }
}
