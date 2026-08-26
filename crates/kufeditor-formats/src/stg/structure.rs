use std::{fmt, mem::size_of, sync::Arc};

use sha2::{Digest, Sha256};

use crate::{
    error::{
        FormatError, STGCollection, STGEncodeError, STGStructuralLocation, STGTarget,
        STGTextEncoding, STGValueKind,
    },
    generated::kuf_stg::{
        EventBlock, StgAction, StgCondition, StgEvent as WireEvent, StgParamValue,
        StgParamValueValue, StgStringParam,
    },
};

use super::{
    STGDocument, STGEvent, STGEventBlock, STGEventTarget, STGModel, STGMutation, STGParameter,
    STGParameterTarget, STGParsedTail, STGReferenceKind, STGScript, STGScriptKind, STGScriptTarget,
    STGTail, STGValue, STGValueTarget, catalog, retained_model_bytes, text, wire,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGStructuralEdit {
    InsertEvent {
        target: STGEventTarget,
    },
    RemoveEvent {
        target: STGEventTarget,
    },
    InsertScript {
        target: STGScriptTarget,
        type_id: u32,
    },
    RemoveScript {
        target: STGScriptTarget,
    },
    ChangeScriptType {
        target: STGScriptTarget,
        type_id: u32,
    },
    ChangeValueType {
        target: STGValueTarget,
        kind: STGValueKind,
    },
}

impl STGStructuralEdit {
    pub const fn location(self) -> STGStructuralLocation {
        match self {
            Self::InsertEvent { target } | Self::RemoveEvent { target } => event_location(target),
            Self::InsertScript { target, .. }
            | Self::RemoveScript { target }
            | Self::ChangeScriptType { target, .. } => STGStructuralLocation::Script(target),
            Self::ChangeValueType { target, .. } => STGStructuralLocation::Value(target),
        }
    }
}

#[derive(Clone)]
pub struct STGStructuralPreview {
    lineage: Arc<()>,
    revision: Arc<()>,
    edit: STGStructuralEdit,
    retained_bytes: usize,
    changed: bool,
    model_limit: usize,
    output_limit: usize,
}

impl STGStructuralPreview {
    pub const fn edit(&self) -> STGStructuralEdit {
        self.edit
    }

    pub const fn is_changed(&self) -> bool {
        self.changed
    }

    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

impl fmt::Debug for STGStructuralPreview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("STGStructuralPreview")
            .field("edit", &self.edit)
            .field("retained_bytes", &self.retained_bytes)
            .field("changed", &self.changed)
            .finish_non_exhaustive()
    }
}

pub struct STGStructuralImage {
    lineage: Arc<()>,
    expected_state: Arc<()>,
    result_state: Arc<()>,
    operation: StructuralOperation,
    retained_bytes: usize,
}

#[derive(Debug)]
enum StructuralOperation {
    InsertEvent {
        target: STGEventTarget,
        event: Arc<WireEvent>,
        guard: EventInsertionGuard,
    },
    RemoveEvent {
        target: STGEventTarget,
        guard: EventRemovalGuard,
    },
    InsertScript {
        target: STGScriptTarget,
        script: ScriptImage,
        guard: ScriptInsertionGuard,
    },
    RemoveScript {
        target: STGScriptTarget,
        guard: ScriptRemovalGuard,
    },
    ReplaceScript {
        target: STGScriptTarget,
        expected: StructuralFingerprint,
        replacement: ScriptImage,
    },
    ReplaceValue {
        target: STGValueTarget,
        expected: StructuralFingerprint,
        replacement: Arc<StgParamValue>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StructuralFingerprint([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EventInsertionGuard {
    create_block: bool,
    block_header: u32,
    event_count: usize,
    before_id: Option<u32>,
    after_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EventRemovalGuard {
    remove_created_block: bool,
    block_header: u32,
    event_count: usize,
    before_id: Option<u32>,
    after_id: Option<u32>,
    expected: StructuralFingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScriptInsertionGuard {
    event_id: u32,
    script_count: usize,
    before_id: Option<u32>,
    after_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScriptRemovalGuard {
    event_id: u32,
    script_count: usize,
    before_id: Option<u32>,
    after_id: Option<u32>,
    expected: StructuralFingerprint,
}

#[derive(Debug)]
enum ScriptImage {
    Condition(Arc<StgCondition>),
    Action(Arc<StgAction>),
}

#[derive(Debug)]
pub struct STGStructuralRestoreFailure {
    error: Box<FormatError>,
    image: Box<STGStructuralImage>,
}

impl STGStructuralRestoreFailure {
    fn new(error: FormatError, image: STGStructuralImage) -> Self {
        Self {
            error: Box::new(error),
            image: Box::new(image),
        }
    }

    pub fn into_parts(self) -> (FormatError, STGStructuralImage) {
        (*self.error, *self.image)
    }
}

struct StructuralTransitionFailure {
    error: FormatError,
    operation: StructuralOperation,
}

impl StructuralTransitionFailure {
    const fn new(error: FormatError, operation: StructuralOperation) -> Self {
        Self { error, operation }
    }

    fn into_parts(self) -> (FormatError, StructuralOperation) {
        (self.error, self.operation)
    }
}

impl STGStructuralImage {
    fn new(
        lineage: Arc<()>,
        expected_state: Arc<()>,
        result_state: Arc<()>,
        operation: StructuralOperation,
        retained_bytes: usize,
    ) -> Self {
        Self {
            lineage,
            expected_state,
            result_state,
            operation,
            retained_bytes,
        }
    }

    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub fn location(&self) -> STGStructuralLocation {
        match &self.operation {
            StructuralOperation::InsertEvent { target, .. }
            | StructuralOperation::RemoveEvent { target, .. } => STGStructuralLocation::Event {
                block: target.block,
                event: target.event,
            },
            StructuralOperation::InsertScript { target, .. }
            | StructuralOperation::RemoveScript { target, .. }
            | StructuralOperation::ReplaceScript { target, .. } => {
                STGStructuralLocation::Script(*target)
            }
            StructuralOperation::ReplaceValue { target, .. } => {
                STGStructuralLocation::Value(*target)
            }
        }
    }
}

impl PartialEq for STGStructuralImage {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.lineage, &other.lineage)
            && Arc::ptr_eq(&self.expected_state, &other.expected_state)
            && Arc::ptr_eq(&self.result_state, &other.result_state)
            && self.retained_bytes == other.retained_bytes
            && operation_eq(&self.operation, &other.operation)
    }
}

impl Eq for STGStructuralImage {}

impl fmt::Debug for STGStructuralImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("STGStructuralImage")
            .field("location", &self.location())
            .field("retained_bytes", &self.retained_bytes())
            .finish_non_exhaustive()
    }
}

impl STGDocument {
    pub fn event_block(&self, block: usize) -> Result<STGEventBlock, FormatError> {
        let location = STGStructuralLocation::EventBlock { block };
        let tail = parsed_tail(&self.model, location)?;
        let block = item(
            &tail.event_blocks,
            STGCollection::EventBlock,
            block,
            STGTarget::Structure(location),
        )?;
        Ok(STGEventBlock {
            header: block.block_header,
            event_count: block.events.len(),
        })
    }

    pub fn event(&self, target: STGEventTarget) -> Result<STGEvent<'_>, FormatError> {
        let event = event_ref(
            &self.model,
            target,
            STGTarget::Structure(event_location(target)),
        )?;
        Ok(STGEvent {
            target,
            description: text::decode_fixed(&event.description, STGTextEncoding::CP949),
            id: event.event_id,
            condition_count: event.conditions.len(),
            action_count: event.actions.len(),
        })
    }

    pub fn script(&self, target: STGScriptTarget) -> Result<STGScript, FormatError> {
        let script = script_ref(&self.model, target, STGTarget::Script(target))?;
        let info = script_info(target.kind, script.id());
        Ok(STGScript {
            target,
            id: script.id(),
            name: info.map(|entry| entry.name),
            parameter_count: script.parameters().len(),
            expected_parameter_count: info
                .and_then(|entry| usize::try_from(entry.parameter_count).ok()),
        })
    }

    pub fn parameter(&self, target: STGParameterTarget) -> Result<STGParameter<'_>, FormatError> {
        let script = script_ref(&self.model, target.script, STGTarget::Parameter(target))?;
        let parameter = item(
            script.parameters(),
            STGCollection::Parameter,
            target.parameter,
            STGTarget::Parameter(target),
        )?;
        let hint = script_info(target.script.kind, script.id())
            .and_then(|entry| entry.parameter_hints.get(target.parameter))
            .copied()
            .filter(|hint| !hint.is_empty());
        Ok(STGParameter {
            target,
            hint,
            reference: hint.and_then(reference_kind),
            value: projected_value(parameter),
        })
    }

    pub fn value(&self, target: STGValueTarget) -> Result<STGValue<'_>, FormatError> {
        value_ref(&self.model, target, STGTarget::Value(target)).map(projected_value)
    }

