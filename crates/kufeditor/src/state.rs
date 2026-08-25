use std::collections::HashMap;

use kufeditor_game::Game;
use kufeditor_workspace::{DocumentID, SaveEquipmentSlot};

#[derive(Debug, Default)]
pub struct RecordSelections {
    records: HashMap<DocumentID, usize>,
}

impl RecordSelections {
    pub fn selected(&self, document: DocumentID) -> usize {
        self.records.get(&document).copied().unwrap_or(0)
    }

    pub fn select(&mut self, document: DocumentID, record: usize) {
        self.records.insert(document, record);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SaveSection {
    #[default]
    Summary,
    Units,
    Equipment,
    Roster,
    Missions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "Task 11 passes save row visibility into presentation reconciliation"
)]
pub enum SaveUnitVisibility<'a> {
    All { unit_count: usize },
    Filtered(&'a [usize]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SaveUnitReconciliation {
    inspected_unit: usize,
    requested_visible: bool,
}

impl SaveUnitVisibility<'_> {
    fn reconcile(self, requested_unit: usize) -> SaveUnitReconciliation {
        match self {
            Self::All { unit_count } => SaveUnitReconciliation {
                inspected_unit: clamp_unit(requested_unit, unit_count),
                requested_visible: requested_unit < unit_count,
            },
            Self::Filtered(indices) => {
                let requested_visible = indices.contains(&requested_unit);
                SaveUnitReconciliation {
                    inspected_unit: if requested_visible {
                        requested_unit
                    } else {
                        indices.first().copied().unwrap_or(0)
                    },
                    requested_visible,
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "Task 11 stores save presentation state in the GPUI frame"
)]
pub struct SavePresentationState {
    section: SaveSection,
    inspected_unit: usize,
    equipment_slot: SaveEquipmentSlot,
    unit_filter: String,
}

#[allow(
    dead_code,
    reason = "Task 11 reads save presentation state while rendering"
)]
impl SavePresentationState {
    pub const fn section(&self) -> SaveSection {
        self.section
    }

    pub const fn inspected_unit(&self) -> usize {
        self.inspected_unit
    }

    pub const fn equipment_slot(&self) -> SaveEquipmentSlot {
        self.equipment_slot
    }

    pub fn unit_filter(&self) -> &str {
        &self.unit_filter
    }
}

impl Default for SavePresentationState {
    fn default() -> Self {
        Self {
            section: SaveSection::Summary,
            inspected_unit: 0,
            equipment_slot: SaveEquipmentSlot::LeaderWeapon,
            unit_filter: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "Task 11 consumes transition outcomes to cancel active editors"
)]
pub enum SavePresentationTransition {
    Unchanged,
    Changed,
    ChangedAndCancelDraft,
}

#[derive(Debug, Default)]
#[allow(
    dead_code,
    reason = "Task 11 stores document-scoped save presentation state in the frame"
)]
pub struct SavePresentationStates {
    documents: HashMap<DocumentID, SavePresentationState>,
    active_document: Option<DocumentID>,
}

#[allow(
    dead_code,
    reason = "Task 11 connects document and control transitions to the frame"
)]
impl SavePresentationStates {
    pub fn for_document(&mut self, document: DocumentID) -> &SavePresentationState {
        self.documents.entry(document).or_default()
    }

    pub fn get(&self, document: DocumentID) -> Option<&SavePresentationState> {
        self.documents.get(&document)
    }

    pub const fn active_document(&self) -> Option<DocumentID> {
        self.active_document
    }

    pub fn activate_document(
        &mut self,
        document: DocumentID,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let reconciliation = self.reconcile_document(document, visibility, draft_active);
        if self.active_document == Some(document) {
            return reconciliation;
        }
        self.active_document = Some(document);
        changed_transition(draft_active)
    }

    pub fn reconcile_document(
        &mut self,
        document: DocumentID,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        let reconciliation = visibility.reconcile(state.inspected_unit);
        let transition = unit_reconciliation_transition(
            state.inspected_unit,
            reconciliation,
            false,
            draft_active,
        );
        state.inspected_unit = reconciliation.inspected_unit;
        transition
    }

