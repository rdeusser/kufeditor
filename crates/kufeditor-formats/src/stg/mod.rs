pub mod catalog;

mod fields;
mod mutation;
mod preflight;
mod structure;
mod text;
mod wire;

#[cfg(test)]
#[allow(
    dead_code,
    reason = "the shared integration fixture exposes offsets used by multiple STG test modules"
)]
#[path = "../../tests/support/stg.rs"]
mod stg_test_support;

use std::{mem::size_of, ops::Range, sync::Arc};

use crate::{
    error::{
        FormatError, STGCleaveError, STGParseError, STGPreflightError, STGRegion, STGTailFailure,
    },
    generated::kuf_stg::{
        self, AreaEntry, EventBlock, FooterEntry, StgAction, StgCondition, StgHeader,
        StgParamValue, StgParamValueValue, StgVariable, UnitBlock,
    },
};
use preflight::{MODEL_LIMIT, SOURCE_LIMIT, STGAllocationBudget, STGTailPlan};

pub use fields::{
    STGAbilityOwner, STGAreaField, STGAreaFloatField, STGChoice, STGEditor, STGEvent,
    STGEventBlock, STGEventTarget, STGFieldAccess, STGFloatTarget, STGFloatValue, STGFooterField,
    STGHeaderTextField, STGMutation, STGNumberTarget, STGParameter, STGParameterTarget,
    STGReferenceKind, STGScript, STGScriptKind, STGScriptLabel, STGScriptTarget, STGSkillField,
    STGSkillOwner, STGTextTarget, STGUnitField, STGUnitFloatField, STGUnitGroup, STGValue,
    STGValueTarget,
};
pub use structure::{
    STGStructuralChange, STGStructuralEdit, STGStructuralImage, STGStructuralPreview,
    STGStructuralRestoreFailure,
};
pub use text::{STGText, STGTextImage, STGTextPreview, STGTextRestoreFailure};

const MAGIC: u32 = 1_001;
const MAGIC_SIZE: usize = size_of::<u32>();
const COUNT_SIZE: usize = size_of::<u32>();
const UNIT_COUNT_OFFSET: usize = MAGIC_SIZE + 620;

pub const MAX_STG_SOURCE_BYTES: usize = SOURCE_LIMIT;

#[derive(Clone, Debug)]
pub struct STGDocument {
    #[allow(
        dead_code,
        reason = "the source image is retained for exact STG encoding and rebasing"
    )]
    source: Arc<Vec<u8>>,
    lineage: Arc<()>,
    state: Arc<()>,
    revision: Arc<()>,
    model: Arc<STGModel>,
}

#[derive(Debug)]
pub struct STGCommittedImage {
    lineage: Arc<()>,
    source: Arc<Vec<u8>>,
    opaque_range: wire::OpaqueRange,
}

impl STGCommittedImage {
    pub fn bytes(&self) -> &[u8] {
        self.source.as_slice()
    }
}

#[derive(Clone, Debug)]
struct STGModel {
    #[allow(
        dead_code,
        reason = "the wire magic is retained for exact STG encoding"
    )]
    magic: u32,
    #[allow(
        dead_code,
        reason = "the header is retained for upcoming STG projections and edits"
    )]
    header: StgHeader,
    units: Vec<UnitBlock>,
    tail: STGTail,
}

#[derive(Clone, Debug)]
enum STGTail {
    Parsed(STGParsedTail),
    Raw {
        source: Arc<Vec<u8>>,
        range: Range<usize>,
        failure: STGTailFailure,
    },
}

#[derive(Clone, Debug)]
struct STGParsedTail {
    areas: Vec<AreaEntry>,
    variables: Vec<StgVariable>,
    event_blocks: Vec<EventBlock>,
    footer_entries: Vec<FooterEntry>,
    suffix_source: Arc<Vec<u8>>,
    suffix_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGTailStatus<'a> {
    Parsed {
        suffix: &'a [u8],
    },
    Raw {
        bytes: &'a [u8],
        failure: &'a STGTailFailure,
    },
}