    pub fn preview_structure(
        &self,
        edit: STGStructuralEdit,
    ) -> Result<STGStructuralPreview, FormatError> {
        self.preview_structure_with_limits(
            edit,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn apply_structure_preview(
        &mut self,
        preview: STGStructuralPreview,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        let STGStructuralPreview {
            lineage,
            revision,
            edit,
            retained_bytes,
            changed,
            model_limit,
            output_limit,
        } = preview;
        if !Arc::ptr_eq(&self.lineage, &lineage) {
            return Err(FormatError::STGStructuralLineageMismatch);
        }
        if !Arc::ptr_eq(&self.revision, &revision) {
            return Err(structural_state_mismatch(edit.location()));
        }
        if !changed {
            return Ok(STGMutation::Unchanged);
        }

        let mut prospective = self.clone();
        let operation = operation_for_edit(&prospective.model, edit, output_limit)?;
        let Some(operation) = operation else {
            return Err(FormatError::STGStructuralChargeMismatch {
                projected: retained_bytes,
                actual: 0,
            });
        };
        let mutation =
            prospective.apply_structure(operation, retained_bytes, model_limit, output_limit)?;
        let actual = match &mutation {
            STGMutation::Changed { previous } => previous.retained_bytes(),
            STGMutation::Unchanged => 0,
        };
        if actual != retained_bytes {
            return Err(FormatError::STGStructuralChargeMismatch {
                projected: retained_bytes,
                actual,
            });
        }
        *self = prospective;
        Ok(mutation)
    }

    pub fn insert_event(
        &mut self,
        block: usize,
        event: usize,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.insert_event_with_limits(
            block,
            event,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn remove_event(
        &mut self,
        block: usize,
        event: usize,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.apply_structural_edit_with_limits(
            STGStructuralEdit::RemoveEvent {
                target: STGEventTarget { block, event },
            },
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    /// Inserts a script at `target.script`; the insertion boundary may equal the current count.
    pub fn insert_script(
        &mut self,
        target: STGScriptTarget,
        type_id: u32,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.insert_script_with_limits(
            target,
            type_id,
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn remove_script(
        &mut self,
        target: STGScriptTarget,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.apply_structural_edit_with_limits(
            STGStructuralEdit::RemoveScript { target },
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn change_script_type(
        &mut self,
        target: STGScriptTarget,
        type_id: u32,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.apply_structural_edit_with_limits(
            STGStructuralEdit::ChangeScriptType { target, type_id },
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn change_value_type(
        &mut self,
        target: STGValueTarget,
        kind: STGValueKind,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.apply_structural_edit_with_limits(
            STGStructuralEdit::ChangeValueType { target, kind },
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        )
    }

    pub fn restore_structure(
        &mut self,
        image: STGStructuralImage,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.restore_structure_recoverable(image)
            .map_err(|failure| failure.into_parts().0)
    }

    pub fn restore_structure_recoverable(
        &mut self,
        image: STGStructuralImage,
    ) -> Result<STGMutation<STGStructuralImage>, STGStructuralRestoreFailure> {
        if !Arc::ptr_eq(&self.lineage, &image.lineage) {
            return Err(STGStructuralRestoreFailure::new(
                FormatError::STGStructuralLineageMismatch,
                image,
            ));
        }
        if !Arc::ptr_eq(&self.state, &image.expected_state) {
            return Err(STGStructuralRestoreFailure::new(
                structural_state_mismatch(image.location()),
                image,
            ));
        }
        let STGStructuralImage {
            lineage,
            expected_state,
            result_state,
            operation,
            retained_bytes,
        } = image;
        let inverse = match self.apply_structure_transition_recoverable(
            operation,
            Arc::clone(&result_state),
            super::preflight::MODEL_LIMIT,
            super::preflight::SOURCE_LIMIT,
        ) {
            Ok(inverse) => inverse,
            Err(failure) => {
                let (error, operation) = failure.into_parts();
                return Err(STGStructuralRestoreFailure::new(
                    error,
                    STGStructuralImage {
                        lineage,
                        expected_state,
                        result_state,
                        operation,
                        retained_bytes,
                    },
                ));
            }
        };
        Ok(STGMutation::Changed {
            previous: STGStructuralImage::new(
                Arc::clone(&self.lineage),
                result_state,
                expected_state,
                inverse,
                retained_bytes,
            ),
        })
    }

    fn preview_structure_with_limits(
        &self,
        edit: STGStructuralEdit,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGStructuralPreview, FormatError> {
        let (changed, retained_bytes) = preview_edit(&self.model, edit, model_limit, output_limit)?;
        Ok(STGStructuralPreview {
            lineage: Arc::clone(&self.lineage),
            revision: Arc::clone(&self.revision),
            edit,
            retained_bytes,
            changed,
            model_limit,
            output_limit,
        })
    }

    fn apply_structural_edit_with_limits(
        &mut self,
        edit: STGStructuralEdit,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        let preview = self.preview_structure_with_limits(edit, model_limit, output_limit)?;
        self.apply_structure_preview(preview)
    }

    fn insert_event_with_limits(
        &mut self,
        block: usize,
        event: usize,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.apply_structural_edit_with_limits(
            STGStructuralEdit::InsertEvent {
                target: STGEventTarget { block, event },
            },
            model_limit,
            output_limit,
        )
    }

    fn insert_script_with_limits(
        &mut self,
        target: STGScriptTarget,
        type_id: u32,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        self.apply_structural_edit_with_limits(
            STGStructuralEdit::InsertScript { target, type_id },
            model_limit,
            output_limit,
        )
    }

    fn apply_structure(
        &mut self,
        operation: StructuralOperation,
        retained_bytes: usize,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<STGMutation<STGStructuralImage>, FormatError> {
        let previous_state = Arc::clone(&self.state);
        let next_state = Arc::new(());
        let inverse = self.apply_structure_transition(
            operation,
            Arc::clone(&next_state),
            model_limit,
            output_limit,
        )?;
        Ok(STGMutation::Changed {
            previous: STGStructuralImage::new(
                Arc::clone(&self.lineage),
                next_state,
                previous_state,
                inverse,
                retained_bytes,
            ),
        })
    }

    fn apply_structure_transition(
        &mut self,
        operation: StructuralOperation,
        next_state: Arc<()>,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<StructuralOperation, FormatError> {
        self.apply_structure_transition_recoverable(
            operation,
            next_state,
            model_limit,
            output_limit,
        )
        .map_err(|failure| failure.into_parts().0)
    }

    fn apply_structure_transition_recoverable(
        &mut self,
        operation: StructuralOperation,
        next_state: Arc<()>,
        model_limit: usize,
        output_limit: usize,
    ) -> Result<StructuralOperation, Box<StructuralTransitionFailure>> {
        let (projected_model, projected_output) =
            match projected_metrics(&self.model, &operation, model_limit, output_limit) {
                Ok(metrics) => metrics,
                Err(error) => {
                    return Err(Box::new(StructuralTransitionFailure::new(error, operation)));
                }
            };
        let mut prospective = Arc::clone(&self.model);
        let inverse = apply_validated_operation(Arc::make_mut(&mut prospective), operation);
        if let Err(error) = validate_actual_metrics(
            &prospective,
            projected_model,
            projected_output,
            model_limit,
            output_limit,
        ) {
            let original = apply_validated_operation(Arc::make_mut(&mut prospective), inverse);
            return Err(Box::new(StructuralTransitionFailure::new(error, original)));
        }
        self.model = prospective;
        self.state = next_state;
        self.revision = Arc::new(());
        Ok(inverse)
    }
}

fn preview_edit(
    model: &STGModel,
    edit: STGStructuralEdit,
    model_limit: usize,
    output_limit: usize,
) -> Result<(bool, usize), FormatError> {
    let (delta, retained_bytes) = match edit {
        STGStructuralEdit::InsertEvent { target } => {
            let guard = event_insertion_guard(model, target)?;
            validate_event_insert_count(model, target, guard)?;
            let event = new_event(model)?;
            (
                event_insert_delta(model, target, &event, guard)?,
                event_image_retained_bytes(&event),
            )
        }
        STGStructuralEdit::RemoveEvent { target } => {
            let guard = event_removal_guard(model, target, false)?;
            let event = event_ref(model, target, STGTarget::Structure(event_location(target)))?;
            (
                event_remove_delta(model, target, guard)?,
                event_image_retained_bytes(event),
            )
        }
        STGStructuralEdit::InsertScript { target, type_id } => {
            let guard = script_insertion_guard(model, target)?;
            count_u32(guard.script_count.saturating_add(1))?;
            let parameter_count = catalog_parameter_count(target.kind, type_id, output_limit)?;
            (
                script_insert_shape_delta(model, target, parameter_count)?,
                script_shape_image_retained_bytes(target.kind, parameter_count),
            )
        }
        STGStructuralEdit::RemoveScript { target } => {
            script_removal_guard(model, target)?;
            let script = script_ref(model, target, STGTarget::Script(target))?;
            (
                script_remove_ref_delta(model, target, script)?,
                script_ref_image_retained_bytes(script),
            )
        }
        STGStructuralEdit::ChangeScriptType { target, type_id } => {
            let current = script_ref(model, target, STGTarget::Script(target))?;
            let parameter_count = catalog_parameter_count(target.kind, type_id, output_limit)?;
            if current.id() == type_id && current.parameters().len() == parameter_count {
                return Ok((false, 0));
            }
            let new_dynamic = resized_script_dynamic_bytes(current, parameter_count);
            let new_wire = resized_script_wire_len(current, parameter_count);
            (
                MetricDelta {
                    old_retained: script_ref_dynamic_bytes(current).unwrap_or(usize::MAX),
                    new_retained: new_dynamic,
                    old_wire: script_ref_wire_len(current).unwrap_or(usize::MAX),
                    new_wire,
                },
                replace_script_image_retained_bytes(current, new_dynamic),
            )
        }
        STGStructuralEdit::ChangeValueType { target, kind } => {
            let current = value_ref(model, target, STGTarget::Value(target))?;
            if value_kind(current) == kind {
                return Ok((false, 0));
            }
            let old_dynamic = parameter_dynamic_bytes(current);
            (
                MetricDelta {
                    old_retained: old_dynamic,
                    new_retained: 0,
                    old_wire: parameter_wire_len(current).unwrap_or(usize::MAX),
                    new_wire: 8,
                },
                replace_value_image_retained_bytes(old_dynamic),
            )
        }
    };
    validate_projected_delta(model, delta, model_limit, output_limit)?;
    Ok((true, retained_bytes))
}

fn operation_for_edit(
    model: &STGModel,
    edit: STGStructuralEdit,
    output_limit: usize,
) -> Result<Option<StructuralOperation>, FormatError> {
    match edit {
        STGStructuralEdit::InsertEvent { target } => {
            let guard = event_insertion_guard(model, target)?;
            Ok(Some(StructuralOperation::InsertEvent {
                target,
                event: Arc::new(new_event(model)?),
                guard,
            }))
        }
        STGStructuralEdit::RemoveEvent { target } => {
            let guard = event_removal_guard(model, target, false)?;
            Ok(Some(StructuralOperation::RemoveEvent { target, guard }))
        }
        STGStructuralEdit::InsertScript { target, type_id } => {
            let guard = script_insertion_guard(model, target)?;
            let parameter_count = catalog_parameter_count(target.kind, type_id, output_limit)?;
            Ok(Some(StructuralOperation::InsertScript {
                target,
                script: new_script(target.kind, type_id, parameter_count),
                guard,
            }))
        }
        STGStructuralEdit::RemoveScript { target } => {
            let guard = script_removal_guard(model, target)?;
            Ok(Some(StructuralOperation::RemoveScript { target, guard }))
        }
        STGStructuralEdit::ChangeScriptType { target, type_id } => {
            let current = script_ref(model, target, STGTarget::Script(target))?;
            let parameter_count = catalog_parameter_count(target.kind, type_id, output_limit)?;
            if current.id() == type_id && current.parameters().len() == parameter_count {
                return Ok(None);
            }
            Ok(Some(StructuralOperation::ReplaceScript {
                target,
                expected: script_fingerprint(current),
                replacement: resized_script(current, type_id, parameter_count),
            }))
        }
        STGStructuralEdit::ChangeValueType { target, kind } => {
            let current = value_ref(model, target, STGTarget::Value(target))?;
            if value_kind(current) == kind {
                return Ok(None);
            }
            Ok(Some(StructuralOperation::ReplaceValue {
                target,
                expected: parameter_fingerprint(current),
                replacement: Arc::new(default_value(kind)),
            }))
        }
    }
}

fn new_event(model: &STGModel) -> Result<WireEvent, FormatError> {
    let mut description = [0_u8; 64];
    let label = b"New Event";
    let Some(destination) = description.get_mut(..label.len()) else {
        unreachable!("new STG event label does not fit its fixed field");
    };
    destination.copy_from_slice(label);
    Ok(WireEvent {
        description,
        event_id: smallest_unused_event_id(model)?,
        condition_count: 0,
        conditions: Vec::new(),
        action_count: 0,
        actions: Vec::new(),
    })
}

fn catalog_parameter_count(
    kind: STGScriptKind,
    type_id: u32,
    output_limit: usize,
) -> Result<usize, FormatError> {
    let info = script_info(kind, type_id)
        .ok_or(FormatError::STGUnknownScriptType { kind, id: type_id })?;
    usize::try_from(info.parameter_count).map_err(|_| {
        FormatError::STGEncode(STGEncodeError::LengthOverflow {
            length: usize::MAX,
            maximum: output_limit,
        })
    })
}

fn validate_event_insert_count(
    model: &STGModel,
    target: STGEventTarget,
    guard: EventInsertionGuard,
) -> Result<(), FormatError> {
    if guard.create_block {
        let tail = parsed_tail(model, event_location(target))?;
        count_u32(tail.event_blocks.len().saturating_add(1))?;
    }
    count_u32(guard.event_count.saturating_add(1)).map(|_| ())
}

#[derive(Clone, Copy)]
enum ScriptRef<'a> {
    Condition(&'a StgCondition),
    Action(&'a StgAction),
}

impl<'a> ScriptRef<'a> {
    const fn kind(self) -> STGScriptKind {
        match self {
            Self::Condition(_) => STGScriptKind::Condition,
            Self::Action(_) => STGScriptKind::Action,
        }
    }

    const fn id(self) -> u32 {
        match self {
            Self::Condition(script) => script.type_id,
            Self::Action(script) => script.type_id,
        }
    }

    fn parameters(self) -> &'a [StgParamValue] {
        match self {
            Self::Condition(script) => &script.params,
            Self::Action(script) => &script.params,
        }
    }
}

impl ScriptImage {
    const fn kind(&self) -> STGScriptKind {
        match self {
            Self::Condition(_) => STGScriptKind::Condition,
            Self::Action(_) => STGScriptKind::Action,
        }
    }

    fn parameters(&self) -> &[StgParamValue] {
        match self {
            Self::Condition(script) => &script.params,
            Self::Action(script) => &script.params,
        }
    }
}

fn parsed_tail(
    model: &STGModel,
    location: STGStructuralLocation,
) -> Result<&STGParsedTail, FormatError> {
    match &model.tail {
        STGTail::Parsed(tail) => Ok(tail),
        STGTail::Raw { .. } => Err(FormatError::STGStructureUnavailable { location }),
    }
}

fn parsed_tail_mut(
    model: &mut STGModel,
    location: STGStructuralLocation,
) -> Result<&mut STGParsedTail, FormatError> {
    match &mut model.tail {
        STGTail::Parsed(tail) => Ok(tail),
        STGTail::Raw { .. } => Err(FormatError::STGStructureUnavailable { location }),
    }
}

fn event_ref(
    model: &STGModel,
    target: STGEventTarget,
    public_target: STGTarget,
) -> Result<&WireEvent, FormatError> {
    let tail = parsed_tail(
        model,
        structural_location(public_target, event_location(target)),
    )?;
    let block = item(
        &tail.event_blocks,
        STGCollection::EventBlock,
        target.block,
        public_target,
    )?;
    item(
        &block.events,
        STGCollection::Event,
        target.event,
        public_target,
    )
}

fn event_mut(
    model: &mut STGModel,
    target: STGEventTarget,
    public_target: STGTarget,
) -> Result<&mut WireEvent, FormatError> {
    let tail = parsed_tail_mut(
        model,
        structural_location(public_target, event_location(target)),
    )?;
    let block = item_mut(
        &mut tail.event_blocks,
        STGCollection::EventBlock,
        target.block,
        public_target,
    )?;
    item_mut(
        &mut block.events,
        STGCollection::Event,
        target.event,
        public_target,
    )
}

fn script_ref(
    model: &STGModel,
    target: STGScriptTarget,
    public_target: STGTarget,
) -> Result<ScriptRef<'_>, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        public_target,
    )?;
    match target.kind {
        STGScriptKind::Condition => item(
            &event.conditions,
            STGCollection::Condition,
            target.script,
            public_target,
        )
        .map(ScriptRef::Condition),
        STGScriptKind::Action => item(
            &event.actions,
            STGCollection::Action,
            target.script,
            public_target,
        )
        .map(ScriptRef::Action),
    }
}

fn value_ref(
    model: &STGModel,
    target: STGValueTarget,
    public_target: STGTarget,
) -> Result<&StgParamValue, FormatError> {
    match target {
        STGValueTarget::VariableInitial { variable } => {
            let tail = parsed_tail(model, STGStructuralLocation::Value(target))?;
            item(
                &tail.variables,
                STGCollection::Variable,
                variable,
                public_target,
            )
            .map(|variable| &variable.initial_value)
        }
        STGValueTarget::ScriptParameter(parameter) => {
            let script = script_ref(model, parameter.script, public_target)?;
            item(
                script.parameters(),
                STGCollection::Parameter,
                parameter.parameter,
                public_target,
            )
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
            let tail = parsed_tail_mut(model, STGStructuralLocation::Value(target))?;
            item_mut(
                &mut tail.variables,
                STGCollection::Variable,
                variable,
                public_target,
            )
            .map(|variable| &mut variable.initial_value)
        }
        STGValueTarget::ScriptParameter(parameter) => {
            let event = event_mut(
                model,
                STGEventTarget {
                    block: parameter.script.block,
                    event: parameter.script.event,
                },
                public_target,
            )?;
            let parameters = match parameter.script.kind {
                STGScriptKind::Condition => {
                    &mut item_mut(
                        &mut event.conditions,
                        STGCollection::Condition,
                        parameter.script.script,
                        public_target,
                    )?
                    .params
                }
                STGScriptKind::Action => {
                    &mut item_mut(
                        &mut event.actions,
                        STGCollection::Action,
                        parameter.script.script,
                        public_target,
                    )?
                    .params
                }
            };
            item_mut(
                parameters,
                STGCollection::Parameter,
                parameter.parameter,
                public_target,
            )
        }
    }
}

fn projected_value(value: &StgParamValue) -> STGValue<'_> {
    match (value.type_tag, &value.value) {
        (0, StgParamValueValue::I32(value)) => STGValue::Integer(*value),
        (1, StgParamValueValue::F32(value)) => {
            STGValue::Float(super::STGFloatValue::from_bits(value.to_bits()))
        }
        (2, StgParamValueValue::StgStringParam(value)) => {
            STGValue::String(text::decode(&value.value, STGTextEncoding::CP949))
        }
        (3, StgParamValueValue::I32(value)) => STGValue::Enum(*value),
        _ => unreachable!("preflight accepted an inconsistent STG parameter"),
    }
}

fn script_info(kind: STGScriptKind, id: u32) -> Option<&'static catalog::STGScriptInfo> {
    match kind {
        STGScriptKind::Condition => catalog::condition(id),
        STGScriptKind::Action => catalog::action(id),
    }
}

pub(super) fn reference_kind(hint: &str) -> Option<STGReferenceKind> {
    if hint.contains("TroopID") || matches!(hint, "TargetID" | "AttackerID") {
        Some(STGReferenceKind::Troop)
    } else {
        match hint {
            "AreaID" => Some(STGReferenceKind::Area),
            "VariableID" => Some(STGReferenceKind::Variable),
            "EventID" => Some(STGReferenceKind::Event),
            "TriggerID" => Some(STGReferenceKind::Trigger),
            _ => None,
        }
    }
}

const fn event_location(target: STGEventTarget) -> STGStructuralLocation {
    STGStructuralLocation::Event {
        block: target.block,
        event: target.event,
    }
}

const fn structural_location(
    target: STGTarget,
    fallback: STGStructuralLocation,
) -> STGStructuralLocation {
    match target {
        STGTarget::Script(target) => STGStructuralLocation::Script(target),
        STGTarget::Parameter(target) => STGStructuralLocation::Parameter(target),
        STGTarget::Value(target) => STGStructuralLocation::Value(target),
        STGTarget::Structure(location) => location,
        STGTarget::Number(_) | STGTarget::Float(_) | STGTarget::Text(_) => fallback,
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

fn validate_insertion(
    index: usize,
    count: usize,
    collection: STGCollection,
    target: STGTarget,
) -> Result<(), FormatError> {
    if index <= count {
        Ok(())
    } else {
        Err(FormatError::STGTargetOutOfRange {
            target,
            collection,
            index,
            count,
        })
    }
}

fn event_insertion_guard(
    model: &STGModel,
    target: STGEventTarget,
) -> Result<EventInsertionGuard, FormatError> {
    let location = event_location(target);
    let tail = parsed_tail(model, location)?;
    if tail.event_blocks.is_empty() {
        if target.block != 0 {
            return Err(FormatError::STGTargetOutOfRange {
                target: STGTarget::Structure(location),
                collection: STGCollection::EventBlock,
                index: target.block,
                count: 0,
            });
        }
        validate_insertion(
            target.event,
            0,
            STGCollection::Event,
            STGTarget::Structure(location),
        )?;
        return Ok(EventInsertionGuard {
            create_block: true,
            block_header: 0,
            event_count: 0,
            before_id: None,
            after_id: None,
        });
    }

    let block = item(
        &tail.event_blocks,
        STGCollection::EventBlock,
        target.block,
        STGTarget::Structure(location),
    )?;
    validate_insertion(
        target.event,
        block.events.len(),
        STGCollection::Event,
        STGTarget::Structure(location),
    )?;
    Ok(EventInsertionGuard {
        create_block: false,
        block_header: block.block_header,
        event_count: block.events.len(),
        before_id: target
            .event
            .checked_sub(1)
            .and_then(|index| block.events.get(index))
            .map(|event| event.event_id),
        after_id: block.events.get(target.event).map(|event| event.event_id),
    })
}

fn event_removal_guard(
    model: &STGModel,
    target: STGEventTarget,
    remove_created_block: bool,
) -> Result<EventRemovalGuard, FormatError> {
    let location = event_location(target);
    let tail = parsed_tail(model, location)?;
    let block = item(
        &tail.event_blocks,
        STGCollection::EventBlock,
        target.block,
        STGTarget::Structure(location),
    )?;
    let event = item(
        &block.events,
        STGCollection::Event,
        target.event,
        STGTarget::Structure(location),
    )?;
    Ok(EventRemovalGuard {
        remove_created_block,
        block_header: block.block_header,
        event_count: block.events.len(),
        before_id: target
            .event
            .checked_sub(1)
            .and_then(|index| block.events.get(index))
            .map(|event| event.event_id),
        after_id: target
            .event
            .checked_add(1)
            .and_then(|index| block.events.get(index))
            .map(|event| event.event_id),
        expected: event_fingerprint(event),
    })
}

fn script_insertion_guard(
    model: &STGModel,
    target: STGScriptTarget,
) -> Result<ScriptInsertionGuard, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    )?;
    let count = script_count(event, target.kind);
    validate_insertion(
        target.script,
        count,
        script_collection(target.kind),
        STGTarget::Script(target),
    )?;
    Ok(ScriptInsertionGuard {
        event_id: event.event_id,
        script_count: count,
        before_id: target
            .script
            .checked_sub(1)
            .and_then(|index| script_id_at(event, target.kind, index)),
        after_id: script_id_at(event, target.kind, target.script),
    })
}

fn script_removal_guard(
    model: &STGModel,
    target: STGScriptTarget,
) -> Result<ScriptRemovalGuard, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    )?;
    let count = script_count(event, target.kind);
    if target.script >= count {
        return Err(FormatError::STGTargetOutOfRange {
            target: STGTarget::Script(target),
            collection: script_collection(target.kind),
            index: target.script,
            count,
        });
    }
    let script = script_ref(model, target, STGTarget::Script(target))?;
    Ok(ScriptRemovalGuard {
        event_id: event.event_id,
        script_count: count,
        before_id: target
            .script
            .checked_sub(1)
            .and_then(|index| script_id_at(event, target.kind, index)),
        after_id: target
            .script
            .checked_add(1)
            .and_then(|index| script_id_at(event, target.kind, index)),
        expected: script_fingerprint(script),
    })
}

fn validate_event_insertion_guard(
    model: &STGModel,
    target: STGEventTarget,
    guard: EventInsertionGuard,
) -> Result<(), FormatError> {
    let location = event_location(target);
    let tail = parsed_tail(model, location)?;
    if guard.create_block {
        if target.block == 0 && target.event == 0 && tail.event_blocks.is_empty() {
            return Ok(());
        }
        return Err(structural_state_mismatch(location));
    }
    let Some(block) = tail.event_blocks.get(target.block) else {
        return Err(structural_state_mismatch(location));
    };
    let before_id = target
        .event
        .checked_sub(1)
        .and_then(|index| block.events.get(index))
        .map(|event| event.event_id);
    let after_id = block.events.get(target.event).map(|event| event.event_id);
    if block.block_header == guard.block_header
        && block.events.len() == guard.event_count
        && target.event <= block.events.len()
        && before_id == guard.before_id
        && after_id == guard.after_id
    {
        Ok(())
    } else {
        Err(structural_state_mismatch(location))
    }
}

fn validate_event_removal_guard(
    model: &STGModel,
    target: STGEventTarget,
    guard: EventRemovalGuard,
) -> Result<(), FormatError> {
    let location = event_location(target);
    let tail = parsed_tail(model, location)?;
    let Some(block) = tail.event_blocks.get(target.block) else {
        return Err(structural_state_mismatch(location));
    };
    let before_id = target
        .event
        .checked_sub(1)
        .and_then(|index| block.events.get(index))
        .map(|event| event.event_id);
    let after_id = target
        .event
        .checked_add(1)
        .and_then(|index| block.events.get(index))
        .map(|event| event.event_id);
    let current_matches = block
        .events
        .get(target.event)
        .is_some_and(|event| event_fingerprint(event) == guard.expected);
    let created_block_matches = !guard.remove_created_block
        || (target.block == 0
            && target.event == 0
            && tail.event_blocks.len() == 1
            && block.events.len() == 1);
    if block.block_header == guard.block_header
        && block.events.len() == guard.event_count
        && before_id == guard.before_id
        && after_id == guard.after_id
        && current_matches
        && created_block_matches
    {
        Ok(())
    } else {
        Err(structural_state_mismatch(location))
    }
}

fn validate_script_insertion_guard(
    model: &STGModel,
    target: STGScriptTarget,
    script: &ScriptImage,
    guard: ScriptInsertionGuard,
) -> Result<(), FormatError> {
    let location = STGStructuralLocation::Script(target);
    if script.kind() != target.kind {
        return Err(structural_state_mismatch(location));
    }
    let Some(event) = event_at(model, target.block, target.event) else {
        return Err(structural_state_mismatch(location));
    };
    let count = script_count(event, target.kind);
    let before_id = target
        .script
        .checked_sub(1)
        .and_then(|index| script_id_at(event, target.kind, index));
    let after_id = script_id_at(event, target.kind, target.script);
    if event.event_id == guard.event_id
        && count == guard.script_count
        && target.script <= count
        && before_id == guard.before_id
        && after_id == guard.after_id
    {
        Ok(())
    } else {
        Err(structural_state_mismatch(location))
    }
}

fn validate_script_removal_guard(
    model: &STGModel,
    target: STGScriptTarget,
    guard: ScriptRemovalGuard,
) -> Result<(), FormatError> {
    let location = STGStructuralLocation::Script(target);
    let Some(event) = event_at(model, target.block, target.event) else {
        return Err(structural_state_mismatch(location));
    };
    let count = script_count(event, target.kind);
    let before_id = target
        .script
        .checked_sub(1)
        .and_then(|index| script_id_at(event, target.kind, index));
    let after_id = target
        .script
        .checked_add(1)
        .and_then(|index| script_id_at(event, target.kind, index));
    let current_matches = script_at(event, target.kind, target.script)
        .is_some_and(|script| script_fingerprint(script) == guard.expected);
    if event.event_id == guard.event_id
        && count == guard.script_count
        && before_id == guard.before_id
        && after_id == guard.after_id
        && current_matches
    {
        Ok(())
    } else {
        Err(structural_state_mismatch(location))
    }
}

const fn structural_state_mismatch(location: STGStructuralLocation) -> FormatError {
    FormatError::STGStructuralStateMismatch { location }
}

fn event_at(model: &STGModel, block: usize, event: usize) -> Option<&WireEvent> {
    let STGTail::Parsed(tail) = &model.tail else {
        return None;
    };
    tail.event_blocks
        .get(block)
        .and_then(|block| block.events.get(event))
}

fn script_count(event: &WireEvent, kind: STGScriptKind) -> usize {
    match kind {
        STGScriptKind::Condition => event.conditions.len(),
        STGScriptKind::Action => event.actions.len(),
    }
}

fn script_id_at(event: &WireEvent, kind: STGScriptKind, index: usize) -> Option<u32> {
    match kind {
        STGScriptKind::Condition => event.conditions.get(index).map(|script| script.type_id),
        STGScriptKind::Action => event.actions.get(index).map(|script| script.type_id),
    }
}

fn script_at(event: &WireEvent, kind: STGScriptKind, index: usize) -> Option<ScriptRef<'_>> {
    match kind {
        STGScriptKind::Condition => event.conditions.get(index).map(ScriptRef::Condition),
        STGScriptKind::Action => event.actions.get(index).map(ScriptRef::Action),
    }
}

const fn script_collection(kind: STGScriptKind) -> STGCollection {
    match kind {
        STGScriptKind::Condition => STGCollection::Condition,
        STGScriptKind::Action => STGCollection::Action,
    }
}

fn apply_validated_operation(
    model: &mut STGModel,
    operation: StructuralOperation,
) -> StructuralOperation {
    match operation {
        StructuralOperation::InsertEvent {
            target,
            event,
            guard,
        } => apply_insert_event(model, target, event, guard),
        StructuralOperation::RemoveEvent { target, guard } => {
            apply_remove_event(model, target, guard)
        }
        StructuralOperation::InsertScript {
            target,
            script,
            guard,
        } => apply_insert_script(model, target, script, guard),
        StructuralOperation::RemoveScript { target, guard } => {
            apply_remove_script(model, target, guard)
        }
        StructuralOperation::ReplaceScript {
            target,
            expected: _,
            replacement,
        } => apply_replace_script(model, target, replacement),
        StructuralOperation::ReplaceValue {
            target,
            expected: _,
            replacement,
        } => apply_replace_value(model, target, replacement),
    }
}

fn apply_insert_event(
    model: &mut STGModel,
    target: STGEventTarget,
    event: Arc<WireEvent>,
    guard: EventInsertionGuard,
) -> StructuralOperation {
    let event = take_structural_payload(event);
    let tail = validated(parsed_tail_mut(model, event_location(target)));
    if guard.create_block {
        let block = EventBlock {
            block_header: guard.block_header,
            event_count: 1,
            events: vec![event],
        };
        insert_exact(&mut tail.event_blocks, target.block, block);
    } else {
        let Some(block) = tail.event_blocks.get_mut(target.block) else {
            unreachable!("validated STG event block disappeared");
        };
        insert_exact(&mut block.events, target.event, event);
        block.event_count = validated(count_u32(block.events.len()));
    }
    let inverse_guard = validated(event_removal_guard(model, target, guard.create_block));
    StructuralOperation::RemoveEvent {
        target,
        guard: inverse_guard,
    }
}

fn apply_remove_event(
    model: &mut STGModel,
    target: STGEventTarget,
    guard: EventRemovalGuard,
) -> StructuralOperation {
    let removed = {
        let tail = validated(parsed_tail_mut(model, event_location(target)));
        if guard.remove_created_block {
            let block = remove_exact(&mut tail.event_blocks, target.block);
            let mut events = block.events;
            remove_exact(&mut events, target.event)
        } else {
            let Some(block) = tail.event_blocks.get_mut(target.block) else {
                unreachable!("validated STG event block disappeared");
            };
            let removed = remove_exact(&mut block.events, target.event);
            block.event_count = validated(count_u32(block.events.len()));
            removed
        }
    };
    let inverse_guard = validated(event_insertion_guard(model, target));
    StructuralOperation::InsertEvent {
        target,
        event: Arc::new(removed),
        guard: inverse_guard,
    }
}

fn apply_insert_script(
    model: &mut STGModel,
    target: STGScriptTarget,
    script: ScriptImage,
    _guard: ScriptInsertionGuard,
) -> StructuralOperation {
    let event = validated(event_mut(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    ));
    match (target.kind, script) {
        (STGScriptKind::Condition, ScriptImage::Condition(script)) => {
            insert_exact(
                &mut event.conditions,
                target.script,
                take_structural_payload(script),
            );
            event.condition_count = validated(count_u32(event.conditions.len()));
        }
        (STGScriptKind::Action, ScriptImage::Action(script)) => {
            insert_exact(
                &mut event.actions,
                target.script,
                take_structural_payload(script),
            );
            event.action_count = validated(count_u32(event.actions.len()));
        }
        (STGScriptKind::Condition, ScriptImage::Action(_))
        | (STGScriptKind::Action, ScriptImage::Condition(_)) => {
            unreachable!("validated STG script kind changed before insertion");
        }
    }
    let inverse_guard = validated(script_removal_guard(model, target));
    StructuralOperation::RemoveScript {
        target,
        guard: inverse_guard,
    }
}

fn apply_remove_script(
    model: &mut STGModel,
    target: STGScriptTarget,
    _guard: ScriptRemovalGuard,
) -> StructuralOperation {
    let event = validated(event_mut(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    ));
    let script = match target.kind {
        STGScriptKind::Condition => {
            let removed = remove_exact(&mut event.conditions, target.script);
            event.condition_count = validated(count_u32(event.conditions.len()));
            ScriptImage::Condition(Arc::new(removed))
        }
        STGScriptKind::Action => {
            let removed = remove_exact(&mut event.actions, target.script);
            event.action_count = validated(count_u32(event.actions.len()));
            ScriptImage::Action(Arc::new(removed))
        }
    };
    let inverse_guard = validated(script_insertion_guard(model, target));
    StructuralOperation::InsertScript {
        target,
        script,
        guard: inverse_guard,
    }
}

fn apply_replace_script(
    model: &mut STGModel,
    target: STGScriptTarget,
    replacement: ScriptImage,
) -> StructuralOperation {
    let expected = script_image_fingerprint(&replacement);
    let event = validated(event_mut(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    ));
    let previous = match replacement {
        ScriptImage::Condition(replacement) => {
            let Some(current) = event.conditions.get_mut(target.script) else {
                unreachable!("validated STG condition disappeared");
            };
            let previous = std::mem::replace(current, take_structural_payload(replacement));
            event.condition_count = validated(count_u32(event.conditions.len()));
            ScriptImage::Condition(Arc::new(previous))
        }
        ScriptImage::Action(replacement) => {
            let Some(current) = event.actions.get_mut(target.script) else {
                unreachable!("validated STG action disappeared");
            };
            let previous = std::mem::replace(current, take_structural_payload(replacement));
            event.action_count = validated(count_u32(event.actions.len()));
            ScriptImage::Action(Arc::new(previous))
        }
    };
    StructuralOperation::ReplaceScript {
        target,
        expected,
        replacement: previous,
    }
}

fn apply_replace_value(
    model: &mut STGModel,
    target: STGValueTarget,
    replacement: Arc<StgParamValue>,
) -> StructuralOperation {
    let expected = parameter_fingerprint(&replacement);
    let current = validated(value_mut(model, target, STGTarget::Value(target)));
    let previous = std::mem::replace(current, take_structural_payload(replacement));
    StructuralOperation::ReplaceValue {
        target,
        expected,
        replacement: Arc::new(previous),
    }
}

fn take_structural_payload<T>(payload: Arc<T>) -> T {
    match Arc::try_unwrap(payload) {
        Ok(payload) => payload,
        Err(_) => unreachable!("STG structural history payload was unexpectedly shared"),
    }
}

fn validated<T>(result: Result<T, FormatError>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => unreachable!("validated STG structural transition became invalid"),
    }
}

fn insert_exact<T>(values: &mut Vec<T>, index: usize, value: T) {
    let old = std::mem::take(values);
    let old_len = old.len();
    let mut inserted = Some(value);
    let mut replacement = Vec::with_capacity(old_len.saturating_add(1));
    for (position, item) in old.into_iter().enumerate() {
        if position == index {
            let Some(value) = inserted.take() else {
                unreachable!("STG replacement value was inserted twice");
            };
            replacement.push(value);
        }
        replacement.push(item);
    }
    if index == old_len {
        let Some(value) = inserted.take() else {
            unreachable!("STG replacement value was inserted twice");
        };
        replacement.push(value);
    }
    debug_assert!(inserted.is_none());
    *values = exact_vec(replacement);
}

fn remove_exact<T>(values: &mut Vec<T>, index: usize) -> T {
    let old = std::mem::take(values);
    let mut removed = None;
    let mut replacement = Vec::with_capacity(old.len().saturating_sub(1));
    for (position, item) in old.into_iter().enumerate() {
        if position == index {
            removed = Some(item);
        } else {
            replacement.push(item);
        }
    }
    *values = exact_vec(replacement);
    match removed {
        Some(removed) => removed,
        None => unreachable!("validated STG removal index disappeared"),
    }
}

fn exact_vec<T>(values: Vec<T>) -> Vec<T> {
    values.into_boxed_slice().into_vec()
}

fn count_u32(count: usize) -> Result<u32, FormatError> {
    u32::try_from(count).map_err(|_| {
        FormatError::STGEncode(STGEncodeError::LengthOverflow {
            length: count,
            maximum: u32::MAX as usize,
        })
    })
}

fn smallest_unused_event_id(model: &STGModel) -> Result<u32, FormatError> {
    let tail = parsed_tail(model, STGStructuralLocation::Event { block: 0, event: 0 })?;
    let count = tail.event_blocks.iter().try_fold(0_usize, |count, block| {
        count.checked_add(block.events.len())
    });
    let Some(count) = count else {
        return Err(FormatError::STGEventIDExhausted);
    };
    let mut ids = Vec::with_capacity(count);
    for block in &tail.event_blocks {
        ids.extend(block.events.iter().map(|event| event.event_id));
    }
    ids.sort_unstable();
    let mut candidate = 0_u32;
    for id in ids {
        if id < candidate {
            continue;
        }
        if id == candidate {
            candidate = candidate
                .checked_add(1)
                .ok_or(FormatError::STGEventIDExhausted)?;
        } else {
            break;
        }
    }
    Ok(candidate)
}

fn new_script(kind: STGScriptKind, id: u32, parameter_count: usize) -> ScriptImage {
    let mut parameters = Vec::with_capacity(parameter_count);
    parameters.resize_with(parameter_count, || default_value(STGValueKind::Integer));
    let parameters = exact_vec(parameters);
    let Ok(parameter_count) = u32::try_from(parameter_count) else {
        unreachable!("catalog STG parameter count does not fit u32");
    };
    match kind {
        STGScriptKind::Condition => ScriptImage::Condition(Arc::new(StgCondition {
            type_id: id,
            param_count: parameter_count,
            params: parameters,
        })),
        STGScriptKind::Action => ScriptImage::Action(Arc::new(StgAction {
            type_id: id,
            param_count: parameter_count,
            params: parameters,
        })),
    }
}

fn resized_script(current: ScriptRef<'_>, id: u32, parameter_count: usize) -> ScriptImage {
    let mut parameters = Vec::with_capacity(parameter_count);
    for parameter in current.parameters().iter().take(parameter_count) {
        parameters.push(exact_parameter(parameter.clone()));
    }
    parameters.resize_with(parameter_count, || default_value(STGValueKind::Integer));
    let parameters = exact_vec(parameters);
    let Ok(parameter_count) = u32::try_from(parameter_count) else {
        unreachable!("catalog STG parameter count does not fit u32");
    };
    match current {
        ScriptRef::Condition(_) => ScriptImage::Condition(Arc::new(StgCondition {
            type_id: id,
            param_count: parameter_count,
            params: parameters,
        })),
        ScriptRef::Action(_) => ScriptImage::Action(Arc::new(StgAction {
            type_id: id,
            param_count: parameter_count,
            params: parameters,
        })),
    }
}

const fn default_value(kind: STGValueKind) -> StgParamValue {
    match kind {
        STGValueKind::Integer => StgParamValue {
            type_tag: 0,
            value: StgParamValueValue::I32(0),
        },
        STGValueKind::Float => StgParamValue {
            type_tag: 1,
            value: StgParamValueValue::F32(0.0),
        },
        STGValueKind::String => StgParamValue {
            type_tag: 2,
            value: StgParamValueValue::StgStringParam(StgStringParam {
                length: 0,
                value: Vec::new(),
            }),
        },
        STGValueKind::Enum => StgParamValue {
            type_tag: 3,
            value: StgParamValueValue::I32(0),
        },
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

fn exact_parameter(mut parameter: StgParamValue) -> StgParamValue {
    if let StgParamValueValue::StgStringParam(value) = &mut parameter.value {
        value.value = exact_vec(std::mem::take(&mut value.value));
        value.length = u32::try_from(value.value.len()).unwrap_or(u32::MAX);
    }
    parameter
}

fn operation_eq(left: &StructuralOperation, right: &StructuralOperation) -> bool {
    match (left, right) {
        (
            StructuralOperation::InsertEvent {
                target: left_target,
                event: left_event,
                guard: left_guard,
            },
            StructuralOperation::InsertEvent {
                target: right_target,
                event: right_event,
                guard: right_guard,
            },
        ) => {
            left_target == right_target
                && left_guard == right_guard
                && event_eq(left_event, right_event)
        }
        (
            StructuralOperation::RemoveEvent {
                target: left_target,
                guard: left_guard,
            },
            StructuralOperation::RemoveEvent {
                target: right_target,
                guard: right_guard,
            },
        ) => left_target == right_target && left_guard == right_guard,
        (
            StructuralOperation::InsertScript {
                target: left_target,
                script: left_script,
                guard: left_guard,
            },
            StructuralOperation::InsertScript {
                target: right_target,
                script: right_script,
                guard: right_guard,
            },
        ) => {
            left_target == right_target
                && left_guard == right_guard
                && script_image_eq(left_script, right_script)
        }
        (
            StructuralOperation::RemoveScript {
                target: left_target,
                guard: left_guard,
            },
            StructuralOperation::RemoveScript {
                target: right_target,
                guard: right_guard,
            },
        ) => left_target == right_target && left_guard == right_guard,
        (
            StructuralOperation::ReplaceScript {
                target: left_target,
                expected: left_expected,
                replacement: left_replacement,
            },
            StructuralOperation::ReplaceScript {
                target: right_target,
                expected: right_expected,
                replacement: right_replacement,
            },
        ) => {
            left_target == right_target
                && left_expected == right_expected
                && script_image_eq(left_replacement, right_replacement)
        }
        (
            StructuralOperation::ReplaceValue {
                target: left_target,
                expected: left_expected,
                replacement: left_replacement,
            },
            StructuralOperation::ReplaceValue {
                target: right_target,
                expected: right_expected,
                replacement: right_replacement,
            },
        ) => {
            left_target == right_target
                && left_expected == right_expected
                && parameter_eq(left_replacement, right_replacement)
        }
        _ => false,
    }
}

fn event_eq(left: &WireEvent, right: &WireEvent) -> bool {
    left.description == right.description
        && left.event_id == right.event_id
        && left.condition_count == right.condition_count
        && left.conditions.len() == right.conditions.len()
        && left
            .conditions
            .iter()
            .zip(&right.conditions)
            .all(|(left, right)| condition_eq(left, right))
        && left.action_count == right.action_count
        && left.actions.len() == right.actions.len()
        && left
            .actions
            .iter()
            .zip(&right.actions)
            .all(|(left, right)| action_eq(left, right))
}

fn condition_eq(left: &StgCondition, right: &StgCondition) -> bool {
    left.type_id == right.type_id
        && left.param_count == right.param_count
        && parameters_eq(&left.params, &right.params)
}

fn action_eq(left: &StgAction, right: &StgAction) -> bool {
    left.type_id == right.type_id
        && left.param_count == right.param_count
        && parameters_eq(&left.params, &right.params)
}

fn parameters_eq(left: &[StgParamValue], right: &[StgParamValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| parameter_eq(left, right))
}

fn parameter_eq(left: &StgParamValue, right: &StgParamValue) -> bool {
    if left.type_tag != right.type_tag {
        return false;
    }
    match (&left.value, &right.value) {
        (StgParamValueValue::I32(left), StgParamValueValue::I32(right)) => left == right,
        (StgParamValueValue::F32(left), StgParamValueValue::F32(right)) => {
            left.to_bits() == right.to_bits()
        }
        (StgParamValueValue::StgStringParam(left), StgParamValueValue::StgStringParam(right)) => {
            left.length == right.length && left.value == right.value
        }
        (
            StgParamValueValue::I32(_)
            | StgParamValueValue::F32(_)
            | StgParamValueValue::StgStringParam(_),
            _,
        ) => false,
    }
}

fn event_fingerprint(event: &WireEvent) -> StructuralFingerprint {
    let mut fingerprint = FingerprintBuilder::new(1);
    fingerprint.write_bytes(&event.description);
    fingerprint.write_u32(event.event_id);
    fingerprint.write_u32(event.condition_count);
    fingerprint.write_usize(event.conditions.len());
    for condition in &event.conditions {
        write_condition_fingerprint(&mut fingerprint, condition);
    }
    fingerprint.write_u32(event.action_count);
    fingerprint.write_usize(event.actions.len());
    for action in &event.actions {
        write_action_fingerprint(&mut fingerprint, action);
    }
    fingerprint.finish()
}

fn script_fingerprint(script: ScriptRef<'_>) -> StructuralFingerprint {
    let mut fingerprint = FingerprintBuilder::new(2);
    match script {
        ScriptRef::Condition(script) => write_condition_fingerprint(&mut fingerprint, script),
        ScriptRef::Action(script) => write_action_fingerprint(&mut fingerprint, script),
    }
    fingerprint.finish()
}

fn script_image_fingerprint(script: &ScriptImage) -> StructuralFingerprint {
    match script {
        ScriptImage::Condition(script) => script_fingerprint(ScriptRef::Condition(script)),
        ScriptImage::Action(script) => script_fingerprint(ScriptRef::Action(script)),
    }
}

fn parameter_fingerprint(parameter: &StgParamValue) -> StructuralFingerprint {
    let mut fingerprint = FingerprintBuilder::new(3);
    write_parameter_fingerprint(&mut fingerprint, parameter);
    fingerprint.finish()
}

fn write_condition_fingerprint(fingerprint: &mut FingerprintBuilder, script: &StgCondition) {
    fingerprint.write_u8(1);
    fingerprint.write_u32(script.type_id);
    fingerprint.write_u32(script.param_count);
    fingerprint.write_usize(script.params.len());
    for parameter in &script.params {
        write_parameter_fingerprint(fingerprint, parameter);
    }
}

fn write_action_fingerprint(fingerprint: &mut FingerprintBuilder, script: &StgAction) {
    fingerprint.write_u8(2);
    fingerprint.write_u32(script.type_id);
    fingerprint.write_u32(script.param_count);
    fingerprint.write_usize(script.params.len());
    for parameter in &script.params {
        write_parameter_fingerprint(fingerprint, parameter);
    }
}

fn write_parameter_fingerprint(fingerprint: &mut FingerprintBuilder, parameter: &StgParamValue) {
    fingerprint.write_u32(parameter.type_tag);
    match &parameter.value {
        StgParamValueValue::I32(value) => {
            fingerprint.write_u8(1);
            fingerprint.write_i32(*value);
        }
        StgParamValueValue::F32(value) => {
            fingerprint.write_u8(2);
            fingerprint.write_u32(value.to_bits());
        }
        StgParamValueValue::StgStringParam(value) => {
            fingerprint.write_u8(3);
            fingerprint.write_u32(value.length);
            fingerprint.write_bytes(&value.value);
        }
    }
}

struct FingerprintBuilder {
    digest: Sha256,
}

impl FingerprintBuilder {
    fn new(domain: u8) -> Self {
        let mut fingerprint = Self {
            digest: Sha256::new(),
        };
        fingerprint.digest.update(b"kufeditor-stg-structure");
        fingerprint.write_u8(domain);
        fingerprint
    }

    fn write_u8(&mut self, value: u8) {
        self.digest.update([value]);
    }

    fn write_u32(&mut self, value: u32) {
        self.digest.update(value.to_le_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.digest.update(value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.digest.update(value.to_le_bytes());
    }

    fn write_bytes(&mut self, value: &[u8]) {
        self.write_usize(value.len());
        self.digest.update(value);
    }

    fn finish(self) -> StructuralFingerprint {
        StructuralFingerprint(self.digest.finalize().into())
    }
}

fn script_image_eq(left: &ScriptImage, right: &ScriptImage) -> bool {
    match (left, right) {
        (ScriptImage::Condition(left), ScriptImage::Condition(right)) => condition_eq(left, right),
        (ScriptImage::Action(left), ScriptImage::Action(right)) => action_eq(left, right),
        (ScriptImage::Condition(_), ScriptImage::Action(_))
        | (ScriptImage::Action(_), ScriptImage::Condition(_)) => false,
    }
}

fn event_dynamic_bytes(event: &WireEvent) -> Option<usize> {
    let mut retained = capacity_bytes_checked::<StgCondition>(&event.conditions)?;
    for script in &event.conditions {
        retained = retained.checked_add(condition_dynamic_bytes(script)?)?;
    }
    retained = retained.checked_add(capacity_bytes_checked::<StgAction>(&event.actions)?)?;
    for script in &event.actions {
        retained = retained.checked_add(action_dynamic_bytes(script)?)?;
    }
    Some(retained)
}

fn condition_dynamic_bytes(script: &StgCondition) -> Option<usize> {
    parameters_dynamic_bytes(&script.params)
}

fn action_dynamic_bytes(script: &StgAction) -> Option<usize> {
    parameters_dynamic_bytes(&script.params)
}

fn parameters_dynamic_bytes(parameters: &Vec<StgParamValue>) -> Option<usize> {
    let mut retained = capacity_bytes_checked::<StgParamValue>(parameters)?;
    for parameter in parameters {
        retained = retained.checked_add(parameter_dynamic_bytes(parameter))?;
    }
    Some(retained)
}

fn parameter_dynamic_bytes(parameter: &StgParamValue) -> usize {
    match &parameter.value {
        StgParamValueValue::StgStringParam(value) => value.value.capacity(),
        StgParamValueValue::I32(_) | StgParamValueValue::F32(_) => 0,
    }
}

fn script_ref_dynamic_bytes(script: ScriptRef<'_>) -> Option<usize> {
    match script {
        ScriptRef::Condition(script) => condition_dynamic_bytes(script),
        ScriptRef::Action(script) => action_dynamic_bytes(script),
    }
}

fn script_image_dynamic_bytes(script: &ScriptImage) -> Option<usize> {
    match script {
        ScriptImage::Condition(script) => condition_dynamic_bytes(script),
        ScriptImage::Action(script) => action_dynamic_bytes(script),
    }
}

fn capacity_bytes_checked<T>(values: &Vec<T>) -> Option<usize> {
    values.capacity().checked_mul(size_of::<T>())
}

fn event_wire_len(event: &WireEvent) -> Option<usize> {
    let mut length = 76_usize;
    for script in &event.conditions {
        length = length.checked_add(condition_wire_len(script)?)?;
    }
    for script in &event.actions {
        length = length.checked_add(action_wire_len(script)?)?;
    }
    Some(length)
}

fn condition_wire_len(script: &StgCondition) -> Option<usize> {
    parameters_wire_len(&script.params)
}

fn action_wire_len(script: &StgAction) -> Option<usize> {
    parameters_wire_len(&script.params)
}

fn parameters_wire_len(parameters: &[StgParamValue]) -> Option<usize> {
    let mut length = 8_usize;
    for parameter in parameters {
        length = length.checked_add(parameter_wire_len(parameter)?)?;
    }
    Some(length)
}

fn parameter_wire_len(parameter: &StgParamValue) -> Option<usize> {
    match &parameter.value {
        StgParamValueValue::I32(_) | StgParamValueValue::F32(_) => Some(8),
        StgParamValueValue::StgStringParam(value) => 8_usize.checked_add(value.value.len()),
    }
}

fn script_ref_wire_len(script: ScriptRef<'_>) -> Option<usize> {
    match script {
        ScriptRef::Condition(script) => condition_wire_len(script),
        ScriptRef::Action(script) => action_wire_len(script),
    }
}

fn script_image_wire_len(script: &ScriptImage) -> Option<usize> {
    match script {
        ScriptImage::Condition(script) => condition_wire_len(script),
        ScriptImage::Action(script) => action_wire_len(script),
    }
}

#[derive(Clone, Copy)]
struct MetricDelta {
    old_retained: usize,
    new_retained: usize,
    old_wire: usize,
    new_wire: usize,
}

fn event_image_retained_bytes(event: &WireEvent) -> usize {
    size_of::<StructuralOperation>()
        .checked_add(size_of::<WireEvent>())
        .and_then(|bytes| bytes.checked_add(event_dynamic_bytes(event)?))
        .unwrap_or(usize::MAX)
}

fn script_shape_image_retained_bytes(kind: STGScriptKind, parameter_count: usize) -> usize {
    size_of::<StructuralOperation>()
        .checked_add(script_struct_size(kind))
        .and_then(|bytes| bytes.checked_add(script_shape_dynamic_bytes(parameter_count)))
        .unwrap_or(usize::MAX)
}

fn script_ref_image_retained_bytes(script: ScriptRef<'_>) -> usize {
    size_of::<StructuralOperation>()
        .checked_add(script_struct_size(script.kind()))
        .and_then(|bytes| bytes.checked_add(script_ref_dynamic_bytes(script).unwrap_or(usize::MAX)))
        .unwrap_or(usize::MAX)
}

fn replace_script_image_retained_bytes(script: ScriptRef<'_>, new_dynamic: usize) -> usize {
    size_of::<StructuralOperation>()
        .checked_add(script_struct_size(script.kind()))
        .and_then(|bytes| {
            bytes.checked_add(
                script_ref_dynamic_bytes(script)
                    .unwrap_or(usize::MAX)
                    .max(new_dynamic),
            )
        })
        .unwrap_or(usize::MAX)
}

fn replace_value_image_retained_bytes(old_dynamic: usize) -> usize {
    size_of::<StructuralOperation>()
        .checked_add(size_of::<StgParamValue>())
        .and_then(|bytes| bytes.checked_add(old_dynamic))
        .unwrap_or(usize::MAX)
}

fn script_shape_dynamic_bytes(parameter_count: usize) -> usize {
    parameter_count.saturating_mul(size_of::<StgParamValue>())
}

fn script_shape_wire_len(parameter_count: usize) -> usize {
    parameter_count
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(8))
        .unwrap_or(usize::MAX)
}

fn resized_script_dynamic_bytes(script: ScriptRef<'_>, parameter_count: usize) -> usize {
    let mut retained = script_shape_dynamic_bytes(parameter_count);
    for parameter in script.parameters().iter().take(parameter_count) {
        retained = retained.saturating_add(parameter_dynamic_bytes(parameter));
    }
    retained
}

fn resized_script_wire_len(script: ScriptRef<'_>, parameter_count: usize) -> usize {
    let prefix_count = script.parameters().len().min(parameter_count);
    let mut length = 8_usize;
    for parameter in script.parameters().iter().take(prefix_count) {
        length = length.saturating_add(parameter_wire_len(parameter).unwrap_or(usize::MAX));
    }
    parameter_count
        .checked_sub(prefix_count)
        .and_then(|added| added.checked_mul(8))
        .and_then(|added| length.checked_add(added))
        .unwrap_or(usize::MAX)
}

fn script_insert_shape_delta(
    model: &STGModel,
    target: STGScriptTarget,
    parameter_count: usize,
) -> Result<MetricDelta, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    )?;
    Ok(MetricDelta {
        old_retained: script_collection_capacity(event, target.kind),
        new_retained: script_count(event, target.kind)
            .checked_add(1)
            .and_then(|count| count.checked_mul(script_struct_size(target.kind)))
            .and_then(|bytes| bytes.checked_add(script_shape_dynamic_bytes(parameter_count)))
            .unwrap_or(usize::MAX),
        old_wire: 0,
        new_wire: script_shape_wire_len(parameter_count),
    })
}

fn script_remove_ref_delta(
    model: &STGModel,
    target: STGScriptTarget,
    script: ScriptRef<'_>,
) -> Result<MetricDelta, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    )?;
    Ok(MetricDelta {
        old_retained: script_collection_capacity(event, target.kind)
            .saturating_add(script_ref_dynamic_bytes(script).unwrap_or(usize::MAX)),
        new_retained: script_count(event, target.kind)
            .saturating_sub(1)
            .saturating_mul(script_struct_size(target.kind)),
        old_wire: script_ref_wire_len(script).unwrap_or(usize::MAX),
        new_wire: 0,
    })
}

fn validate_projected_delta(
    model: &STGModel,
    delta: MetricDelta,
    model_limit: usize,
    output_limit: usize,
) -> Result<(usize, usize), FormatError> {
    let current_model = retained_model_bytes(model).unwrap_or(usize::MAX);
    let current_output = wire::encoded_len(model).unwrap_or(usize::MAX);
    let projected_model = replace_metric(current_model, delta.old_retained, delta.new_retained);
    let projected_output = replace_metric(current_output, delta.old_wire, delta.new_wire);
    validate_model_limit(projected_model, model_limit)?;
    validate_output_limit(projected_output, output_limit)?;
    Ok((projected_model, projected_output))
}

fn projected_metrics(
    model: &STGModel,
    operation: &StructuralOperation,
    model_limit: usize,
    output_limit: usize,
) -> Result<(usize, usize), FormatError> {
    validate_operation(model, operation)?;
    let current_model = retained_model_bytes(model).unwrap_or(usize::MAX);
    let current_output = wire::encoded_len(model).unwrap_or(usize::MAX);
    let delta = operation_delta(model, operation)?;
    let projected_model = replace_metric(current_model, delta.old_retained, delta.new_retained);
    let projected_output = replace_metric(current_output, delta.old_wire, delta.new_wire);
    validate_model_limit(projected_model, model_limit)?;
    validate_output_limit(projected_output, output_limit)?;
    Ok((projected_model, projected_output))
}

fn validate_operation(
    model: &STGModel,
    operation: &StructuralOperation,
) -> Result<(), FormatError> {
    match operation {
        StructuralOperation::InsertEvent {
            target,
            event: _,
            guard,
        } => {
            validate_event_insertion_guard(model, *target, *guard)?;
            if guard.create_block {
                let tail = parsed_tail(model, event_location(*target))?;
                count_u32(tail.event_blocks.len().saturating_add(1))?;
            }
            count_u32(guard.event_count.saturating_add(1)).map(|_| ())
        }
        StructuralOperation::RemoveEvent { target, guard } => {
            validate_event_removal_guard(model, *target, *guard)
        }
        StructuralOperation::InsertScript {
            target,
            script,
            guard,
        } => {
            validate_script_insertion_guard(model, *target, script, *guard)?;
            count_u32(guard.script_count.saturating_add(1)).map(|_| ())
        }
        StructuralOperation::RemoveScript { target, guard } => {
            validate_script_removal_guard(model, *target, *guard)
        }
        StructuralOperation::ReplaceScript {
            target,
            expected,
            replacement,
        } => {
            let current = script_ref(model, *target, STGTarget::Script(*target))?;
            if replacement.kind() == target.kind && script_fingerprint(current) == *expected {
                count_u32(replacement.parameters().len()).map(|_| ())
            } else {
                Err(structural_state_mismatch(STGStructuralLocation::Script(
                    *target,
                )))
            }
        }
        StructuralOperation::ReplaceValue {
            target, expected, ..
        } => {
            let current = value_ref(model, *target, STGTarget::Value(*target))?;
            if parameter_fingerprint(current) == *expected {
                Ok(())
            } else {
                Err(structural_state_mismatch(STGStructuralLocation::Value(
                    *target,
                )))
            }
        }
    }
}

fn operation_delta(
    model: &STGModel,
    operation: &StructuralOperation,
) -> Result<MetricDelta, FormatError> {
    match operation {
        StructuralOperation::InsertEvent {
            target,
            event,
            guard,
        } => event_insert_delta(model, *target, event, *guard),
        StructuralOperation::RemoveEvent { target, guard } => {
            event_remove_delta(model, *target, *guard)
        }
        StructuralOperation::InsertScript {
            target,
            script,
            guard: _,
        } => script_insert_delta(model, *target, script),
        StructuralOperation::RemoveScript { target, guard: _ } => {
            script_remove_delta(model, *target)
        }
        StructuralOperation::ReplaceScript {
            target,
            replacement,
            ..
        } => {
            let current = script_ref(model, *target, STGTarget::Script(*target))?;
            Ok(MetricDelta {
                old_retained: script_ref_dynamic_bytes(current).unwrap_or(usize::MAX),
                new_retained: script_image_dynamic_bytes(replacement).unwrap_or(usize::MAX),
                old_wire: script_ref_wire_len(current).unwrap_or(usize::MAX),
                new_wire: script_image_wire_len(replacement).unwrap_or(usize::MAX),
            })
        }
        StructuralOperation::ReplaceValue {
            target,
            replacement,
            ..
        } => {
            let current = value_ref(model, *target, STGTarget::Value(*target))?;
            Ok(MetricDelta {
                old_retained: parameter_dynamic_bytes(current),
                new_retained: parameter_dynamic_bytes(replacement),
                old_wire: parameter_wire_len(current).unwrap_or(usize::MAX),
                new_wire: parameter_wire_len(replacement).unwrap_or(usize::MAX),
            })
        }
    }
}

fn event_insert_delta(
    model: &STGModel,
    target: STGEventTarget,
    event: &WireEvent,
    guard: EventInsertionGuard,
) -> Result<MetricDelta, FormatError> {
    let tail = parsed_tail(model, event_location(target))?;
    let event_retained = event_dynamic_bytes(event).unwrap_or(usize::MAX);
    let event_wire = event_wire_len(event).unwrap_or(usize::MAX);
    if guard.create_block {
        Ok(MetricDelta {
            old_retained: capacity_bytes::<EventBlock>(&tail.event_blocks),
            new_retained: tail
                .event_blocks
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(size_of::<EventBlock>()))
                .and_then(|bytes| bytes.checked_add(size_of::<WireEvent>()))
                .and_then(|bytes| bytes.checked_add(event_retained))
                .unwrap_or(usize::MAX),
            old_wire: 0,
            new_wire: 8_usize.saturating_add(event_wire),
        })
    } else {
        let block = item(
            &tail.event_blocks,
            STGCollection::EventBlock,
            target.block,
            STGTarget::Structure(event_location(target)),
        )?;
        Ok(MetricDelta {
            old_retained: capacity_bytes::<WireEvent>(&block.events),
            new_retained: block
                .events
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(size_of::<WireEvent>()))
                .and_then(|bytes| bytes.checked_add(event_retained))
                .unwrap_or(usize::MAX),
            old_wire: 0,
            new_wire: event_wire,
        })
    }
}

fn event_remove_delta(
    model: &STGModel,
    target: STGEventTarget,
    guard: EventRemovalGuard,
) -> Result<MetricDelta, FormatError> {
    let tail = parsed_tail(model, event_location(target))?;
    let block = item(
        &tail.event_blocks,
        STGCollection::EventBlock,
        target.block,
        STGTarget::Structure(event_location(target)),
    )?;
    let event = item(
        &block.events,
        STGCollection::Event,
        target.event,
        STGTarget::Structure(event_location(target)),
    )?;
    let event_retained = event_dynamic_bytes(event).unwrap_or(usize::MAX);
    let event_wire = event_wire_len(event).unwrap_or(usize::MAX);
    if guard.remove_created_block {
        Ok(MetricDelta {
            old_retained: capacity_bytes::<EventBlock>(&tail.event_blocks)
                .checked_add(capacity_bytes::<WireEvent>(&block.events))
                .and_then(|bytes| bytes.checked_add(event_retained))
                .unwrap_or(usize::MAX),
            new_retained: tail
                .event_blocks
                .len()
                .saturating_sub(1)
                .saturating_mul(size_of::<EventBlock>()),
            old_wire: 8_usize.saturating_add(event_wire),
            new_wire: 0,
        })
    } else {
        Ok(MetricDelta {
            old_retained: capacity_bytes::<WireEvent>(&block.events).saturating_add(event_retained),
            new_retained: block
                .events
                .len()
                .saturating_sub(1)
                .saturating_mul(size_of::<WireEvent>()),
            old_wire: event_wire,
            new_wire: 0,
        })
    }
}

fn script_insert_delta(
    model: &STGModel,
    target: STGScriptTarget,
    script: &ScriptImage,
) -> Result<MetricDelta, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    )?;
    let old_retained = script_collection_capacity(event, target.kind);
    let new_retained = script_count(event, target.kind)
        .checked_add(1)
        .and_then(|count| count.checked_mul(script_struct_size(target.kind)))
        .and_then(|bytes| {
            bytes.checked_add(script_image_dynamic_bytes(script).unwrap_or(usize::MAX))
        })
        .unwrap_or(usize::MAX);
    Ok(MetricDelta {
        old_retained,
        new_retained,
        old_wire: 0,
        new_wire: script_image_wire_len(script).unwrap_or(usize::MAX),
    })
}

fn script_remove_delta(
    model: &STGModel,
    target: STGScriptTarget,
) -> Result<MetricDelta, FormatError> {
    let event = event_ref(
        model,
        STGEventTarget {
            block: target.block,
            event: target.event,
        },
        STGTarget::Script(target),
    )?;
    let script = script_ref(model, target, STGTarget::Script(target))?;
    Ok(MetricDelta {
        old_retained: script_collection_capacity(event, target.kind)
            .saturating_add(script_ref_dynamic_bytes(script).unwrap_or(usize::MAX)),
        new_retained: script_count(event, target.kind)
            .saturating_sub(1)
            .saturating_mul(script_struct_size(target.kind)),
        old_wire: script_ref_wire_len(script).unwrap_or(usize::MAX),
        new_wire: 0,
    })
}

fn replace_metric(current: usize, old: usize, new: usize) -> usize {
    current
        .checked_sub(old)
        .and_then(|value| value.checked_add(new))
        .unwrap_or(usize::MAX)
}

fn validate_actual_metrics(
    model: &STGModel,
    projected_model: usize,
    projected_output: usize,
    model_limit: usize,
    output_limit: usize,
) -> Result<(), FormatError> {
    let actual_model = retained_model_bytes(model).unwrap_or(usize::MAX);
    validate_model_limit(actual_model, model_limit)?;
    if actual_model > projected_model {
        return Err(FormatError::STGEncode(
            STGEncodeError::ModelProjectionMismatch {
                projected: projected_model,
                actual: actual_model,
            },
        ));
    }
    let actual_output = wire::encoded_len(model).unwrap_or(usize::MAX);
    validate_output_limit(actual_output, output_limit)?;
    if actual_output > projected_output {
        return Err(FormatError::STGEncode(
            STGEncodeError::LengthProjectionMismatch {
                projected: projected_output,
                actual: actual_output,
            },
        ));
    }
    Ok(())
}

fn validate_model_limit(retained: usize, maximum: usize) -> Result<(), FormatError> {
    if retained > maximum {
        Err(FormatError::STGEncode(
            STGEncodeError::ModelBudgetExceeded { retained, maximum },
        ))
    } else {
        Ok(())
    }
}

fn validate_output_limit(length: usize, maximum: usize) -> Result<(), FormatError> {
    if length > maximum {
        Err(FormatError::STGEncode(STGEncodeError::LengthOverflow {
            length,
            maximum,
        }))
    } else {
        Ok(())
    }
}

fn capacity_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(size_of::<T>())
}

fn script_collection_capacity(event: &WireEvent, kind: STGScriptKind) -> usize {
    match kind {
        STGScriptKind::Condition => capacity_bytes::<StgCondition>(&event.conditions),
        STGScriptKind::Action => capacity_bytes::<StgAction>(&event.actions),
    }
}

const fn script_struct_size(kind: STGScriptKind) -> usize {
    match kind {
        STGScriptKind::Condition => size_of::<StgCondition>(),
        STGScriptKind::Action => size_of::<StgAction>(),
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    reason = "synthetic fixtures expose checked offsets for direct wire corruption"
)]
mod tests {
    use super::super::stg_test_support::{complete_stg_fixture, empty_stg_fixture};
    use super::*;
    use crate::stg::mutation::tests::test_wire_image;

