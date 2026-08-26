use crate::generated::kuf_stg::{StgAction, StgCondition, StgParamValue, StgParamValueValue};

use super::{STGModel, STGParsedTail, STGTail, source_range};

const COUNT_SIZE: usize = size_of::<u32>();
const MAGIC_SIZE: usize = size_of::<u32>();
const HEADER_SIZE: usize = 620;
const UNIT_SIZE: usize = 544;
const AREA_SIZE: usize = 84;
const VARIABLE_FIXED_SIZE: usize = 68;
const EVENT_BLOCK_FIXED_SIZE: usize = 8;
const EVENT_FIXED_SIZE: usize = 72;
const SCRIPT_FIXED_SIZE: usize = 8;
const PARAMETER_TAG_SIZE: usize = size_of::<u32>();
const SCALAR_PAYLOAD_SIZE: usize = size_of::<u32>();
const STRING_LENGTH_SIZE: usize = size_of::<u32>();
const FOOTER_SIZE: usize = 8;

pub(super) fn encoded_len(model: &STGModel) -> Option<usize> {
    let mut length = Length::new(
        MAGIC_SIZE
            .checked_add(HEADER_SIZE)?
            .checked_add(COUNT_SIZE)?,
    );
    length.add_items(model.units.len(), UNIT_SIZE)?;
    match &model.tail {
        STGTail::Parsed(tail) => add_parsed_tail(&mut length, tail)?,
        STGTail::Raw { source, range, .. } => {
            length.add(source_range(source, range).len())?;
        }
    }
    Some(length.value)
}

fn add_parsed_tail(length: &mut Length, tail: &STGParsedTail) -> Option<()> {
    length.add(COUNT_SIZE)?;
    length.add_items(tail.areas.len(), AREA_SIZE)?;

    length.add(COUNT_SIZE)?;
    for variable in &tail.variables {
        length.add(VARIABLE_FIXED_SIZE)?;
        length.add(parameter_len(&variable.initial_value)?)?;
    }

    length.add(COUNT_SIZE)?;
    for block in &tail.event_blocks {
        length.add(EVENT_BLOCK_FIXED_SIZE)?;
        for event in &block.events {
            length.add(EVENT_FIXED_SIZE)?;
            for condition in &event.conditions {
                length.add(condition_len(condition)?)?;
            }
            length.add(COUNT_SIZE)?;
            for action in &event.actions {
                length.add(action_len(action)?)?;
            }
        }
    }

    length.add(COUNT_SIZE)?;
    length.add_items(tail.footer_entries.len(), FOOTER_SIZE)?;
    length.add(source_range(&tail.suffix_source, &tail.suffix_range).len())
}

fn condition_len(condition: &StgCondition) -> Option<usize> {
    script_len(&condition.params)
}

fn action_len(action: &StgAction) -> Option<usize> {
    script_len(&action.params)
}

fn script_len(parameters: &[StgParamValue]) -> Option<usize> {
    let mut length = Length::new(SCRIPT_FIXED_SIZE);
    for parameter in parameters {
        length.add(parameter_len(parameter)?)?;
    }
    Some(length.value)
}

fn parameter_len(parameter: &StgParamValue) -> Option<usize> {
    let payload = match &parameter.value {
        StgParamValueValue::I32(_) | StgParamValueValue::F32(_) => SCALAR_PAYLOAD_SIZE,
        StgParamValueValue::StgStringParam(value) => {
            STRING_LENGTH_SIZE.checked_add(value.value.len())?
        }
    };
    PARAMETER_TAG_SIZE.checked_add(payload)
}

struct Length {
    value: usize,
}

impl Length {
    const fn new(value: usize) -> Self {
        Self { value }
    }

    fn add(&mut self, amount: usize) -> Option<()> {
        self.value = self.value.checked_add(amount)?;
        Some(())
    }

    fn add_items(&mut self, count: usize, item_size: usize) -> Option<()> {
        self.add(count.checked_mul(item_size)?)
    }
}