impl STGDocument {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, FormatError> {
        Self::parse_with_limits(bytes, SOURCE_LIMIT, MODEL_LIMIT)
    }

    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        wire::encode(&self.model, SOURCE_LIMIT).map(|image| image.bytes)
    }

    pub fn prepare_commit(&self) -> Result<STGCommittedImage, FormatError> {
        let wire::EncodedSTG {
            bytes,
            opaque_range,
        } = wire::encode(&self.model, SOURCE_LIMIT)?;
        let (source, opaque_range) = if bytes.as_slice() == self.source.as_slice() {
            drop(bytes);
            (Arc::clone(&self.source), self.baseline_opaque_range()?)
        } else {
            (Arc::new(bytes), opaque_range)
        };
        Ok(STGCommittedImage {
            lineage: Arc::clone(&self.lineage),
            source,
            opaque_range,
        })
    }

    pub fn rebase_source(&mut self, committed: STGCommittedImage) -> Result<(), FormatError> {
        if !Arc::ptr_eq(&self.lineage, &committed.lineage) {
            return Err(FormatError::STGRebase(
                crate::error::STGRebaseError::ForeignLineage,
            ));
        }
        let committed_opaque =
            committed
                .source
                .get(committed.opaque_range.range())
                .ok_or(FormatError::STGRebase(
                    crate::error::STGRebaseError::InvalidLayout,
                ))?;
        let live_range = self
            .baseline_opaque_range()
            .map_err(|_| FormatError::STGRebase(crate::error::STGRebaseError::InvalidLayout))?;
        if !live_range.same_kind(&committed.opaque_range) {
            return Err(FormatError::STGRebase(
                crate::error::STGRebaseError::InvalidLayout,
            ));
        }
        let live_opaque = self
            .source
            .get(live_range.range())
            .ok_or(FormatError::STGRebase(
                crate::error::STGRebaseError::InvalidLayout,
            ))?;
        if live_opaque != committed_opaque {
            return Err(FormatError::STGRebase(
                crate::error::STGRebaseError::InconsistentImage,
            ));
        }

        self.source = Arc::clone(&committed.source);
        match (
            &mut Arc::make_mut(&mut self.model).tail,
            committed.opaque_range,
        ) {
            (STGTail::Parsed(tail), wire::OpaqueRange::Parsed(range)) => {
                tail.suffix_source = committed.source;
                tail.suffix_range = range;
            }
            (STGTail::Raw { source, range, .. }, wire::OpaqueRange::Raw(committed_range)) => {
                *source = committed.source;
                *range = committed_range;
            }
            _ => unreachable!("validated STG opaque kinds changed during rebase"),
        }
        Ok(())
    }

    fn parse_with_limits(
        bytes: Vec<u8>,
        source_limit: usize,
        model_limit: usize,
    ) -> Result<Self, FormatError> {
        if bytes.len() > source_limit {
            return Err(FormatError::STGParse(STGParseError::SourceTooLarge {
                length: bytes.len(),
                maximum: source_limit,
            }));
        }
        if bytes.capacity() > source_limit {
            return Err(FormatError::STGParse(
                STGParseError::SourceCapacityTooLarge {
                    capacity: bytes.capacity(),
                    maximum: source_limit,
                },
            ));
        }

        let source = Arc::new(bytes.into_boxed_slice().into_vec());
        debug_assert_eq!(source.capacity(), source.len());

        let magic = preflight::magic(source.as_slice())
            .map_err(STGParseError::PrefixPreflight)
            .map_err(FormatError::STGParse)?;
        if magic != MAGIC {
            return Err(FormatError::STGParse(STGParseError::InvalidMagic {
                offset: 0,
                actual: magic,
            }));
        }

        let mut budget = STGAllocationBudget::new(model_limit);
        budget
            .charge(STGRegion::Source, 0, size_of::<STGModel>())
            .map_err(STGParseError::PrefixPreflight)
            .map_err(FormatError::STGParse)?;
        let plan = preflight::prefix(source.as_slice(), magic, &mut budget)
            .map_err(STGParseError::PrefixPreflight)
            .map_err(FormatError::STGParse)?;

        let mut offset = MAGIC_SIZE;
        let header_start = offset;
        let header = StgHeader::parse(source.as_slice(), &mut offset)
            .map_err(|error| prefix_cleave_error(STGRegion::Header, header_start, error))?;
        offset = offset.checked_add(COUNT_SIZE).ok_or({
            FormatError::STGParse(STGParseError::PrefixPreflight(
                STGPreflightError::ArithmeticOverflow {
                    region: STGRegion::Units,
                    offset,
                },
            ))
        })?;

        let mut units = Vec::with_capacity(plan.unit_count);
        for _ in 0..plan.unit_count {
            let item_start = offset;
            let unit = UnitBlock::parse(source.as_slice(), &mut offset)
                .map_err(|error| prefix_cleave_error(STGRegion::Units, item_start, error))?;
            units.push(unit);
        }
        debug_assert_eq!(offset, plan.tail_start);
        let units = exact_vec(units);
        let prefix_retained = retained_prefix_bytes(&units, model_limit)
            .map_err(STGParseError::PrefixPreflight)
            .map_err(FormatError::STGParse)?;
        debug_assert!(prefix_retained <= budget.retained());
        let tail = parse_or_preserve_tail(
            Arc::clone(&source),
            plan.tail_start,
            prefix_retained,
            model_limit,
            &mut budget,
        );

        let model = STGModel {
            magic: plan.magic,
            header,
            units,
            tail,
        };
        debug_assert_eq!(wire::encoded_len(&model), Some(source.len()));
        Ok(Self {
            source,
            lineage: Arc::new(()),
            state: Arc::new(()),
            revision: Arc::new(()),
            model: Arc::new(model),
        })
    }

    pub fn tail_status(&self) -> STGTailStatus<'_> {
        match &self.model.tail {
            STGTail::Parsed(tail) => STGTailStatus::Parsed {
                suffix: source_range(&tail.suffix_source, &tail.suffix_range),
            },
            STGTail::Raw {
                source,
                range,
                failure,
            } => STGTailStatus::Raw {
                bytes: source_range(source, range),
                failure,
            },
        }
    }

    pub fn unit_count(&self) -> usize {
        self.model.units.len()
    }

    pub fn area_count(&self) -> Option<usize> {
        self.parsed_tail().map(|tail| tail.areas.len())
    }

    pub fn variable_count(&self) -> Option<usize> {
        self.parsed_tail().map(|tail| tail.variables.len())
    }

    pub fn event_block_count(&self) -> Option<usize> {
        self.parsed_tail().map(|tail| tail.event_blocks.len())
    }

    pub fn footer_count(&self) -> Option<usize> {
        self.parsed_tail().map(|tail| tail.footer_entries.len())
    }

    fn parsed_tail(&self) -> Option<&STGParsedTail> {
        match &self.model.tail {
            STGTail::Parsed(tail) => Some(tail),
            STGTail::Raw { .. } => None,
        }
    }

    fn baseline_opaque_range(&self) -> Result<wire::OpaqueRange, FormatError> {
        let range = match &self.model.tail {
            STGTail::Parsed(tail) => {
                if !Arc::ptr_eq(&self.source, &tail.suffix_source)
                    || tail.suffix_source.get(tail.suffix_range.clone()).is_none()
                {
                    return Err(FormatError::STGEncode(
                        crate::error::STGEncodeError::InvalidTailLayout,
                    ));
                }
                wire::OpaqueRange::Parsed(tail.suffix_range.clone())
            }
            STGTail::Raw { source, range, .. } => {
                if !Arc::ptr_eq(&self.source, source) || source.get(range.clone()).is_none() {
                    return Err(FormatError::STGEncode(
                        crate::error::STGEncodeError::InvalidTailLayout,
                    ));
                }
                wire::OpaqueRange::Raw(range.clone())
            }
        };
        Ok(range)
    }
}

