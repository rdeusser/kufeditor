use kufeditor_formats::{
    STGDocument, STGEvent, STGEventBlock, STGEventTarget, STGFloatTarget, STGFloatValue,
    STGMutation, STGNumberTarget, STGParameter, STGParameterTarget, STGScript, STGScriptTarget,
    STGStructuralEdit, STGStructuralImage, STGTailStatus, STGText, STGTextImage, STGTextTarget,
    STGValue, STGValueTarget,
};

use crate::{
    Document, DocumentEdit, DocumentID, Workspace, WorkspaceError, history::STGHistoryAction,
};

#[cfg(test)]
#[allow(
    dead_code,
    reason = "the shared STG fixture exposes offsets and variants used by integration tests"
)]
#[path = "../../kufeditor-formats/tests/support/stg.rs"]
mod stg_test_support;

const HISTORY_ENTRY_ALLOCATION_OVERHEAD: usize = 256;

pub(crate) enum PreparedSTGEdit {
    Unchanged,
    Changed {
        document: STGDocument,
        inverse: Box<STGHistoryAction>,
        retained_bytes: usize,
    },
}

#[derive(Debug)]
pub(crate) struct STGHistoryActionFailure {
    pub(crate) error: WorkspaceError,
    pub(crate) action: STGHistoryAction,
}

pub(crate) fn prepare_edit(
    document: &Document,
    id: DocumentID,
    edit: DocumentEdit,
    history_limit: usize,
) -> Result<PreparedSTGEdit, WorkspaceError> {
    let Document::STG(document) = document else {
        return Err(WorkspaceError::NotSTG(id));
    };
    match edit {
        DocumentEdit::SetSTGNumber { target, value } => {
            prepare_number(document, id, target, value, history_limit)
        }
        DocumentEdit::SetSTGFloat { target, value } => {
            prepare_float(document, id, target, value, history_limit)
        }
        DocumentEdit::SetSTGText { target, value } => {
            prepare_text(document, id, target, value, history_limit)
        }
        DocumentEdit::EditSTGStructure { edit } => {
            prepare_structure(document, id, edit, history_limit)
        }
        DocumentEdit::SetTroopField { .. }
        | DocumentEdit::SetSkillID { .. }
        | DocumentEdit::SetSkillType { .. }
        | DocumentEdit::SetSkillMaxLevel { .. }
        | DocumentEdit::SetSkillText { .. }
        | DocumentEdit::SetTextSOXText { .. }
        | DocumentEdit::SetSaveNumber { .. }
        | DocumentEdit::SetSaveText { .. } => {
            unreachable!("non-STG edit reached STG history preparation")
        }
    }
}

pub(crate) fn apply_history_action(
    document: &STGDocument,
    id: DocumentID,
    action: STGHistoryAction,
) -> Result<(STGDocument, STGHistoryAction), Box<STGHistoryActionFailure>> {
    let mut prospective = document.clone();
    let inverse = match action {
        STGHistoryAction::Number {
            target,
            value,
            retained_bytes,
        } => apply_number_history(&mut prospective, id, target, value, retained_bytes)?,
        STGHistoryAction::Float {
            target,
            value,
            retained_bytes,
        } => apply_float_history(&mut prospective, id, target, value, retained_bytes)?,
        STGHistoryAction::Text {
            target,
            image,
            opposite_retained_bytes,
            retained_bytes,
        } => apply_text_history(
            &mut prospective,
            id,
            target,
            image,
            opposite_retained_bytes,
            retained_bytes,
        )?,
        STGHistoryAction::Structure {
            image,
            opposite_retained_bytes,
            retained_bytes,
        } => apply_structure_history(
            &mut prospective,
            id,
            image,
            opposite_retained_bytes,
            retained_bytes,
        )?,
    };
    Ok((prospective, inverse))
}

fn apply_number_history(
    document: &mut STGDocument,
    id: DocumentID,
    target: STGNumberTarget,
    value: i64,
    retained_bytes: usize,
) -> Result<STGHistoryAction, Box<STGHistoryActionFailure>> {
    let original = || STGHistoryAction::Number {
        target,
        value,
        retained_bytes,
    };
    match document.set_number(target, value) {
        Err(error) => Err(history_action_failure(error.into(), original())),
        Ok(STGMutation::Unchanged) => Err(history_action_failure(
            WorkspaceError::HistoryStateMismatch(id),
            original(),
        )),
        Ok(STGMutation::Changed { previous }) => Ok(STGHistoryAction::Number {
            target,
            value: previous,
            retained_bytes,
        }),
    }
}

