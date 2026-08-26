use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    diagnostic::{Diagnostic, DiagnosticLocation, Severity},
    error::{
        FormatError, STGCollection, STGEncodeError, STGTarget, STGTextEncoding, STGTextError,
        STGValueKind,
    },
    generated::kuf_stg::{
        AreaEntry, EventBlock, FooterEntry, StgHeader, StgParamValue, StgParamValueValue,
        StgStringParam, StgVariable, UnitBlock,
    },
};

use super::{
    STGAbilityOwner, STGAreaField, STGAreaFloatField, STGDocument, STGFloatTarget, STGFloatValue,
    STGFooterField, STGHeaderTextField, STGModel, STGMutation, STGNumberTarget, STGParameterTarget,
    STGParsedTail, STGReferenceKind, STGScriptKind, STGScriptTarget, STGSkillField, STGSkillOwner,
    STGTail, STGText, STGTextImage, STGTextPreview, STGTextRestoreFailure, STGTextTarget,
    STGUnitField, STGUnitFloatField, STGValueTarget, catalog, retained_model_bytes,
    text::{self, STGTextImageKind},
    wire,
};

impl STGDocument {
    pub fn number(&self, target: STGNumberTarget) -> Result<i64, FormatError> {
        number_value(&self.model, target)
    }

    pub fn set_number(
        &mut self,
        target: STGNumberTarget,
        value: i64,
    ) -> Result<STGMutation<i64>, FormatError> {
        let previous = match self.preview_number(target, value)? {
            STGMutation::Unchanged => return Ok(STGMutation::Unchanged),
            STGMutation::Changed { previous } => previous,
        };

        let projected = projected_model_bytes(&self.model, super::preflight::MODEL_LIMIT)?;
        let mut prospective = Arc::clone(&self.model);
        assign_number(Arc::make_mut(&mut prospective), target, value)?;
        validate_actual_model(&prospective, projected, super::preflight::MODEL_LIMIT)?;
        self.model = prospective;
        self.revision = Arc::new(());
        Ok(STGMutation::Changed { previous })
    }

    pub fn preview_number(
        &self,
        target: STGNumberTarget,
        value: i64,
    ) -> Result<STGMutation<i64>, FormatError> {
        if target.access() == super::STGFieldAccess::ReadOnly {
            return Err(FormatError::STGReadOnlyTarget {
                target: STGTarget::Number(target),
            });
        }

        let previous = self.number(target)?;
        validate_number(target, value)?;
        if previous == value {
            return Ok(STGMutation::Unchanged);
        }
        Ok(STGMutation::Changed { previous })
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = DiagnosticCollector::new();
        diagnose_units(&self.model.units, &mut diagnostics);

        let tail = match &self.model.tail {
            STGTail::Parsed(tail) => tail,
            STGTail::Raw { failure, .. } => {
                diagnostics.push_required(stg_diagnostic(
                    Severity::Warning,
                    DiagnosticLocation::STGTail {
                        region: failure.region(),
                        offset: failure.offset(),
                    },
                    "STG tail is preserved as raw bytes",
                ));
                return diagnostics.finish();
            }
        };
        diagnose_parsed_tail(&self.model, tail, &mut diagnostics);
        diagnostics.finish()
    }

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