fn parse_or_preserve_tail(
    source: Arc<Vec<u8>>,
    tail_start: usize,
    prefix_retained: usize,
    model_limit: usize,
    budget: &mut STGAllocationBudget,
) -> STGTail {
    let plan = match preflight::tail(source.as_slice(), tail_start, budget) {
        Ok(plan) => plan,
        Err(error) => {
            return raw_tail(source, tail_start, STGTailFailure::Preflight(error));
        }
    };
    let parsed = match parse_tail(Arc::clone(&source), plan) {
        Ok(parsed) => parsed,
        Err(failure) => return raw_tail(source, tail_start, failure),
    };
    let Some(tail_retained) = retained_tail_bytes(&parsed) else {
        return raw_tail(
            source,
            tail_start,
            arithmetic_tail_failure(STGRegion::Suffix, plan.end),
        );
    };
    let actual_total = prefix_retained.checked_add(tail_retained);
    if actual_total.is_none_or(|retained| retained > model_limit) {
        return raw_tail(
            source,
            tail_start,
            STGTailFailure::Preflight(STGPreflightError::AllocationBudgetExceeded {
                region: STGRegion::Suffix,
                offset: plan.end,
                retained: prefix_retained,
                requested: tail_retained,
                maximum: model_limit,
            }),
        );
    }

    debug_assert!(actual_total.is_some_and(|retained| retained <= budget.retained()));
    STGTail::Parsed(parsed)
}

