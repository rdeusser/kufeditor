use std::sync::Arc;

use crate::{
    error::{
        FormatError, STGCollection, STGEncodeError, STGTarget, STGTextEncoding, STGTextError,
        STGValueKind,
    },
    generated::kuf_stg::{
        AreaEntry, EventBlock, StgHeader, StgParamValue, StgParamValueValue, StgStringParam,
        StgVariable, UnitBlock,
    },
};

use super::{
    STGAreaFloatField, STGDocument, STGFloatTarget, STGFloatValue, STGHeaderTextField, STGModel,
    STGMutation, STGScriptKind, STGTail, STGText, STGTextImage, STGTextTarget, STGUnitFloatField,
    STGValueTarget, retained_model_bytes,
    text::{self, STGTextImageKind},
    wire,
};

impl STGDocument {
    pub fn text(&self, target: STGTextTarget) -> Result<STGText<'_>, FormatError> {
        text_slot(&self.model, target).map(TextSlot::text)
    }

    pub fn set_text(
        &mut self,
        target: STGTextTarget,
        value: String,
    ) -> Result<STGMutation<STGTextImage>, FormatError> {
        self.set_text_with_limits(
            target,
            value,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
            u32::MAX,
        )
    }

    pub fn restore_text(
        &mut self,
        target: STGTextTarget,
        image: STGTextImage,
    ) -> Result<STGMutation<STGTextImage>, FormatError> {
        self.replace_text(
            target,
            image,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn float(&self, target: STGFloatTarget) -> Result<STGFloatValue, FormatError> {
        float_slot(&self.model, target).map(|value| STGFloatValue::from_bits(value.to_bits()))
    }

    pub fn set_float(
        &mut self,
        target: STGFloatTarget,
        value: STGFloatValue,
    ) -> Result<STGMutation<STGFloatValue>, FormatError> {
        if target.access() == super::STGFieldAccess::ReadOnly {
            return Err(FormatError::STGReadOnlyTarget {
                target: STGTarget::Float(target),
            });
        }

        let previous = self.float(target)?;
        if previous == value {
            return Ok(STGMutation::Unchanged);
        }
        let projected = projected_model_bytes(&self.model, super::preflight::MODEL_LIMIT)?;
        let mut prospective = Arc::clone(&self.model);
        *float_slot_mut(Arc::make_mut(&mut prospective), target)? = f32::from_bits(value.to_bits());
        validate_actual_model(&prospective, projected, super::preflight::MODEL_LIMIT)?;
        self.model = prospective;
        Ok(STGMutation::Changed { previous })
    }

    fn set_text_with_limits(
        &mut self,
        target: STGTextTarget,
        value: String,
        model_limit: usize,
        output_limit: usize,
        dynamic_length_limit: u32,
    ) -> Result<STGMutation<STGTextImage>, FormatError> {
        let equal = {
            let current = self.text(target)?;
            current.decoded() == Some(value.as_str())
        };
        if equal {
            return Ok(STGMutation::Unchanged);
        }

        let replacement = text_slot(&self.model, target)?.encode(
            &self.model,
            target,
            value,
            model_limit,
            output_limit,
            dynamic_length_limit,
        )?;
        self.replace_text(target, replacement, model_limit, output_limit)
    }

    fn replace_text(
        &mut self,
        target: STGTextTarget,
        replacement: STGTextImage,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGMutation<STGTextImage>, FormatError> {
        let (current_kind, current_dynamic, is_equal) = {
            let current = text_slot(&self.model, target)?;
            (
                current.kind(),
                current.dynamic_metrics(),
                current.bytes() == replacement.as_bytes(),
            )
        };
        if replacement.target() != target || replacement.kind() != current_kind {
            return Err(FormatError::STGText {
                target,
                source: STGTextError::ImageKindMismatch,
            });
        }
        if is_equal {
            return Ok(STGMutation::Unchanged);
        }

        let (projected_model, projected_output) =
            if let Some((old_capacity, old_length)) = current_dynamic {
                (
                    projected_dynamic_model_bytes(
                        &self.model,
                        old_capacity,
                        replacement.as_bytes().len(),
                        model_limit,
                    )?,
                    projected_dynamic_output_len(
                        &self.model,
                        old_length,
                        replacement.as_bytes().len(),
                        output_limit,
                    )?,
                )
            } else {
                (
                    projected_model_bytes(&self.model, model_limit)?,
                    projected_output_len(&self.model, output_limit)?,
                )
            };

        let mut prospective = Arc::clone(&self.model);
        let previous =
            text_slot_mut(Arc::make_mut(&mut prospective), target)?.replace(target, replacement);
        validate_actual_model(&prospective, projected_model, model_limit)?;
        validate_actual_output(&prospective, projected_output, output_limit)?;
        self.model = prospective;
        Ok(STGMutation::Changed { previous })
    }
}

#[derive(Clone, Copy)]
enum TextSlot<'a> {
    Fixed32 {
        value: &'a [u8; 32],
        encoding: STGTextEncoding,
    },
    Fixed64 {
        value: &'a [u8; 64],
        encoding: STGTextEncoding,
    },
    Dynamic(&'a StgStringParam),
}

impl<'a> TextSlot<'a> {
    fn text(self) -> STGText<'a> {
        match self {
            Self::Fixed32 { value, encoding } => text::decode_fixed(value, encoding),
            Self::Fixed64 { value, encoding } => text::decode_fixed(value, encoding),
            Self::Dynamic(value) => text::decode(&value.value, STGTextEncoding::CP949),
        }
    }

    const fn kind(self) -> STGTextImageKind {
        match self {
            Self::Fixed32 { .. } => STGTextImageKind::Fixed32,
            Self::Fixed64 { .. } => STGTextImageKind::Fixed64,
            Self::Dynamic(_) => STGTextImageKind::Dynamic,
        }
    }

    fn bytes(self) -> &'a [u8] {
        match self {
            Self::Fixed32 { value, .. } => value,
            Self::Fixed64 { value, .. } => value,
            Self::Dynamic(value) => &value.value,
        }
    }

    fn dynamic_metrics(self) -> Option<(usize, usize)> {
        match self {
            Self::Dynamic(value) => Some((value.value.capacity(), value.value.len())),
            Self::Fixed32 { .. } | Self::Fixed64 { .. } => None,
        }
    }

    fn encode(
        self,
        model: &STGModel,
        target: STGTextTarget,
        value: String,
        model_limit: usize,
        output_limit: usize,
        dynamic_length_limit: u32,
    ) -> Result<STGTextImage, FormatError> {
        match self {
            Self::Fixed32 { encoding, .. } => text::encode_fixed(value, encoding)
                .map(|value| STGTextImage::fixed32(target, value))
                .map_err(|source| map_text_error(target, source)),
            Self::Fixed64 { encoding, .. } => text::encode_fixed(value, encoding)
                .map(|value| STGTextImage::fixed64(target, value))
                .map_err(|source| map_text_error(target, source)),
            Self::Dynamic(current) => prepare_dynamic_text(
                model,
                target,
                current,
                value,
                model_limit,
                output_limit,
                dynamic_length_limit,
            ),
        }
    }
}