    #[test]
    fn structural_images_restore_exact_event_script_and_value_wire_images() {
        let fixture = complete_stg_fixture();
        let mut source = fixture.bytes;
        source[fixture.offsets.condition_float_type + 4..fixture.offsets.condition_float_type + 8]
            .copy_from_slice(&0x7fc0_1234_u32.to_le_bytes());
        source[fixture.offsets.action_string_type + 8] = 0xff;

        let mut event_document = parse(source.clone());
        let event_inverse = changed(event_document.remove_event(0, 0).unwrap());
        assert!(event_inverse.retained_bytes() < source.len());
        changed(event_document.restore_structure(event_inverse).unwrap());
        assert_eq!(test_wire_image(&event_document), source);

        let condition = STGScriptTarget {
            block: 0,
            event: 0,
            kind: STGScriptKind::Condition,
            script: 0,
        };
        let mut script_document = parse(source.clone());
        let script_inverse = changed(script_document.change_script_type(condition, 1).unwrap());
        changed(script_document.restore_structure(script_inverse).unwrap());
        assert_eq!(test_wire_image(&script_document), source);

        let value = STGValueTarget::ScriptParameter(STGParameterTarget {
            script: STGScriptTarget {
                kind: STGScriptKind::Action,
                ..condition
            },
            parameter: 0,
        });
        let mut value_document = parse(source.clone());
        let value_inverse = changed(
            value_document
                .change_value_type(value, STGValueKind::Float)
                .unwrap(),
        );
        changed(value_document.restore_structure(value_inverse).unwrap());
        assert_eq!(test_wire_image(&value_document), source);
    }