fn parse_tail(source: Arc<Vec<u8>>, plan: STGTailPlan) -> Result<STGParsedTail, STGTailFailure> {
    let mut offset = plan.start;

    let area_count = read_planned_count(source.as_slice(), &mut offset, STGRegion::Areas)?;
    debug_assert_eq!(area_count, plan.area_count);
    let areas = parse_items(
        source.as_slice(),
        &mut offset,
        area_count,
        STGRegion::Areas,
        AreaEntry::parse,
    )?;

    let variable_count = read_planned_count(source.as_slice(), &mut offset, STGRegion::Variables)?;
    debug_assert_eq!(variable_count, plan.variable_count);
    let mut variables = parse_items(
        source.as_slice(),
        &mut offset,
        variable_count,
        STGRegion::Variables,
        StgVariable::parse,
    )?;
    for variable in &mut variables {
        canonicalize_parameter(&mut variable.initial_value);
    }

    let event_block_count =
        read_planned_count(source.as_slice(), &mut offset, STGRegion::EventBlocks)?;
    debug_assert_eq!(event_block_count, plan.event_block_count);
    let mut event_blocks = parse_items(
        source.as_slice(),
        &mut offset,
        event_block_count,
        STGRegion::EventBlocks,
        EventBlock::parse,
    )?;
    canonicalize_event_blocks(&mut event_blocks);

    let footer_count = read_planned_count(source.as_slice(), &mut offset, STGRegion::Footer)?;
    debug_assert_eq!(footer_count, plan.footer_count);
    let footer_entries = parse_items(
        source.as_slice(),
        &mut offset,
        footer_count,
        STGRegion::Footer,
        FooterEntry::parse,
    )?;
    debug_assert_eq!(offset, plan.end);
    let suffix_end = source.len();

    Ok(STGParsedTail {
        areas: exact_vec(areas),
        variables: exact_vec(variables),
        event_blocks: exact_vec(event_blocks),
        footer_entries: exact_vec(footer_entries),
        suffix_source: source,
        suffix_range: plan.end..suffix_end,
    })
}

fn parse_items<T>(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
    region: STGRegion,
    parse: fn(&[u8], &mut usize) -> Result<T, kuf_stg::Error>,
) -> Result<Vec<T>, STGTailFailure> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let item_start = *offset;
        match parse(bytes, offset) {
            Ok(value) => values.push(value),
            Err(error) => return Err(tail_cleave_error(region, item_start, error)),
        }
    }
    Ok(values)
}

fn read_planned_count(
    bytes: &[u8],
    offset: &mut usize,
    region: STGRegion,
) -> Result<usize, STGTailFailure> {
    let start = *offset;
    let Some(end) = start.checked_add(COUNT_SIZE) else {
        return Err(arithmetic_tail_failure(region, start));
    };
    let Some(raw) = bytes.get(start..end) else {
        return Err(STGTailFailure::Cleave {
            region,
            offset: start,
            source: STGCleaveError::UnexpectedEOF {
                offset: start,
                needed: COUNT_SIZE,
                remaining: bytes.len().saturating_sub(start),
            },
        });
    };
    let Ok(raw) = <[u8; COUNT_SIZE]>::try_from(raw) else {
        unreachable!("planned STG count has the wrong byte width");
    };
    *offset = end;
    usize::try_from(u32::from_le_bytes(raw)).map_err(|_| STGTailFailure::Cleave {
        region,
        offset: start,
        source: STGCleaveError::InvalidLength {
            field: "count",
            value: i128::from(u32::from_le_bytes(raw)),
        },
    })
}

fn prefix_cleave_error(
    region: STGRegion,
    fallback_offset: usize,
    error: kuf_stg::Error,
) -> FormatError {
    let source = STGCleaveError::from(error);
    let offset = source.offset().unwrap_or(fallback_offset);
    FormatError::STGParse(STGParseError::PrefixCleave {
        region,
        offset,
        source,
    })
}