fn map_text_error(target: STGTextTarget, source: STGTextError) -> FormatError {
    FormatError::STGText { target, source }
}

fn prepare_dynamic_text(
    model: &STGModel,
    target: STGTextTarget,
    current: &StgStringParam,
    value: String,
    model_limit: usize,
    output_limit: usize,
    dynamic_length_limit: u32,
) -> Result<STGTextImage, FormatError> {
    let length = text::dynamic_encoded_len(&value, dynamic_length_limit)
        .map_err(|source| map_text_error(target, source))?;
    projected_dynamic_model_bytes(model, current.value.capacity(), length, model_limit)?;
    projected_dynamic_output_len(model, current.value.len(), length, output_limit)?;
    Ok(STGTextImage::dynamic(
        target,
        text::encode_dynamic(value, length),
    ))
}

enum TextSlotMut<'a> {
    Fixed32(&'a mut [u8; 32]),
    Fixed64(&'a mut [u8; 64]),
    Dynamic(&'a mut StgStringParam),
}

impl TextSlotMut<'_> {
    fn replace(self, target: STGTextTarget, replacement: STGTextImage) -> STGTextImage {
        match self {
            Self::Fixed32(current) => {
                let Some(replacement) = replacement.into_fixed32() else {
                    unreachable!("validated STG text image changed kind");
                };
                STGTextImage::fixed32(target, std::mem::replace(current, replacement))
            }
            Self::Fixed64(current) => {
                let Some(replacement) = replacement.into_fixed64() else {
                    unreachable!("validated STG text image changed kind");
                };
                STGTextImage::fixed64(target, std::mem::replace(current, replacement))
            }
            Self::Dynamic(current) => {
                let Some(replacement) = replacement.into_dynamic() else {
                    unreachable!("validated STG text image changed kind");
                };
                let Ok(length) = u32::try_from(replacement.len()) else {
                    unreachable!("validated STG dynamic text length changed");
                };
                current.length = length;
                STGTextImage::dynamic(target, std::mem::replace(&mut current.value, replacement))
            }
        }
    }
}

fn text_slot(model: &STGModel, target: STGTextTarget) -> Result<TextSlot<'_>, FormatError> {
    match target {
        STGTextTarget::Header(field) => Ok(TextSlot::Fixed64 {
            value: header_text(&model.header, field),
            encoding: STGTextEncoding::UTF8,
        }),
        STGTextTarget::UnitName { unit } => {
            let unit = item(
                &model.units,
                STGCollection::Unit,
                unit,
                STGTarget::Text(target),
            )?;
            Ok(TextSlot::Fixed32 {
                value: &unit.name,
                encoding: STGTextEncoding::CP949,
            })
        }
        STGTextTarget::AreaDescription { area } => {
            let areas = areas(model);
            let area = item(areas, STGCollection::Area, area, STGTarget::Text(target))?;
            Ok(TextSlot::Fixed32 {
                value: &area.description,
                encoding: STGTextEncoding::CP949,
            })
        }
        STGTextTarget::VariableName { variable } => {
            let variables = variables(model);
            let variable = item(
                variables,
                STGCollection::Variable,
                variable,
                STGTarget::Text(target),
            )?;
            Ok(TextSlot::Fixed64 {
                value: &variable.name,
                encoding: STGTextEncoding::CP949,
            })
        }
        STGTextTarget::EventDescription { block, event } => {
            let event = event_ref(model, block, event, STGTarget::Text(target))?;
            Ok(TextSlot::Fixed64 {
                value: &event.description,
                encoding: STGTextEncoding::CP949,
            })
        }
        STGTextTarget::ParameterString { value } => {
            let value = value_ref(model, value, STGTarget::Text(target))?;
            parameter_string(value, target).map(TextSlot::Dynamic)
        }
    }
}