    #[test]
    fn structural_images_exclude_large_unrelated_suffixes() {
        let fixture = complete_stg_fixture();
        let mut small = parse(fixture.bytes.clone());
        let small_image = changed(small.remove_event(0, 0).unwrap());

        let mut large_source = fixture.bytes;
        large_source.extend(std::iter::repeat_n(0xa5, 256 * 1_024));
        let large_source_len = large_source.len();
        let mut large = parse(large_source);
        let large_image = changed(large.remove_event(0, 0).unwrap());

        assert_eq!(large_image.retained_bytes(), small_image.retained_bytes());
        assert!(large_image.retained_bytes() < large_source_len / 100);
    }

    #[test]
    fn structural_event_restore_moves_large_payloads_between_history_and_model() {
        let mut document = parse(large_action_string_fixture(256 * 1_024));
        let mut insert = changed(document.remove_event(0, 0).unwrap());

        for _ in 0..8 {
            let history_pointer = inserted_event_string_pointer(&insert);
            let remove = changed(document.restore_structure(insert).unwrap());
            assert_eq!(model_event_string_pointer(&document), history_pointer);
            insert = changed(document.restore_structure(remove).unwrap());
        }
    }

    #[test]
    fn structural_script_restore_moves_large_payloads_between_history_and_model() {
        let mut document = parse(large_action_string_fixture(256 * 1_024));
        let target = STGScriptTarget {
            block: 0,
            event: 0,
            kind: STGScriptKind::Action,
            script: 0,
        };
        let mut restore_original = changed(document.change_script_type(target, 7).unwrap());

        for _ in 0..8 {
            let history_pointer = replacement_script_string_pointer(&restore_original);
            let restore_replacement =
                changed(document.restore_structure(restore_original).unwrap());
            assert_eq!(model_event_string_pointer(&document), history_pointer);
            restore_original = changed(document.restore_structure(restore_replacement).unwrap());
        }
    }

