#![allow(
    dead_code,
    reason = "pure STG projection contracts precede their GPUI render and edit consumers"
)]

use std::sync::Arc;

use kufeditor_workspace::{
    STGEventTarget, STGParameterTarget, STGReferenceKind, STGScriptKind, STGScriptTarget,
};

use crate::state::{
    STGEventBlockRange, STGEventVisibility, STGIndexVisibility, STGReferenceCursor,
    STGReferenceVisibility,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGIndexRowsData {
    Range { count: usize },
    Filtered(Arc<[usize]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGIndexRows {
    data: STGIndexRowsData,
}

impl STGIndexRows {
    pub const fn range(count: usize) -> Self {
        Self {
            data: STGIndexRowsData::Range { count },
        }
    }

    pub fn filtered(indices: Vec<usize>) -> Self {
        Self {
            data: STGIndexRowsData::Filtered(Arc::from(indices.into_boxed_slice())),
        }
    }

    pub fn len(&self) -> usize {
        match &self.data {
            STGIndexRowsData::Range { count } => *count,
            STGIndexRowsData::Filtered(indices) => indices.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn source_index(&self, position: usize) -> Option<usize> {
        match &self.data {
            STGIndexRowsData::Range { count } => (position < *count).then_some(position),
            STGIndexRowsData::Filtered(indices) => indices.get(position).copied(),
        }
    }

    pub fn position_of(&self, source_index: usize) -> Option<usize> {
        match &self.data {
            STGIndexRowsData::Range { count } => (source_index < *count).then_some(source_index),
            STGIndexRowsData::Filtered(indices) => indices
                .iter()
                .position(|candidate| *candidate == source_index),
        }
    }

    pub fn stored_index_count(&self) -> usize {
        match &self.data {
            STGIndexRowsData::Range { .. } => 0,
            STGIndexRowsData::Filtered(indices) => indices.len(),
        }
    }

    pub fn visibility(&self) -> STGIndexVisibility<'_> {
        match &self.data {
            STGIndexRowsData::Range { count } => STGIndexVisibility::Range(0..*count),
            STGIndexRowsData::Filtered(indices) => STGIndexVisibility::Sparse(indices),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGEventBlockProjection {
    block: usize,
    header: u32,
    event_count: usize,
    flat_start: usize,
}

impl STGEventBlockProjection {
    pub const fn new(block: usize, header: u32, event_count: usize) -> Self {
        Self {
            block,
            header,
            event_count,
            flat_start: 0,
        }
    }

    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn header(self) -> u32 {
        self.header
    }

    pub const fn event_count(self) -> usize {
        self.event_count
    }

    pub const fn flat_start(self) -> usize {
        self.flat_start
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGEventRowsData {
    Blocks(Arc<[STGEventBlockProjection]>),
    Filtered(Arc<[STGEventTarget]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGEventRows {
    data: STGEventRowsData,
    count: usize,
    block_ranges: Arc<[STGEventBlockRange]>,
}

pub struct STGEventTargets<'a> {
    data: &'a STGEventRowsData,
    block: usize,
    event: usize,
    filtered: usize,
}

impl Iterator for STGEventTargets<'_> {
    type Item = STGEventTarget;

    fn next(&mut self) -> Option<Self::Item> {
        match self.data {
            STGEventRowsData::Filtered(targets) => {
                let target = targets.get(self.filtered).copied()?;
                self.filtered += 1;
                Some(target)
            }
            STGEventRowsData::Blocks(blocks) => loop {
                let block = blocks.get(self.block)?;
                if self.event < block.event_count {
                    let target = STGEventTarget {
                        block: block.block,
                        event: self.event,
                    };
                    self.event += 1;
                    return Some(target);
                }
                self.block += 1;
                self.event = 0;
            },
        }
    }
}

impl STGEventRows {
    pub fn from_blocks(mut blocks: Vec<STGEventBlockProjection>) -> Self {
        let mut count = 0_usize;
        for block in &mut blocks {
            block.flat_start = count;
            count = count.saturating_add(block.event_count);
        }
        let block_ranges = blocks
            .iter()
            .map(|block| STGEventBlockRange::new(block.block, block.event_count))
            .collect::<Vec<_>>();
        Self {
            data: STGEventRowsData::Blocks(Arc::from(blocks.into_boxed_slice())),
            count,
            block_ranges: Arc::from(block_ranges.into_boxed_slice()),
        }
    }

    pub fn filtered(targets: Vec<STGEventTarget>) -> Self {
        let count = targets.len();
        Self {
            data: STGEventRowsData::Filtered(Arc::from(targets.into_boxed_slice())),
            count,
            block_ranges: Arc::from(Vec::new().into_boxed_slice()),
        }
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub const fn targets(&self) -> STGEventTargets<'_> {
        STGEventTargets {
            data: &self.data,
            block: 0,
            event: 0,
            filtered: 0,
        }
    }

    pub fn target(&self, position: usize) -> Option<STGEventTarget> {
        if position >= self.count {
            return None;
        }
        match &self.data {
            STGEventRowsData::Filtered(targets) => targets.get(position).copied(),
            STGEventRowsData::Blocks(blocks) => {
                let block = blocks.get(blocks.partition_point(|block| {
                    block.flat_start.saturating_add(block.event_count) <= position
                }))?;
                Some(STGEventTarget {
                    block: block.block,
                    event: position.checked_sub(block.flat_start)?,
                })
            }
        }
    }

    pub fn position_of(&self, target: STGEventTarget) -> Option<usize> {
        match &self.data {
            STGEventRowsData::Filtered(targets) => {
                targets.iter().position(|candidate| *candidate == target)
            }
            STGEventRowsData::Blocks(blocks) => {
                let block = blocks
                    .binary_search_by_key(&target.block, |block| block.block)
                    .ok()
                    .and_then(|position| blocks.get(position))?;
                (target.event < block.event_count)
                    .then(|| block.flat_start.saturating_add(target.event))
            }
        }
    }

    pub fn stored_block_count(&self) -> usize {
        match &self.data {
            STGEventRowsData::Blocks(blocks) => blocks.len(),
            STGEventRowsData::Filtered(_) => 0,
        }
    }

    pub fn stored_target_count(&self) -> usize {
        match &self.data {
            STGEventRowsData::Blocks(_) => 0,
            STGEventRowsData::Filtered(targets) => targets.len(),
        }
    }

    pub fn blocks(&self) -> Option<&[STGEventBlockProjection]> {
        match &self.data {
            STGEventRowsData::Blocks(blocks) => Some(blocks),
            STGEventRowsData::Filtered(_) => None,
        }
    }

    pub fn filtered_targets(&self) -> Option<&[STGEventTarget]> {
        match &self.data {
            STGEventRowsData::Blocks(_) => None,
            STGEventRowsData::Filtered(targets) => Some(targets),
        }
    }

    pub fn visibility(&self) -> STGEventVisibility<'_> {
        match &self.data {
            STGEventRowsData::Blocks(_) => STGEventVisibility::Blocks(&self.block_ranges),
            STGEventRowsData::Filtered(targets) => STGEventVisibility::Filtered(targets),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGReferenceRowsData {
    Indices(STGIndexRows),
    Events(STGEventRows),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGReferenceRows {
    kind: STGReferenceKind,
    data: STGReferenceRowsData,
}

impl STGReferenceRows {
    pub const fn from_rows(kind: STGReferenceKind, rows: STGIndexRows) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Indices(rows),
        }
    }

    pub const fn from_event_rows(kind: STGReferenceKind, rows: STGEventRows) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Events(rows),
        }
    }

    pub const fn range(kind: STGReferenceKind, count: usize) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Indices(STGIndexRows::range(count)),
        }
    }

    pub fn filtered(kind: STGReferenceKind, indices: Vec<usize>) -> Self {
        Self {
            kind,
            data: STGReferenceRowsData::Indices(STGIndexRows::filtered(indices)),
        }
    }

    pub const fn kind(&self) -> STGReferenceKind {
        self.kind
    }

    pub fn len(&self) -> usize {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => rows.len(),
            STGReferenceRowsData::Events(rows) => rows.len(),
        }
    }

    pub fn source_index(&self, position: usize) -> Option<usize> {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => rows.source_index(position),
            STGReferenceRowsData::Events(_) => None,
        }
    }

    pub fn cursor(&self, position: usize) -> Option<STGReferenceCursor> {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => {
                rows.source_index(position).map(STGReferenceCursor::Index)
            }
            STGReferenceRowsData::Events(rows) => {
                rows.target(position).map(STGReferenceCursor::Event)
            }
        }
    }

    pub fn stored_index_count(&self) -> usize {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => rows.stored_index_count(),
            STGReferenceRowsData::Events(_) => 0,
        }
    }

    pub fn visibility(&self) -> STGReferenceVisibility<'_> {
        match &self.data {
            STGReferenceRowsData::Indices(rows) => {
                STGReferenceVisibility::Indices(rows.visibility())
            }
            STGReferenceRowsData::Events(rows) => STGReferenceVisibility::Events(rows.visibility()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGTailProjection {
    Parsed {
        suffix_bytes: usize,
    },
    Raw {
        bytes: usize,
        region: String,
        offset: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGCollectionProjection {
    Units,
    Areas,
    Variables,
    Events,
    Footer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGDocumentProjection {
    units: STGIndexRows,
    areas: Option<STGIndexRows>,
    variables: Option<STGIndexRows>,
    events: Option<STGEventRows>,
    footer: Option<STGIndexRows>,
    tail: STGTailProjection,
}

impl STGDocumentProjection {
    pub fn new(
        units: usize,
        areas: Option<usize>,
        variables: Option<usize>,
        events: Option<Vec<STGEventBlockProjection>>,
        footer: Option<usize>,
        tail: STGTailProjection,
    ) -> Self {
        Self {
            units: STGIndexRows::range(units),
            areas: areas.map(STGIndexRows::range),
            variables: variables.map(STGIndexRows::range),
            events: events.map(STGEventRows::from_blocks),
            footer: footer.map(STGIndexRows::range),
            tail,
        }
    }

    pub const fn units(&self) -> &STGIndexRows {
        &self.units
    }

    pub const fn areas(&self) -> Option<&STGIndexRows> {
        self.areas.as_ref()
    }

    pub const fn variables(&self) -> Option<&STGIndexRows> {
        self.variables.as_ref()
    }

    pub const fn events(&self) -> Option<&STGEventRows> {
        self.events.as_ref()
    }

    pub const fn footer(&self) -> Option<&STGIndexRows> {
        self.footer.as_ref()
    }

    pub const fn tail(&self) -> &STGTailProjection {
        &self.tail
    }

    pub fn section(&self, section: STGCollectionProjection) -> Option<usize> {
        match section {
            STGCollectionProjection::Units => Some(self.units.len()),
            STGCollectionProjection::Areas => self.areas.as_ref().map(STGIndexRows::len),
            STGCollectionProjection::Variables => self.variables.as_ref().map(STGIndexRows::len),
            STGCollectionProjection::Events => self.events.as_ref().map(STGEventRows::len),
            STGCollectionProjection::Footer => self.footer.as_ref().map(STGIndexRows::len),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGSearchRecord<'a> {
    source_index: usize,
    source_text: Option<&'a str>,
    derived_text: Option<&'a str>,
    raw_id: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct STGSearchQuery {
    folded: String,
}

impl STGSearchQuery {
    pub(crate) fn new(query: &str) -> Self {
        Self {
            folded: query.trim().to_lowercase(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.folded.is_empty()
    }

    fn folded(&self) -> &str {
        &self.folded
    }
}

impl<'a> STGSearchRecord<'a> {
    pub const fn new(
        source_index: usize,
        source_text: Option<&'a str>,
        derived_text: Option<&'a str>,
        raw_id: Option<i64>,
    ) -> Self {
        Self {
            source_index,
            source_text,
            derived_text,
            raw_id,
        }
    }

    pub(crate) fn matches(self, query: &STGSearchQuery) -> bool {
        if query.is_empty() {
            return true;
        }
        self.source_text
            .is_some_and(|text| contains_folded(text, query.folded()))
            || self
                .derived_text
                .is_some_and(|text| contains_folded(text, query.folded()))
            || self
                .raw_id
                .is_some_and(|raw_id| raw_id.to_string().contains(query.folded()))
            || self.source_index.to_string().contains(query.folded())
    }

    pub const fn draft_seed(self) -> Option<&'a str> {
        self.source_text
    }

    pub const fn derived_text(self) -> Option<&'a str> {
        self.derived_text
    }
}

fn contains_folded(value: &str, folded_query: &str) -> bool {
    value.to_lowercase().contains(folded_query)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGCatalogTextSuggestion<'a> {
    source_text: &'a str,
    display_text: &'a str,
}

impl<'a> STGCatalogTextSuggestion<'a> {
    pub const fn new(source_text: &'a str, display_text: &'a str) -> Self {
        Self {
            source_text,
            display_text,
        }
    }

    pub const fn source_preview(self) -> &'a str {
        self.source_text
    }

    pub const fn display_text(self) -> &'a str {
        self.display_text
    }

    pub fn apply_to(self, draft: &mut String) {
        draft.clear();
        draft.push_str(self.source_text);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGEventDetailField {
    Description,
    ID,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGEventDetailRow {
    EventField(STGEventDetailField),
    ScriptHeader(STGScriptTarget),
    Parameter(STGParameterTarget),
    AddScript(STGScriptKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct STGScriptRowGroup {
    target: STGScriptTarget,
    start: usize,
    parameter_count: usize,
}

impl STGScriptRowGroup {
    fn row(self, position: usize) -> Option<STGEventDetailRow> {
        if position == self.start {
            return Some(STGEventDetailRow::ScriptHeader(self.target));
        }
        let parameter = position.checked_sub(self.start.checked_add(1)?)?;
        (parameter < self.parameter_count).then_some(STGEventDetailRow::Parameter(
            STGParameterTarget {
                script: self.target,
                parameter,
            },
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGEventDetailRows {
    groups: Arc<[STGScriptRowGroup]>,
    condition_add: usize,
    action_add: usize,
    count: usize,
}

impl STGEventDetailRows {
    pub fn from_parameter_counts(
        event: STGEventTarget,
        condition_parameter_counts: &[usize],
        action_parameter_counts: &[usize],
    ) -> Option<Self> {
        let mut groups = Vec::with_capacity(
            condition_parameter_counts
                .len()
                .checked_add(action_parameter_counts.len())?,
        );
        let mut next = 2_usize;
        append_script_groups(
            &mut groups,
            event,
            STGScriptKind::Condition,
            condition_parameter_counts,
            &mut next,
        )?;
        let condition_add = next;
        next = next.checked_add(1)?;
        append_script_groups(
            &mut groups,
            event,
            STGScriptKind::Action,
            action_parameter_counts,
            &mut next,
        )?;
        let action_add = next;
        let count = next.checked_add(1)?;
        Some(Self {
            groups: Arc::from(groups.into_boxed_slice()),
            condition_add,
            action_add,
            count,
        })
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub fn stored_script_group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn row(&self, position: usize) -> Option<STGEventDetailRow> {
        match position {
            0 => {
                return Some(STGEventDetailRow::EventField(
                    STGEventDetailField::Description,
                ));
            }
            1 => {
                return Some(STGEventDetailRow::EventField(STGEventDetailField::ID));
            }
            _ if position == self.condition_add => {
                return Some(STGEventDetailRow::AddScript(STGScriptKind::Condition));
            }
            _ if position == self.action_add => {
                return Some(STGEventDetailRow::AddScript(STGScriptKind::Action));
            }
            _ if position >= self.count => return None,
            _ => {}
        }
        self.groups
            .partition_point(|group| group.start <= position)
            .checked_sub(1)
            .and_then(|group| self.groups.get(group))
            .and_then(|group| group.row(position))
    }
}

fn append_script_groups(
    groups: &mut Vec<STGScriptRowGroup>,
    event: STGEventTarget,
    kind: STGScriptKind,
    parameter_counts: &[usize],
    next: &mut usize,
) -> Option<()> {
    for (script, parameter_count) in parameter_counts.iter().copied().enumerate() {
        groups.push(STGScriptRowGroup {
            target: STGScriptTarget {
                block: event.block,
                event: event.event,
                kind,
                script,
            },
            start: *next,
            parameter_count,
        });
        *next = next.checked_add(parameter_count.checked_add(1)?)?;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use kufeditor_workspace::{
        STGEventTarget, STGParameterTarget, STGReferenceKind, STGScriptKind, STGScriptTarget,
    };

    use super::{
        STGCatalogTextSuggestion, STGCollectionProjection, STGDocumentProjection,
        STGEventBlockProjection, STGEventDetailField, STGEventDetailRow, STGEventDetailRows,
        STGEventRows, STGIndexRows, STGReferenceRows, STGSearchQuery, STGSearchRecord,
        STGTailProjection,
    };
    use crate::state::STGReferenceCursor;

    fn event(block: usize, event: usize) -> STGEventTarget {
        STGEventTarget { block, event }
    }

    fn script(block: usize, event: usize, kind: STGScriptKind, script: usize) -> STGScriptTarget {
        STGScriptTarget {
            block,
            event,
            kind,
            script,
        }
    }

    #[test]
    fn stg_projection_searches_source_display_raw_id_and_source_index() {
        let record = STGSearchRecord::new(
            17,
            Some("PaladinInternal"),
            Some("Holy Guard 수호자"),
            Some(4_001),
        );

        assert!(record.matches(&STGSearchQuery::new("paladininternal")));
        assert!(record.matches(&STGSearchQuery::new("  HOLY GUARD  ")));
        assert!(record.matches(&STGSearchQuery::new("수호자")));
        assert!(record.matches(&STGSearchQuery::new("4001")));
        assert!(record.matches(&STGSearchQuery::new("17")));
        assert!(!record.matches(&STGSearchQuery::new("archer")));
        assert!(record.matches(&STGSearchQuery::new("")));
        assert_eq!(record.draft_seed(), Some("PaladinInternal"));
        assert_eq!(record.derived_text(), Some("Holy Guard 수호자"));
    }

    #[test]
    fn stg_projection_derived_match_never_replaces_source_draft_implicitly() {
        let record =
            STGSearchRecord::new(3, Some("AreaInternal"), Some("Forest Crossing"), Some(22));
        assert!(record.matches(&STGSearchQuery::new("forest")));

        let mut draft = record.draft_seed().unwrap().to_owned();
        assert_eq!(draft, "AreaInternal");
        let suggestion = STGCatalogTextSuggestion::new("CatalogAreaSource", "Forest Crossing");
        assert_eq!(suggestion.source_preview(), "CatalogAreaSource");
        assert_eq!(suggestion.display_text(), "Forest Crossing");
        assert_eq!(draft, "AreaInternal");

        suggestion.apply_to(&mut draft);
        assert_eq!(draft, "CatalogAreaSource");
    }

    #[test]
    fn stg_projection_large_collections_store_ranges_and_block_metadata() {
        let units = STGIndexRows::range(1_000_000);
        assert_eq!(units.len(), 1_000_000);
        assert_eq!(units.source_index(999_999), Some(999_999));
        assert_eq!(units.stored_index_count(), 0);

        let filtered = STGIndexRows::filtered(vec![7, 70, 700]);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered.source_index(1), Some(70));
        assert_eq!(filtered.stored_index_count(), 3);

        let events = STGEventRows::from_blocks(vec![
            STGEventBlockProjection::new(0, 9, 750_000),
            STGEventBlockProjection::new(1, 10, 250_000),
        ]);
        assert_eq!(events.len(), 1_000_000);
        assert_eq!(events.target(749_999), Some(event(0, 749_999)));
        assert_eq!(events.target(750_000), Some(event(1, 0)));
        assert_eq!(events.target(999_999), Some(event(1, 249_999)));
        assert_eq!(events.stored_block_count(), 2);
        assert_eq!(events.stored_target_count(), 0);

        let references = STGReferenceRows::range(STGReferenceKind::Troop, 2_000_000);
        assert_eq!(references.len(), 2_000_000);
        assert_eq!(references.source_index(1_999_999), Some(1_999_999));
        assert_eq!(references.stored_index_count(), 0);
    }

    #[test]
    fn stg_reference_rows_preserve_event_targets_across_blocks_and_filters() {
        let unfiltered = STGReferenceRows::from_event_rows(
            STGReferenceKind::Trigger,
            STGEventRows::from_blocks(vec![
                STGEventBlockProjection::new(1, 8, 2),
                STGEventBlockProjection::new(4, 9, 1),
            ]),
        );

        assert_eq!(
            unfiltered.cursor(0),
            Some(STGReferenceCursor::Event(event(1, 0)))
        );
        assert_eq!(
            unfiltered.cursor(2),
            Some(STGReferenceCursor::Event(event(4, 0)))
        );

        let filtered = STGReferenceRows::from_event_rows(
            STGReferenceKind::Event,
            STGEventRows::filtered(vec![event(4, 0), event(1, 1)]),
        );

        assert_eq!(
            filtered.cursor(0),
            Some(STGReferenceCursor::Event(event(4, 0)))
        );
        assert_eq!(
            filtered.cursor(1),
            Some(STGReferenceCursor::Event(event(1, 1)))
        );
        assert_eq!(filtered.cursor(2), None);
    }

    #[test]
    fn stg_projection_iterates_event_targets_once_in_flat_source_order() {
        let events = STGEventRows::from_blocks(vec![
            STGEventBlockProjection::new(0, 7, 0),
            STGEventBlockProjection::new(1, 8, 2),
            STGEventBlockProjection::new(2, 9, 1),
        ]);

        assert_eq!(
            events.targets().collect::<Vec<_>>(),
            vec![event(1, 0), event(1, 1), event(2, 0)]
        );
    }

    #[test]
    fn stg_projection_indexes_late_rows_across_many_event_blocks() {
        let blocks = (0..100_000)
            .map(|block| STGEventBlockProjection::new(block, u32::try_from(block).unwrap(), 1))
            .collect();
        let events = STGEventRows::from_blocks(blocks);
        let last = event(99_999, 0);

        assert_eq!(
            events.blocks().unwrap().last().unwrap().flat_start(),
            99_999
        );
        assert_eq!(events.target(99_999), Some(last));
        assert_eq!(events.position_of(last), Some(99_999));
    }

    #[test]
    fn stg_projection_document_sections_hold_counts_instead_of_rendered_rows() {
        let projection = STGDocumentProjection::new(
            1_000_000,
            Some(900_000),
            Some(800_000),
            Some(vec![STGEventBlockProjection::new(4, 12, 700_000)]),
            Some(600_000),
            STGTailProjection::Parsed { suffix_bytes: 31 },
        );

        assert_eq!(projection.units().len(), 1_000_000);
        assert_eq!(projection.units().stored_index_count(), 0);
        assert_eq!(projection.areas().unwrap().len(), 900_000);
        assert_eq!(projection.variables().unwrap().len(), 800_000);
        assert_eq!(projection.events().unwrap().len(), 700_000);
        assert_eq!(projection.events().unwrap().stored_block_count(), 1);
        assert_eq!(projection.events().unwrap().stored_target_count(), 0);
        assert_eq!(projection.footer().unwrap().len(), 600_000);
        assert_eq!(
            projection.tail(),
            &STGTailProjection::Parsed { suffix_bytes: 31 }
        );
        assert_eq!(
            projection.section(STGCollectionProjection::Units),
            Some(1_000_000)
        );
        assert_eq!(
            projection.section(STGCollectionProjection::Events),
            Some(700_000)
        );
    }

    #[test]
    fn stg_projection_flattens_event_details_without_storing_parameter_rows() {
        let target = event(2, 4);
        let conditions = [1_000_000, 2];
        let actions = [3];
        let rows =
            STGEventDetailRows::from_parameter_counts(target, &conditions, &actions).unwrap();

        assert_eq!(rows.len(), 1_000_012);
        assert_eq!(rows.stored_script_group_count(), 3);
        assert_eq!(
            rows.row(0),
            Some(STGEventDetailRow::EventField(
                STGEventDetailField::Description
            ))
        );
        assert_eq!(
            rows.row(1),
            Some(STGEventDetailRow::EventField(STGEventDetailField::ID))
        );
        assert_eq!(
            rows.row(2),
            Some(STGEventDetailRow::ScriptHeader(script(
                2,
                4,
                STGScriptKind::Condition,
                0
            )))
        );
        assert_eq!(
            rows.row(3),
            Some(STGEventDetailRow::Parameter(STGParameterTarget {
                script: script(2, 4, STGScriptKind::Condition, 0),
                parameter: 0,
            }))
        );
        assert_eq!(
            rows.row(1_000_003),
            Some(STGEventDetailRow::ScriptHeader(script(
                2,
                4,
                STGScriptKind::Condition,
                1
            )))
        );
        assert_eq!(
            rows.row(1_000_006),
            Some(STGEventDetailRow::AddScript(STGScriptKind::Condition))
        );
        assert_eq!(
            rows.row(1_000_007),
            Some(STGEventDetailRow::ScriptHeader(script(
                2,
                4,
                STGScriptKind::Action,
                0
            )))
        );
        assert_eq!(
            rows.row(1_000_011),
            Some(STGEventDetailRow::AddScript(STGScriptKind::Action))
        );
        assert_eq!(rows.row(rows.len()), None);
    }

    #[test]
    fn stg_projection_checked_event_detail_length_rejects_overflow() {
        let target = event(0, 0);
        assert!(STGEventDetailRows::from_parameter_counts(target, &[usize::MAX], &[]).is_none());
    }
}