fn text_slot_mut(
    model: &mut STGModel,
    target: STGTextTarget,
) -> Result<TextSlotMut<'_>, FormatError> {
    match target {
        STGTextTarget::Header(field) => Ok(TextSlotMut::Fixed64(header_text_mut(
            &mut model.header,
            field,
        ))),
        STGTextTarget::UnitName { unit } => {
            let unit = item_mut(
                &mut model.units,
                STGCollection::Unit,
                unit,
                STGTarget::Text(target),
            )?;
            Ok(TextSlotMut::Fixed32(&mut unit.name))
        }
        STGTextTarget::AreaDescription { area } => {
            let areas = areas_mut(model);
            let area = item_mut(areas, STGCollection::Area, area, STGTarget::Text(target))?;
            Ok(TextSlotMut::Fixed32(&mut area.description))
        }
        STGTextTarget::VariableName { variable } => {
            let variables = variables_mut(model);
            let variable = item_mut(
                variables,
                STGCollection::Variable,
                variable,
                STGTarget::Text(target),
            )?;
            Ok(TextSlotMut::Fixed64(&mut variable.name))
        }
        STGTextTarget::EventDescription { block, event } => {
            let event = event_mut(model, block, event, STGTarget::Text(target))?;
            Ok(TextSlotMut::Fixed64(&mut event.description))
        }
        STGTextTarget::ParameterString {
            value: target_value,
        } => {
            let value = value_mut(model, target_value, STGTarget::Text(target))?;
            parameter_string_mut(value, target).map(TextSlotMut::Dynamic)
        }
    }
}

fn float_slot(model: &STGModel, target: STGFloatTarget) -> Result<&f32, FormatError> {
    match target {
        STGFloatTarget::Unit { unit, field } => {
            let unit = item(
                &model.units,
                STGCollection::Unit,
                unit,
                STGTarget::Float(target),
            )?;
            Ok(unit_float(unit, field))
        }
        STGFloatTarget::StatOverride { unit, slot } => {
            let unit = item(
                &model.units,
                STGCollection::Unit,
                unit,
                STGTarget::Float(target),
            )?;
            item(
                &unit.stat_overrides,
                STGCollection::StatOverride,
                slot,
                STGTarget::Float(target),
            )
        }
        STGFloatTarget::Area { area, field } => {
            let areas = areas(model);
            let area = item(areas, STGCollection::Area, area, STGTarget::Float(target))?;
            Ok(area_float(area, field))
        }
        STGFloatTarget::Parameter {
            value: target_value,
        } => {
            let value = value_ref(model, target_value, STGTarget::Float(target))?;
            parameter_float(value, target_value)
        }
    }
}