fn apply_float_history(
    document: &mut STGDocument,
    id: DocumentID,
    target: STGFloatTarget,
    value: STGFloatValue,
    retained_bytes: usize,
) -> Result<STGHistoryAction, Box<STGHistoryActionFailure>> {
    let original = || STGHistoryAction::Float {
        target,
        value,
        retained_bytes,
    };
    match document.set_float(target, value) {
        Err(error) => Err(history_action_failure(error.into(), original())),
        Ok(STGMutation::Unchanged) => Err(history_action_failure(
            WorkspaceError::HistoryStateMismatch(id),
            original(),
        )),
        Ok(STGMutation::Changed { previous }) => Ok(STGHistoryAction::Float {
            target,
            value: previous,
            retained_bytes,
        }),
    }
}

fn apply_text_history(
    document: &mut STGDocument,
    id: DocumentID,
    target: STGTextTarget,
    image: STGTextImage,
    opposite_retained_bytes: usize,
    retained_bytes: usize,
) -> Result<STGHistoryAction, Box<STGHistoryActionFailure>> {
    match document.preview_text_restore(target, &image) {
        Err(error) => {
            return Err(history_action_failure(
                error.into(),
                text_action(target, image, opposite_retained_bytes, retained_bytes),
            ));
        }
        Ok(false) => {
            return Err(history_action_failure(
                WorkspaceError::HistoryStateMismatch(id),
                text_action(target, image, opposite_retained_bytes, retained_bytes),
            ));
        }
        Ok(true) => {}
    }
    let restored_retained_bytes = image.retained_bytes();
    let previous = match document.restore_text_recoverable(target, image) {
        Err(failure) => {
            let (error, image) = failure.into_parts();
            return Err(history_action_failure(
                error.into(),
                text_action(target, image, opposite_retained_bytes, retained_bytes),
            ));
        }
        Ok(STGMutation::Unchanged) => unreachable!("previewed STG text restore became unchanged"),
        Ok(STGMutation::Changed { previous }) => previous,
    };
    let actual = previous.retained_bytes();
    if actual != opposite_retained_bytes {
        let recovered = match document.restore_text_recoverable(target, previous) {
            Ok(STGMutation::Changed { previous }) => previous,
            Ok(STGMutation::Unchanged) | Err(_) => {
                unreachable!("STG text charge recovery failed")
            }
        };
        return Err(history_action_failure(
            WorkspaceError::HistoryChargeMismatch {
                projected: opposite_retained_bytes,
                actual,
            },
            text_action(target, recovered, opposite_retained_bytes, retained_bytes),
        ));
    }
    Ok(text_action(
        target,
        previous,
        restored_retained_bytes,
        retained_bytes,
    ))
}

fn apply_structure_history(
    document: &mut STGDocument,
    _id: DocumentID,
    image: STGStructuralImage,
    opposite_retained_bytes: usize,
    retained_bytes: usize,
) -> Result<STGHistoryAction, Box<STGHistoryActionFailure>> {
    let restored_retained_bytes = image.retained_bytes();
    let previous = match document.restore_structure_recoverable(image) {
        Err(failure) => {
            let (error, image) = failure.into_parts();
            return Err(history_action_failure(
                error.into(),
                structure_action(image, opposite_retained_bytes, retained_bytes),
            ));
        }
        Ok(STGMutation::Unchanged) => unreachable!("STG structural restore became unchanged"),
        Ok(STGMutation::Changed { previous }) => previous,
    };
    let actual = previous.retained_bytes();
    if actual != opposite_retained_bytes {
        let recovered = match document.restore_structure_recoverable(previous) {
            Ok(STGMutation::Changed { previous }) => previous,
            Ok(STGMutation::Unchanged) | Err(_) => {
                unreachable!("STG structural charge recovery failed")
            }
        };
        return Err(history_action_failure(
            WorkspaceError::HistoryChargeMismatch {
                projected: opposite_retained_bytes,
                actual,
            },
            structure_action(recovered, opposite_retained_bytes, retained_bytes),
        ));
    }
    Ok(structure_action(
        previous,
        restored_retained_bytes,
        retained_bytes,
    ))
}

const fn text_action(
    target: STGTextTarget,
    image: STGTextImage,
    opposite_retained_bytes: usize,
    retained_bytes: usize,
) -> STGHistoryAction {
    STGHistoryAction::Text {
        target,
        image,
        opposite_retained_bytes,
        retained_bytes,
    }
}

