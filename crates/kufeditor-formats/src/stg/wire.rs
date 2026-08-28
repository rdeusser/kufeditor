use std::ops::Range;

use crate::{
    error::{FormatError, STGEncodeError},
    generated::kuf_stg::{
        self, AreaEntry, EventBlock, FooterEntry, StgAction, StgCondition, StgEvent, StgHeader,
        StgParamValue, StgParamValueValue, StgVariable, UnitBlock,
    },
};

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

pub(super) struct EncodedSTG {
    pub(super) bytes: Vec<u8>,
    pub(super) opaque_range: OpaqueRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OpaqueRange {
    Parsed(Range<usize>),
    Raw(Range<usize>),
}

impl OpaqueRange {
    pub(super) fn range(&self) -> Range<usize> {
        match self {
            Self::Parsed(range) | Self::Raw(range) => range.clone(),
        }
    }

    pub(super) const fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Parsed(_), Self::Parsed(_)) | (Self::Raw(_), Self::Raw(_))
        )
    }
}

pub(super) fn encode(model: &STGModel, maximum: usize) -> Result<EncodedSTG, FormatError> {
    validate_counts(model)?;
    let length = encoded_len(model).ok_or(FormatError::STGEncode(
        STGEncodeError::LengthArithmeticOverflow,
    ))?;
    if length > maximum {
        return Err(FormatError::STGEncode(STGEncodeError::LengthOverflow {
            length,
            maximum,
        }));
    }

    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| FormatError::STGEncode(STGEncodeError::Allocation { requested: length }))?;
    bytes.resize(length, 0);
    let mut bytes = bytes.into_boxed_slice();
    let opaque_range = {
        let mut cursor = SliceCursor::new(&mut bytes);
        cursor.write(&model.magic.to_le_bytes())?;
        write_header(&mut cursor, &model.header)?;
        cursor.write_count("unit_count", model.units.len())?;
        for unit in &model.units {
            write_unit(&mut cursor, unit)?;
        }

        let range = match &model.tail {
            STGTail::Parsed(tail) => {
                cursor.write_count("area_count", tail.areas.len())?;
                for area in &tail.areas {
                    write_area(&mut cursor, area)?;
                }
                cursor.write_count("variable_count", tail.variables.len())?;
                for variable in &tail.variables {
                    write_variable(&mut cursor, variable)?;
                }
                cursor.write_count("event_block_count", tail.event_blocks.len())?;
                for block in &tail.event_blocks {
                    write_event_block(&mut cursor, block)?;
                }
                cursor.write_count("footer_count", tail.footer_entries.len())?;
                for entry in &tail.footer_entries {
                    write_footer(&mut cursor, entry)?;
                }
                let start = cursor.position();
                cursor.write(source_range(&tail.suffix_source, &tail.suffix_range))?;
                OpaqueRange::Parsed(start..cursor.position())
            }
            STGTail::Raw { source, range, .. } => {
                let start = cursor.position();
                cursor.write(source_range(source, range))?;
                OpaqueRange::Raw(start..cursor.position())
            }
        };
        cursor.finish()?;
        range
    };

    Ok(EncodedSTG {
        bytes: bytes.into_vec(),
        opaque_range,
    })
}

fn write_header(cursor: &mut SliceCursor<'_>, header: &StgHeader) -> Result<(), FormatError> {
    cursor.write_i32(header.reserved_0)?;
    cursor.write_u32(header.unknown_1)?;
    cursor.write_u32(header.unknown_2)?;
    cursor.write_u32(header.unknown_3)?;
    cursor.write(&header.reserved_1)?;
    cursor.write_u32(header.unknown_4)?;
    cursor.write(&header.reserved_2)?;
    cursor.write(&header.map_filename)?;
    cursor.write(&header.bitmap_filename)?;
    cursor.write(&header.default_camera)?;
    cursor.write(&header.user_camera)?;
    cursor.write(&header.settings_file)?;
    cursor.write(&header.sky_effects)?;
    cursor.write(&header.ai_script)?;
    cursor.write(&header.padding_208)?;
    cursor.write(&header.cubemap_texture)?;
    cursor.write(&header.config_data)
}