fn float_slot_mut(model: &mut STGModel, target: STGFloatTarget) -> Result<&mut f32, FormatError> {
    match target {
        STGFloatTarget::Unit { unit, field } => {
            let unit = item_mut(
                &mut model.units,
                STGCollection::Unit,
                unit,
                STGTarget::Float(target),
            )?;
            Ok(unit_float_mut(unit, field))
        }
        STGFloatTarget::StatOverride { unit, slot } => {
            let unit = item_mut(
                &mut model.units,
                STGCollection::Unit,
                unit,
                STGTarget::Float(target),
            )?;
            item_mut(
                &mut unit.stat_overrides,
                STGCollection::StatOverride,
                slot,
                STGTarget::Float(target),
            )
        }
        STGFloatTarget::Area { area, field } => {
            let areas = areas_mut(model);
            let area = item_mut(areas, STGCollection::Area, area, STGTarget::Float(target))?;
            Ok(area_float_mut(area, field))
        }
        STGFloatTarget::Parameter {
            value: target_value,
        } => {
            let value = value_mut(model, target_value, STGTarget::Float(target))?;
            parameter_float_mut(value, target_value)
        }
    }
}

fn projected_model_bytes(model: &STGModel, maximum: usize) -> Result<usize, FormatError> {
    let retained = retained_model_bytes(model).unwrap_or(usize::MAX);
    validate_model_limit(retained, maximum)?;
    Ok(retained)
}

fn projected_dynamic_model_bytes(
    model: &STGModel,
    old_capacity: usize,
    new_capacity: usize,
    maximum: usize,
) -> Result<usize, FormatError> {
    let retained = retained_model_bytes(model).unwrap_or(usize::MAX);
    let prospective = retained
        .checked_sub(old_capacity)
        .and_then(|retained| retained.checked_add(new_capacity))
        .unwrap_or(usize::MAX);
    validate_model_limit(prospective, maximum)?;
    Ok(prospective)
}

fn projected_output_len(model: &STGModel, maximum: usize) -> Result<usize, FormatError> {
    let length = wire::encoded_len(model).unwrap_or(usize::MAX);
    validate_output_limit(length, maximum)?;
    Ok(length)
}

fn projected_dynamic_output_len(
    model: &STGModel,
    old_length: usize,
    new_length: usize,
    maximum: usize,
) -> Result<usize, FormatError> {
    let current = wire::encoded_len(model).unwrap_or(usize::MAX);
    let prospective = current
        .checked_sub(old_length)
        .and_then(|length| length.checked_add(new_length))
        .unwrap_or(usize::MAX);
    validate_output_limit(prospective, maximum)?;
    Ok(prospective)
}

fn validate_actual_model(
    model: &STGModel,
    projected: usize,
    maximum: usize,
) -> Result<(), FormatError> {
    let actual = retained_model_bytes(model).unwrap_or(usize::MAX);
    validate_model_limit(actual, maximum)?;
    if actual > projected {
        return Err(FormatError::STGEncode(
            STGEncodeError::ModelProjectionMismatch { projected, actual },
        ));
    }
    Ok(())
}

fn validate_actual_output(
    model: &STGModel,
    projected: usize,
    maximum: usize,
) -> Result<(), FormatError> {
    let actual = wire::encoded_len(model).unwrap_or(usize::MAX);
    validate_output_limit(actual, maximum)?;
    if actual > projected {
        return Err(FormatError::STGEncode(
            STGEncodeError::LengthProjectionMismatch { projected, actual },
        ));
    }
    Ok(())
}

fn validate_model_limit(retained: usize, maximum: usize) -> Result<(), FormatError> {
    if retained > maximum {
        return Err(FormatError::STGEncode(
            STGEncodeError::ModelBudgetExceeded { retained, maximum },
        ));
    }
    Ok(())
}

fn validate_output_limit(length: usize, maximum: usize) -> Result<(), FormatError> {
    if length > maximum {
        return Err(FormatError::STGEncode(STGEncodeError::LengthOverflow {
            length,
            maximum,
        }));
    }
    Ok(())
}

