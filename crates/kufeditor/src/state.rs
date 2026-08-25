use kufeditor_game::Game;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeLevel {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Notice {
    level: NoticeLevel,
    summary: String,
    detail: String,
}

impl Notice {
    pub fn info(summary: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Info,
            summary: summary.into(),
            detail: String::new(),
        }
    }

    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            level: NoticeLevel::Success,
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
            summary: summary.into(),
            detail,
        }
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestId(u64);

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