fn write_unit(cursor: &mut SliceCursor<'_>, unit: &UnitBlock) -> Result<(), FormatError> {
    cursor.write(&unit.name)?;
    cursor.write_u32(unit.unique_id)?;
    cursor.write_u8(unit.ucd)?;
    cursor.write_u8(unit.is_hero)?;
    cursor.write_u8(unit.is_enabled)?;
    cursor.write_u8(unit.reserved_27)?;
    cursor.write_f32(unit.leader_hp_override)?;
    cursor.write_f32(unit.unit_hp_override)?;
    cursor.write_f32(unit.unknown_30)?;
    cursor.write(&unit.reserved_34)?;
    cursor.write_f32(unit.pos_x)?;
    cursor.write_f32(unit.pos_y)?;
    cursor.write_u8(unit.facing_direction)?;
    cursor.write_u8(unit.extra_flags_1)?;
    cursor.write_u8(unit.extra_flags_2)?;
    cursor.write_u8(unit.category)?;
    cursor.write_i32(unit.reserved_50)?;
    cursor.write_u8(unit.leader_job_type)?;
    cursor.write_u8(unit.leader_model_id)?;
    cursor.write_u8(unit.leader_worldmap_id)?;
    cursor.write_u8(unit.leader_level)?;
    cursor.write(&unit.leader_skills)?;
    for ability in unit.leader_abilities {
        cursor.write_i32(ability)?;
    }
    cursor.write_u32(unit.officer_count)?;
    cursor.write_u8(unit.officer1_job_type)?;
    cursor.write_u8(unit.officer1_model_id)?;
    cursor.write_u8(unit.officer1_worldmap_id)?;
    cursor.write_u8(unit.officer1_level)?;
    cursor.write(&unit.officer1_data)?;
    cursor.write_u8(unit.officer2_job_type)?;
    cursor.write_u8(unit.officer2_model_id)?;
    cursor.write_u8(unit.officer2_worldmap_id)?;
    cursor.write_u8(unit.officer2_level)?;
    cursor.write(&unit.officer2_data)?;
    cursor.write(&unit.padding_180)?;
    cursor.write_u32(unit.animation_config)?;
    cursor.write_u32(unit.grid_x)?;
    cursor.write_u32(unit.grid_y)?;
    cursor.write(&unit.reserved_198)?;
    cursor.write_i32(unit.troop_info_index)?;
    cursor.write_u32(unit.formation_type)?;
    for value in unit.stat_overrides {
        cursor.write_f32(value)?;
    }
    Ok(())
}

fn write_area(cursor: &mut SliceCursor<'_>, area: &AreaEntry) -> Result<(), FormatError> {
    cursor.write(&area.description)?;
    cursor.write_u32(area.unknown_20)?;
    cursor.write_u32(area.unknown_24)?;
    cursor.write(&area.unknown_28)?;
    cursor.write_u32(area.area_id)?;
    cursor.write_f32(area.bound_x1)?;
    cursor.write_f32(area.bound_y1)?;
    cursor.write_f32(area.bound_x2)?;
    cursor.write_f32(area.bound_y2)
}

fn write_variable(cursor: &mut SliceCursor<'_>, variable: &StgVariable) -> Result<(), FormatError> {
    cursor.write(&variable.name)?;
    cursor.write_u32(variable.variable_id)?;
    write_parameter(cursor, &variable.initial_value)
}

fn write_event_block(cursor: &mut SliceCursor<'_>, block: &EventBlock) -> Result<(), FormatError> {
    cursor.write_u32(block.block_header)?;
    cursor.write_count("event_count", block.events.len())?;
    for event in &block.events {
        write_event(cursor, event)?;
    }
    Ok(())
}

fn write_event(cursor: &mut SliceCursor<'_>, event: &StgEvent) -> Result<(), FormatError> {
    cursor.write(&event.description)?;
    cursor.write_u32(event.event_id)?;
    cursor.write_count("condition_count", event.conditions.len())?;
    for condition in &event.conditions {
        write_condition(cursor, condition)?;
    }
    cursor.write_count("action_count", event.actions.len())?;
    for action in &event.actions {
        write_action(cursor, action)?;
    }
    Ok(())
}

fn write_condition(
    cursor: &mut SliceCursor<'_>,
    condition: &StgCondition,
) -> Result<(), FormatError> {
    write_script(cursor, condition.type_id, &condition.params)
}

fn write_action(cursor: &mut SliceCursor<'_>, action: &StgAction) -> Result<(), FormatError> {
    write_script(cursor, action.type_id, &action.params)
}

fn write_script(
    cursor: &mut SliceCursor<'_>,
    type_id: u32,
    parameters: &[StgParamValue],
) -> Result<(), FormatError> {
    cursor.write_u32(type_id)?;
    cursor.write_count("param_count", parameters.len())?;
    for parameter in parameters {
        write_parameter(cursor, parameter)?;
    }
    Ok(())
}

fn write_parameter(
    cursor: &mut SliceCursor<'_>,
    parameter: &StgParamValue,
) -> Result<(), FormatError> {
    cursor.write_u32(parameter.type_tag)?;
    match parameter_payload(parameter)? {
        ParameterPayload::I32(value) => cursor.write_i32(value),
        ParameterPayload::F32(value) => cursor.write_f32(value),
        ParameterPayload::String(value) => {
            cursor.write_count("length", value.len())?;
            cursor.write(value)
        }
    }
}

fn write_footer(cursor: &mut SliceCursor<'_>, footer: &FooterEntry) -> Result<(), FormatError> {
    cursor.write_u32(footer.slot_data_1)?;
    cursor.write_u32(footer.slot_data_2)
}

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

fn validate_counts(model: &STGModel) -> Result<(), FormatError> {
    checked_count("unit_count", model.units.len())?;
    let STGTail::Parsed(tail) = &model.tail else {
        return Ok(());
    };
    checked_count("area_count", tail.areas.len())?;
    checked_count("variable_count", tail.variables.len())?;
    for variable in &tail.variables {
        validate_parameter(&variable.initial_value)?;
    }
    checked_count("event_block_count", tail.event_blocks.len())?;
    for block in &tail.event_blocks {
        validate_event_block(block)?;
    }
    checked_count("footer_count", tail.footer_entries.len())?;
    Ok(())
}