const fn structure_action(
    image: STGStructuralImage,
    opposite_retained_bytes: usize,
    retained_bytes: usize,
) -> STGHistoryAction {
    STGHistoryAction::Structure {
        image,
        opposite_retained_bytes,
        retained_bytes,
    }
}

fn history_action_failure(
    error: WorkspaceError,
    action: STGHistoryAction,
) -> Box<STGHistoryActionFailure> {
    Box::new(STGHistoryActionFailure { error, action })
}

fn prepare_number(
    document: &STGDocument,
    id: DocumentID,
    target: STGNumberTarget,
    value: i64,
    history_limit: usize,
) -> Result<PreparedSTGEdit, WorkspaceError> {
    let previous = match document.preview_number(target, value)? {
        STGMutation::Unchanged => return Ok(PreparedSTGEdit::Unchanged),
        STGMutation::Changed { previous } => previous,
    };
    let retained_bytes = admitted_charge(0, history_limit)?;
    let mut prospective = document.clone();
    match prospective.set_number(target, value)? {
        STGMutation::Changed { previous: actual } if actual == previous => {}
        STGMutation::Changed { .. } | STGMutation::Unchanged => {
            return Err(WorkspaceError::HistoryStateMismatch(id));
        }
    }
    Ok(PreparedSTGEdit::Changed {
        document: prospective,
        inverse: Box::new(STGHistoryAction::Number {
            target,
            value: previous,
            retained_bytes,
        }),
        retained_bytes,
    })
}

fn prepare_float(
    document: &STGDocument,
    id: DocumentID,
    target: STGFloatTarget,
    value: STGFloatValue,
    history_limit: usize,
) -> Result<PreparedSTGEdit, WorkspaceError> {
    let previous = match document.preview_float(target, value)? {
        STGMutation::Unchanged => return Ok(PreparedSTGEdit::Unchanged),
        STGMutation::Changed { previous } => previous,
    };
    let retained_bytes = admitted_charge(0, history_limit)?;
    let mut prospective = document.clone();
    match prospective.set_float(target, value)? {
        STGMutation::Changed { previous: actual } if actual == previous => {}
        STGMutation::Changed { .. } | STGMutation::Unchanged => {
            return Err(WorkspaceError::HistoryStateMismatch(id));
        }
    }
    Ok(PreparedSTGEdit::Changed {
        document: prospective,
        inverse: Box::new(STGHistoryAction::Float {
            target,
            value: previous,
            retained_bytes,
        }),
        retained_bytes,
    })
}

fn prepare_text(
    document: &STGDocument,
    id: DocumentID,
    target: STGTextTarget,
    value: String,
    history_limit: usize,
) -> Result<PreparedSTGEdit, WorkspaceError> {
    let preview = document.preview_text(target, &value)?;
    if !preview.is_changed() {
        return Ok(PreparedSTGEdit::Unchanged);
    }
    let payload = preview
        .current_retained_bytes()
        .max(preview.replacement_retained_bytes());
    let retained_bytes = admitted_charge(payload, history_limit)?;
    let mut prospective = document.clone();
    let previous = match prospective.set_text(target, value)? {
        STGMutation::Unchanged => {
            return Err(WorkspaceError::HistoryStateMismatch(id));
        }
        STGMutation::Changed { previous } => previous,
    };
    let actual = previous.retained_bytes();
    if actual != preview.current_retained_bytes() {
        return Err(WorkspaceError::HistoryChargeMismatch {
            projected: preview.current_retained_bytes(),
            actual,
        });
    }
    Ok(PreparedSTGEdit::Changed {
        document: prospective,
        inverse: Box::new(STGHistoryAction::Text {
            target,
            image: previous,
            opposite_retained_bytes: preview.replacement_retained_bytes(),
            retained_bytes,
        }),
        retained_bytes,
    })
}

fn prepare_structure(
    document: &STGDocument,
    id: DocumentID,
    edit: STGStructuralEdit,
    history_limit: usize,
) -> Result<PreparedSTGEdit, WorkspaceError> {
    let preview = document.preview_structure(edit)?;
    if !preview.is_changed() {
        return Ok(PreparedSTGEdit::Unchanged);
    }
    let payload = preview.retained_bytes();
    let retained_bytes = admitted_charge(payload, history_limit)?;
    let mut prospective = document.clone();
    let previous = match prospective.apply_structure_preview(preview)? {
        STGMutation::Unchanged => {
            return Err(WorkspaceError::HistoryStateMismatch(id));
        }
        STGMutation::Changed { previous } => previous,
    };
    let actual = previous.retained_bytes();
    if actual != payload {
        return Err(WorkspaceError::HistoryChargeMismatch {
            projected: payload,
            actual,
        });
    }
    Ok(PreparedSTGEdit::Changed {
        document: prospective,
        inverse: Box::new(STGHistoryAction::Structure {
            image: previous,
            opposite_retained_bytes: payload,
            retained_bytes,
        }),
        retained_bytes,
    })
}