fn header_text(header: &StgHeader, field: STGHeaderTextField) -> &[u8; 64] {
    match field {
        STGHeaderTextField::MapFilename => &header.map_filename,
        STGHeaderTextField::BitmapFilename => &header.bitmap_filename,
        STGHeaderTextField::DefaultCamera => &header.default_camera,
        STGHeaderTextField::UserCamera => &header.user_camera,
        STGHeaderTextField::SettingsFile => &header.settings_file,
        STGHeaderTextField::SkyEffects => &header.sky_effects,
        STGHeaderTextField::AIScript => &header.ai_script,
        STGHeaderTextField::CubemapTexture => &header.cubemap_texture,
    }
}

fn header_text_mut(header: &mut StgHeader, field: STGHeaderTextField) -> &mut [u8; 64] {
    match field {
        STGHeaderTextField::MapFilename => &mut header.map_filename,
        STGHeaderTextField::BitmapFilename => &mut header.bitmap_filename,
        STGHeaderTextField::DefaultCamera => &mut header.default_camera,
        STGHeaderTextField::UserCamera => &mut header.user_camera,
        STGHeaderTextField::SettingsFile => &mut header.settings_file,
        STGHeaderTextField::SkyEffects => &mut header.sky_effects,
        STGHeaderTextField::AIScript => &mut header.ai_script,
        STGHeaderTextField::CubemapTexture => &mut header.cubemap_texture,
    }
}

fn unit_float(unit: &UnitBlock, field: STGUnitFloatField) -> &f32 {
    match field {
        STGUnitFloatField::LeaderHPOverride => &unit.leader_hp_override,
        STGUnitFloatField::UnitHPOverride => &unit.unit_hp_override,
        STGUnitFloatField::Unknown30 => &unit.unknown_30,
        STGUnitFloatField::PositionX => &unit.pos_x,
        STGUnitFloatField::PositionY => &unit.pos_y,
    }
}

fn unit_float_mut(unit: &mut UnitBlock, field: STGUnitFloatField) -> &mut f32 {
    match field {
        STGUnitFloatField::LeaderHPOverride => &mut unit.leader_hp_override,
        STGUnitFloatField::UnitHPOverride => &mut unit.unit_hp_override,
        STGUnitFloatField::Unknown30 => &mut unit.unknown_30,
        STGUnitFloatField::PositionX => &mut unit.pos_x,
        STGUnitFloatField::PositionY => &mut unit.pos_y,
    }
}

fn area_float(area: &AreaEntry, field: STGAreaFloatField) -> &f32 {
    match field {
        STGAreaFloatField::BoundX1 => &area.bound_x1,
        STGAreaFloatField::BoundY1 => &area.bound_y1,
        STGAreaFloatField::BoundX2 => &area.bound_x2,
        STGAreaFloatField::BoundY2 => &area.bound_y2,
    }
}

fn area_float_mut(area: &mut AreaEntry, field: STGAreaFloatField) -> &mut f32 {
    match field {
        STGAreaFloatField::BoundX1 => &mut area.bound_x1,
        STGAreaFloatField::BoundY1 => &mut area.bound_y1,
        STGAreaFloatField::BoundX2 => &mut area.bound_x2,
        STGAreaFloatField::BoundY2 => &mut area.bound_y2,
    }
}

fn parameter_string(
    value: &StgParamValue,
    target: STGTextTarget,
) -> Result<&StgStringParam, FormatError> {
    let actual = value_kind(value);
    if actual != STGValueKind::String {
        return Err(FormatError::STGValueKindMismatch {
            target: text_value_target(target),
            expected: STGValueKind::String,
            actual,
        });
    }
    match &value.value {
        StgParamValueValue::StgStringParam(value) => Ok(value),
        StgParamValueValue::I32(_) | StgParamValueValue::F32(_) => {
            unreachable!("STG string tag has a non-string generated payload");
        }
    }
}

fn parameter_string_mut(
    value: &mut StgParamValue,
    target: STGTextTarget,
) -> Result<&mut StgStringParam, FormatError> {
    let actual = value_kind(value);
    if actual != STGValueKind::String {
        return Err(FormatError::STGValueKindMismatch {
            target: text_value_target(target),
            expected: STGValueKind::String,
            actual,
        });
    }
    match &mut value.value {
        StgParamValueValue::StgStringParam(value) => Ok(value),
        StgParamValueValue::I32(_) | StgParamValueValue::F32(_) => {
            unreachable!("STG string tag has a non-string generated payload");
        }
    }
}