fn tail_cleave_error(
    region: STGRegion,
    fallback_offset: usize,
    error: kuf_stg::Error,
) -> STGTailFailure {
    let source = STGCleaveError::from(error);
    let offset = source.offset().unwrap_or(fallback_offset);
    STGTailFailure::Cleave {
        region,
        offset,
        source,
    }
}

fn arithmetic_tail_failure(region: STGRegion, offset: usize) -> STGTailFailure {
    STGTailFailure::Preflight(STGPreflightError::ArithmeticOverflow { region, offset })
}

fn raw_tail(source: Arc<Vec<u8>>, start: usize, failure: STGTailFailure) -> STGTail {
    let end = source.len();
    STGTail::Raw {
        source,
        range: start..end,
        failure,
    }
}

fn source_range<'a>(source: &'a Arc<Vec<u8>>, range: &Range<usize>) -> &'a [u8] {
    let Some(bytes) = source.get(range.clone()) else {
        unreachable!("retained STG source range is invalid");
    };
    bytes
}

fn exact_vec<T>(values: Vec<T>) -> Vec<T> {
    values.into_boxed_slice().into_vec()
}

fn canonicalize_parameter(parameter: &mut StgParamValue) {
    if let StgParamValueValue::StgStringParam(value) = &mut parameter.value {
        value.value = exact_vec(std::mem::take(&mut value.value));
    }
}

fn canonicalize_event_blocks(blocks: &mut [EventBlock]) {
    for block in blocks {
        for event in &mut block.events {
            canonicalize_scripts(&mut event.conditions, &mut event.actions);
            event.conditions = exact_vec(std::mem::take(&mut event.conditions));
            event.actions = exact_vec(std::mem::take(&mut event.actions));
        }
        block.events = exact_vec(std::mem::take(&mut block.events));
    }
}

fn canonicalize_scripts(conditions: &mut [StgCondition], actions: &mut [StgAction]) {
    for condition in conditions {
        for parameter in &mut condition.params {
            canonicalize_parameter(parameter);
        }
        condition.params = exact_vec(std::mem::take(&mut condition.params));
    }
    for action in actions {
        for parameter in &mut action.params {
            canonicalize_parameter(parameter);
        }
        action.params = exact_vec(std::mem::take(&mut action.params));
    }
}

fn retained_tail_bytes(tail: &STGParsedTail) -> Option<usize> {
    let mut retained = 0_usize;
    charge_capacity::<AreaEntry>(&mut retained, &tail.areas)?;
    charge_capacity::<StgVariable>(&mut retained, &tail.variables)?;
    for variable in &tail.variables {
        charge_parameter_capacity(&mut retained, &variable.initial_value)?;
    }
    charge_capacity::<EventBlock>(&mut retained, &tail.event_blocks)?;
    for block in &tail.event_blocks {
        charge_capacity::<kuf_stg::StgEvent>(&mut retained, &block.events)?;
        for event in &block.events {
            charge_capacity::<StgCondition>(&mut retained, &event.conditions)?;
            for condition in &event.conditions {
                charge_capacity::<StgParamValue>(&mut retained, &condition.params)?;
                for parameter in &condition.params {
                    charge_parameter_capacity(&mut retained, parameter)?;
                }
            }
            charge_capacity::<StgAction>(&mut retained, &event.actions)?;
            for action in &event.actions {
                charge_capacity::<StgParamValue>(&mut retained, &action.params)?;
                for parameter in &action.params {
                    charge_parameter_capacity(&mut retained, parameter)?;
                }
            }
        }
    }
    charge_capacity::<FooterEntry>(&mut retained, &tail.footer_entries)?;
    Some(retained)
}

fn retained_model_bytes(model: &STGModel) -> Option<usize> {
    let mut retained = size_of::<STGModel>();
    charge_capacity::<UnitBlock>(&mut retained, &model.units)?;
    if let STGTail::Parsed(tail) = &model.tail {
        retained = retained.checked_add(retained_tail_bytes(tail)?)?;
    }
    Some(retained)
}

