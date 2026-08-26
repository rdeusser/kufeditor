//! GPUI-free document sessions, history, validation, and save coordination.

mod document;
mod history;
mod recent;
mod save;
mod stg;
mod storage;

use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

use kufeditor_formats::{FormatError, STGRebaseError};

pub use document::{Document, DocumentEdit, DocumentID, DocumentKind, StateID};
pub use history::DEFAULT_STG_HISTORY_LIMIT;
pub use kufeditor_formats::{
    Diagnostic, DiagnosticLocation, STGAbilityOwner, STGAreaField, STGAreaFloatField, STGChoice,
    STGDocument, STGEditor, STGEvent, STGEventBlock, STGEventTarget, STGFieldAccess,
    STGFloatTarget, STGFloatValue, STGFooterField, STGHeaderTextField, STGNumberTarget,
    STGParameter, STGParameterTarget, STGReferenceKind, STGScript, STGScriptKind, STGScriptLabel,
    STGScriptTarget, STGSkillField, STGSkillOwner, STGStructuralEdit, STGTailStatus, STGText,
    STGTextTarget, STGUnitField, STGUnitFloatField, STGUnitGroup, STGValue, STGValueKind,
    STGValueTarget, SaveChoice, SaveDocument, SaveEditor, SaveEquipmentField, SaveEquipmentGroup,
    SaveEquipmentSlot, SaveMainField, SaveNumberTarget, SaveRosterField, SaveTextField,
    SaveUnitField, SaveUnitGroup, Severity, SkillDocument, SkillTextField, TextSOXDocument,
    TextSOXField, TroopDocument, TroopField, TroopGroup,
};
pub use recent::{
    DEFAULT_RECENT_FILE_LIMIT, RECENT_FILE_LIMITS, RecentFiles, normalize_recent_limit,
};
pub use storage::{
    LoadedDocument, SUPPORTED_OPEN_EXTENSIONS, SaveRequest, SaveToken, SavedDocument, load_path,
};
use thiserror::Error;

use crate::{
    history::{DocumentMutation, HistoryAction, HistoryEntry},
    stg::PreparedSTGEdit,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplyOutcome {
    Changed,
    Unchanged,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("document {0:?} is not open")]
    UnknownDocument(DocumentID),

    #[error("document {0:?} is not a TroopInfo document")]
    NotTroop(DocumentID),

    #[error("document {0:?} is not a SkillInfo document")]
    NotSkill(DocumentID),

    #[error("document {0:?} is not a text SOX document")]
    NotTextSOX(DocumentID),

    #[error("document {0:?} is not a Crusaders save document")]
    NotSave(DocumentID),

    #[error("document {0:?} is not a Crusaders STG document")]
    NotSTG(DocumentID),

    #[error("history entry retains {requested} bytes, exceeding the {maximum}-byte limit")]
    HistoryBudgetExceeded { requested: usize, maximum: usize },

    #[error("history retained-byte calculation overflowed")]
    HistoryChargeOverflow,

    #[error("history charge projected {projected} bytes, but produced {actual} bytes")]
    HistoryChargeMismatch { projected: usize, actual: usize },

    #[error("history for document {0:?} no longer matches its document state")]
    HistoryStateMismatch(DocumentID),

    #[error(transparent)]
    Format(#[from] kufeditor_formats::FormatError),

    #[error(
        "unsupported file {path}: expected a .sox TroopInfo, SkillInfo, or text SOX file, a .sav Crusaders save file, or a .stg Crusaders STG file"
    )]
    UnsupportedFile { path: PathBuf },

    #[error("cannot save {path}: expected .{expected}, found .{actual}")]
    WrongExtension {
        path: PathBuf,
        expected: &'static str,
        actual: String,
    },

    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: kufeditor_formats::FormatError,
    },

    #[error("failed to encode {path}: {source}")]
    Encode {
        path: PathBuf,
        #[source]
        source: kufeditor_formats::FormatError,
    },

    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "target file was already committed at {path}, but save reconciliation failed: {source}"
    )]
    CommittedSaveReconciliation {
        path: PathBuf,
        #[source]
        source: kufeditor_formats::FormatError,
    },

    #[error("document {0:?} already has a save in progress")]
    SaveInProgress(DocumentID),

    #[error("save completion {token:?} does not match document {document:?}")]
    StaleSave {
        document: DocumentID,
        token: SaveToken,
    },
}

