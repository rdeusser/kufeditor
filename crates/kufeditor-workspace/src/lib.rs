//! GPUI-free document sessions, history, validation, and save coordination.

mod document;
mod history;
mod recent;
mod save;
mod storage;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use document::{Document, DocumentEdit, DocumentID, DocumentKind, StateID};
pub use kufeditor_formats::{
    Diagnostic, DiagnosticLocation, SaveChoice, SaveDocument, SaveEditor, SaveEquipmentField,
    SaveEquipmentGroup, SaveEquipmentSlot, SaveMainField, SaveNumberTarget, SaveRosterField,
    SaveTextField, SaveUnitField, SaveUnitGroup, Severity, SkillDocument, SkillTextField,
    TextSOXDocument, TextSOXField, TroopDocument, TroopField, TroopGroup,
};
pub use recent::{
    DEFAULT_RECENT_FILE_LIMIT, RECENT_FILE_LIMITS, RecentFiles, normalize_recent_limit,
};
pub use storage::{LoadedDocument, SaveRequest, SaveToken, SavedDocument, load_path};
use thiserror::Error;

use crate::history::{DocumentMutation, HistoryAction, HistoryEntry};

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

    #[error(transparent)]
    Format(#[from] kufeditor_formats::FormatError),

    #[error(
        "unsupported file {path}: expected a .sox TroopInfo, SkillInfo, or text SOX file, or a .sav Crusaders save file"
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

    #[error("save reconciliation failed after commit to {path}: {source}")]
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
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    save_in_flight: Option<SaveToken>,
}

#[derive(Debug)]
pub struct Workspace {
    sessions: HashMap<DocumentID, Session>,
    open_order: Vec<DocumentID>,
    next_document: u64,
    next_state: u64,
    next_save: u64,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            open_order: Vec::new(),
            next_document: 1,
            next_state: 1,
            next_save: 1,
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
                undo: Vec::new(),
                redo: Vec::new(),
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
        let session = self.session_mut(saved.document_id)?;
        if session.save_in_flight != Some(saved.token) {
            return Err(WorkspaceError::StaleSave {
                document: saved.document_id,
                token: saved.token,
            });
        }

        let mut document = session.document.clone();
        if let Err(source) = document.rebase_source(&saved.snapshot, saved.bytes) {
            session.save_in_flight = None;
            return Err(WorkspaceError::CommittedSaveReconciliation {
                path: saved.path,
                source,
            });
        }
        session.document = document;
        session.path = saved.path;
        session.saved_state = saved.state;
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
        session.undo.push(HistoryEntry {
            forward: action,
            inverse,
            before,
            after,
        });
        session.redo.clear();
        session.current_state = after;
        Ok(ApplyOutcome::Changed)
    }

    pub fn undo(&mut self, id: DocumentID) -> Result<bool, WorkspaceError> {
        let session = self.session_mut(id)?;
        let Some(mut entry) = session.undo.pop() else {
            return Ok(false);
        };

        let inverse = match session.document.apply(id, entry.inverse.clone()) {
            Ok(DocumentMutation::Changed { inverse }) => inverse,
            Ok(DocumentMutation::Unchanged) => {
                session.undo.push(entry);
                return Ok(false);
            }
            Err(error) => {
                session.undo.push(entry);
                return Err(error);
            }
        };

        entry.forward = inverse;
        session.current_state = entry.before;
        session.redo.push(entry);
        Ok(true)
    }

    pub fn redo(&mut self, id: DocumentID) -> Result<bool, WorkspaceError> {
        let session = self.session_mut(id)?;
        let Some(mut entry) = session.redo.pop() else {
            return Ok(false);
        };

        let inverse = match session.document.apply(id, entry.forward.clone()) {
            Ok(DocumentMutation::Changed { inverse }) => inverse,
            Ok(DocumentMutation::Unchanged) => {
                session.redo.push(entry);
                return Ok(false);
            }
            Err(error) => {
                session.redo.push(entry);
                return Err(error);
            }
        };

        entry.inverse = inverse;
        session.current_state = entry.after;
        session.undo.push(entry);
        Ok(true)
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
        saved.bytes.clear();

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
}