    #[test]
    fn structural_event_limits_reject_before_install_and_keep_exact_capacities() {
        let original = parse(empty_stg_fixture());
        let mut accepted = original.clone();
        changed(
            accepted
                .insert_event_with_limits(0, 0, usize::MAX, usize::MAX)
                .unwrap(),
        );
        let required_model = retained_model_bytes(&accepted.model).unwrap();
        let required_output = wire::encoded_len(&accepted.model).unwrap();

        let mut model_rejected = original.clone();
        let before_model = Arc::clone(&model_rejected.model);
        let before_state = Arc::clone(&model_rejected.state);
        match model_rejected.insert_event_with_limits(0, 0, required_model - 1, usize::MAX) {
            Err(FormatError::STGEncode(STGEncodeError::ModelBudgetExceeded {
                retained,
                maximum,
            })) => {
                assert_eq!(retained, required_model);
                assert_eq!(maximum, required_model - 1);
            }
            Err(other) => panic!("unexpected model-budget error: {other}"),
            Ok(_) => panic!("expected structural model-budget rejection"),
        }
        assert!(Arc::ptr_eq(&model_rejected.model, &before_model));
        assert!(Arc::ptr_eq(&model_rejected.state, &before_state));

        let mut output_rejected = original;
        let before_wire = test_wire_image(&output_rejected);
        match output_rejected.insert_event_with_limits(0, 0, required_model, required_output - 1) {
            Err(FormatError::STGEncode(STGEncodeError::LengthOverflow { length, maximum })) => {
                assert_eq!(length, required_output);
                assert_eq!(maximum, required_output - 1);
            }
            Err(other) => panic!("unexpected output-budget error: {other}"),
            Ok(_) => panic!("expected structural output-budget rejection"),
        }
        assert_eq!(test_wire_image(&output_rejected), before_wire);

        for _ in 0..16 {
            let inverse = changed(accepted.remove_event(0, 0).unwrap());
            assert_exact_capacities(&accepted);
            changed(accepted.restore_structure(inverse).unwrap());
            assert_exact_capacities(&accepted);
        }
    }