fn parameter_float(value: &StgParamValue, target: STGValueTarget) -> Result<&f32, FormatError> {
    let actual = value_kind(value);
    if actual != STGValueKind::Float {
        return Err(FormatError::STGValueKindMismatch {
            target,
            expected: STGValueKind::Float,
            actual,
        });
    }
    match &value.value {
        StgParamValueValue::F32(value) => Ok(value),
        StgParamValueValue::I32(_) | StgParamValueValue::StgStringParam(_) => {
            unreachable!("STG float tag has a non-float generated payload");
        }
    }
}

fn parameter_float_mut(
    value: &mut StgParamValue,
    target: STGValueTarget,
) -> Result<&mut f32, FormatError> {
    let actual = value_kind(value);
    if actual != STGValueKind::Float {
        return Err(FormatError::STGValueKindMismatch {
            target,
            expected: STGValueKind::Float,
            actual,
        });
    }
    match &mut value.value {
        StgParamValueValue::F32(value) => Ok(value),
        StgParamValueValue::I32(_) | StgParamValueValue::StgStringParam(_) => {
            unreachable!("STG float tag has a non-float generated payload");
        }
    }
}

fn value_kind(value: &StgParamValue) -> STGValueKind {
    match value.type_tag {
        0 => STGValueKind::Integer,
        1 => STGValueKind::Float,
        2 => STGValueKind::String,
        3 => STGValueKind::Enum,
        _ => unreachable!("preflight accepted an unknown STG parameter tag"),
    }
}

fn text_value_target(target: STGTextTarget) -> STGValueTarget {
    match target {
        STGTextTarget::ParameterString { value } => value,
        STGTextTarget::Header(_)
        | STGTextTarget::UnitName { .. }
        | STGTextTarget::AreaDescription { .. }
        | STGTextTarget::VariableName { .. }
        | STGTextTarget::EventDescription { .. } => {
            unreachable!("non-parameter STG text target reached a parameter resolver")
        }
    }
}

fn value_ref(
    model: &STGModel,
    target: STGValueTarget,
    public_target: STGTarget,
) -> Result<&StgParamValue, FormatError> {
    match target {
        STGValueTarget::VariableInitial { variable } => {
            let variables = variables(model);
            item(variables, STGCollection::Variable, variable, public_target)
                .map(|variable| &variable.initial_value)
        }
        STGValueTarget::ScriptParameter(parameter) => {
            let script = parameter.script;
            let event = event_ref(model, script.block, script.event, public_target)?;
            match script.kind {
                STGScriptKind::Condition => {
                    let script = item(
                        &event.conditions,
                        STGCollection::Condition,
                        script.script,
                        public_target,
                    )?;
                    item(
                        &script.params,
                        STGCollection::Parameter,
                        parameter.parameter,
                        public_target,
                    )
                }
                STGScriptKind::Action => {
                    let script = item(
                        &event.actions,
                        STGCollection::Action,
                        script.script,
                        public_target,
                    )?;
                    item(
                        &script.params,
                        STGCollection::Parameter,
                        parameter.parameter,
                        public_target,
                    )
                }
            }
        }
    }
}

fn value_mut(
    model: &mut STGModel,
    target: STGValueTarget,
    public_target: STGTarget,
) -> Result<&mut StgParamValue, FormatError> {
    match target {
        STGValueTarget::VariableInitial { variable } => {
            let variables = variables_mut(model);
            item_mut(variables, STGCollection::Variable, variable, public_target)
                .map(|variable| &mut variable.initial_value)
        }
        STGValueTarget::ScriptParameter(parameter) => {
            let script_target = parameter.script;
            let event = event_mut(
                model,
                script_target.block,
                script_target.event,
                public_target,
            )?;
            match script_target.kind {
                STGScriptKind::Condition => {
                    let script = item_mut(
                        &mut event.conditions,
                        STGCollection::Condition,
                        script_target.script,
                        public_target,
                    )?;
                    item_mut(
                        &mut script.params,
                        STGCollection::Parameter,
                        parameter.parameter,
                        public_target,
                    )
                }
                STGScriptKind::Action => {
                    let script = item_mut(
                        &mut event.actions,
                        STGCollection::Action,
                        script_target.script,
                        public_target,
                    )?;
                    item_mut(
                        &mut script.params,
                        STGCollection::Parameter,
                        parameter.parameter,
                        public_target,
                    )
                }
            }
        }
    }
}

fn event_ref(
    model: &STGModel,
    block: usize,
    event: usize,
    target: STGTarget,
) -> Result<&crate::generated::kuf_stg::StgEvent, FormatError> {
    let blocks = event_blocks(model);
    let block = item(blocks, STGCollection::EventBlock, block, target)?;
    item(&block.events, STGCollection::Event, event, target)
}