    pub fn preview_text(
        &self,
        target: STGTextTarget,
        value: &str,
    ) -> Result<STGTextPreview, FormatError> {
        self.preview_text_with_limits(
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
        self.restore_text_recoverable(target, image)
            .map_err(|failure| failure.into_parts().0)
    }

    pub fn restore_text_recoverable(
        &mut self,
        target: STGTextTarget,
        image: STGTextImage,
    ) -> Result<STGMutation<STGTextImage>, STGTextRestoreFailure> {
        self.replace_text_recoverable(
            target,
            image,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn preview_text_restore(
        &self,
        target: STGTextTarget,
        image: &STGTextImage,
    ) -> Result<bool, FormatError> {
        self.preview_text_replacement(
            target,
            image,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
        .map(|preview| preview.changed)
    }

    pub fn float(&self, target: STGFloatTarget) -> Result<STGFloatValue, FormatError> {
        float_slot(&self.model, target).map(|value| STGFloatValue::from_bits(value.to_bits()))
    }

    pub fn set_float(
        &mut self,
        target: STGFloatTarget,
        value: STGFloatValue,
    ) -> Result<STGMutation<STGFloatValue>, FormatError> {
        let previous = match self.preview_float(target, value)? {
            STGMutation::Unchanged => return Ok(STGMutation::Unchanged),
            STGMutation::Changed { previous } => previous,
        };

        let projected = projected_model_bytes(&self.model, super::preflight::MODEL_LIMIT)?;
        let mut prospective = Arc::clone(&self.model);
        *float_slot_mut(Arc::make_mut(&mut prospective), target)? = f32::from_bits(value.to_bits());
        validate_actual_model(&prospective, projected, super::preflight::MODEL_LIMIT)?;
        self.model = prospective;
        self.revision = Arc::new(());
        Ok(STGMutation::Changed { previous })
    }

    pub fn preview_float(
        &self,
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
        Ok(STGMutation::Changed { previous })
    }

    fn preview_text_with_limits(
        &self,
        target: STGTextTarget,
        value: &str,
        model_limit: usize,
        output_limit: usize,
        dynamic_length_limit: u32,
    ) -> Result<STGTextPreview, FormatError> {
        let slot = text_slot(&self.model, target)?;
        let current_retained_bytes = slot.image_retained_bytes();
        if slot.text().decoded() == Some(value) {
            return Ok(STGTextPreview::new(
                false,
                current_retained_bytes,
                current_retained_bytes,
            ));
        }
        let replacement_retained_bytes = slot.preview_replacement_retained_bytes(
            &self.model,
            target,
            value,
            model_limit,
            output_limit,
            dynamic_length_limit,
        )?;
        Ok(STGTextPreview::new(
            true,
            current_retained_bytes,
            replacement_retained_bytes,
        ))
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
        self.replace_text_recoverable(target, replacement, model_limit, output_limit)
            .map_err(|failure| failure.into_parts().0)
    }

    fn replace_text_recoverable(
        &mut self,
        target: STGTextTarget,
        replacement: STGTextImage,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGMutation<STGTextImage>, STGTextRestoreFailure> {
        let preview =
            match self.preview_text_replacement(target, &replacement, model_limit, output_limit) {
                Ok(preview) => preview,
                Err(error) => return Err(STGTextRestoreFailure::new(error, replacement)),
            };
        if !preview.changed {
            return Ok(STGMutation::Unchanged);
        }

        let mut prospective = Arc::clone(&self.model);
        let previous = match text_slot_mut(Arc::make_mut(&mut prospective), target) {
            Ok(slot) => slot.replace(target, replacement),
            Err(error) => return Err(STGTextRestoreFailure::new(error, replacement)),
        };
        if let Err(error) =
            validate_actual_model(&prospective, preview.projected_model, model_limit)
        {
            let replacement = recover_text_replacement(&mut prospective, target, previous);
            return Err(STGTextRestoreFailure::new(error, replacement));
        }
        if let Err(error) =
            validate_actual_output(&prospective, preview.projected_output, output_limit)
        {
            let replacement = recover_text_replacement(&mut prospective, target, previous);
            return Err(STGTextRestoreFailure::new(error, replacement));
        }
        self.model = prospective;
        self.revision = Arc::new(());
        Ok(STGMutation::Changed { previous })
    }

    fn preview_text_replacement(
        &self,
        target: STGTextTarget,
        replacement: &STGTextImage,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<TextReplacementPreview, FormatError> {
        let (current_kind, current_dynamic, changed) = {
            let current = text_slot(&self.model, target)?;
            (
                current.kind(),
                current.dynamic_metrics(),
                current.bytes() != replacement.as_bytes(),
            )
        };
        if replacement.target() != target || replacement.kind() != current_kind {
            return Err(FormatError::STGText {
                target,
                source: STGTextError::ImageKindMismatch,
            });
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
        Ok(TextReplacementPreview {
            changed,
            projected_model,
            projected_output,
        })
    }
}

struct TextReplacementPreview {
    changed: bool,
    projected_model: usize,
    projected_output: usize,
}

fn recover_text_replacement(
    prospective: &mut Arc<STGModel>,
    target: STGTextTarget,
    previous: STGTextImage,
) -> STGTextImage {
    let Ok(slot) = text_slot_mut(Arc::make_mut(prospective), target) else {
        unreachable!("validated STG text target disappeared during replacement recovery");
    };
    slot.replace(target, previous)
}

const MAX_STG_DIAGNOSTICS: usize = 4_096;
const STG_DIAGNOSTIC_PAYLOAD_LIMIT: usize = MAX_STG_DIAGNOSTICS - 1;

struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
    omitted: bool,
}

impl DiagnosticCollector {
    const fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            omitted: false,
        }
    }

    fn push(&mut self, diagnostic: Diagnostic) {
        if self.diagnostics.len() < STG_DIAGNOSTIC_PAYLOAD_LIMIT {
            self.diagnostics.push(diagnostic);
        } else {
            self.omitted = true;
        }
    }

    fn push_required(&mut self, diagnostic: Diagnostic) {
        if self.diagnostics.len() == STG_DIAGNOSTIC_PAYLOAD_LIMIT {
            let _ = self.diagnostics.pop();
            self.omitted = true;
        }
        self.diagnostics.push(diagnostic);
    }

    fn finish(mut self) -> Vec<Diagnostic> {
        if self.omitted {
            self.diagnostics.push(stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGDocument,
                "Additional STG diagnostics were omitted",
            ));
        }
        self.diagnostics
    }
}

struct STGReferenceIndex {
    troops: HashSet<u32>,
    areas: HashSet<u32>,
    variables: HashSet<u32>,
    events: HashSet<u32>,
}

impl STGReferenceIndex {
    fn new(model: &STGModel, tail: &STGParsedTail) -> Self {
        Self {
            troops: model.units.iter().map(|unit| unit.unique_id).collect(),
            areas: tail.areas.iter().map(|area| area.area_id).collect(),
            variables: tail
                .variables
                .iter()
                .map(|variable| variable.variable_id)
                .collect(),
            events: tail
                .event_blocks
                .iter()
                .flat_map(|block| block.events.iter().map(|event| event.event_id))
                .collect(),
        }
    }

    fn contains(&self, kind: STGReferenceKind, id: u32) -> bool {
        match kind {
            STGReferenceKind::Troop => self.troops.contains(&id),
            STGReferenceKind::Area => self.areas.contains(&id),
            STGReferenceKind::Variable => self.variables.contains(&id),
            STGReferenceKind::Event | STGReferenceKind::Trigger => self.events.contains(&id),
        }
    }
}

fn diagnose_units(units: &[UnitBlock], diagnostics: &mut DiagnosticCollector) {
    let mut remaining_ids = HashMap::new();
    for unit in units {
        *remaining_ids.entry(unit.unique_id).or_insert(0_usize) += 1;
    }

    for (unit_index, unit) in units.iter().enumerate() {
        let has_later_duplicate = match remaining_ids.get_mut(&unit.unique_id) {
            Some(remaining) => {
                *remaining -= 1;
                *remaining > 0
            }
            None => unreachable!("every STG unit ID was counted before validation"),
        };
        if unit.name.first().copied() == Some(0) {
            diagnostics.push(stg_diagnostic(
                Severity::Warning,
                DiagnosticLocation::STGText(STGTextTarget::UnitName { unit: unit_index }),
                "Unit has no name",
            ));
        }
        if unit.ucd > 3 {
            diagnostics.push(unit_number_diagnostic(
                Severity::Error,
                unit_index,
                STGUnitField::UCD,
                "Invalid UCD value",
            ));
        }
        if unit.leader_level == 0 || unit.leader_level > 99 {
            diagnostics.push(unit_number_diagnostic(
                Severity::Warning,
                unit_index,
                STGUnitField::LeaderLevel,
                "Level outside typical range (1-99)",
            ));
        }
        if unit.leader_worldmap_id != u8::MAX && unit.leader_worldmap_id > 20 {
            diagnostics.push(unit_number_diagnostic(
                Severity::Warning,
                unit_index,
                STGUnitField::LeaderWorldmapID,
                "Worldmap ID may cause post-mission issues",
            ));
        }
        if has_later_duplicate {
            diagnostics.push(unit_number_diagnostic(
                Severity::Error,
                unit_index,
                STGUnitField::UniqueID,
                "Duplicate unique ID",
            ));
        }
        if unit.officer_count > 2 {
            diagnostics.push(unit_number_diagnostic(
                Severity::Error,
                unit_index,
                STGUnitField::OfficerCount,
                "Officer count exceeds maximum of 2",
            ));
        }
    }
}

fn diagnose_parsed_tail(
    model: &STGModel,
    tail: &STGParsedTail,
    diagnostics: &mut DiagnosticCollector,
) {
    diagnose_tail_ids_and_text(tail, diagnostics);
    let references = STGReferenceIndex::new(model, tail);
    diagnose_event_scripts(tail, &references, diagnostics);
}

fn diagnose_tail_ids_and_text(tail: &STGParsedTail, diagnostics: &mut DiagnosticCollector) {
    diagnose_duplicate_ids(
        tail.areas.iter().enumerate().map(|(area, entry)| {
            (
                DiagnosticLocation::STGNumber(STGNumberTarget::Area {
                    area,
                    field: STGAreaField::AreaID,
                }),
                entry.area_id,
            )
        }),
        "Duplicate area ID",
        diagnostics,
    );
    diagnose_duplicate_ids(
        tail.variables.iter().enumerate().map(|(variable, entry)| {
            (
                DiagnosticLocation::STGNumber(STGNumberTarget::VariableID { variable }),
                entry.variable_id,
            )
        }),
        "Duplicate variable ID",
        diagnostics,
    );
    diagnose_duplicate_ids(
        tail.event_blocks
            .iter()
            .enumerate()
            .flat_map(|(block, entry)| {
                entry.events.iter().enumerate().map(move |(event, entry)| {
                    (
                        DiagnosticLocation::STGNumber(STGNumberTarget::EventID { block, event }),
                        entry.event_id,
                    )
                })
            }),
        "Duplicate event ID",
        diagnostics,
    );

    for (variable, entry) in tail.variables.iter().enumerate() {
        diagnose_parameter_text(
            &entry.initial_value,
            STGValueTarget::VariableInitial { variable },
            diagnostics,
        );
    }
}

fn diagnose_event_scripts(
    tail: &STGParsedTail,
    references: &STGReferenceIndex,
    diagnostics: &mut DiagnosticCollector,
) {
    for (block, event_block) in tail.event_blocks.iter().enumerate() {
        for (event, entry) in event_block.events.iter().enumerate() {
            for (script, condition) in entry.conditions.iter().enumerate() {
                diagnose_script(
                    STGScriptTarget {
                        block,
                        event,
                        kind: STGScriptKind::Condition,
                        script,
                    },
                    condition.type_id,
                    &condition.params,
                    references,
                    diagnostics,
                );
            }
            for (script, action) in entry.actions.iter().enumerate() {
                diagnose_script(
                    STGScriptTarget {
                        block,
                        event,
                        kind: STGScriptKind::Action,
                        script,
                    },
                    action.type_id,
                    &action.params,
                    references,
                    diagnostics,
                );
            }
        }
    }
}

fn diagnose_duplicate_ids<I>(
    entries: I,
    message: &'static str,
    diagnostics: &mut DiagnosticCollector,
) where
    I: Clone + Iterator<Item = (DiagnosticLocation, u32)>,
{
    let mut remaining = HashMap::new();
    for (_, id) in entries.clone() {
        *remaining.entry(id).or_insert(0_usize) += 1;
    }
    for (location, id) in entries {
        let has_later_duplicate = match remaining.get_mut(&id) {
            Some(count) => {
                *count -= 1;
                *count > 0
            }
            None => unreachable!("every STG ID was counted before validation"),
        };
        if has_later_duplicate {
            diagnostics.push(stg_diagnostic(Severity::Warning, location, message));
        }
    }
}

fn diagnose_script(
    target: STGScriptTarget,
    type_id: u32,
    parameters: &[StgParamValue],
    references: &STGReferenceIndex,
    diagnostics: &mut DiagnosticCollector,
) {
    let info = match target.kind {
        STGScriptKind::Condition => catalog::condition(type_id),
        STGScriptKind::Action => catalog::action(type_id),
    };
    let unknown_message = match target.kind {
        STGScriptKind::Condition => "Unknown condition type",
        STGScriptKind::Action => "Unknown action type",
    };
    let shape_message = match target.kind {
        STGScriptKind::Condition => "Condition parameter count differs from catalog",
        STGScriptKind::Action => "Action parameter count differs from catalog",
    };
    match info {
        Some(info) => {
            if usize::try_from(info.parameter_count).ok() != Some(parameters.len()) {
                diagnostics.push(stg_diagnostic(
                    Severity::Warning,
                    DiagnosticLocation::STGScript(target),
                    shape_message,
                ));
            }
        }
        None => diagnostics.push(stg_diagnostic(
            Severity::Warning,
            DiagnosticLocation::STGScript(target),
            unknown_message,
        )),
    }

    for (parameter, value) in parameters.iter().enumerate() {
        let value_target = STGValueTarget::ScriptParameter(STGParameterTarget {
            script: target,
            parameter,
        });
        diagnose_parameter_text(value, value_target, diagnostics);

        let Some(reference) = info
            .and_then(|entry| entry.parameter_hints.get(parameter))
            .filter(|hint| !hint.is_empty())
            .and_then(|hint| super::structure::reference_kind(hint))
        else {
            continue;
        };
        let Some(id) = parameter_id_bits(value) else {
            continue;
        };
        if references.contains(reference, id) {
            continue;
        }
        diagnostics.push(stg_diagnostic(
            Severity::Warning,
            DiagnosticLocation::STGNumber(STGNumberTarget::ParameterInteger {
                value: value_target,
            }),
            missing_reference_message(reference),
        ));
    }
}

fn diagnose_parameter_text(
    value: &StgParamValue,
    target: STGValueTarget,
    diagnostics: &mut DiagnosticCollector,
) {
    let StgParamValueValue::StgStringParam(value) = &value.value else {
        return;
    };
    if text::decode(&value.value, STGTextEncoding::CP949)
        .raw()
        .is_some()
    {
        diagnostics.push(stg_diagnostic(
            Severity::Warning,
            DiagnosticLocation::STGText(STGTextTarget::ParameterString { value: target }),
            "String parameter is not valid CP949",
        ));
    }
}

fn parameter_id_bits(value: &StgParamValue) -> Option<u32> {
    match (value.type_tag, &value.value) {
        (0 | 3, StgParamValueValue::I32(value)) => Some(u32::from_ne_bytes(value.to_ne_bytes())),
        _ => None,
    }
}

const fn missing_reference_message(reference: STGReferenceKind) -> &'static str {
    match reference {
        STGReferenceKind::Troop => "Missing troop reference",
        STGReferenceKind::Area => "Missing area reference",
        STGReferenceKind::Variable => "Missing variable reference",
        STGReferenceKind::Event => "Missing event reference",
        STGReferenceKind::Trigger => "Missing trigger reference",
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

    fn image_retained_bytes(self) -> usize {
        match self {
            Self::Fixed32 { .. } | Self::Fixed64 { .. } => STGTextImage::fixed_retained_bytes(),
            Self::Dynamic(value) => STGTextImage::dynamic_retained_bytes(value.value.capacity()),
        }
    }

    fn preview_replacement_retained_bytes(
        self,
        model: &STGModel,
        target: STGTextTarget,
        value: &str,
        model_limit: usize,
        output_limit: usize,
        dynamic_length_limit: u32,
    ) -> Result<usize, FormatError> {
        match self {
            Self::Fixed32 { encoding, .. } => text::fixed_encoded_len::<32>(value, encoding)
                .map(|_| STGTextImage::fixed_retained_bytes())
                .map_err(|source| map_text_error(target, source)),
            Self::Fixed64 { encoding, .. } => text::fixed_encoded_len::<64>(value, encoding)
                .map(|_| STGTextImage::fixed_retained_bytes())
                .map_err(|source| map_text_error(target, source)),
            Self::Dynamic(current) => {
                let length = text::dynamic_encoded_len(value, dynamic_length_limit)
                    .map_err(|source| map_text_error(target, source))?;
                projected_dynamic_model_bytes(
                    model,
                    current.value.capacity(),
                    length,
                    model_limit,
                )?;
                projected_dynamic_output_len(model, current.value.len(), length, output_limit)?;
                Ok(STGTextImage::dynamic_retained_bytes(length))
            }
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

fn number_value(model: &STGModel, target: STGNumberTarget) -> Result<i64, FormatError> {
    let public_target = STGTarget::Number(target);
    match target {
        STGNumberTarget::Unit { unit, field } => {
            item(&model.units, STGCollection::Unit, unit, public_target)
                .map(|unit| unit_number(unit, field))
        }
        STGNumberTarget::Skill {
            unit,
            owner,
            slot,
            field,
        } => {
            let unit = item(&model.units, STGCollection::Unit, unit, public_target)?;
            skill_number(unit, owner, slot, field, public_target)
        }
        STGNumberTarget::Ability { unit, owner, slot } => {
            let unit = item(&model.units, STGCollection::Unit, unit, public_target)?;
            ability_number(unit, owner, slot, public_target)
        }
        STGNumberTarget::Area { area, field } => {
            let area = item(areas(model), STGCollection::Area, area, public_target)?;
            Ok(area_number(area, field))
        }
        STGNumberTarget::VariableID { variable } => item(
            variables(model),
            STGCollection::Variable,
            variable,
            public_target,
        )
        .map(|variable| i64::from(variable.variable_id)),
        STGNumberTarget::EventBlockHeader { block } => item(
            event_blocks(model),
            STGCollection::EventBlock,
            block,
            public_target,
        )
        .map(|block| i64::from(block.block_header)),
        STGNumberTarget::EventID { block, event } => {
            event_ref(model, block, event, public_target).map(|event| i64::from(event.event_id))
        }
        STGNumberTarget::ParameterInteger { value } => {
            parameter_integer(value_ref(model, value, public_target)?, value)
        }
        STGNumberTarget::Footer { entry, field } => {
            let footer = item(
                footer_entries(model),
                STGCollection::FooterEntry,
                entry,
                public_target,
            )?;
            Ok(footer_number(footer, field))
        }
    }
}

fn assign_number(
    model: &mut STGModel,
    target: STGNumberTarget,
    value: i64,
) -> Result<(), FormatError> {
    let public_target = STGTarget::Number(target);
    match target {
        STGNumberTarget::Unit { unit, field } => {
            let unit = item_mut(&mut model.units, STGCollection::Unit, unit, public_target)?;
            assign_unit_number(unit, field, target, value)
        }
        STGNumberTarget::Skill {
            unit,
            owner,
            slot,
            field,
        } => {
            let unit = item_mut(&mut model.units, STGCollection::Unit, unit, public_target)?;
            assign_skill_number(unit, owner, slot, field, public_target, target, value)
        }
        STGNumberTarget::Ability { unit, owner, slot } => {
            let unit = item_mut(&mut model.units, STGCollection::Unit, unit, public_target)?;
            assign_ability_number(unit, owner, slot, public_target, target, value)
        }
        STGNumberTarget::Area { area, field } => {
            let area = item_mut(areas_mut(model), STGCollection::Area, area, public_target)?;
            assign_area_number(area, field, target, value)
        }
        STGNumberTarget::VariableID { variable } => {
            let variable = item_mut(
                variables_mut(model),
                STGCollection::Variable,
                variable,
                public_target,
            )?;
            variable.variable_id = number_u32(target, value)?;
            Ok(())
        }
        STGNumberTarget::EventBlockHeader { block } => {
            let block = item_mut(
                event_blocks_mut(model),
                STGCollection::EventBlock,
                block,
                public_target,
            )?;
            block.block_header = number_u32(target, value)?;
            Ok(())
        }
        STGNumberTarget::EventID { block, event } => {
            event_mut(model, block, event, public_target)?.event_id = number_u32(target, value)?;
            Ok(())
        }
        STGNumberTarget::ParameterInteger {
            value: value_target,
        } => {
            *parameter_integer_mut(value_mut(model, value_target, public_target)?, value_target)? =
                number_i32(target, value)?;
            Ok(())
        }
        STGNumberTarget::Footer { entry, field } => {
            let footer = item_mut(
                footer_entries_mut(model),
                STGCollection::FooterEntry,
                entry,
                public_target,
            )?;
            assign_footer_number(footer, field, target, value)
        }
    }
}

fn validate_number(target: STGNumberTarget, value: i64) -> Result<(), FormatError> {
    let (minimum, maximum) = target.storage_bounds();
    if value < minimum || value > maximum {
        return Err(FormatError::STGNumberOutOfRange {
            target,
            value,
            minimum,
            maximum,
        });
    }
    Ok(())
}

fn unit_number(unit: &UnitBlock, field: STGUnitField) -> i64 {
    match field {
        STGUnitField::UniqueID => i64::from(unit.unique_id),
        STGUnitField::UCD => i64::from(unit.ucd),
        STGUnitField::HeroFlag => i64::from(unit.is_hero),
        STGUnitField::EnabledFlag => i64::from(unit.is_enabled),
        STGUnitField::Reserved27 => i64::from(unit.reserved_27),
        STGUnitField::FacingDirection => i64::from(unit.facing_direction),
        STGUnitField::ExtraFlags1 => i64::from(unit.extra_flags_1),
        STGUnitField::ExtraFlags2 => i64::from(unit.extra_flags_2),
        STGUnitField::Category => i64::from(unit.category),
        STGUnitField::Reserved50 => i64::from(unit.reserved_50),
        STGUnitField::LeaderJobType => i64::from(unit.leader_job_type),
        STGUnitField::LeaderModelID => i64::from(unit.leader_model_id),
        STGUnitField::LeaderWorldmapID => i64::from(unit.leader_worldmap_id),
        STGUnitField::LeaderLevel => i64::from(unit.leader_level),
        STGUnitField::OfficerCount => i64::from(unit.officer_count),
        STGUnitField::Officer1JobType => i64::from(unit.officer1_job_type),
        STGUnitField::Officer1ModelID => i64::from(unit.officer1_model_id),
        STGUnitField::Officer1WorldmapID => i64::from(unit.officer1_worldmap_id),
        STGUnitField::Officer1Level => i64::from(unit.officer1_level),
        STGUnitField::Officer2JobType => i64::from(unit.officer2_job_type),
        STGUnitField::Officer2ModelID => i64::from(unit.officer2_model_id),
        STGUnitField::Officer2WorldmapID => i64::from(unit.officer2_worldmap_id),
        STGUnitField::Officer2Level => i64::from(unit.officer2_level),
        STGUnitField::AnimationConfig => i64::from(unit.animation_config),
        STGUnitField::GridX => i64::from(unit.grid_x),
        STGUnitField::GridY => i64::from(unit.grid_y),
        STGUnitField::TroopInfoIndex => i64::from(unit.troop_info_index),
        STGUnitField::FormationType => i64::from(unit.formation_type),
    }
}

fn assign_unit_number(
    unit: &mut UnitBlock,
    field: STGUnitField,
    target: STGNumberTarget,
    value: i64,
) -> Result<(), FormatError> {
    match field {
        STGUnitField::UniqueID => unit.unique_id = number_u32(target, value)?,
        STGUnitField::UCD => unit.ucd = number_u8(target, value)?,
        STGUnitField::HeroFlag => unit.is_hero = number_u8(target, value)?,
        STGUnitField::EnabledFlag => unit.is_enabled = number_u8(target, value)?,
        STGUnitField::FacingDirection => unit.facing_direction = number_u8(target, value)?,
        STGUnitField::LeaderJobType => unit.leader_job_type = number_u8(target, value)?,
        STGUnitField::LeaderModelID => unit.leader_model_id = number_u8(target, value)?,
        STGUnitField::LeaderWorldmapID => unit.leader_worldmap_id = number_u8(target, value)?,
        STGUnitField::LeaderLevel => unit.leader_level = number_u8(target, value)?,
        STGUnitField::OfficerCount => unit.officer_count = number_u32(target, value)?,
        STGUnitField::Officer1JobType => unit.officer1_job_type = number_u8(target, value)?,
        STGUnitField::Officer1ModelID => unit.officer1_model_id = number_u8(target, value)?,
        STGUnitField::Officer1WorldmapID => unit.officer1_worldmap_id = number_u8(target, value)?,
        STGUnitField::Officer1Level => unit.officer1_level = number_u8(target, value)?,
        STGUnitField::Officer2JobType => unit.officer2_job_type = number_u8(target, value)?,
        STGUnitField::Officer2ModelID => unit.officer2_model_id = number_u8(target, value)?,
        STGUnitField::Officer2WorldmapID => unit.officer2_worldmap_id = number_u8(target, value)?,
        STGUnitField::Officer2Level => unit.officer2_level = number_u8(target, value)?,
        STGUnitField::AnimationConfig => unit.animation_config = number_u32(target, value)?,
        STGUnitField::GridX => unit.grid_x = number_u32(target, value)?,
        STGUnitField::GridY => unit.grid_y = number_u32(target, value)?,
        STGUnitField::TroopInfoIndex => unit.troop_info_index = number_i32(target, value)?,
        STGUnitField::FormationType => unit.formation_type = number_u32(target, value)?,
        STGUnitField::Reserved27
        | STGUnitField::ExtraFlags1
        | STGUnitField::ExtraFlags2
        | STGUnitField::Category
        | STGUnitField::Reserved50 => {
            return Err(FormatError::STGReadOnlyTarget {
                target: STGTarget::Number(target),
            });
        }
    }
    Ok(())
}

fn skill_number(
    unit: &UnitBlock,
    owner: STGSkillOwner,
    slot: usize,
    field: STGSkillField,
    target: STGTarget,
) -> Result<i64, FormatError> {
    validate_collection_index(slot, 4, STGCollection::Skill, target)?;
    let offset = slot * 2
        + match field {
            STGSkillField::ID => 0,
            STGSkillField::Level => 1,
        };
    let data = skill_data(unit, owner);
    let Some(value) = data.get(offset) else {
        unreachable!("validated STG skill offset is outside its fixed wire field");
    };
    Ok(i64::from(*value))
}

fn assign_skill_number(
    unit: &mut UnitBlock,
    owner: STGSkillOwner,
    slot: usize,
    field: STGSkillField,
    public_target: STGTarget,
    target: STGNumberTarget,
    value: i64,
) -> Result<(), FormatError> {
    validate_collection_index(slot, 4, STGCollection::Skill, public_target)?;
    let offset = slot * 2
        + match field {
            STGSkillField::ID => 0,
            STGSkillField::Level => 1,
        };
    let data = skill_data_mut(unit, owner);
    let Some(slot) = data.get_mut(offset) else {
        unreachable!("validated STG skill offset is outside its fixed wire field");
    };
    *slot = number_u8(target, value)?;
    Ok(())
}

fn skill_data(unit: &UnitBlock, owner: STGSkillOwner) -> &[u8] {
    match owner {
        STGSkillOwner::Leader => &unit.leader_skills,
        STGSkillOwner::Officer1 => &unit.officer1_data,
        STGSkillOwner::Officer2 => &unit.officer2_data,
    }
}

fn skill_data_mut(unit: &mut UnitBlock, owner: STGSkillOwner) -> &mut [u8] {
    match owner {
        STGSkillOwner::Leader => &mut unit.leader_skills,
        STGSkillOwner::Officer1 => &mut unit.officer1_data,
        STGSkillOwner::Officer2 => &mut unit.officer2_data,
    }
}

fn ability_number(
    unit: &UnitBlock,
    owner: STGAbilityOwner,
    slot: usize,
    target: STGTarget,
) -> Result<i64, FormatError> {
    match owner {
        STGAbilityOwner::Leader => {
            item(&unit.leader_abilities, STGCollection::Ability, slot, target)
                .map(|value| i64::from(*value))
        }
        STGAbilityOwner::Officer1 => officer_ability(&unit.officer1_data, slot, 23, target),
        STGAbilityOwner::Officer2 => officer_ability(&unit.officer2_data, slot, 19, target),
    }
}

fn assign_ability_number(
    unit: &mut UnitBlock,
    owner: STGAbilityOwner,
    slot: usize,
    public_target: STGTarget,
    target: STGNumberTarget,
    value: i64,
) -> Result<(), FormatError> {
    let value = number_i32(target, value)?;
    match owner {
        STGAbilityOwner::Leader => {
            *item_mut(
                &mut unit.leader_abilities,
                STGCollection::Ability,
                slot,
                public_target,
            )? = value;
            Ok(())
        }
        STGAbilityOwner::Officer1 => {
            assign_officer_ability(&mut unit.officer1_data, slot, 23, public_target, value)
        }
        STGAbilityOwner::Officer2 => {
            assign_officer_ability(&mut unit.officer2_data, slot, 19, public_target, value)
        }
    }
}

fn officer_ability(
    data: &[u8],
    slot: usize,
    count: usize,
    target: STGTarget,
) -> Result<i64, FormatError> {
    validate_collection_index(slot, count, STGCollection::Ability, target)?;
    let start = 8 + slot * 4;
    let Some(bytes) = data.get(start..start + 4) else {
        unreachable!("validated STG officer ability is outside its fixed wire field");
    };
    let Ok(bytes) = <[u8; 4]>::try_from(bytes) else {
        unreachable!("validated STG officer ability does not contain four bytes");
    };
    Ok(i64::from(i32::from_le_bytes(bytes)))
}

fn assign_officer_ability(
    data: &mut [u8],
    slot: usize,
    count: usize,
    target: STGTarget,
    value: i32,
) -> Result<(), FormatError> {
    validate_collection_index(slot, count, STGCollection::Ability, target)?;
    let start = 8 + slot * 4;
    let Some(bytes) = data.get_mut(start..start + 4) else {
        unreachable!("validated STG officer ability is outside its fixed wire field");
    };
    bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn area_number(area: &AreaEntry, field: STGAreaField) -> i64 {
    match field {
        STGAreaField::Unknown20 => i64::from(area.unknown_20),
        STGAreaField::Unknown24 => i64::from(area.unknown_24),
        STGAreaField::AreaID => i64::from(area.area_id),
    }
}

fn assign_area_number(
    area: &mut AreaEntry,
    field: STGAreaField,
    target: STGNumberTarget,
    value: i64,
) -> Result<(), FormatError> {
    match field {
        STGAreaField::AreaID => area.area_id = number_u32(target, value)?,
        STGAreaField::Unknown20 | STGAreaField::Unknown24 => {
            return Err(FormatError::STGReadOnlyTarget {
                target: STGTarget::Number(target),
            });
        }
    }
    Ok(())
}

fn footer_number(footer: &FooterEntry, field: STGFooterField) -> i64 {
    match field {
        STGFooterField::SlotData1 => i64::from(footer.slot_data_1),
        STGFooterField::SlotData2 => i64::from(footer.slot_data_2),
    }
}

fn assign_footer_number(
    footer: &mut FooterEntry,
    field: STGFooterField,
    target: STGNumberTarget,
    value: i64,
) -> Result<(), FormatError> {
    match field {
        STGFooterField::SlotData1 => footer.slot_data_1 = number_u32(target, value)?,
        STGFooterField::SlotData2 => footer.slot_data_2 = number_u32(target, value)?,
    }
    Ok(())
}

fn parameter_integer(value: &StgParamValue, target: STGValueTarget) -> Result<i64, FormatError> {
    let actual = value_kind(value);
    if !matches!(actual, STGValueKind::Integer | STGValueKind::Enum) {
        return Err(FormatError::STGValueKindMismatch {
            target,
            expected: STGValueKind::Integer,
            actual,
        });
    }
    match &value.value {
        StgParamValueValue::I32(value) => Ok(i64::from(*value)),
        StgParamValueValue::F32(_) | StgParamValueValue::StgStringParam(_) => {
            unreachable!("STG integer or enum tag has a non-integer generated payload");
        }
    }
}

fn parameter_integer_mut(
    value: &mut StgParamValue,
    target: STGValueTarget,
) -> Result<&mut i32, FormatError> {
    let actual = value_kind(value);
    if !matches!(actual, STGValueKind::Integer | STGValueKind::Enum) {
        return Err(FormatError::STGValueKindMismatch {
            target,
            expected: STGValueKind::Integer,
            actual,
        });
    }
    match &mut value.value {
        StgParamValueValue::I32(value) => Ok(value),
        StgParamValueValue::F32(_) | StgParamValueValue::StgStringParam(_) => {
            unreachable!("STG integer or enum tag has a non-integer generated payload");
        }
    }
}

fn number_u8(target: STGNumberTarget, value: i64) -> Result<u8, FormatError> {
    u8::try_from(value).map_err(|_| number_out_of_range(target, value))
}

fn number_u32(target: STGNumberTarget, value: i64) -> Result<u32, FormatError> {
    u32::try_from(value).map_err(|_| number_out_of_range(target, value))
}

fn number_i32(target: STGNumberTarget, value: i64) -> Result<i32, FormatError> {
    i32::try_from(value).map_err(|_| number_out_of_range(target, value))
}

fn number_out_of_range(target: STGNumberTarget, value: i64) -> FormatError {
    let (minimum, maximum) = target.storage_bounds();
    FormatError::STGNumberOutOfRange {
        target,
        value,
        minimum,
        maximum,
    }
}

fn validate_collection_index(
    index: usize,
    count: usize,
    collection: STGCollection,
    target: STGTarget,
) -> Result<(), FormatError> {
    if index >= count {
        return Err(FormatError::STGTargetOutOfRange {
            target,
            collection,
            index,
            count,
        });
    }
    Ok(())
}

const fn unit_number_diagnostic(
    severity: Severity,
    unit: usize,
    field: STGUnitField,
    message: &'static str,
) -> Diagnostic {
    stg_diagnostic(
        severity,
        DiagnosticLocation::STGNumber(STGNumberTarget::Unit { unit, field }),
        message,
    )
}

const fn stg_diagnostic(
    severity: Severity,
    location: DiagnosticLocation,
    message: &'static str,
) -> Diagnostic {
    Diagnostic {
        severity,
        location,
        message,
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

fn footer_entries(model: &STGModel) -> &[FooterEntry] {
    match &model.tail {
        STGTail::Parsed(tail) => &tail.footer_entries,
        STGTail::Raw { .. } => &[],
    }
}

fn footer_entries_mut(model: &mut STGModel) -> &mut Vec<FooterEntry> {
    match &mut model.tail {
        STGTail::Parsed(tail) => &mut tail.footer_entries,
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
pub(super) mod tests {
    use std::ops::Range;

    use super::super::stg_test_support::{
        STGFixtureOffsets, complete_stg_fixture, stg_prefix_fixture,
    };
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

    #[test]
    fn every_numeric_mutation_changes_only_its_wire_field() {
        let fixture = complete_stg_fixture();
        let cases = number_wire_cases(fixture.offsets);
        assert_eq!(cases.len(), 129);

        for (target, range) in cases {
            let mut document = parse_test_document(fixture.bytes.clone());
            let before = test_wire_image(&document);
            assert_eq!(before, fixture.bytes);
            let previous = test_number(&document, target);
            let (minimum, maximum) = target.storage_bounds();
            let replacement = if previous == maximum {
                minimum
            } else {
                maximum
            };
            if let Err(error) = document.set_number(target, replacement) {
                panic!("failed to mutate {target:?}: {error}");
            }

            let mut expected = before;
            replace_wire_range(
                &mut expected,
                range,
                &number_wire_bytes(target, replacement),
            );
            assert_eq!(test_wire_image(&document), expected, "{target:?}");
        }
    }

    #[test]
    fn every_float_mutation_changes_only_its_wire_field() {
        let fixture = complete_stg_fixture();
        let cases = float_wire_cases(fixture.offsets);
        assert_eq!(cases.len(), 32);

        for (target, range) in cases {
            let mut document = parse_test_document(fixture.bytes.clone());
            let before = test_wire_image(&document);
            assert_eq!(before, fixture.bytes);
            let previous = match document.float(target) {
                Ok(value) => value,
                Err(error) => panic!("failed to read {target:?}: {error}"),
            };
            let replacement = STGFloatValue::from_bits(previous.to_bits() ^ 0x55aa_33cc);
            if let Err(error) = document.set_float(target, replacement) {
                panic!("failed to mutate {target:?}: {error}");
            }

            let mut expected = before;
            replace_wire_range(&mut expected, range, &replacement.to_bits().to_le_bytes());
            assert_eq!(test_wire_image(&document), expected, "{target:?}");
        }
    }

    #[test]
    fn numeric_prefix_mutation_keeps_the_exact_raw_tail_wire_image() {
        let mut source = stg_prefix_fixture(1);
        let unit_start = source.len() - 544;
        source.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let mut document = parse_test_document(source.clone());
        assert_eq!(test_wire_image(&document), source);

        let target = STGNumberTarget::Unit {
            unit: 0,
            field: STGUnitField::UniqueID,
        };
        if let Err(error) = document.set_number(target, 0xfedc_ba98) {
            panic!("failed to mutate a raw-tail STG prefix: {error}");
        }
        replace_wire_range(
            &mut source,
            unit_start + 32..unit_start + 36,
            &0xfedc_ba98_u32.to_le_bytes(),
        );
        assert_eq!(test_wire_image(&document), source);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact-wire failure matrix audits every STG target boundary together"
    )]
    fn failed_field_mutations_keep_the_complete_wire_image() {
        let source = complete_stg_fixture().bytes;
        let invalid_numbers = [
            STGNumberTarget::Unit {
                unit: 1,
                field: STGUnitField::UniqueID,
            },
            STGNumberTarget::Skill {
                unit: 0,
                owner: STGSkillOwner::Leader,
                slot: 4,
                field: STGSkillField::ID,
            },
            STGNumberTarget::Skill {
                unit: 0,
                owner: STGSkillOwner::Officer1,
                slot: 4,
                field: STGSkillField::ID,
            },
            STGNumberTarget::Skill {
                unit: 0,
                owner: STGSkillOwner::Officer2,
                slot: 4,
                field: STGSkillField::ID,
            },
            STGNumberTarget::Ability {
                unit: 0,
                owner: STGAbilityOwner::Leader,
                slot: 23,
            },
            STGNumberTarget::Ability {
                unit: 0,
                owner: STGAbilityOwner::Officer1,
                slot: 23,
            },
            STGNumberTarget::Ability {
                unit: 0,
                owner: STGAbilityOwner::Officer2,
                slot: 19,
            },
            STGNumberTarget::Area {
                area: 1,
                field: STGAreaField::AreaID,
            },
            STGNumberTarget::VariableID { variable: 4 },
            STGNumberTarget::EventBlockHeader { block: 2 },
            STGNumberTarget::EventID { block: 0, event: 2 },
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::VariableInitial { variable: 4 },
            },
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(super::super::STGParameterTarget {
                    script: super::super::STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Condition,
                        script: 1,
                    },
                    parameter: 0,
                }),
            },
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(super::super::STGParameterTarget {
                    script: super::super::STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Action,
                        script: 1,
                    },
                    parameter: 0,
                }),
            },
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(super::super::STGParameterTarget {
                    script: super::super::STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Action,
                        script: 0,
                    },
                    parameter: 2,
                }),
            },
            STGNumberTarget::Footer {
                entry: 2,
                field: STGFooterField::SlotData1,
            },
        ];
        for target in invalid_numbers {
            assert_failed_wire_unchanged(&source, |document| document.set_number(target, 1));
        }

        let invalid_floats = [
            STGFloatTarget::Unit {
                unit: 1,
                field: STGUnitFloatField::PositionX,
            },
            STGFloatTarget::StatOverride { unit: 0, slot: 22 },
            STGFloatTarget::Area {
                area: 1,
                field: super::super::STGAreaFloatField::BoundX1,
            },
            STGFloatTarget::Parameter {
                value: STGValueTarget::VariableInitial { variable: 4 },
            },
        ];
        for target in invalid_floats {
            assert_failed_wire_unchanged(&source, |document| {
                document.set_float(target, STGFloatValue::from_bits(1))
            });
        }

        for (target, value) in [
            (
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UCD,
                },
                -1,
            ),
            (
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UCD,
                },
                256,
            ),
            (
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UniqueID,
                },
                -1,
            ),
            (
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::UniqueID,
                },
                i64::from(u32::MAX) + 1,
            ),
            (
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::TroopInfoIndex,
                },
                i64::from(i32::MIN) - 1,
            ),
            (
                STGNumberTarget::Unit {
                    unit: 0,
                    field: STGUnitField::TroopInfoIndex,
                },
                i64::from(i32::MAX) + 1,
            ),
        ] {
            assert_failed_wire_unchanged(&source, |document| document.set_number(target, value));
        }

        for target in [
            STGNumberTarget::Unit {
                unit: 0,
                field: STGUnitField::Reserved27,
            },
            STGNumberTarget::Area {
                area: 0,
                field: STGAreaField::Unknown20,
            },
        ] {
            assert_failed_wire_unchanged(&source, |document| document.set_number(target, 1));
        }
        assert_failed_wire_unchanged(&source, |document| {
            document.set_float(
                STGFloatTarget::Unit {
                    unit: 0,
                    field: STGUnitFloatField::Unknown30,
                },
                STGFloatValue::from_bits(1),
            )
        });
        assert_failed_wire_unchanged(&source, |document| {
            document.set_number(
                STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::VariableInitial { variable: 1 },
                },
                1,
            )
        });
        assert_failed_wire_unchanged(&source, |document| {
            document.set_float(
                STGFloatTarget::Parameter {
                    value: STGValueTarget::VariableInitial { variable: 0 },
                },
                STGFloatValue::from_bits(1),
            )
        });

        let mut raw_source = stg_prefix_fixture(1);
        raw_source.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        assert_failed_wire_unchanged(&raw_source, |document| {
            document.set_number(
                STGNumberTarget::Area {
                    area: 0,
                    field: STGAreaField::AreaID,
                },
                1,
            )
        });
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive wire map is easier to audit against the STG schema in one table"
    )]
    fn number_wire_cases(offsets: STGFixtureOffsets) -> Vec<(STGNumberTarget, Range<usize>)> {
        let mut cases = Vec::new();
        let unit = offsets.unit_name;
        for field in STGUnitField::ALL {
            let target = STGNumberTarget::Unit { unit: 0, field };
            if target.access() == super::super::STGFieldAccess::Editable {
                let (relative, width) = unit_number_wire(field);
                cases.push((target, unit + relative..unit + relative + width));
            }
        }

        for owner in STGSkillOwner::ALL {
            let owner_start = unit
                + match owner {
                    STGSkillOwner::Leader => 88,
                    STGSkillOwner::Officer1 => 196,
                    STGSkillOwner::Officer2 => 300,
                };
            for slot in 0..4 {
                for field in STGSkillField::ALL {
                    let relative = slot * 2
                        + match field {
                            STGSkillField::ID => 0,
                            STGSkillField::Level => 1,
                        };
                    cases.push((
                        STGNumberTarget::Skill {
                            unit: 0,
                            owner,
                            slot,
                            field,
                        },
                        owner_start + relative..owner_start + relative + 1,
                    ));
                }
            }
        }

        for owner in STGAbilityOwner::ALL {
            let (owner_start, count) = match owner {
                STGAbilityOwner::Leader => (unit + 96, 23),
                STGAbilityOwner::Officer1 => (unit + 204, 23),
                STGAbilityOwner::Officer2 => (unit + 308, 19),
            };
            for slot in 0..count {
                let start = owner_start + slot * 4;
                cases.push((
                    STGNumberTarget::Ability {
                        unit: 0,
                        owner,
                        slot,
                    },
                    start..start + 4,
                ));
            }
        }

        cases.push((
            STGNumberTarget::Area {
                area: 0,
                field: STGAreaField::AreaID,
            },
            offsets.area_description + 64..offsets.area_description + 68,
        ));
        for (variable, type_offset) in [
            offsets.variable_integer_type,
            offsets.variable_float_type,
            offsets.variable_string_type,
            offsets.variable_enum_type,
        ]
        .into_iter()
        .enumerate()
        {
            cases.push((
                STGNumberTarget::VariableID { variable },
                type_offset - 4..type_offset,
            ));
        }
        for (variable, type_offset) in [
            (0, offsets.variable_integer_type),
            (3, offsets.variable_enum_type),
        ] {
            cases.push((
                STGNumberTarget::ParameterInteger {
                    value: STGValueTarget::VariableInitial { variable },
                },
                type_offset + 4..type_offset + 8,
            ));
        }
        cases.push((
            STGNumberTarget::EventBlockHeader { block: 0 },
            offsets.event_block_count + 4..offsets.event_block_count + 8,
        ));
        cases.push((
            STGNumberTarget::EventBlockHeader { block: 1 },
            offsets.footer_count - 8..offsets.footer_count - 4,
        ));
        cases.push((
            STGNumberTarget::EventID { block: 0, event: 0 },
            offsets.event_description + 64..offsets.event_description + 68,
        ));
        let second_event = offsets.action_enum_type + 8;
        cases.push((
            STGNumberTarget::EventID { block: 0, event: 1 },
            second_event + 64..second_event + 68,
        ));
        cases.push((
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(super::super::STGParameterTarget {
                    script: super::super::STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Condition,
                        script: 0,
                    },
                    parameter: 0,
                }),
            },
            offsets.condition_integer_type + 4..offsets.condition_integer_type + 8,
        ));
        cases.push((
            STGNumberTarget::ParameterInteger {
                value: STGValueTarget::ScriptParameter(super::super::STGParameterTarget {
                    script: super::super::STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Action,
                        script: 0,
                    },
                    parameter: 1,
                }),
            },
            offsets.action_enum_type + 4..offsets.action_enum_type + 8,
        ));
        for entry in 0..2 {
            for field in STGFooterField::ALL {
                let field_offset = match field {
                    STGFooterField::SlotData1 => 0,
                    STGFooterField::SlotData2 => 4,
                };
                let start = offsets.footer_count + 4 + entry * 8 + field_offset;
                cases.push((STGNumberTarget::Footer { entry, field }, start..start + 4));
            }
        }
        cases
    }

    const fn unit_number_wire(field: STGUnitField) -> (usize, usize) {
        match field {
            STGUnitField::UniqueID => (32, 4),
            STGUnitField::UCD => (36, 1),
            STGUnitField::HeroFlag => (37, 1),
            STGUnitField::EnabledFlag => (38, 1),
            STGUnitField::Reserved27 => (39, 1),
            STGUnitField::FacingDirection => (76, 1),
            STGUnitField::ExtraFlags1 => (77, 1),
            STGUnitField::ExtraFlags2 => (78, 1),
            STGUnitField::Category => (79, 1),
            STGUnitField::Reserved50 => (80, 4),
            STGUnitField::LeaderJobType => (84, 1),
            STGUnitField::LeaderModelID => (85, 1),
            STGUnitField::LeaderWorldmapID => (86, 1),
            STGUnitField::LeaderLevel => (87, 1),
            STGUnitField::OfficerCount => (188, 4),
            STGUnitField::Officer1JobType => (192, 1),
            STGUnitField::Officer1ModelID => (193, 1),
            STGUnitField::Officer1WorldmapID => (194, 1),
            STGUnitField::Officer1Level => (195, 1),
            STGUnitField::Officer2JobType => (296, 1),
            STGUnitField::Officer2ModelID => (297, 1),
            STGUnitField::Officer2WorldmapID => (298, 1),
            STGUnitField::Officer2Level => (299, 1),
            STGUnitField::AnimationConfig => (396, 4),
            STGUnitField::GridX => (400, 4),
            STGUnitField::GridY => (404, 4),
            STGUnitField::TroopInfoIndex => (448, 4),
            STGUnitField::FormationType => (452, 4),
        }
    }

    fn float_wire_cases(offsets: STGFixtureOffsets) -> Vec<(STGFloatTarget, Range<usize>)> {
        let mut cases = Vec::new();
        let unit = offsets.unit_name;
        for field in STGUnitFloatField::ALL {
            let target = STGFloatTarget::Unit { unit: 0, field };
            if target.access() == super::super::STGFieldAccess::Editable {
                let relative = match field {
                    STGUnitFloatField::LeaderHPOverride => 40,
                    STGUnitFloatField::UnitHPOverride => 44,
                    STGUnitFloatField::Unknown30 => 48,
                    STGUnitFloatField::PositionX => 68,
                    STGUnitFloatField::PositionY => 72,
                };
                cases.push((target, unit + relative..unit + relative + 4));
            }
        }
        for slot in 0..22 {
            let start = unit + 456 + slot * 4;
            cases.push((
                STGFloatTarget::StatOverride { unit: 0, slot },
                start..start + 4,
            ));
        }
        for field in super::super::STGAreaFloatField::ALL {
            let relative = match field {
                super::super::STGAreaFloatField::BoundX1 => 68,
                super::super::STGAreaFloatField::BoundY1 => 72,
                super::super::STGAreaFloatField::BoundX2 => 76,
                super::super::STGAreaFloatField::BoundY2 => 80,
            };
            let start = offsets.area_description + relative;
            cases.push((STGFloatTarget::Area { area: 0, field }, start..start + 4));
        }
        cases.push((
            STGFloatTarget::Parameter {
                value: STGValueTarget::VariableInitial { variable: 1 },
            },
            offsets.variable_float_type + 4..offsets.variable_float_type + 8,
        ));
        cases.push((
            STGFloatTarget::Parameter {
                value: STGValueTarget::ScriptParameter(super::super::STGParameterTarget {
                    script: super::super::STGScriptTarget {
                        block: 0,
                        event: 0,
                        kind: STGScriptKind::Condition,
                        script: 0,
                    },
                    parameter: 1,
                }),
            },
            offsets.condition_float_type + 4..offsets.condition_float_type + 8,
        ));
        cases
    }

    fn number_wire_bytes(target: STGNumberTarget, value: i64) -> Vec<u8> {
        match target.storage_bounds() {
            (0, 255) => match u8::try_from(value) {
                Ok(value) => vec![value],
                Err(error) => {
                    panic!("test value does not fit the target's u8 wire field: {error}")
                }
            },
            (minimum, maximum)
                if minimum == i64::from(i32::MIN) && maximum == i64::from(i32::MAX) =>
            {
                match i32::try_from(value) {
                    Ok(value) => value.to_le_bytes().to_vec(),
                    Err(error) => {
                        panic!("test value does not fit the target's i32 wire field: {error}")
                    }
                }
            }
            (0, maximum) if maximum == i64::from(u32::MAX) => match u32::try_from(value) {
                Ok(value) => value.to_le_bytes().to_vec(),
                Err(error) => {
                    panic!("test value does not fit the target's u32 wire field: {error}")
                }
            },
            bounds => panic!("unexpected STG numeric wire bounds: {bounds:?}"),
        }
    }

    fn test_number(document: &STGDocument, target: STGNumberTarget) -> i64 {
        match document.number(target) {
            Ok(value) => value,
            Err(error) => panic!("failed to read {target:?}: {error}"),
        }
    }

    fn parse_test_document(bytes: Vec<u8>) -> STGDocument {
        match STGDocument::parse(bytes) {
            Ok(document) => document,
            Err(error) => panic!("test STG document failed to parse: {error}"),
        }
    }

    fn assert_failed_wire_unchanged<T>(
        source: &[u8],
        operation: impl FnOnce(&mut STGDocument) -> Result<T, FormatError>,
    ) {
        let mut document = parse_test_document(source.to_vec());
        let before = test_wire_image(&document);
        match operation(&mut document) {
            Err(_error) => {}
            Ok(_) => panic!("expected STG field mutation to fail"),
        }
        assert_eq!(test_wire_image(&document), before);
    }

    pub(crate) fn test_wire_image(document: &STGDocument) -> Vec<u8> {
        let model = &document.model;
        let mut bytes = Vec::new();
        push_u32(&mut bytes, model.magic);
        append_generated(&mut bytes, model.header.to_bytes(), "header");
        push_count(&mut bytes, model.units.len(), "units");
        for unit in &model.units {
            append_generated(&mut bytes, unit.to_bytes(), "unit");
        }

        match &model.tail {
            STGTail::Parsed(tail) => {
                push_count(&mut bytes, tail.areas.len(), "areas");
                for area in &tail.areas {
                    append_generated(&mut bytes, area.to_bytes(), "area");
                }
                push_count(&mut bytes, tail.variables.len(), "variables");
                for variable in &tail.variables {
                    append_generated(&mut bytes, variable.to_bytes(), "variable");
                }
                push_count(&mut bytes, tail.event_blocks.len(), "event blocks");
                for block in &tail.event_blocks {
                    append_generated(&mut bytes, block.to_bytes(), "event block");
                }
                push_count(&mut bytes, tail.footer_entries.len(), "footer entries");
                for footer in &tail.footer_entries {
                    append_generated(&mut bytes, footer.to_bytes(), "footer entry");
                }
                bytes.extend_from_slice(test_source_range(
                    tail.suffix_source.as_slice(),
                    &tail.suffix_range,
                ));
            }
            STGTail::Raw { source, range, .. } => {
                bytes.extend_from_slice(test_source_range(source.as_slice(), range));
            }
        }
        bytes
    }

    fn append_generated(
        bytes: &mut Vec<u8>,
        generated: Result<Vec<u8>, crate::generated::kuf_stg::Error>,
        region: &str,
    ) {
        match generated {
            Ok(generated) => bytes.extend_from_slice(&generated),
            Err(error) => panic!("failed to encode test STG {region}: {error}"),
        }
    }

    fn push_count(bytes: &mut Vec<u8>, count: usize, region: &str) {
        match u32::try_from(count) {
            Ok(count) => push_u32(bytes, count),
            Err(error) => panic!("test STG {region} count does not fit u32: {error}"),
        }
    }

    fn test_source_range<'a>(source: &'a [u8], range: &Range<usize>) -> &'a [u8] {
        match source.get(range.clone()) {
            Some(bytes) => bytes,
            None => panic!("test STG source range is invalid"),
        }
    }

    fn replace_wire_range(bytes: &mut [u8], range: Range<usize>, replacement: &[u8]) {
        let Some(target) = bytes.get_mut(range) else {
            panic!("test STG wire range is invalid");
        };
        assert_eq!(target.len(), replacement.len());
        target.copy_from_slice(replacement);
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