fn validate_event_block(block: &EventBlock) -> Result<(), FormatError> {
    checked_count("event_count", block.events.len())?;
    for event in &block.events {
        checked_count("condition_count", event.conditions.len())?;
        for condition in &event.conditions {
            validate_script("params", &condition.params)?;
        }
        checked_count("action_count", event.actions.len())?;
        for action in &event.actions {
            validate_script("params", &action.params)?;
        }
    }
    Ok(())
}

fn validate_script(field: &'static str, parameters: &[StgParamValue]) -> Result<(), FormatError> {
    checked_count(field, parameters.len())?;
    for parameter in parameters {
        validate_parameter(parameter)?;
    }
    Ok(())
}

fn validate_parameter(parameter: &StgParamValue) -> Result<(), FormatError> {
    if let ParameterPayload::String(value) = parameter_payload(parameter)? {
        checked_count("length", value.len())?;
    }
    Ok(())
}

enum ParameterPayload<'a> {
    I32(i32),
    F32(f32),
    String(&'a [u8]),
}

fn parameter_payload(parameter: &StgParamValue) -> Result<ParameterPayload<'_>, FormatError> {
    match (parameter.type_tag, &parameter.value) {
        (0 | 3, StgParamValueValue::I32(value)) => Ok(ParameterPayload::I32(*value)),
        (1, StgParamValueValue::F32(value)) => Ok(ParameterPayload::F32(*value)),
        (2, StgParamValueValue::StgStringParam(value)) => {
            Ok(ParameterPayload::String(&value.value))
        }
        (0..=3, _) => Err(cleave_error(kuf_stg::Error::MatchType {
            field: "value",
            tag: i128::from(parameter.type_tag),
        })),
        _ => Err(cleave_error(kuf_stg::Error::UnknownTag {
            struct_name: "StgParamValue",
            field: "value",
            value: i128::from(parameter.type_tag),
        })),
    }
}

fn checked_count(field: &'static str, count: usize) -> Result<u32, FormatError> {
    u32::try_from(count).map_err(|_| {
        cleave_error(kuf_stg::Error::LengthOverflow {
            field,
            value: count.to_string(),
            target: "u32",
        })
    })
}

fn cleave_error(error: kuf_stg::Error) -> FormatError {
    FormatError::STGEncode(STGEncodeError::Cleave(error.into()))
}

struct SliceCursor<'a> {
    bytes: &'a mut [u8],
    position: usize,
}

impl<'a> SliceCursor<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    const fn position(&self) -> usize {
        self.position
    }

    fn write_count(&mut self, field: &'static str, count: usize) -> Result<(), FormatError> {
        self.write(&checked_count(field, count)?.to_le_bytes())
    }

    fn write_u8(&mut self, value: u8) -> Result<(), FormatError> {
        self.write(&value.to_ne_bytes())
    }

    fn write_u32(&mut self, value: u32) -> Result<(), FormatError> {
        self.write(&value.to_le_bytes())
    }

    fn write_i32(&mut self, value: i32) -> Result<(), FormatError> {
        self.write(&value.to_le_bytes())
    }

    fn write_f32(&mut self, value: f32) -> Result<(), FormatError> {
        self.write(&value.to_le_bytes())
    }

    fn write(&mut self, value: &[u8]) -> Result<(), FormatError> {
        let Some(end) = self.position.checked_add(value.len()) else {
            return Err(FormatError::STGEncode(
                STGEncodeError::LengthArithmeticOverflow,
            ));
        };
        let Some(destination) = self.bytes.get_mut(self.position..end) else {
            return Err(FormatError::STGEncode(
                STGEncodeError::LengthProjectionMismatch {
                    projected: self.bytes.len(),
                    actual: end,
                },
            ));
        };
        destination.copy_from_slice(value);
        self.position = end;
        Ok(())
    }

    fn finish(self) -> Result<(), FormatError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(FormatError::STGEncode(STGEncodeError::CursorMismatch {
                expected: self.bytes.len(),
                actual: self.position,
            }))
        }
    }
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

#[cfg(test)]
mod tests {
    use crate::generated::kuf_stg::File;

    use super::super::{SOURCE_LIMIT, STGDocument, stg_test_support};
    use super::encode;

    #[test]
    fn stg_generated_reference_matches_direct_sink() {
        let fixture = stg_test_support::complete_stg_fixture();
        let mut source = fixture.bytes;
        source.truncate(fixture.offsets.suffix);

        let mut offset = 0;
        let generated =
            File::parse(&source, &mut offset).expect("generated parser accepts fixture");
        assert_eq!(offset, source.len());
        let expected = generated
            .to_bytes()
            .expect("generated reference serializer accepts fixture");

        let document = STGDocument::parse(source).expect("STG document accepts fixture");
        let actual = encode(&document.model, SOURCE_LIMIT)
            .expect("direct serializer accepts fixture")
            .bytes;

        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), actual.len());
    }
}
