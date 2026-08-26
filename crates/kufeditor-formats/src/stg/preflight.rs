use std::mem::size_of;

use crate::{
    error::{STGPreflightError, STGRegion},
    generated::kuf_stg::{
        AreaEntry, EventBlock, FooterEntry, StgAction, StgCondition, StgEvent, StgParamValue,
        StgVariable, UnitBlock,
    },
};

const COUNT_SIZE: usize = size_of::<u32>();
const HEADER_SIZE: usize = 620;
const UNIT_SIZE: usize = 544;
const AREA_SIZE: usize = 84;
const VARIABLE_FIXED_SIZE: usize = 68;
const EVENT_BLOCK_FIXED_SIZE: usize = 4;
const EVENT_FIXED_SIZE: usize = 68;
const SCRIPT_FIXED_SIZE: usize = 4;
const PARAMETER_MINIMUM_SIZE: usize = 8;
const FOOTER_SIZE: usize = 8;

pub(super) const SOURCE_LIMIT: usize = 64 * 1024 * 1024;
pub(super) const MODEL_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct STGPrefixPlan {
    pub magic: u32,
    pub unit_count: usize,
    pub tail_start: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct STGTailPlan {
    pub start: usize,
    pub area_count: usize,
    pub variable_count: usize,
    pub event_block_count: usize,
    pub footer_count: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct STGAllocationBudget {
    retained: usize,
    maximum: usize,
}

impl STGAllocationBudget {
    pub(super) const fn new(maximum: usize) -> Self {
        Self {
            retained: 0,
            maximum,
        }
    }

    pub(super) const fn retained(self) -> usize {
        self.retained
    }

    pub(super) fn charge(
        &mut self,
        region: STGRegion,
        offset: usize,
        requested: usize,
    ) -> Result<(), STGPreflightError> {
        let Some(next) = self.retained.checked_add(requested) else {
            return Err(STGPreflightError::ArithmeticOverflow { region, offset });
        };
        if next > self.maximum {
            return Err(STGPreflightError::AllocationBudgetExceeded {
                region,
                offset,
                retained: self.retained,
                requested,
                maximum: self.maximum,
            });
        }
        self.retained = next;
        Ok(())
    }
}

pub(super) fn prefix(
    bytes: &[u8],
    magic: u32,
    budget: &mut STGAllocationBudget,
) -> Result<STGPrefixPlan, STGPreflightError> {
    let unit_count_offset = require(bytes, COUNT_SIZE, HEADER_SIZE, STGRegion::Header)?;
    let unit_count = read_count(bytes, unit_count_offset, STGRegion::Units)?;
    let units_start = checked_add(
        unit_count_offset,
        COUNT_SIZE,
        STGRegion::Units,
        unit_count_offset,
    )?;
    prove_count(
        bytes,
        units_start,
        unit_count_offset,
        unit_count,
        UNIT_SIZE,
        STGRegion::Units,
    )?;
    charge_elements::<UnitBlock>(budget, STGRegion::Units, unit_count_offset, unit_count)?;
    let unit_bytes = checked_multiply(unit_count, UNIT_SIZE, STGRegion::Units, unit_count_offset)?;
    let tail_start = checked_add(units_start, unit_bytes, STGRegion::Units, unit_count_offset)?;

    Ok(STGPrefixPlan {
        magic,
        unit_count,
        tail_start,
    })
}

pub(super) fn magic(bytes: &[u8]) -> Result<u32, STGPreflightError> {
    read_u32(bytes, 0, STGRegion::Magic)
}

pub(super) fn tail(
    bytes: &[u8],
    tail_start: usize,
    budget: &mut STGAllocationBudget,
) -> Result<STGTailPlan, STGPreflightError> {
    let (area_count, offset) =
        fixed_collection::<AreaEntry>(bytes, tail_start, budget, STGRegion::Areas, AREA_SIZE)?;
    let (variable_count, offset) = variables(bytes, offset, budget)?;
    let (event_block_count, offset) = event_blocks(bytes, offset, budget)?;
    let (footer_count, offset) =
        fixed_collection::<FooterEntry>(bytes, offset, budget, STGRegion::Footer, FOOTER_SIZE)?;

    Ok(STGTailPlan {
        start: tail_start,
        area_count,
        variable_count,
        event_block_count,
        footer_count,
        end: offset,
    })
}

fn fixed_collection<T>(
    bytes: &[u8],
    count_offset: usize,
    budget: &mut STGAllocationBudget,
    region: STGRegion,
    item_size: usize,
) -> Result<(usize, usize), STGPreflightError> {
    let count = read_count(bytes, count_offset, region)?;
    let items_offset = checked_add(count_offset, COUNT_SIZE, region, count_offset)?;
    prove_count(bytes, items_offset, count_offset, count, item_size, region)?;
    charge_elements::<T>(budget, region, count_offset, count)?;
    let end = advance_items(items_offset, count, item_size, region, count_offset)?;
    Ok((count, end))
}

fn variables(
    bytes: &[u8],
    count_offset: usize,
    budget: &mut STGAllocationBudget,
) -> Result<(usize, usize), STGPreflightError> {
    let count = read_count(bytes, count_offset, STGRegion::Variables)?;
    let mut offset = checked_add(count_offset, COUNT_SIZE, STGRegion::Variables, count_offset)?;
    prove_count(
        bytes,
        offset,
        count_offset,
        count,
        VARIABLE_FIXED_SIZE + PARAMETER_MINIMUM_SIZE,
        STGRegion::Variables,
    )?;
    charge_elements::<StgVariable>(budget, STGRegion::Variables, count_offset, count)?;
    for _ in 0..count {
        offset = require(bytes, offset, VARIABLE_FIXED_SIZE, STGRegion::Variables)?;
        offset = parameter(bytes, offset, budget)?;
    }
    Ok((count, offset))
}

fn event_blocks(
    bytes: &[u8],
    count_offset: usize,
    budget: &mut STGAllocationBudget,
) -> Result<(usize, usize), STGPreflightError> {
    let count = read_count(bytes, count_offset, STGRegion::EventBlocks)?;
    let mut offset = checked_add(
        count_offset,
        COUNT_SIZE,
        STGRegion::EventBlocks,
        count_offset,
    )?;
    prove_count(
        bytes,
        offset,
        count_offset,
        count,
        EVENT_BLOCK_FIXED_SIZE + COUNT_SIZE,
        STGRegion::EventBlocks,
    )?;
    charge_elements::<EventBlock>(budget, STGRegion::EventBlocks, count_offset, count)?;
    for _ in 0..count {
        offset = require(
            bytes,
            offset,
            EVENT_BLOCK_FIXED_SIZE,
            STGRegion::EventBlocks,
        )?;
        let event_count_offset = offset;
        let event_count = read_count(bytes, offset, STGRegion::Events)?;
        offset = checked_add(offset, COUNT_SIZE, STGRegion::Events, offset)?;
        prove_count(
            bytes,
            offset,
            event_count_offset,
            event_count,
            EVENT_FIXED_SIZE + COUNT_SIZE + COUNT_SIZE,
            STGRegion::Events,
        )?;
        charge_elements::<StgEvent>(budget, STGRegion::Events, event_count_offset, event_count)?;
        for _ in 0..event_count {
            offset = event(bytes, offset, budget)?;
        }
    }
    Ok((count, offset))
}

fn event(
    bytes: &[u8],
    mut offset: usize,
    budget: &mut STGAllocationBudget,
) -> Result<usize, STGPreflightError> {
    offset = require(bytes, offset, EVENT_FIXED_SIZE, STGRegion::Events)?;

    let condition_count_offset = offset;
    let condition_count = read_count(bytes, offset, STGRegion::Conditions)?;
    offset = checked_add(offset, COUNT_SIZE, STGRegion::Conditions, offset)?;
    prove_count(
        bytes,
        offset,
        condition_count_offset,
        condition_count,
        SCRIPT_FIXED_SIZE + COUNT_SIZE,
        STGRegion::Conditions,
    )?;
    charge_elements::<StgCondition>(
        budget,
        STGRegion::Conditions,
        condition_count_offset,
        condition_count,
    )?;
    for _ in 0..condition_count {
        offset = script(bytes, offset, budget, STGRegion::Conditions)?;
    }

    let action_count_offset = offset;
    let action_count = read_count(bytes, offset, STGRegion::Actions)?;
    offset = checked_add(offset, COUNT_SIZE, STGRegion::Actions, offset)?;
    prove_count(
        bytes,
        offset,
        action_count_offset,
        action_count,
        SCRIPT_FIXED_SIZE + COUNT_SIZE,
        STGRegion::Actions,
    )?;
    charge_elements::<StgAction>(
        budget,
        STGRegion::Actions,
        action_count_offset,
        action_count,
    )?;
    for _ in 0..action_count {
        offset = script(bytes, offset, budget, STGRegion::Actions)?;
    }

    Ok(offset)
}

fn script(
    bytes: &[u8],
    mut offset: usize,
    budget: &mut STGAllocationBudget,
    region: STGRegion,
) -> Result<usize, STGPreflightError> {
    offset = require(bytes, offset, SCRIPT_FIXED_SIZE, region)?;
    let parameter_count_offset = offset;
    let parameter_count = read_count(bytes, offset, STGRegion::Parameters)?;
    offset = checked_add(offset, COUNT_SIZE, STGRegion::Parameters, offset)?;
    prove_count(
        bytes,
        offset,
        parameter_count_offset,
        parameter_count,
        PARAMETER_MINIMUM_SIZE,
        STGRegion::Parameters,
    )?;
    charge_elements::<StgParamValue>(
        budget,
        STGRegion::Parameters,
        parameter_count_offset,
        parameter_count,
    )?;
    for _ in 0..parameter_count {
        offset = parameter(bytes, offset, budget)?;
    }
    Ok(offset)
}

fn parameter(
    bytes: &[u8],
    mut offset: usize,
    budget: &mut STGAllocationBudget,
) -> Result<usize, STGPreflightError> {
    let tag_offset = offset;
    let tag = read_u32(bytes, tag_offset, STGRegion::Parameters)?;
    offset = checked_add(offset, COUNT_SIZE, STGRegion::Parameters, offset)?;
    match tag {
        0 | 1 | 3 => require(bytes, offset, COUNT_SIZE, STGRegion::Parameters),
        2 => {
            let length_offset = offset;
            let length = read_count(bytes, offset, STGRegion::Parameters)?;
            offset = checked_add(offset, COUNT_SIZE, STGRegion::Parameters, offset)?;
            prove_count(
                bytes,
                offset,
                length_offset,
                length,
                1,
                STGRegion::Parameters,
            )?;
            budget.charge(STGRegion::Parameters, length_offset, length)?;
            checked_add(offset, length, STGRegion::Parameters, length_offset)
        }
        tag => Err(STGPreflightError::UnknownParameterType {
            offset: tag_offset,
            tag,
        }),
    }
}

fn read_count(bytes: &[u8], offset: usize, region: STGRegion) -> Result<usize, STGPreflightError> {
    usize::try_from(read_u32(bytes, offset, region)?)
        .map_err(|_| STGPreflightError::ArithmeticOverflow { region, offset })
}

fn read_u32(bytes: &[u8], offset: usize, region: STGRegion) -> Result<u32, STGPreflightError> {
    let end = require(bytes, offset, COUNT_SIZE, region)?;
    let Some(raw) = bytes.get(offset..end) else {
        unreachable!("require returned an unavailable STG range");
    };
    let Ok(raw) = <[u8; COUNT_SIZE]>::try_from(raw) else {
        unreachable!("STG u32 range has the wrong length");
    };
    Ok(u32::from_le_bytes(raw))
}

fn prove_count(
    bytes: &[u8],
    items_offset: usize,
    count_offset: usize,
    count: usize,
    minimum_item_size: usize,
    region: STGRegion,
) -> Result<(), STGPreflightError> {
    let Some(raw_count) = u32::try_from(count).ok() else {
        return Err(STGPreflightError::ArithmeticOverflow {
            region,
            offset: count_offset,
        });
    };
    let required = checked_multiply(count, minimum_item_size, region, count_offset)?;
    let remaining = bytes.len().saturating_sub(items_offset);
    if required > remaining {
        return Err(STGPreflightError::ImpossibleCount {
            region,
            offset: count_offset,
            count: raw_count,
            minimum_item_size,
            remaining,
        });
    }
    Ok(())
}

fn require(
    bytes: &[u8],
    offset: usize,
    needed: usize,
    region: STGRegion,
) -> Result<usize, STGPreflightError> {
    let end = checked_add(offset, needed, region, offset)?;
    if end > bytes.len() {
        return Err(STGPreflightError::Truncated {
            region,
            offset,
            needed,
            remaining: bytes.len().saturating_sub(offset),
        });
    }
    Ok(end)
}

fn advance_items(
    offset: usize,
    count: usize,
    item_size: usize,
    region: STGRegion,
    count_offset: usize,
) -> Result<usize, STGPreflightError> {
    let bytes = checked_multiply(count, item_size, region, count_offset)?;
    checked_add(offset, bytes, region, count_offset)
}

fn charge_elements<T>(
    budget: &mut STGAllocationBudget,
    region: STGRegion,
    offset: usize,
    count: usize,
) -> Result<(), STGPreflightError> {
    let requested = checked_multiply(count, size_of::<T>(), region, offset)?;
    budget.charge(region, offset, requested)
}

fn checked_multiply(
    left: usize,
    right: usize,
    region: STGRegion,
    offset: usize,
) -> Result<usize, STGPreflightError> {
    left.checked_mul(right)
        .ok_or(STGPreflightError::ArithmeticOverflow { region, offset })
}

fn checked_add(
    left: usize,
    right: usize,
    region: STGRegion,
    offset: usize,
) -> Result<usize, STGPreflightError> {
    left.checked_add(right)
        .ok_or(STGPreflightError::ArithmeticOverflow { region, offset })
}