    pub fn select_section(
        &mut self,
        document: DocumentID,
        section: SaveSection,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        if state.section == section {
            return SavePresentationTransition::Unchanged;
        }
        state.section = section;
        changed_transition(draft_active)
    }

    pub fn inspect_unit(
        &mut self,
        document: DocumentID,
        unit: usize,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        let reconciliation = visibility.reconcile(unit);
        let transition = unit_reconciliation_transition(
            state.inspected_unit,
            reconciliation,
            false,
            draft_active,
        );
        state.inspected_unit = reconciliation.inspected_unit;
        transition
    }

    pub fn select_equipment_slot(
        &mut self,
        document: DocumentID,
        slot: SaveEquipmentSlot,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        if state.equipment_slot == slot {
            return SavePresentationTransition::Unchanged;
        }
        state.equipment_slot = slot;
        changed_transition(draft_active)
    }

    pub fn set_unit_filter(
        &mut self,
        document: DocumentID,
        filter: String,
        visibility: SaveUnitVisibility<'_>,
        draft_active: bool,
    ) -> SavePresentationTransition {
        let state = self.documents.entry(document).or_default();
        let reconciliation = visibility.reconcile(state.inspected_unit);
        let transition = unit_reconciliation_transition(
            state.inspected_unit,
            reconciliation,
            state.unit_filter != filter,
            draft_active,
        );
        state.unit_filter = filter;
        state.inspected_unit = reconciliation.inspected_unit;
        transition
    }

    pub fn remove_document(&mut self, document: DocumentID) -> bool {
        if self.active_document == Some(document) {
            self.active_document = None;
        }
        self.documents.remove(&document).is_some()
    }
}

#[allow(
    dead_code,
    reason = "Task 11 reaches this helper through save presentation reconciliation"
)]
const fn clamp_unit(unit: usize, unit_count: usize) -> usize {
    if unit_count == 0 {
        0
    } else if unit >= unit_count {
        unit_count - 1
    } else {
        unit
    }
}

#[allow(
    dead_code,
    reason = "Task 11 reaches this helper through save presentation controls"
)]
const fn changed_transition(draft_active: bool) -> SavePresentationTransition {
    if draft_active {
        SavePresentationTransition::ChangedAndCancelDraft
    } else {
        SavePresentationTransition::Changed
    }
}