    #[test]
    fn every_structural_delta_preflights_exact_model_and_output_limits() {
        assert_edit_limits(
            parse(empty_stg_fixture()),
            STGStructuralEdit::InsertEvent {
                target: STGEventTarget { block: 0, event: 0 },
            },
        );

        let condition = STGScriptTarget {
            block: 0,
            event: 0,
            kind: STGScriptKind::Condition,
            script: 0,
        };
        let action = STGScriptTarget {
            kind: STGScriptKind::Action,
            ..condition
        };
        for edit in [
            STGStructuralEdit::RemoveEvent {
                target: STGEventTarget { block: 0, event: 0 },
            },
            STGStructuralEdit::InsertScript {
                target: STGScriptTarget {
                    block: 0,
                    event: 1,
                    kind: STGScriptKind::Action,
                    script: 0,
                },
                type_id: 7,
            },
            STGStructuralEdit::RemoveScript { target: action },
            STGStructuralEdit::ChangeScriptType {
                target: condition,
                type_id: 1,
            },
            STGStructuralEdit::ChangeValueType {
                target: STGValueTarget::VariableInitial { variable: 2 },
                kind: STGValueKind::Float,
            },
        ] {
            assert_edit_limits(parse(complete_stg_fixture().bytes), edit);
        }
    }

