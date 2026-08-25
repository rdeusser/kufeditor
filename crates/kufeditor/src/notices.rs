use std::{collections::HashMap, error::Error};

use kufeditor_game::Game;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NoticeLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NoticeScope {
    Workspace,
    Editor,
}

#[derive(Clone, Debug)]
pub(crate) struct Notice {
    level: NoticeLevel,
    scope: NoticeScope,
    summary: String,
    detail: String,
}

impl Notice {
    pub(crate) fn plain(level: NoticeLevel, summary: impl Into<String>) -> Self {
        Self {
            level,
            scope: NoticeScope::Workspace,
            summary: summary.into(),
            detail: String::new(),
        }
    }

    pub(crate) fn info(summary: impl Into<String>) -> Self {
        Self::plain(NoticeLevel::Info, summary)
    }

    pub(crate) fn success(summary: impl Into<String>) -> Self {
        Self::plain(NoticeLevel::Success, summary)
    }

    pub(crate) fn error(summary: impl Into<String>, error: &dyn Error) -> Self {
        Self {
            level: NoticeLevel::Error,
            scope: NoticeScope::Workspace,
            summary: summary.into(),
            detail: format_error(error),
        }
    }

    #[allow(
        dead_code,
        reason = "batch file-open integration consumes this staged notice API"
    )]
    pub(crate) fn error_lines<I, S, E>(summary: impl Into<String>, lines: I) -> Self
    where
        I: IntoIterator<Item = (S, E)>,
        S: AsRef<str>,
        E: Error,
    {
        let detail = lines
            .into_iter()
            .map(|(prefix, error)| format!("{}: {}", prefix.as_ref(), format_error(&error)))
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            level: NoticeLevel::Error,
            scope: NoticeScope::Workspace,
            summary: summary.into(),
            detail,
        }
    }

    pub(crate) fn editor_info(summary: impl Into<String>) -> Self {
        let mut notice = Self::info(summary);
        notice.scope = NoticeScope::Editor;
        notice
    }

    pub(crate) fn editor_error(summary: impl Into<String>, error: &dyn Error) -> Self {
        let mut notice = Self::error(summary, error);
        notice.scope = NoticeScope::Editor;
        notice
    }

    pub(crate) const fn level(&self) -> NoticeLevel {
        self.level
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }

    #[allow(
        dead_code,
        reason = "existing editor-feedback tests retain the moved notice contract"
    )]
    pub(crate) const fn is_editor_feedback(&self) -> bool {
        matches!(self.scope, NoticeScope::Editor)
    }
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[allow(
    dead_code,
    reason = "later settings stages consume the complete source set"
)]
pub(crate) enum NoticeSource {
    Workspace,
    Editor,
    Startup,
    SettingsWrite,
    Open,
    Browse(Game),
    Discovery,
    Catalog,
}

struct NoticeSlot {
    identity: NoticeIdentity,
    sequence: u64,
    notice: Option<Notice>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NoticeIdentity {
    Local(u64),
    External(u64),
}

#[derive(Default)]
pub(crate) struct NoticeCenter {
    slots: HashMap<NoticeSource, NoticeSlot>,
    next_identity: u64,
    next_sequence: u64,
}

impl NoticeCenter {
    pub(crate) fn replace(&mut self, source: NoticeSource, notice: Notice) {
        let identity = self.next_identity;
        self.next_identity += 1;
        self.insert(source, NoticeIdentity::Local(identity), Some(notice));
    }

    pub(crate) fn begin(&mut self, source: NoticeSource, identity: u64, notice: Notice) {
        self.insert(source, NoticeIdentity::External(identity), Some(notice));
    }

    pub(crate) fn begin_pending(&mut self, source: NoticeSource, identity: u64) {
        if let Some(slot) = self.slots.get_mut(&source) {
            slot.identity = NoticeIdentity::External(identity);
        } else {
            self.insert(source, NoticeIdentity::External(identity), None);
        }
    }

    pub(crate) fn cancel(&mut self, source: NoticeSource, identity: u64) -> bool {
        let identity = NoticeIdentity::External(identity);
        if self.slots.get(&source).map(|slot| slot.identity) != Some(identity) {
            return false;
        }
        if self
            .slots
            .get(&source)
            .is_some_and(|slot| slot.notice.is_some())
        {
            let local_identity = self.next_identity;
            self.next_identity += 1;
            if let Some(slot) = self.slots.get_mut(&source) {
                slot.identity = NoticeIdentity::Local(local_identity);
            }
        } else {
            self.slots.remove(&source);
        }
        true
    }

    fn insert(&mut self, source: NoticeSource, identity: NoticeIdentity, notice: Option<Notice>) {
        let sequence = self.allocate_sequence();
        self.slots.insert(
            source,
            NoticeSlot {
                identity,
                sequence,
                notice,
            },
        );
    }