fn event_mut(
    model: &mut STGModel,
    block: usize,
    event: usize,
    target: STGTarget,
) -> Result<&mut crate::generated::kuf_stg::StgEvent, FormatError> {
    let blocks = event_blocks_mut(model);
    let block = item_mut(blocks, STGCollection::EventBlock, block, target)?;
    item_mut(&mut block.events, STGCollection::Event, event, target)
}

fn areas(model: &STGModel) -> &[AreaEntry] {
    match &model.tail {
        STGTail::Parsed(tail) => &tail.areas,
        STGTail::Raw { .. } => &[],
    }
}

fn areas_mut(model: &mut STGModel) -> &mut Vec<AreaEntry> {
    match &mut model.tail {
        STGTail::Parsed(tail) => &mut tail.areas,
        STGTail::Raw { .. } => unreachable!("validated raw STG tail became mutable"),
    }
}

fn variables(model: &STGModel) -> &[StgVariable] {
    match &model.tail {
        STGTail::Parsed(tail) => &tail.variables,
        STGTail::Raw { .. } => &[],
    }
}

fn variables_mut(model: &mut STGModel) -> &mut Vec<StgVariable> {
    match &mut model.tail {
        STGTail::Parsed(tail) => &mut tail.variables,
        STGTail::Raw { .. } => unreachable!("validated raw STG tail became mutable"),
    }
}

fn event_blocks(model: &STGModel) -> &[EventBlock] {
    match &model.tail {
        STGTail::Parsed(tail) => &tail.event_blocks,
        STGTail::Raw { .. } => &[],
    }
}

fn event_blocks_mut(model: &mut STGModel) -> &mut Vec<EventBlock> {
    match &mut model.tail {
        STGTail::Parsed(tail) => &mut tail.event_blocks,
        STGTail::Raw { .. } => unreachable!("validated raw STG tail became mutable"),
    }
}

fn item<T>(
    values: &[T],
    collection: STGCollection,
    index: usize,
    target: STGTarget,
) -> Result<&T, FormatError> {
    values.get(index).ok_or(FormatError::STGTargetOutOfRange {
        target,
        collection,
        index,
        count: values.len(),
    })
}