    fn parse(bytes: Vec<u8>) -> STGDocument {
        STGDocument::parse(bytes).unwrap_or_else(|error| panic!("test STG parse failed: {error}"))
    }

    fn large_action_string_fixture(length: usize) -> Vec<u8> {
        let fixture = complete_stg_fixture();
        let mut source = fixture.bytes;
        let value_start = fixture.offsets.action_string_length + size_of::<u32>();
        let value_end = value_start + b"action".len();
        source.splice(value_start..value_end, std::iter::repeat_n(b'x', length));
        let length = u32::try_from(length).expect("test string length must fit u32");
        source[fixture.offsets.action_string_length
            ..fixture.offsets.action_string_length + size_of::<u32>()]
            .copy_from_slice(&length.to_le_bytes());
        source
    }

    fn inserted_event_string_pointer(image: &STGStructuralImage) -> *const u8 {
        let StructuralOperation::InsertEvent { event, .. } = &image.operation else {
            panic!("expected an insert-event history action");
        };
        event_string_pointer(event)
    }

    fn replacement_script_string_pointer(image: &STGStructuralImage) -> *const u8 {
        let StructuralOperation::ReplaceScript { replacement, .. } = &image.operation else {
            panic!("expected a replace-script history action");
        };
        let ScriptImage::Action(script) = replacement else {
            panic!("expected an action-script history payload");
        };
        parameter_string_pointer(&script.params[0])
    }

