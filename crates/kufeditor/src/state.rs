use std::collections::HashMap;

use kufeditor_game::Game;
use kufeditor_workspace::DocumentId;

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

impl RequestId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

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
    Settings,
}

impl Area {
    pub const PRIMARY: [Self; 4] = [Self::Home, Self::Files, Self::Mods, Self::Patches];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Files => "Files",
            Self::Mods => "Mods",
            Self::Patches => "Patches",
            Self::Settings => "Settings",
        }
    }

    pub const fn element_id(self) -> &'static str {
        match self {
            Self::Home => "rail-home",
            Self::Files => "rail-files",
            Self::Mods => "rail-mods",
            Self::Patches => "rail-patches",
            Self::Settings => "rail-settings",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NavigationProjection {
    pub(crate) primary: &'static [Area],
    pub(crate) bottom: Area,
}

pub(crate) const fn navigation_projection() -> NavigationProjection {
    NavigationProjection {
        primary: &Area::PRIMARY,
        bottom: Area::Settings,
    }
}

#[derive(Debug, Default)]
pub struct ShellState {
    area: Area,
    game: Game,
    next_request_id: u64,
    active_open_request: Option<RequestId>,
    active_catalog_request: Option<RequestId>,
    active_crusaders_browse_request: Option<RequestId>,
    active_heroes_browse_request: Option<RequestId>,
    active_discovery_request: Option<RequestId>,
}

impl ShellState {
    pub fn with_game(game: Game) -> Self {
        Self {
            game,
            ..Self::default()
        }
    }

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

    pub fn begin_catalog(&mut self) -> RequestId {
        self.next_request_id += 1;
        let request = RequestId(self.next_request_id);
        self.active_catalog_request = Some(request);
        request
    }

    pub fn accepts_catalog(&self, request: RequestId) -> bool {
        self.active_catalog_request == Some(request)
    }

    pub fn invalidate_catalog(&mut self) {
        self.active_catalog_request = None;
    }

    pub fn begin_browse(&mut self, game: Game) -> RequestId {
        self.next_request_id += 1;
        let request = RequestId(self.next_request_id);
        match game {
            Game::Crusaders => self.active_crusaders_browse_request = Some(request),
            Game::Heroes => self.active_heroes_browse_request = Some(request),
        }
        request
    }

    pub fn accepts_browse(&self, game: Game, request: RequestId) -> bool {
        (match game {
            Game::Crusaders => self.active_crusaders_browse_request,
            Game::Heroes => self.active_heroes_browse_request,
        }) == Some(request)
    }

    pub fn invalidate_browse(&mut self, game: Game) {
        match game {
            Game::Crusaders => self.active_crusaders_browse_request = None,
            Game::Heroes => self.active_heroes_browse_request = None,
        }
    }

    pub fn begin_discovery(&mut self) -> RequestId {
        self.next_request_id += 1;
        let request = RequestId(self.next_request_id);
        self.active_discovery_request = Some(request);
        request
    }

    pub fn accepts_discovery(&self, request: RequestId) -> bool {
        self.active_discovery_request == Some(request)
    }

    pub fn invalidate_discovery(&mut self) {
        self.active_discovery_request = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, ShellState, navigation_projection};
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
    fn shell_can_start_with_the_persisted_game() {
        let state = ShellState::with_game(Game::Heroes);

        assert_eq!(state.area(), Area::Home);
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
    fn catalog_requests_can_be_superseded_or_invalidated() {
        let mut state = ShellState::default();
        let first = state.begin_catalog();
        let second = state.begin_catalog();

        assert!(!state.accepts_catalog(first));
        assert!(state.accepts_catalog(second));

        state.invalidate_catalog();
        assert!(!state.accepts_catalog(second));
    }

    #[test]
    fn navigation_keeps_settings_below_the_primary_destinations() {
        assert_eq!(
            Area::PRIMARY,
            [Area::Home, Area::Files, Area::Mods, Area::Patches],
        );
        assert_eq!(Area::Settings.label(), "Settings");
        assert_eq!(Area::Settings.element_id(), "rail-settings");

        let projection = navigation_projection();
        assert_eq!(projection.primary, Area::PRIMARY);
        assert_eq!(projection.bottom, Area::Settings);
    }

    #[test]
    fn browse_discovery_catalog_and_open_request_gates_are_independent() {
        let mut state = ShellState::default();
        let crusaders_browse = state.begin_browse(Game::Crusaders);
        let heroes_browse = state.begin_browse(Game::Heroes);
        let discovery = state.begin_discovery();
        let catalog = state.begin_catalog();
        let open = state.begin_open();

        assert!(state.accepts_browse(Game::Crusaders, crusaders_browse));
        assert!(state.accepts_browse(Game::Heroes, heroes_browse));
        assert!(state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        state.invalidate_browse(Game::Crusaders);
        assert!(!state.accepts_browse(Game::Crusaders, crusaders_browse));
        assert!(state.accepts_browse(Game::Heroes, heroes_browse));
        assert!(state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        let next_heroes_browse = state.begin_browse(Game::Heroes);
        assert!(state.accepts_browse(Game::Heroes, next_heroes_browse));
        assert!(state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        state.invalidate_discovery();
        assert!(state.accepts_browse(Game::Heroes, next_heroes_browse));
        assert!(!state.accepts_discovery(discovery));
        assert!(state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));

        state.invalidate_catalog();
        assert!(state.accepts_browse(Game::Heroes, next_heroes_browse));
        assert!(!state.accepts_discovery(discovery));
        assert!(!state.accepts_catalog(catalog));
        assert!(state.accepts_open(open));
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