    pub(crate) fn complete(
        &mut self,
        source: NoticeSource,
        identity: u64,
        notice: Option<Notice>,
    ) -> bool {
        let identity = NoticeIdentity::External(identity);
        if self.slots.get(&source).map(|slot| slot.identity) != Some(identity) {
            return false;
        }
        match notice {
            Some(notice) => self.insert(source, identity, Some(notice)),
            None => {
                self.slots.remove(&source);
            }
        }
        true
    }

    pub(crate) fn clear(&mut self, source: NoticeSource) {
        self.slots.remove(&source);
    }

    pub(crate) fn current(&self) -> Option<&Notice> {
        self.slots
            .values()
            .filter_map(|slot| {
                slot.notice
                    .as_ref()
                    .map(|notice| ((notice_priority(notice.level), slot.sequence), notice))
            })
            .max_by_key(|(priority, _)| *priority)
            .map(|(_, notice)| notice)
    }

    fn allocate_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }
}

const fn notice_priority(level: NoticeLevel) -> u8 {
    match level {
        NoticeLevel::Info => 0,
        NoticeLevel::Success => 1,
        NoticeLevel::Warning => 2,
        NoticeLevel::Error => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fmt};

    use kufeditor_game::Game;

    use super::{Notice, NoticeCenter, NoticeLevel, NoticeSource};

    #[test]
    fn only_the_current_identity_can_complete_a_source_slot() {
        let mut center = NoticeCenter::default();
        center.begin(
            NoticeSource::SettingsWrite,
            1,
            Notice::plain(NoticeLevel::Error, "first"),
        );
        center.begin(NoticeSource::SettingsWrite, 2, Notice::info("second"));

        assert!(!center.complete(NoticeSource::SettingsWrite, 1, None));
        assert_eq!(center.current().map(Notice::summary), Some("second"));
        assert!(center.complete(NoticeSource::SettingsWrite, 2, None));
        assert!(center.current().is_none());
    }

    #[test]
    fn a_stale_external_completion_cannot_clear_a_local_replacement() {
        let mut center = NoticeCenter::default();
        center.begin(NoticeSource::Workspace, 0, Notice::info("external request"));
        center.replace(
            NoticeSource::Workspace,
            Notice::plain(NoticeLevel::Error, "local replacement"),
        );

        assert!(!center.complete(NoticeSource::Workspace, 0, None));
        assert_eq!(
            center.current().map(Notice::summary),
            Some("local replacement")
        );
    }

    #[test]
    fn current_notice_uses_level_priority_then_newest_sequence() {
        let mut center = NoticeCenter::default();
        center.replace(NoticeSource::Workspace, Notice::info("info"));
        center.replace(NoticeSource::Editor, Notice::success("success"));
        center.replace(
            NoticeSource::Startup,
            Notice::plain(NoticeLevel::Warning, "warning"),
        );
        center.replace(
            NoticeSource::SettingsWrite,
            Notice::plain(NoticeLevel::Error, "older error"),
        );
        center.replace(
            NoticeSource::Open,
            Notice::plain(NoticeLevel::Error, "newer error"),
        );

        assert_eq!(center.current().map(Notice::summary), Some("newer error"));
        center.clear(NoticeSource::Open);
        assert_eq!(center.current().map(Notice::summary), Some("older error"));
        center.clear(NoticeSource::SettingsWrite);
        assert_eq!(center.current().map(Notice::summary), Some("warning"));
        center.clear(NoticeSource::Startup);
        assert_eq!(center.current().map(Notice::summary), Some("success"));
        center.clear(NoticeSource::Editor);
        assert_eq!(center.current().map(Notice::summary), Some("info"));
    }

    #[test]
    fn browse_slots_are_independent_per_game() {
        let mut center = NoticeCenter::default();
        center.begin(
            NoticeSource::Browse(Game::Crusaders),
            7,
            Notice::info("crusaders"),
        );
        center.begin(
            NoticeSource::Browse(Game::Heroes),
            7,
            Notice::plain(NoticeLevel::Warning, "heroes"),
        );

        assert!(center.complete(
            NoticeSource::Browse(Game::Crusaders),
            7,
            Some(Notice::success("crusaders complete")),
        ));
        assert_eq!(center.current().map(Notice::summary), Some("heroes"));
        center.clear(NoticeSource::Browse(Game::Heroes));
        assert_eq!(
            center.current().map(Notice::summary),
            Some("crusaders complete")
        );
    }

    #[derive(Debug)]
    struct OuterError {
        source: std::io::Error,
    }

    impl fmt::Display for OuterError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("outer failure")
        }
    }

    impl Error for OuterError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.source)
        }
    }

    #[test]
    fn error_notices_keep_complete_source_chains_for_one_or_many_failures() {
        let first = OuterError {
            source: std::io::Error::other("first cause"),
        };
        let second = OuterError {
            source: std::io::Error::other("second cause"),
        };

        let single = Notice::error("single", &first);
        assert_eq!(single.detail(), "outer failure\nCaused by: first cause");

        let batch = Notice::error_lines("batch", [("one", first), ("two", second)]);
        assert_eq!(
            batch.detail(),
            "one: outer failure\nCaused by: first cause\ntwo: outer failure\nCaused by: second cause"
        );
    }
}