    fn model_event_string_pointer(document: &STGDocument) -> *const u8 {
        let STGTail::Parsed(tail) = &document.model.tail else {
            panic!("test STG unexpectedly has an opaque tail");
        };
        event_string_pointer(&tail.event_blocks[0].events[0])
    }

    fn event_string_pointer(event: &WireEvent) -> *const u8 {
        parameter_string_pointer(&event.actions[0].params[0])
    }

    fn parameter_string_pointer(parameter: &StgParamValue) -> *const u8 {
        let StgParamValueValue::StgStringParam(value) = &parameter.value else {
            panic!("expected an STG string parameter");
        };
        value.value.as_ptr()
    }

    fn changed(mutation: STGMutation<STGStructuralImage>) -> STGStructuralImage {
        match mutation {
            STGMutation::Changed { previous } => previous,
            STGMutation::Unchanged => panic!("expected an STG structural change"),
        }
    }

    fn assert_exact_capacities(document: &STGDocument) {
        let STGTail::Parsed(tail) = &document.model.tail else {
            panic!("test STG unexpectedly has an opaque tail");
        };
        assert_eq!(tail.event_blocks.capacity(), tail.event_blocks.len());
        for block in &tail.event_blocks {
            assert_eq!(block.events.capacity(), block.events.len());
            for event in &block.events {
                assert_eq!(event.conditions.capacity(), event.conditions.len());
                assert_eq!(event.actions.capacity(), event.actions.len());
                for condition in &event.conditions {
                    assert_eq!(condition.params.capacity(), condition.params.len());
                }
                for action in &event.actions {
                    assert_eq!(action.params.capacity(), action.params.len());
                }
            }
        }
    }

    fn assert_edit_limits(original: STGDocument, edit: STGStructuralEdit) {
        let mut accepted = original.clone();
        let preview = accepted
            .preview_structure_with_limits(edit, usize::MAX, usize::MAX)
            .unwrap_or_else(|error| panic!("structural preview failed for {edit:?}: {error}"));
        assert!(preview.is_changed(), "test edit must change the document");
        let expected_charge = preview.retained_bytes();
        let image = changed(
            accepted
                .apply_structure_preview(preview)
                .unwrap_or_else(|error| panic!("structural apply failed for {edit:?}: {error}")),
        );
        assert_eq!(image.retained_bytes(), expected_charge);
        let required_model = retained_model_bytes(&accepted.model).unwrap();
        let required_output = wire::encoded_len(&accepted.model).unwrap();

        let mut model_rejected = original.clone();
        let original_model = Arc::clone(&model_rejected.model);
        let original_state = Arc::clone(&model_rejected.state);
        let original_revision = Arc::clone(&model_rejected.revision);
        match model_rejected.apply_structural_edit_with_limits(edit, required_model - 1, usize::MAX)
        {
            Err(FormatError::STGEncode(STGEncodeError::ModelBudgetExceeded {
                retained,
                maximum,
            })) => {
                assert_eq!(retained, required_model, "{edit:?}");
                assert_eq!(maximum, required_model - 1, "{edit:?}");
            }
            Err(other) => panic!("unexpected model limit error for {edit:?}: {other}"),
            Ok(_) => panic!("expected model limit rejection for {edit:?}"),
        }
        assert!(Arc::ptr_eq(&model_rejected.model, &original_model));
        assert!(Arc::ptr_eq(&model_rejected.state, &original_state));
        assert!(Arc::ptr_eq(&model_rejected.revision, &original_revision));

        let mut output_rejected = original;
        let original_wire = test_wire_image(&output_rejected);
        match output_rejected.apply_structural_edit_with_limits(
            edit,
            required_model,
            required_output - 1,
        ) {
            Err(FormatError::STGEncode(STGEncodeError::LengthOverflow { length, maximum })) => {
                assert_eq!(length, required_output, "{edit:?}");
                assert_eq!(maximum, required_output - 1, "{edit:?}");
            }
            Err(other) => panic!("unexpected output limit error for {edit:?}: {other}"),
            Ok(_) => panic!("expected output limit rejection for {edit:?}"),
        }
        assert_eq!(test_wire_image(&output_rejected), original_wire);
    }
}