const fn unit_reconciliation_transition(
    previous_unit: usize,
    reconciliation: SaveUnitReconciliation,
    presentation_changed: bool,
    draft_active: bool,
) -> SavePresentationTransition {
    if presentation_changed || previous_unit != reconciliation.inspected_unit {
        changed_transition(draft_active)
    } else if draft_active && !reconciliation.requested_visible {
        SavePresentationTransition::ChangedAndCancelDraft
    } else {
        SavePresentationTransition::Unchanged
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestID(u64);

impl RequestID {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaveCatalogRequestID(u64);

impl SaveCatalogRequestID {
    #[allow(
        dead_code,
        reason = "save-catalog request identities are exposed to later inline status views"
    )]
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
    next_save_catalog_request_id: u64,
    active_open_request: Option<RequestID>,
    active_catalog_request: Option<RequestID>,
    active_save_catalog_request: Option<SaveCatalogRequestID>,
    active_crusaders_browse_request: Option<RequestID>,
    active_heroes_browse_request: Option<RequestID>,
    active_discovery_request: Option<RequestID>,
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

    pub fn begin_open(&mut self) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        self.active_open_request = Some(request);
        request
    }

    pub fn accepts_open(&self, request: RequestID) -> bool {
        self.active_open_request == Some(request)
    }

    pub fn begin_catalog(&mut self) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        self.active_catalog_request = Some(request);
        request
    }

    pub fn accepts_catalog(&self, request: RequestID) -> bool {
        self.active_catalog_request == Some(request)
    }

    pub fn invalidate_catalog(&mut self) {
        self.active_catalog_request = None;
    }

    pub fn begin_save_catalog(&mut self) -> SaveCatalogRequestID {
        self.next_save_catalog_request_id += 1;
        let request = SaveCatalogRequestID(self.next_save_catalog_request_id);
        self.active_save_catalog_request = Some(request);
        request
    }

    pub fn accepts_save_catalog(&self, request: SaveCatalogRequestID) -> bool {
        self.active_save_catalog_request == Some(request)
    }

    pub fn invalidate_save_catalog(&mut self) {
        self.active_save_catalog_request = None;
    }

    pub fn begin_browse(&mut self, game: Game) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        match game {
            Game::Crusaders => self.active_crusaders_browse_request = Some(request),
            Game::Heroes => self.active_heroes_browse_request = Some(request),
        }
        request
    }

    pub fn accepts_browse(&self, game: Game, request: RequestID) -> bool {
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

    pub fn begin_discovery(&mut self) -> RequestID {
        self.next_request_id += 1;
        let request = RequestID(self.next_request_id);
        self.active_discovery_request = Some(request);
        request
    }

    pub fn accepts_discovery(&self, request: RequestID) -> bool {
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
    fn global_and_save_catalog_requests_use_independent_generations() {
        let mut state = ShellState::default();

        let first_global = state.begin_catalog();
        let first_save = state.begin_save_catalog();
        let second_global = state.begin_catalog();
        let second_save = state.begin_save_catalog();

        assert_eq!(first_global.get(), 1);
        assert_eq!(second_global.get(), 2);
        assert_eq!(first_save.get(), 1);
        assert_eq!(second_save.get(), 2);
        assert!(!state.accepts_catalog(first_global));
        assert!(state.accepts_catalog(second_global));
        assert!(!state.accepts_save_catalog(first_save));
        assert!(state.accepts_save_catalog(second_save));
    }

    #[test]
    fn global_and_save_catalog_invalidations_are_independent() {
        let mut state = ShellState::default();
        let global = state.begin_catalog();
        let save = state.begin_save_catalog();

        state.invalidate_save_catalog();

        assert!(state.accepts_catalog(global));
        assert!(!state.accepts_save_catalog(save));

        let next_save = state.begin_save_catalog();
        state.invalidate_catalog();

        assert!(!state.accepts_catalog(global));
        assert!(state.accepts_save_catalog(next_save));
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

#[cfg(test)]
mod save_presentation_tests {
    use std::path::PathBuf;

    use kufeditor_workspace::{Document, DocumentID, SaveEquipmentSlot, TroopDocument, Workspace};

    use super::{
        SavePresentationStates, SavePresentationTransition, SaveSection, SaveUnitVisibility,
    };

    const fn all_units(unit_count: usize) -> SaveUnitVisibility<'static> {
        SaveUnitVisibility::All { unit_count }
    }

    const fn filtered_units(indices: &[usize]) -> SaveUnitVisibility<'_> {
        SaveUnitVisibility::Filtered(indices)
    }

    fn document_ids() -> (DocumentID, DocumentID) {
        let mut workspace = Workspace::new();
        let first = workspace.open_loaded(
            PathBuf::from("first.sox"),
            Document::Troop(troop_document()),
        );
        let second = workspace.open_loaded(
            PathBuf::from("second.sox"),
            Document::Troop(troop_document()),
        );
        (first, second)
    }

    fn troop_document() -> TroopDocument {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(..8)
            .unwrap()
            .copy_from_slice(&[100, 0, 0, 0, 1, 0, 0, 0]);
        TroopDocument::parse(bytes).unwrap()
    }

    #[test]
    fn save_presentation_starts_at_summary_and_retains_each_document_section() {
        let (first, second) = document_ids();
        let mut states = SavePresentationStates::default();

        assert_eq!(states.for_document(first).section(), SaveSection::Summary);
        assert_eq!(
            states.select_section(first, SaveSection::Units, false),
            SavePresentationTransition::Changed,
        );
        assert_eq!(
            states.select_section(second, SaveSection::Equipment, false),
            SavePresentationTransition::Changed,
        );

        assert_eq!(states.get(first).unwrap().section(), SaveSection::Units);
        assert_eq!(
            states.get(second).unwrap().section(),
            SaveSection::Equipment,
        );
    }

    #[test]
    fn save_presentation_clamps_inspected_unit_after_document_replacement() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, all_units(10), false);
        states.inspect_unit(document, 8, all_units(10), false);

        assert_eq!(
            states.reconcile_document(document, all_units(3), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 2);
    }

    #[test]
    fn save_presentation_activation_preserves_a_reconciliation_cancel_result() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, all_units(10), false);
        states.inspect_unit(document, 8, all_units(10), false);

        assert_eq!(
            states.activate_document(document, all_units(3), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 2);
    }

    #[test]
    fn save_presentation_keeps_a_valid_equipment_slot_after_reconciliation() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.select_equipment_slot(document, SaveEquipmentSlot::TroopArmor, false);

        states.reconcile_document(document, all_units(1), false);

        assert_eq!(
            states.get(document).unwrap().equipment_slot(),
            SaveEquipmentSlot::TroopArmor,
        );
    }

    #[test]
    fn save_presentation_filter_moves_inspection_to_the_first_visible_unit() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.inspect_unit(document, 7, all_units(10), false);

        assert_eq!(
            states.set_unit_filter(document, "gerald".to_owned(), filtered_units(&[2, 4]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.unit_filter(), "gerald");
        assert_eq!(state.inspected_unit(), 2);
    }

    #[test]
    fn save_presentation_changes_explicitly_cancel_active_drafts() {
        let (first, second) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(first, all_units(5), false);

        assert_eq!(
            states.select_section(first, SaveSection::Units, true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.inspect_unit(first, 1, all_units(5), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.set_unit_filter(first, "leader".to_owned(), filtered_units(&[1, 3]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.select_equipment_slot(first, SaveEquipmentSlot::LeaderArmor, true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(
            states.activate_document(second, all_units(2), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
    }

    #[test]
    fn save_presentation_same_count_replacement_reconciles_changed_filter_membership() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_unit_filter(
            document,
            "leader".to_owned(),
            filtered_units(&[1, 3]),
            false,
        );
        states.inspect_unit(document, 3, filtered_units(&[1, 3]), false);

        assert_eq!(
            states.reconcile_document(document, filtered_units(&[1, 2]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        let state = states.get(document).unwrap();
        assert_eq!(state.unit_filter(), "leader");
        assert_eq!(state.inspected_unit(), 1);
    }

    #[test]
    fn save_presentation_empty_filtered_results_reset_inspection() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.inspect_unit(document, 3, all_units(5), false);

        assert_eq!(
            states.set_unit_filter(document, "missing".to_owned(), filtered_units(&[]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_unchanged_filter_cancels_unit_zero_draft_when_visibility_empties() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.set_unit_filter(document, "leader".to_owned(), filtered_units(&[0]), false);

        assert_eq!(
            states.set_unit_filter(document, "leader".to_owned(), filtered_units(&[]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_zero_count_reconciliation_cancels_unit_zero_draft() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.reconcile_document(document, all_units(1), false);

        assert_eq!(
            states.reconcile_document(document, all_units(0), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_inspection_cancels_unit_zero_draft_when_visibility_empties() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.inspect_unit(document, 0, all_units(1), false);

        assert_eq!(
            states.inspect_unit(document, 0, all_units(0), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_active_activation_cancels_unit_zero_draft_when_visibility_empties() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, filtered_units(&[0]), false);

        assert_eq!(
            states.activate_document(document, filtered_units(&[]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 0);
    }

    #[test]
    fn save_presentation_active_document_activation_reconciles_changed_membership() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, filtered_units(&[1, 3]), false);
        states.inspect_unit(document, 3, filtered_units(&[1, 3]), false);

        assert_eq!(
            states.activate_document(document, filtered_units(&[1, 2]), true),
            SavePresentationTransition::ChangedAndCancelDraft,
        );
        assert_eq!(states.get(document).unwrap().inspected_unit(), 1);
    }

    #[test]
    fn save_presentation_removes_document_state_on_close() {
        let (document, _) = document_ids();
        let mut states = SavePresentationStates::default();
        states.activate_document(document, all_units(1), false);

        assert!(states.remove_document(document));
        assert!(states.get(document).is_none());
        assert_eq!(states.active_document(), None);
    }
}
