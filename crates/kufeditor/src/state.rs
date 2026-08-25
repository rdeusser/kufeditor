use std::collections::HashMap;

use kufeditor_game::Game;
use kufeditor_workspace::DocumentId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeLevel {
    Info,
    Success,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NoticeScope {
    Workspace,
    Editor,
}

#[derive(Clone, Debug)]
pub struct Notice {
    level: NoticeLevel,
    scope: NoticeScope,
    summary: String,
    detail: String,
}

impl Notice {
    pub fn info(summary: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Info,
            scope: NoticeScope::Workspace,
            summary: summary.into(),
            detail: String::new(),
        }
    }

    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Success,
            scope: NoticeScope::Workspace,
            summary: summary.into(),
            detail: String::new(),
        }
    }

    pub fn error(summary: impl Into<String>, error: &(dyn std::error::Error + 'static)) -> Self {
        let mut detail = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            detail.push_str("\nCaused by: ");
            detail.push_str(&cause.to_string());
            source = cause.source();
        }
        Self {
            level: NoticeLevel::Error,
            scope: NoticeScope::Workspace,
            summary: summary.into(),
            detail,
        }
    }

    pub fn editor_info(summary: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Info,
            scope: NoticeScope::Editor,
            summary: summary.into(),
            detail: String::new(),
        }
    }

    pub fn editor_error(
        summary: impl Into<String>,
        error: &(dyn std::error::Error + 'static),
    ) -> Self {
        let mut notice = Self::error(summary, error);
        notice.scope = NoticeScope::Editor;
        notice
    }

    pub const fn level(&self) -> NoticeLevel {
        self.level
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub const fn is_editor_feedback(&self) -> bool {
        matches!(self.scope, NoticeScope::Editor)
    }
}

#[derive(Debug, Default)]
pub struct RecordSelections {
    records: HashMap<DocumentId, usize>,
}

impl RecordSelections {
    pub fn selected(&self, document: DocumentId) -> usize {
        self.records.get(&document).copied().unwrap_or(0)
    }

    pub fn select(&mut self, document: DocumentId, record: usize) {
        self.records.insert(document, record);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosePolicy {
    Allow,
    PromptForUnsaved { count: usize },
}

impl ClosePolicy {
    pub const fn from_dirty_count(count: usize) -> Self {
        if count == 0 {
            Self::Allow
        } else {
            Self::PromptForUnsaved { count }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Area {
    #[default]
    Home,
    Files,
    Mods,
    Patches,
}

impl Area {
    pub const ALL: [Self; 4] = [Self::Home, Self::Files, Self::Mods, Self::Patches];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Files => "Files",
            Self::Mods => "Mods",
            Self::Patches => "Patches",
        }
    }

    pub const fn element_id(self) -> &'static str {
        match self {
            Self::Home => "rail-home",
            Self::Files => "rail-files",
            Self::Mods => "rail-mods",
            Self::Patches => "rail-patches",
        }
    }
}

#[derive(Debug, Default)]
pub struct ShellState {
    area: Area,
    game: Game,
    next_request_id: u64,
    active_open_request: Option<RequestId>,
}

impl ShellState {
    pub const fn area(&self) -> Area {
        self.area
    }

    pub const fn game(&self) -> Game {
        self.game
    }

    pub fn select_area(&mut self, area: Area) {
        self.area = area;
    }

    pub fn select_game(&mut self, game: Game) {
        self.game = game;
    }

    pub fn begin_open(&mut self) -> RequestId {
        self.next_request_id += 1;
        let request = RequestId(self.next_request_id);
        self.active_open_request = Some(request);
        request
    }

    pub fn accepts_open(&self, request: RequestId) -> bool {
        self.active_open_request == Some(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, ShellState};
    use kufeditor_game::Game;

    #[test]
    fn shell_starts_at_home_for_crusaders() {
        let state = ShellState::default();
        assert_eq!(state.area(), Area::Home);
        assert_eq!(state.game(), Game::Crusaders);
    }

    #[test]
    fn navigation_and_game_selection_are_independent() {
        let mut state = ShellState::default();
        state.select_area(Area::Files);
        state.select_game(Game::Heroes);
        assert_eq!(state.area(), Area::Files);
        assert_eq!(state.game(), Game::Heroes);
    }

    #[test]
    fn request_ids_make_old_open_results_stale() {
        let mut state = ShellState::default();
        let first = state.begin_open();
        let second = state.begin_open();
        assert!(!state.accepts_open(first));
        assert!(state.accepts_open(second));
    }

    #[test]
    fn an_error_notice_keeps_summary_and_source_chain() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "fixture missing");
        let notice = super::Notice::error("Could not open file", &error);
        assert_eq!(notice.summary(), "Could not open file");
        assert!(notice.detail().contains("fixture missing"));
    }
}

#[cfg(test)]
mod close_tests {
    use super::ClosePolicy;

    #[test]
    fn only_dirty_documents_require_a_prompt() {
        assert_eq!(ClosePolicy::from_dirty_count(0), ClosePolicy::Allow);
        assert_eq!(
            ClosePolicy::from_dirty_count(2),
            ClosePolicy::PromptForUnsaved { count: 2 },
        );
    }
}