fn retained_prefix_bytes(
    units: &Vec<UnitBlock>,
    maximum: usize,
) -> Result<usize, STGPreflightError> {
    let retained = size_of::<STGModel>();
    let requested = units.capacity().checked_mul(size_of::<UnitBlock>()).ok_or(
        STGPreflightError::ArithmeticOverflow {
            region: STGRegion::Units,
            offset: UNIT_COUNT_OFFSET,
        },
    )?;
    let total = retained
        .checked_add(requested)
        .ok_or(STGPreflightError::ArithmeticOverflow {
            region: STGRegion::Units,
            offset: UNIT_COUNT_OFFSET,
        })?;
    if total > maximum {
        return Err(STGPreflightError::AllocationBudgetExceeded {
            region: STGRegion::Units,
            offset: UNIT_COUNT_OFFSET,
            retained,
            requested,
            maximum,
        });
    }
    Ok(total)
}

fn charge_capacity<T>(retained: &mut usize, values: &Vec<T>) -> Option<()> {
    let bytes = values.capacity().checked_mul(size_of::<T>())?;
    *retained = retained.checked_add(bytes)?;
    Some(())
}

fn charge_parameter_capacity(retained: &mut usize, parameter: &StgParamValue) -> Option<()> {
    if let StgParamValueValue::StgStringParam(value) = &parameter.value {
        *retained = retained.checked_add(value.value.capacity())?;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_limits_source_length_and_incoming_capacity_before_canonicalizing() {
        let bytes = empty_document(0, 0);
        let source_limit = bytes.len() - 1;
        assert_eq!(
            parse_error(bytes.clone(), source_limit, MODEL_LIMIT),
            STGParseError::SourceTooLarge {
                length: bytes.len(),
                maximum: source_limit,
            }
        );

        let capacity_limit = bytes.len() + 8;
        let mut excessive_capacity = Vec::with_capacity(capacity_limit + 64);
        excessive_capacity.extend_from_slice(&bytes);
        let incoming_capacity = excessive_capacity.capacity();
        assert!(incoming_capacity > capacity_limit);
        assert_eq!(
            parse_error(excessive_capacity, capacity_limit, MODEL_LIMIT),
            STGParseError::SourceCapacityTooLarge {
                capacity: incoming_capacity,
                maximum: capacity_limit,
            }
        );

        let mut accepted_capacity = Vec::with_capacity(bytes.len() + 64);
        accepted_capacity.extend_from_slice(&bytes);
        let accepted_limit = accepted_capacity.capacity();
        let document =
            match STGDocument::parse_with_limits(accepted_capacity, accepted_limit, MODEL_LIMIT) {
                Ok(document) => document,
                Err(error) => panic!("accepted STG source failed: {error}"),
            };
        assert_eq!(document.source.capacity(), document.source.len());
        assert_eq!(document.source.len(), bytes.len());
    }

    #[test]
    fn prefix_allocation_budget_failure_is_fatal() {
        let bytes = empty_document(1, 0);
        let retained = size_of::<STGModel>();
        let requested = size_of::<UnitBlock>();
        let maximum = retained + requested - 1;

        assert_eq!(
            parse_error(bytes, SOURCE_LIMIT, maximum),
            STGParseError::PrefixPreflight(STGPreflightError::AllocationBudgetExceeded {
                region: STGRegion::Units,
                offset: 624,
                retained,
                requested,
                maximum,
            })
        );
    }

    #[test]
    fn tail_allocation_budget_failure_preserves_the_prefix_as_raw() {
        let bytes = empty_document(0, 1);
        let tail_start = MAGIC_SIZE + 620 + COUNT_SIZE;
        let retained = size_of::<STGModel>();
        let requested = size_of::<AreaEntry>();
        let maximum = retained + requested - 1;
        let document = match STGDocument::parse_with_limits(bytes, SOURCE_LIMIT, maximum) {
            Ok(document) => document,
            Err(error) => panic!("tail budget failure rejected the STG prefix: {error}"),
        };

        assert_eq!(document.unit_count(), 0);
        assert_eq!(
            document.tail_status(),
            STGTailStatus::Raw {
                bytes: source_range(&document.source, &(tail_start..document.source.len())),
                failure: &STGTailFailure::Preflight(STGPreflightError::AllocationBudgetExceeded {
                    region: STGRegion::Areas,
                    offset: tail_start,
                    retained,
                    requested,
                    maximum,
                }),
            }
        );
    }

    #[test]
    fn one_budget_is_cumulative_across_prefix_and_tail() {
        let bytes = empty_document(1, 1);
        let tail_start = MAGIC_SIZE + 620 + COUNT_SIZE + 544;
        let base = size_of::<STGModel>();
        let unit_bytes = size_of::<UnitBlock>();
        let requested = size_of::<AreaEntry>();
        let retained = base + unit_bytes;
        let maximum = retained + requested - 1;
        assert!(base + unit_bytes <= maximum);
        assert!(base + requested <= maximum);

        let document = match STGDocument::parse_with_limits(bytes, SOURCE_LIMIT, maximum) {
            Ok(document) => document,
            Err(error) => panic!("cumulative tail budget rejected the STG prefix: {error}"),
        };
        assert_eq!(document.unit_count(), 1);
        assert_eq!(
            document.tail_status(),
            STGTailStatus::Raw {
                bytes: source_range(&document.source, &(tail_start..document.source.len())),
                failure: &STGTailFailure::Preflight(STGPreflightError::AllocationBudgetExceeded {
                    region: STGRegion::Areas,
                    offset: tail_start,
                    retained,
                    requested,
                    maximum,
                }),
            }
        );
    }

    #[test]
    fn wire_length_matches_parsed_and_raw_source_layouts() {
        let parsed_bytes = empty_document(1, 1);
        let parsed_length = parsed_bytes.len();
        let parsed = match STGDocument::parse(parsed_bytes) {
            Ok(document) => document,
            Err(error) => panic!("parsed-tail length fixture failed: {error}"),
        };
        assert_eq!(wire::encoded_len(&parsed.model), Some(parsed_length));

        let mut raw_bytes = empty_document(1, 0);
        raw_bytes.pop();
        let raw_length = raw_bytes.len();
        let raw = match STGDocument::parse(raw_bytes) {
            Ok(document) => document,
            Err(error) => panic!("raw-tail length fixture failed: {error}"),
        };
        assert!(matches!(raw.tail_status(), STGTailStatus::Raw { .. }));
        assert_eq!(wire::encoded_len(&raw.model), Some(raw_length));
    }

    #[test]
    fn equal_commit_images_share_the_exact_parsed_source_allocation() {
        let document = STGDocument::parse(empty_document(1, 1)).unwrap();

        let first = document.prepare_commit().unwrap();
        let second = document.prepare_commit().unwrap();

        assert!(Arc::ptr_eq(&first.source, &document.source));
        assert!(Arc::ptr_eq(&first.source, &second.source));
    }

    #[test]
    fn rebase_reuses_a_unique_live_model_and_installs_one_committed_source() {
        let target = STGNumberTarget::Unit {
            unit: 0,
            field: STGUnitField::UniqueID,
        };
        let mut live = STGDocument::parse(empty_document(1, 1)).unwrap();
        live.set_number(target, 1).unwrap();
        let snapshot = live.clone();
        let committed = snapshot.prepare_commit().unwrap();
        let committed_source = Arc::clone(&committed.source);
        live.set_number(target, 2).unwrap();
        drop(snapshot);
        let model_before = Arc::as_ptr(&live.model);

        live.rebase_source(committed).unwrap();

        assert_eq!(live.number(target).unwrap(), 2);
        assert_eq!(Arc::as_ptr(&live.model), model_before);
        assert!(Arc::ptr_eq(&live.source, &committed_source));
        let STGTail::Parsed(tail) = &live.model.tail else {
            panic!("parsed STG rebase changed its tail kind");
        };
        assert!(Arc::ptr_eq(&tail.suffix_source, &committed_source));
    }

    fn empty_document(unit_count: usize, area_count: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC.to_le_bytes());
        bytes.resize(bytes.len() + 620, 0);
        push_count(&mut bytes, unit_count);
        bytes.resize(bytes.len() + unit_count * 544, 0);
        push_count(&mut bytes, area_count);
        bytes.resize(bytes.len() + area_count * 84, 0);
        push_count(&mut bytes, 0);
        push_count(&mut bytes, 0);
        push_count(&mut bytes, 0);
        bytes
    }

    fn push_count(bytes: &mut Vec<u8>, count: usize) {
        let Ok(count) = u32::try_from(count) else {
            panic!("test STG count does not fit u32");
        };
        bytes.extend_from_slice(&count.to_le_bytes());
    }

    fn parse_error(bytes: Vec<u8>, source_limit: usize, model_limit: usize) -> STGParseError {
        match STGDocument::parse_with_limits(bytes, source_limit, model_limit) {
            Err(FormatError::STGParse(error)) => error,
            Err(other) => panic!("unexpected STG error: {other}"),
            Ok(_) => panic!("expected STG parse failure"),
        }
    }
}