#[derive(Debug)]
struct Session {
    path: PathBuf,
    document: Document,
    current_state: StateID,
    saved_state: StateID,
    undo: VecDeque<HistoryEntry>,
    redo: VecDeque<HistoryEntry>,
    history_retained_bytes: usize,
    save_in_flight: Option<SaveToken>,
}

impl Session {
    fn clear_redo_history(&mut self) -> Result<(), WorkspaceError> {
        while let Some(entry) = self.redo.pop_back() {
            self.history_retained_bytes = self
                .history_retained_bytes
                .checked_sub(entry.retained_bytes())
                .ok_or(WorkspaceError::HistoryChargeOverflow)?;
        }
        Ok(())
    }

    fn evict_undo_for(
        &mut self,
        retained_bytes: usize,
        history_limit: usize,
    ) -> Result<(), WorkspaceError> {
        while self
            .history_retained_bytes
            .checked_add(retained_bytes)
            .ok_or(WorkspaceError::HistoryChargeOverflow)?
            > history_limit
        {
            let Some(entry) = self.undo.pop_front() else {
                return Err(WorkspaceError::HistoryBudgetExceeded {
                    requested: retained_bytes,
                    maximum: history_limit,
                });
            };
            self.history_retained_bytes = self
                .history_retained_bytes
                .checked_sub(entry.retained_bytes())
                .ok_or(WorkspaceError::HistoryChargeOverflow)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Workspace {
    sessions: HashMap<DocumentID, Session>,
    open_order: Vec<DocumentID>,
    next_document: u64,
    next_state: u64,
    next_save: u64,
    stg_history_limit: usize,
}

impl Workspace {
    pub fn new() -> Self {
        Self::with_stg_history_limit(DEFAULT_STG_HISTORY_LIMIT)
    }

    pub fn with_stg_history_limit(stg_history_limit: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            open_order: Vec::new(),
            next_document: 1,
            next_state: 1,
            next_save: 1,
            stg_history_limit,
        }
    }

    pub fn open_loaded(&mut self, path: PathBuf, document: Document) -> DocumentID {
        let id = self.allocate_document();
        let state = self.allocate_state();
        self.sessions.insert(
            id,
            Session {
                path,
                document,
                current_state: state,
                saved_state: state,
                undo: VecDeque::new(),
                redo: VecDeque::new(),
                history_retained_bytes: 0,
                save_in_flight: None,
            },
        );
        self.open_order.push(id);
        id
    }

    pub fn document_ids(&self) -> &[DocumentID] {
        &self.open_order
    }

    pub fn insert_loaded(&mut self, loaded: LoadedDocument) -> DocumentID {
        let (path, document) = loaded.into_parts();
        self.open_loaded(path, document)
    }

    pub fn prepare_save(
        &mut self,
        id: DocumentID,
        target: Option<PathBuf>,
    ) -> Result<SaveRequest, WorkspaceError> {
        let (path, state, snapshot) = {
            let session = self.session(id)?;
            if session.save_in_flight.is_some() {
                return Err(WorkspaceError::SaveInProgress(id));
            }
            let path = match target {
                Some(path) => storage::normalize_save_target(path, session.document.kind())?,
                None => session.path.clone(),
            };
            (path, session.current_state, session.document.clone())
        };
        let token = self.allocate_save();
        self.session_mut(id)?.save_in_flight = Some(token);
        Ok(SaveRequest {
            document_id: id,
            token,
            path,
            state,
            snapshot,
        })
    }

    pub fn finish_save(&mut self, saved: SavedDocument) -> Result<(), WorkspaceError> {
        let SavedDocument {
            document_id,
            token,
            path,
            state,
            committed,
        } = saved;
        let session = self.session_mut(document_id)?;
        if session.save_in_flight != Some(token) {
            return Err(WorkspaceError::StaleSave {
                document: document_id,
                token,
            });
        }

        let reconciliation = match committed {
            storage::CommittedDocumentImage::Standard { snapshot, bytes } => {
                let mut document = session.document.clone();
                document
                    .rebase_source(&snapshot, bytes)
                    .map(|()| session.document = document)
            }
            storage::CommittedDocumentImage::STG(image) => match &mut session.document {
                Document::STG(document) => document.rebase_source(image),
                _ => Err(FormatError::STGRebase(STGRebaseError::InconsistentImage)),
            },
        };
        if let Err(source) = reconciliation {
            session.save_in_flight = None;
            return Err(WorkspaceError::CommittedSaveReconciliation { path, source });
        }
        session.path = path;
        session.saved_state = state;
        session.save_in_flight = None;
        Ok(())
    }

    pub fn finish_save_failure(
        &mut self,
        id: DocumentID,
        token: SaveToken,
    ) -> Result<(), WorkspaceError> {
        let session = self.session_mut(id)?;
        if session.save_in_flight != Some(token) {
            return Err(WorkspaceError::StaleSave {
                document: id,
                token,
            });
        }
        session.save_in_flight = None;
        Ok(())
    }

    pub fn apply(
        &mut self,
        id: DocumentID,
        edit: DocumentEdit,
    ) -> Result<ApplyOutcome, WorkspaceError> {
        if edit.is_stg() {
            return self.apply_stg(id, edit);
        }
        let action = HistoryAction::Edit(edit);
        let (before, inverse) = {
            let session = self.session_mut(id)?;
            let before = session.current_state;
            match session.document.apply(id, action.clone())? {
                DocumentMutation::Unchanged => return Ok(ApplyOutcome::Unchanged),
                DocumentMutation::Changed { inverse } => (before, inverse),
            }
        };
        let after = self.allocate_state();
        let session = self.session_mut(id)?;
        session.undo.push_back(HistoryEntry::Standard {
            forward: action,
            inverse,
            before,
            after,
        });
        session.redo.clear();
        session.current_state = after;
        Ok(ApplyOutcome::Changed)
    }

    fn apply_stg(
        &mut self,
        id: DocumentID,
        edit: DocumentEdit,
    ) -> Result<ApplyOutcome, WorkspaceError> {
        let (before, prepared) = {
            let session = self.session(id)?;
            (
                session.current_state,
                stg::prepare_edit(&session.document, id, edit, self.stg_history_limit)?,
            )
        };
        let PreparedSTGEdit::Changed {
            document,
            inverse,
            retained_bytes,
        } = prepared
        else {
            return Ok(ApplyOutcome::Unchanged);
        };
        let inverse = *inverse;
        if inverse.retained_bytes() != retained_bytes {
            return Err(WorkspaceError::HistoryChargeMismatch {
                projected: retained_bytes,
                actual: inverse.retained_bytes(),
            });
        }

        let after = self.allocate_state();
        let history_limit = self.stg_history_limit;
        let session = self.session_mut(id)?;
        session.clear_redo_history()?;
        session.evict_undo_for(retained_bytes, history_limit)?;
        session.document = Document::STG(document);
        session.undo.push_back(HistoryEntry::STG {
            action: inverse,
            before,
            after,
            retained_bytes,
        });
        session.history_retained_bytes = session
            .history_retained_bytes
            .checked_add(retained_bytes)
            .ok_or(WorkspaceError::HistoryChargeOverflow)?;
        session.current_state = after;
        Ok(ApplyOutcome::Changed)
    }

    pub fn undo(&mut self, id: DocumentID) -> Result<bool, WorkspaceError> {
        let session = self.session_mut(id)?;
        let Some(entry) = session.undo.pop_back() else {
            return Ok(false);
        };
        match entry {
            HistoryEntry::Standard {
                mut forward,
                inverse,
                before,
                after,
            } => {
                let next = match session.document.apply(id, inverse.clone()) {
                    Ok(DocumentMutation::Changed { inverse }) => inverse,
                    Ok(DocumentMutation::Unchanged) => {
                        session.undo.push_back(HistoryEntry::Standard {
                            forward,
                            inverse,
                            before,
                            after,
                        });
                        return Ok(false);
                    }
                    Err(error) => {
                        session.undo.push_back(HistoryEntry::Standard {
                            forward,
                            inverse,
                            before,
                            after,
                        });
                        return Err(error);
                    }
                };
                forward = next;
                session.current_state = before;
                session.redo.push_back(HistoryEntry::Standard {
                    forward,
                    inverse,
                    before,
                    after,
                });
                Ok(true)
            }
            HistoryEntry::STG {
                action,
                before,
                after,
                retained_bytes,
            } => {
                let Document::STG(document) = &session.document else {
                    session.undo.push_back(HistoryEntry::STG {
                        action,
                        before,
                        after,
                        retained_bytes,
                    });
                    return Err(WorkspaceError::HistoryStateMismatch(id));
                };
                let (document, action) = match stg::apply_history_action(document, id, action) {
                    Ok(result) => result,
                    Err(failure) => {
                        session.undo.push_back(HistoryEntry::STG {
                            action: failure.action,
                            before,
                            after,
                            retained_bytes,
                        });
                        return Err(failure.error);
                    }
                };
                debug_assert_eq!(action.retained_bytes(), retained_bytes);
                session.document = Document::STG(document);
                session.current_state = before;
                session.redo.push_back(HistoryEntry::STG {
                    action,
                    before,
                    after,
                    retained_bytes,
                });
                Ok(true)
            }
        }
    }

    pub fn redo(&mut self, id: DocumentID) -> Result<bool, WorkspaceError> {
        let session = self.session_mut(id)?;
        let Some(entry) = session.redo.pop_back() else {
            return Ok(false);
        };
        match entry {
            HistoryEntry::Standard {
                forward,
                mut inverse,
                before,
                after,
            } => {
                let next = match session.document.apply(id, forward.clone()) {
                    Ok(DocumentMutation::Changed { inverse }) => inverse,
                    Ok(DocumentMutation::Unchanged) => {
                        session.redo.push_back(HistoryEntry::Standard {
                            forward,
                            inverse,
                            before,
                            after,
                        });
                        return Ok(false);
                    }
                    Err(error) => {
                        session.redo.push_back(HistoryEntry::Standard {
                            forward,
                            inverse,
                            before,
                            after,
                        });
                        return Err(error);
                    }
                };
                inverse = next;
                session.current_state = after;
                session.undo.push_back(HistoryEntry::Standard {
                    forward,
                    inverse,
                    before,
                    after,
                });
                Ok(true)
            }
            HistoryEntry::STG {
                action,
                before,
                after,
                retained_bytes,
            } => {
                let Document::STG(document) = &session.document else {
                    session.redo.push_back(HistoryEntry::STG {
                        action,
                        before,
                        after,
                        retained_bytes,
                    });
                    return Err(WorkspaceError::HistoryStateMismatch(id));
                };
                let (document, action) = match stg::apply_history_action(document, id, action) {
                    Ok(result) => result,
                    Err(failure) => {
                        session.redo.push_back(HistoryEntry::STG {
                            action: failure.action,
                            before,
                            after,
                            retained_bytes,
                        });
                        return Err(failure.error);
                    }
                };
                debug_assert_eq!(action.retained_bytes(), retained_bytes);
                session.document = Document::STG(document);
                session.current_state = after;
                session.undo.push_back(HistoryEntry::STG {
                    action,
                    before,
                    after,
                    retained_bytes,
                });
                Ok(true)
            }
        }
    }

    pub fn can_undo(&self, id: DocumentID) -> Result<bool, WorkspaceError> {
        self.session(id).map(|session| !session.undo.is_empty())
    }

    pub fn can_redo(&self, id: DocumentID) -> Result<bool, WorkspaceError> {
        self.session(id).map(|session| !session.redo.is_empty())
    }

    pub fn is_dirty(&self, id: DocumentID) -> Result<bool, WorkspaceError> {
        self.session(id)
            .map(|session| session.current_state != session.saved_state)
    }

    pub fn save_in_progress(&self, id: DocumentID) -> Result<bool, WorkspaceError> {
        self.session(id)
            .map(|session| session.save_in_flight.is_some())
    }

    pub fn state_id(&self, id: DocumentID) -> Result<StateID, WorkspaceError> {
        self.session(id).map(|session| session.current_state)
    }

    pub fn history_retained_bytes(&self, id: DocumentID) -> Result<usize, WorkspaceError> {
        self.session(id)
            .map(|session| session.history_retained_bytes)
    }

    pub fn path(&self, id: DocumentID) -> Result<&Path, WorkspaceError> {
        self.session(id).map(|session| session.path.as_path())
    }

    pub fn title(&self, id: DocumentID) -> Result<String, WorkspaceError> {
        self.session(id).map(|session| {
            session
                .path
                .file_name()
                .unwrap_or(session.path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
    }

    pub fn document_kind(&self, id: DocumentID) -> Result<DocumentKind, WorkspaceError> {
        self.session(id).map(|session| session.document.kind())
    }

    pub fn record_count(&self, id: DocumentID) -> Result<usize, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => Ok(document.record_count()),
            Document::Skill(document) => Ok(document.record_count()),
            Document::TextSOX(document) => Ok(document.record_count()),
            Document::Save(document) => Ok(document.unit_count()),
            Document::STG(document) => Ok(document.unit_count()),
        }
    }

    pub fn troop_value(
        &self,
        id: DocumentID,
        record: usize,
        field: TroopField,
    ) -> Result<i32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => document.value(record, field).map_err(Into::into),
            _ => Err(WorkspaceError::NotTroop(id)),
        }
    }

    pub fn skill_id(&self, id: DocumentID, record: usize) -> Result<i32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.skill_id(record).map_err(Into::into),
            _ => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn skill_type(&self, id: DocumentID, record: usize) -> Result<u32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.skill_type(record).map_err(Into::into),
            _ => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn skill_max_level(&self, id: DocumentID, record: usize) -> Result<u32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.max_level(record).map_err(Into::into),
            _ => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn skill_text(
        &self,
        id: DocumentID,
        record: usize,
        field: SkillTextField,
    ) -> Result<&str, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.text(record, field).map_err(Into::into),
            _ => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn text_sox_index(&self, id: DocumentID, record: usize) -> Result<u32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::TextSOX(document) => document.record_index(record).map_err(Into::into),
            _ => Err(WorkspaceError::NotTextSOX(id)),
        }
    }

    pub fn text_sox_max_length(
        &self,
        id: DocumentID,
        record: usize,
    ) -> Result<u16, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::TextSOX(document) => document.max_length(record).map_err(Into::into),
            _ => Err(WorkspaceError::NotTextSOX(id)),
        }
    }

    pub fn text_sox_text(&self, id: DocumentID, record: usize) -> Result<&str, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::TextSOX(document) => document.text(record).map_err(Into::into),
            _ => Err(WorkspaceError::NotTextSOX(id)),
        }
    }

    pub fn diagnostics(&self, id: DocumentID) -> Result<Vec<Diagnostic>, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => Ok(document.diagnostics()),
            Document::Skill(document) => Ok(document.diagnostics()),
            Document::TextSOX(document) => Ok(document.diagnostics()),
            Document::Save(document) => Ok(document.diagnostics()),
            Document::STG(document) => Ok(document.diagnostics()),
        }
    }

    fn session(&self, id: DocumentID) -> Result<&Session, WorkspaceError> {
        self.sessions
            .get(&id)
            .ok_or(WorkspaceError::UnknownDocument(id))
    }

    fn session_mut(&mut self, id: DocumentID) -> Result<&mut Session, WorkspaceError> {
        self.sessions
            .get_mut(&id)
            .ok_or(WorkspaceError::UnknownDocument(id))
    }

    fn allocate_document(&mut self) -> DocumentID {
        let id = DocumentID(self.next_document);
        self.next_document += 1;
        id
    }

    fn allocate_state(&mut self) -> StateID {
        let id = StateID(self.next_state);
        self.next_state += 1;
        id
    }

    fn allocate_save(&mut self) -> SaveToken {
        let token = SaveToken(self.next_save);
        self.next_save += 1;
        token
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use kufeditor_formats::FormatError;
    use tempfile::tempdir;

    use super::*;

    fn troop_fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(0..4)
            .unwrap()
            .copy_from_slice(&100_u32.to_le_bytes());
        bytes
            .get_mut(4..8)
            .unwrap()
            .copy_from_slice(&1_u32.to_le_bytes());
        bytes
            .get_mut(16..20)
            .unwrap()
            .copy_from_slice(&130_i32.to_le_bytes());
        bytes
            .get_mut(64..68)
            .unwrap()
            .copy_from_slice(&100_i32.to_le_bytes());
        bytes
            .get_mut(108..112)
            .unwrap()
            .copy_from_slice(&800_i32.to_le_bytes());
        bytes
    }

    fn move_speed(value: i32) -> DocumentEdit {
        DocumentEdit::SetTroopField {
            record: 0,
            field: TroopField::MoveSpeed,
            value,
        }
    }

    fn empty_stg_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_001_u32.to_le_bytes());
        bytes.resize(bytes.len() + 620, 0);
        for _ in 0..6 {
            bytes.extend_from_slice(&0_u32.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn committed_save_reconciliation_failure_is_atomic_and_unlocks_the_document() {
        let directory = tempdir().unwrap();
        let original_path = PathBuf::from("TroopInfo.sox");
        let committed_path = directory.path().join("Committed.sox");
        let document = TroopDocument::parse(troop_fixture()).unwrap();
        let mut workspace = Workspace::new();
        let id = workspace.open_loaded(original_path.clone(), Document::Troop(document));

        workspace.apply(id, move_speed(175)).unwrap();
        let request = workspace
            .prepare_save(id, Some(committed_path.clone()))
            .unwrap();
        let mut saved = request.run().unwrap();
        let committed_bytes = fs::read(&committed_path).unwrap();
        workspace.apply(id, move_speed(200)).unwrap();

        let before = workspace.session(id).unwrap();
        let before_document = before.document.encode().unwrap();
        let before_state = before.current_state;
        let before_saved_state = before.saved_state;
        let before_undo_len = before.undo.len();
        let before_redo_len = before.redo.len();
        let storage::CommittedDocumentImage::Standard { bytes, .. } = &mut saved.committed else {
            panic!("TroopInfo save produced an STG committed image");
        };
        bytes.clear();

        let error = workspace.finish_save(saved).unwrap_err();

        assert!(matches!(
            error,
            WorkspaceError::CommittedSaveReconciliation {
                path,
                source: FormatError::InconsistentSOXRebase,
            } if path == committed_path
        ));
        let after = workspace.session(id).unwrap();
        assert_eq!(after.path, original_path);
        assert_eq!(after.document.encode().unwrap(), before_document);
        assert_eq!(after.current_state, before_state);
        assert_eq!(after.saved_state, before_saved_state);
        assert_eq!(after.undo.len(), before_undo_len);
        assert_eq!(after.redo.len(), before_redo_len);
        assert!(after.save_in_flight.is_none());
        assert_eq!(fs::read(&committed_path).unwrap(), committed_bytes);

        let loaded = load_path(committed_path).unwrap();
        let Document::Troop(document) = loaded.document() else {
            panic!("committed TroopInfo was detected as another document kind");
        };
        assert_eq!(document.value(0, TroopField::MoveSpeed).unwrap(), 175);

        let retry = workspace
            .prepare_save(id, Some(directory.path().join("Retry.sox")))
            .unwrap();
        workspace.finish_save_failure(id, retry.token()).unwrap();
    }

    #[test]
    fn committed_stg_image_failure_is_atomic_and_reports_the_committed_target() {
        let directory = tempdir().unwrap();
        let original_path = PathBuf::from("original.stg");
        let committed_path = directory.path().join("committed.stg");
        let document = STGDocument::parse(empty_stg_fixture()).unwrap();
        let mut workspace = Workspace::new();
        let id = workspace.open_loaded(original_path.clone(), Document::STG(document));
        workspace
            .apply(
                id,
                DocumentEdit::EditSTGStructure {
                    edit: STGStructuralEdit::InsertEvent {
                        target: STGEventTarget { block: 0, event: 0 },
                    },
                },
            )
            .unwrap();

        let request = workspace
            .prepare_save(id, Some(committed_path.clone()))
            .unwrap();
        let mut saved = request.run().unwrap();
        let committed_bytes = fs::read(&committed_path).unwrap();
        workspace
            .apply(
                id,
                DocumentEdit::SetSTGNumber {
                    target: STGNumberTarget::EventID { block: 0, event: 0 },
                    value: 9,
                },
            )
            .unwrap();

        let foreign = STGDocument::parse(empty_stg_fixture())
            .unwrap()
            .prepare_commit()
            .unwrap();
        saved.committed = storage::CommittedDocumentImage::STG(foreign);
        let before = workspace.session(id).unwrap();
        let before_document = before.document.encode().unwrap();
        let before_state = before.current_state;
        let before_saved_state = before.saved_state;
        let before_undo_len = before.undo.len();
        let before_redo_len = before.redo.len();

        let error = workspace.finish_save(saved).unwrap_err();

        assert!(matches!(
            &error,
            WorkspaceError::CommittedSaveReconciliation {
                path,
                source: FormatError::STGRebase(STGRebaseError::ForeignLineage),
            } if path == &committed_path
        ));
        assert!(
            error
                .to_string()
                .contains("target file was already committed")
        );
        let after = workspace.session(id).unwrap();
        assert_eq!(after.path, original_path);
        assert_eq!(after.document.encode().unwrap(), before_document);
        assert_eq!(after.current_state, before_state);
        assert_eq!(after.saved_state, before_saved_state);
        assert_eq!(after.undo.len(), before_undo_len);
        assert_eq!(after.redo.len(), before_redo_len);
        assert!(after.save_in_flight.is_none());
        assert!(workspace.is_dirty(id).unwrap());
        assert_eq!(fs::read(committed_path).unwrap(), committed_bytes);
    }

    #[test]
    fn wrong_save_extension_does_not_allocate_a_token() {
        let document = TroopDocument::parse(troop_fixture()).unwrap();
        let mut workspace = Workspace::new();
        let id = workspace.open_loaded(PathBuf::from("TroopInfo.sox"), Document::Troop(document));
        let next_save = workspace.next_save;

        let error = workspace
            .prepare_save(id, Some(PathBuf::from("TroopInfo.sav")))
            .unwrap_err();

        assert!(matches!(error, WorkspaceError::WrongExtension { .. }));
        assert_eq!(workspace.next_save, next_save);
        assert!(workspace.session(id).unwrap().save_in_flight.is_none());
    }

    #[test]
    fn wrong_document_kind_keeps_popped_stg_history_entries_and_charges() {
        let stg = STGDocument::parse(empty_stg_fixture()).unwrap();
        let troop = TroopDocument::parse(troop_fixture()).unwrap();
        let edit = STGStructuralEdit::InsertEvent {
            target: STGEventTarget { block: 0, event: 0 },
        };

        let mut undo_workspace = Workspace::new();
        let undo_id = undo_workspace.open_loaded(PathBuf::from("undo.stg"), Document::STG(stg));
        undo_workspace
            .apply(undo_id, DocumentEdit::EditSTGStructure { edit })
            .unwrap();
        let undo_charge = undo_workspace.history_retained_bytes(undo_id).unwrap();
        undo_workspace.session_mut(undo_id).unwrap().document = Document::Troop(troop.clone());
        assert!(matches!(
            undo_workspace.undo(undo_id),
            Err(WorkspaceError::HistoryStateMismatch(id)) if id == undo_id
        ));
        assert!(undo_workspace.can_undo(undo_id).unwrap());
        assert_eq!(
            undo_workspace.history_retained_bytes(undo_id).unwrap(),
            undo_charge
        );

        let mut redo_workspace = Workspace::new();
        let redo_id = redo_workspace.open_loaded(
            PathBuf::from("redo.stg"),
            Document::STG(STGDocument::parse(empty_stg_fixture()).unwrap()),
        );
        redo_workspace
            .apply(redo_id, DocumentEdit::EditSTGStructure { edit })
            .unwrap();
        assert!(redo_workspace.undo(redo_id).unwrap());
        let redo_charge = redo_workspace.history_retained_bytes(redo_id).unwrap();
        redo_workspace.session_mut(redo_id).unwrap().document = Document::Troop(troop);
        assert!(matches!(
            redo_workspace.redo(redo_id),
            Err(WorkspaceError::HistoryStateMismatch(id)) if id == redo_id
        ));
        assert!(redo_workspace.can_redo(redo_id).unwrap());
        assert_eq!(
            redo_workspace.history_retained_bytes(redo_id).unwrap(),
            redo_charge
        );
    }
}