fn item_mut<T>(
    values: &mut [T],
    collection: STGCollection,
    index: usize,
    target: STGTarget,
) -> Result<&mut T, FormatError> {
    let count = values.len();
    values
        .get_mut(index)
        .ok_or(FormatError::STGTargetOutOfRange {
            target,
            collection,
            index,
            count,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_length_limit_is_checked_without_a_large_allocation() {
        let mut document = dynamic_document();
        let target = dynamic_target();
        let result = document.set_text_with_limits(
            target,
            "ab".to_owned(),
            super::super::preflight::MODEL_LIMIT,
            super::super::preflight::SOURCE_LIMIT,
            1,
        );
        match result {
            Err(FormatError::STGText {
                target: actual_target,
                source: STGTextError::DynamicLengthOverflow { length, maximum },
            }) => {
                assert_eq!(actual_target, target);
                assert_eq!(length, 2);
                assert_eq!(maximum, 1);
            }
            Err(other) => panic!("unexpected dynamic-length error: {other}"),
            Ok(_) => panic!("expected dynamic-length failure"),
        }
        assert_eq!(decoded(&document, target), "a");
    }

    #[test]
    fn dynamic_replacement_budget_is_atomic_and_success_retains_exact_capacity() {
        let target = dynamic_target();
        let mut rejected = dynamic_document();
        let Some(retained) = retained_model_bytes(&rejected.model) else {
            panic!("test STG model size overflowed");
        };
        let prospective = retained + 3;
        let maximum = prospective - 1;
        let result = rejected.set_text_with_limits(
            target,
            "abcd".to_owned(),
            maximum,
            super::super::preflight::SOURCE_LIMIT,
            u32::MAX,
        );
        match result {
            Err(FormatError::STGEncode(STGEncodeError::ModelBudgetExceeded {
                retained: actual,
                maximum: actual_maximum,
            })) => {
                assert_eq!(actual, prospective);
                assert_eq!(actual_maximum, maximum);
            }
            Err(other) => panic!("unexpected dynamic-budget error: {other}"),
            Ok(_) => panic!("expected dynamic-budget failure"),
        }
        assert_eq!(decoded(&rejected, target), "a");
        assert_eq!(dynamic_capacity(&rejected), 1);

        let mut accepted = dynamic_document();
        let Some(retained) = retained_model_bytes(&accepted.model) else {
            panic!("accepted STG model size overflowed");
        };
        let exact_limit = retained + 3;
        if let Err(error) = accepted.set_text_with_limits(
            target,
            "abcd".to_owned(),
            exact_limit,
            super::super::preflight::SOURCE_LIMIT,
            u32::MAX,
        ) {
            panic!("exact-capacity dynamic replacement failed: {error}");
        }
        assert_eq!(decoded(&accepted, target), "abcd");
        assert_eq!(dynamic_capacity(&accepted), 4);

        let snapshot = accepted.clone();
        for value in ["b", "abcdefgh", "cc"] {
            if let Err(error) = accepted.set_text(target, value.to_owned()) {
                panic!("repeated exact-capacity replacement failed: {error}");
            }
            assert_eq!(decoded(&accepted, target), value);
            assert_eq!(dynamic_capacity(&accepted), value.len());
        }
        assert_eq!(decoded(&snapshot, target), "abcd");
        assert_eq!(dynamic_capacity(&snapshot), 4);
    }

    #[test]
    fn dynamic_output_limit_is_checked_before_mutation() {
        let mut document = dynamic_document();
        let target = dynamic_target();
        let current_length = document.source.len();
        let prospective = current_length + 3;
        let maximum = prospective - 1;
        let result = document.set_text_with_limits(
            target,
            "abcd".to_owned(),
            super::super::preflight::MODEL_LIMIT,
            maximum,
            u32::MAX,
        );
        match result {
            Err(FormatError::STGEncode(STGEncodeError::LengthOverflow {
                length,
                maximum: actual_maximum,
            })) => {
                assert_eq!(length, prospective);
                assert_eq!(actual_maximum, maximum);
            }
            Err(other) => panic!("unexpected dynamic-output error: {other}"),
            Ok(_) => panic!("expected dynamic-output failure"),
        }
        assert_eq!(decoded(&document, target), "a");
        assert_eq!(dynamic_capacity(&document), 1);
    }

    #[test]
    fn changed_fixed_text_is_zero_filled_after_the_encoded_value() {
        let mut document = dynamic_document();
        let target = STGTextTarget::Header(STGHeaderTextField::MapFilename);
        if let Err(error) = document.set_text(target, "map".to_owned()) {
            panic!("fixed STG replacement failed: {error}");
        }
        let Some(prefix) = document.model.header.map_filename.get(..3) else {
            panic!("fixed STG text prefix is missing");
        };
        let Some(padding) = document.model.header.map_filename.get(3..) else {
            panic!("fixed STG text padding is missing");
        };
        assert_eq!(prefix, b"map");
        assert!(padding.iter().all(|byte| *byte == 0));
    }

    fn dynamic_document() -> STGDocument {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&super::super::MAGIC.to_le_bytes());
        bytes.resize(bytes.len() + 620, 0);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 1);
        bytes.resize(bytes.len() + 64, 0);
        push_u32(&mut bytes, 7);
        push_u32(&mut bytes, 2);
        push_u32(&mut bytes, 1);
        bytes.push(b'a');
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 0);
        match STGDocument::parse(bytes) {
            Ok(document) => document,
            Err(error) => panic!("test STG document failed to parse: {error}"),
        }
    }

    const fn dynamic_target() -> STGTextTarget {
        STGTextTarget::ParameterString {
            value: STGValueTarget::VariableInitial { variable: 0 },
        }
    }

    fn decoded(document: &STGDocument, target: STGTextTarget) -> &str {
        match document.text(target) {
            Ok(STGText::Decoded(value)) => match value {
                std::borrow::Cow::Borrowed(value) => value,
                std::borrow::Cow::Owned(_) => {
                    panic!("ASCII dynamic test text unexpectedly required ownership")
                }
            },
            Ok(STGText::Raw(_)) => panic!("test STG text did not decode"),
            Err(error) => panic!("test STG text lookup failed: {error}"),
        }
    }

    fn dynamic_capacity(document: &STGDocument) -> usize {
        let target = dynamic_target();
        let value_target = text_value_target(target);
        let value = match value_ref(&document.model, value_target, STGTarget::Text(target)) {
            Ok(value) => value,
            Err(error) => panic!("test STG value lookup failed: {error}"),
        };
        match &value.value {
            StgParamValueValue::StgStringParam(value) => value.value.capacity(),
            StgParamValueValue::I32(_) | StgParamValueValue::F32(_) => {
                panic!("test STG value is not a string")
            }
        }
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