fn admitted_charge(payload: usize, history_limit: usize) -> Result<usize, WorkspaceError> {
    let retained_bytes = HISTORY_ENTRY_ALLOCATION_OVERHEAD
        .checked_add(payload)
        .ok_or(WorkspaceError::HistoryChargeOverflow)?;
    if retained_bytes > history_limit {
        return Err(WorkspaceError::HistoryBudgetExceeded {
            requested: retained_bytes,
            maximum: history_limit,
        });
    }
    Ok(retained_bytes)
}

impl Workspace {
    pub fn stg_tail_status(&self, id: DocumentID) -> Result<STGTailStatus<'_>, WorkspaceError> {
        Ok(self.stg_document(id)?.tail_status())
    }

    pub fn stg_unit_count(&self, id: DocumentID) -> Result<usize, WorkspaceError> {
        Ok(self.stg_document(id)?.unit_count())
    }

    pub fn stg_area_count(&self, id: DocumentID) -> Result<Option<usize>, WorkspaceError> {
        Ok(self.stg_document(id)?.area_count())
    }

    pub fn stg_variable_count(&self, id: DocumentID) -> Result<Option<usize>, WorkspaceError> {
        Ok(self.stg_document(id)?.variable_count())
    }

    pub fn stg_event_block_count(&self, id: DocumentID) -> Result<Option<usize>, WorkspaceError> {
        Ok(self.stg_document(id)?.event_block_count())
    }

    pub fn stg_footer_count(&self, id: DocumentID) -> Result<Option<usize>, WorkspaceError> {
        Ok(self.stg_document(id)?.footer_count())
    }

    pub fn stg_event_block(
        &self,
        id: DocumentID,
        block: usize,
    ) -> Result<STGEventBlock, WorkspaceError> {
        self.stg_document(id)?
            .event_block(block)
            .map_err(Into::into)
    }

    pub fn stg_event(
        &self,
        id: DocumentID,
        target: STGEventTarget,
    ) -> Result<STGEvent<'_>, WorkspaceError> {
        self.stg_document(id)?.event(target).map_err(Into::into)
    }

    pub fn stg_script(
        &self,
        id: DocumentID,
        target: STGScriptTarget,
    ) -> Result<STGScript, WorkspaceError> {
        self.stg_document(id)?.script(target).map_err(Into::into)
    }

    pub fn stg_parameter(
        &self,
        id: DocumentID,
        target: STGParameterTarget,
    ) -> Result<STGParameter<'_>, WorkspaceError> {
        self.stg_document(id)?.parameter(target).map_err(Into::into)
    }

    pub fn stg_value(
        &self,
        id: DocumentID,
        target: STGValueTarget,
    ) -> Result<STGValue<'_>, WorkspaceError> {
        self.stg_document(id)?.value(target).map_err(Into::into)
    }

    pub fn stg_number(
        &self,
        id: DocumentID,
        target: STGNumberTarget,
    ) -> Result<i64, WorkspaceError> {
        self.stg_document(id)?.number(target).map_err(Into::into)
    }

    pub fn stg_float(
        &self,
        id: DocumentID,
        target: STGFloatTarget,
    ) -> Result<STGFloatValue, WorkspaceError> {
        self.stg_document(id)?.float(target).map_err(Into::into)
    }

    pub fn stg_text(
        &self,
        id: DocumentID,
        target: STGTextTarget,
    ) -> Result<STGText<'_>, WorkspaceError> {
        self.stg_document(id)?.text(target).map_err(Into::into)
    }

    fn stg_document(&self, id: DocumentID) -> Result<&STGDocument, WorkspaceError> {
        match &self.session(id)?.document {
            Document::STG(document) => Ok(document),
            Document::Troop(_) | Document::Skill(_) | Document::TextSOX(_) | Document::Save(_) => {
                Err(WorkspaceError::NotSTG(id))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_mismatch_returns_a_reusable_text_history_action() {
        let id = DocumentID(7);
        let target = STGTextTarget::ParameterString {
            value: STGValueTarget::ScriptParameter(STGParameterTarget {
                script: STGScriptTarget {
                    block: 0,
                    event: 0,
                    kind: kufeditor_formats::STGScriptKind::Action,
                    script: 0,
                },
                parameter: 0,
            }),
        };
        let source = STGDocument::parse(stg_test_support::complete_stg_fixture().bytes).unwrap();
        let prepared = prepare_edit(
            &Document::STG(source),
            id,
            DocumentEdit::SetSTGText {
                target,
                value: "a much longer action".to_owned(),
            },
            usize::MAX,
        )
        .unwrap();
        let PreparedSTGEdit::Changed {
            document,
            inverse,
            retained_bytes,
        } = prepared
        else {
            panic!("different STG text must prepare a history entry");
        };
        let inverse = *inverse;
        let STGHistoryAction::Text {
            target,
            image,
            opposite_retained_bytes,
            retained_bytes: action_retained_bytes,
        } = inverse
        else {
            panic!("STG text edit must prepare a text inverse");
        };
        let corrupted = STGHistoryAction::Text {
            target,
            image,
            opposite_retained_bytes: opposite_retained_bytes + 1,
            retained_bytes: action_retained_bytes,
        };

        let Err(failure) = apply_history_action(&document, id, corrupted) else {
            panic!("corrupted history charge must fail");
        };
        let WorkspaceError::HistoryChargeMismatch { actual, .. } = failure.error else {
            panic!("corrupted history charge returned another error");
        };
        assert_eq!(
            document.text(target).unwrap().decoded(),
            Some("a much longer action")
        );
        let STGHistoryAction::Text {
            target,
            image,
            retained_bytes: recovered_retained_bytes,
            ..
        } = failure.action
        else {
            panic!("failed text history action changed kind");
        };
        assert_eq!(recovered_retained_bytes, retained_bytes);
        let recovered = STGHistoryAction::Text {
            target,
            image,
            opposite_retained_bytes: actual,
            retained_bytes: recovered_retained_bytes,
        };

        let (restored, redo) = apply_history_action(&document, id, recovered).unwrap();
        assert_eq!(restored.text(target).unwrap().decoded(), Some("action"));
        assert_eq!(redo.retained_bytes(), retained_bytes);
    }

    #[test]
    fn charge_mismatch_returns_a_reusable_structural_history_action() {
        let id = DocumentID(8);
        let edit = STGStructuralEdit::RemoveEvent {
            target: STGEventTarget { block: 0, event: 0 },
        };
        let source = STGDocument::parse(stg_test_support::complete_stg_fixture().bytes).unwrap();
        let prepared = prepare_edit(
            &Document::STG(source),
            id,
            DocumentEdit::EditSTGStructure { edit },
            usize::MAX,
        )
        .unwrap();
        let PreparedSTGEdit::Changed {
            document,
            inverse,
            retained_bytes,
        } = prepared
        else {
            panic!("different STG structure must prepare a history entry");
        };
        let STGHistoryAction::Structure {
            image,
            opposite_retained_bytes,
            retained_bytes: action_retained_bytes,
        } = *inverse
        else {
            panic!("STG structural edit must prepare a structural inverse");
        };
        let corrupted = STGHistoryAction::Structure {
            image,
            opposite_retained_bytes: opposite_retained_bytes + 1,
            retained_bytes: action_retained_bytes,
        };

        let Err(failure) = apply_history_action(&document, id, corrupted) else {
            panic!("corrupted structural history charge must fail");
        };
        let WorkspaceError::HistoryChargeMismatch { actual, .. } = failure.error else {
            panic!("corrupted structural history charge returned another error");
        };
        assert_eq!(document.event_block(0).unwrap().event_count, 1);
        let STGHistoryAction::Structure {
            image,
            retained_bytes: recovered_retained_bytes,
            ..
        } = failure.action
        else {
            panic!("failed structural history action changed kind");
        };
        assert_eq!(recovered_retained_bytes, retained_bytes);
        let recovered = STGHistoryAction::Structure {
            image,
            opposite_retained_bytes: actual,
            retained_bytes: recovered_retained_bytes,
        };

        let (restored, redo) = apply_history_action(&document, id, recovered).unwrap();
        assert_eq!(restored.event_block(0).unwrap().event_count, 2);
        assert_eq!(redo.retained_bytes(), retained_bytes);
    }
}
