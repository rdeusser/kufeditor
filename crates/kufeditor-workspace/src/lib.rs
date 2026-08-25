//! GPUI-free document sessions, history, validation, and save coordination.

mod document;
mod history;
mod storage;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use document::{Document, DocumentEdit, DocumentId, DocumentKind, StateId};
pub use kufeditor_formats::{
    Diagnostic, Severity, SkillDocument, SkillTextField, TroopDocument, TroopField, TroopGroup,
};
pub use storage::{LoadedDocument, SaveRequest, SaveToken, SavedDocument, load_path};
use thiserror::Error;

use crate::history::HistoryEntry;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("document {0:?} is not open")]
    UnknownDocument(DocumentId),

    #[error("document {0:?} is not a TroopInfo document")]
    NotTroop(DocumentId),

    #[error("document {0:?} is not a SkillInfo document")]
    NotSkill(DocumentId),

    #[error(transparent)]
    Format(#[from] kufeditor_formats::FormatError),

    #[error("unsupported file {path}: expected a .sox TroopInfo or SkillInfo file")]
    UnsupportedFile { path: PathBuf },

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

    #[error("document {0:?} already has a save in progress")]
    SaveInProgress(DocumentId),

    #[error("save completion {token:?} does not match document {document:?}")]
    StaleSave {
        document: DocumentId,
        token: SaveToken,
    },
}

#[derive(Debug)]
struct Session {
    path: PathBuf,
    document: Document,
    current_state: StateId,
    saved_state: StateId,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    save_in_flight: Option<SaveToken>,
}

#[derive(Debug)]
pub struct Workspace {
    sessions: HashMap<DocumentId, Session>,
    open_order: Vec<DocumentId>,
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

    pub fn open_loaded(&mut self, path: PathBuf, document: Document) -> DocumentId {
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

    pub fn document_ids(&self) -> &[DocumentId] {
        &self.open_order
    }

    pub fn insert_loaded(&mut self, loaded: LoadedDocument) -> DocumentId {
        let (path, document) = loaded.into_parts();
        self.open_loaded(path, document)
    }

    pub fn prepare_save(
        &mut self,
        id: DocumentId,
        target: Option<PathBuf>,
    ) -> Result<SaveRequest, WorkspaceError> {
        let (path, state, snapshot) = {
            let session = self.session(id)?;
            if session.save_in_flight.is_some() {
                return Err(WorkspaceError::SaveInProgress(id));
            }
            (
                target.unwrap_or_else(|| session.path.clone()),
                session.current_state,
                session.document.clone(),
            )
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

        session
            .document
            .rebase_source(&saved.snapshot, saved.bytes)?;
        session.path = saved.path;
        session.saved_state = saved.state;
        session.save_in_flight = None;
        Ok(())
    }

    pub fn finish_save_failure(
        &mut self,
        id: DocumentId,
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

    pub fn apply(&mut self, id: DocumentId, edit: DocumentEdit) -> Result<(), WorkspaceError> {
        let (before, inverse) = {
            let session = self.session_mut(id)?;
            let before = session.current_state;
            let inverse = session.document.apply(id, edit.clone())?;
            (before, inverse)
        };
        let after = self.allocate_state();
        let session = self.session_mut(id)?;
        session.undo.push(HistoryEntry {
            forward: edit,
            inverse,
            before,
            after,
        });
        session.redo.clear();
        session.current_state = after;
        Ok(())
    }

    pub fn undo(&mut self, id: DocumentId) -> Result<bool, WorkspaceError> {
        let session = self.session_mut(id)?;
        let Some(entry) = session.undo.pop() else {
            return Ok(false);
        };

        if let Err(error) = session.document.apply(id, entry.inverse.clone()) {
            session.undo.push(entry);
            return Err(error);
        }

        session.current_state = entry.before;
        session.redo.push(entry);
        Ok(true)
    }

    pub fn redo(&mut self, id: DocumentId) -> Result<bool, WorkspaceError> {
        let session = self.session_mut(id)?;
        let Some(entry) = session.redo.pop() else {
            return Ok(false);
        };

        if let Err(error) = session.document.apply(id, entry.forward.clone()) {
            session.redo.push(entry);
            return Err(error);
        }

        session.current_state = entry.after;
        session.undo.push(entry);
        Ok(true)
    }

    pub fn can_undo(&self, id: DocumentId) -> Result<bool, WorkspaceError> {
        self.session(id).map(|session| !session.undo.is_empty())
    }

    pub fn can_redo(&self, id: DocumentId) -> Result<bool, WorkspaceError> {
        self.session(id).map(|session| !session.redo.is_empty())
    }

    pub fn is_dirty(&self, id: DocumentId) -> Result<bool, WorkspaceError> {
        self.session(id)
            .map(|session| session.current_state != session.saved_state)
    }

    pub fn save_in_progress(&self, id: DocumentId) -> Result<bool, WorkspaceError> {
        self.session(id)
            .map(|session| session.save_in_flight.is_some())
    }

    pub fn state_id(&self, id: DocumentId) -> Result<StateId, WorkspaceError> {
        self.session(id).map(|session| session.current_state)
    }

    pub fn path(&self, id: DocumentId) -> Result<&Path, WorkspaceError> {
        self.session(id).map(|session| session.path.as_path())
    }

    pub fn title(&self, id: DocumentId) -> Result<String, WorkspaceError> {
        self.session(id).map(|session| {
            session
                .path
                .file_name()
                .unwrap_or(session.path.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
    }

    pub fn document_kind(&self, id: DocumentId) -> Result<DocumentKind, WorkspaceError> {
        self.session(id).map(|session| session.document.kind())
    }

    pub fn record_count(&self, id: DocumentId) -> Result<usize, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => Ok(document.record_count()),
            Document::Skill(document) => Ok(document.record_count()),
        }
    }

    pub fn troop_value(
        &self,
        id: DocumentId,
        record: usize,
        field: TroopField,
    ) -> Result<i32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => document.value(record, field).map_err(Into::into),
            Document::Skill(_) => Err(WorkspaceError::NotTroop(id)),
        }
    }

    pub fn skill_id(&self, id: DocumentId, record: usize) -> Result<i32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.skill_id(record).map_err(Into::into),
            Document::Troop(_) => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn skill_type(&self, id: DocumentId, record: usize) -> Result<u32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.skill_type(record).map_err(Into::into),
            Document::Troop(_) => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn skill_max_level(&self, id: DocumentId, record: usize) -> Result<u32, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.max_level(record).map_err(Into::into),
            Document::Troop(_) => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn skill_text(
        &self,
        id: DocumentId,
        record: usize,
        field: SkillTextField,
    ) -> Result<&str, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Skill(document) => document.text(record, field).map_err(Into::into),
            Document::Troop(_) => Err(WorkspaceError::NotSkill(id)),
        }
    }

    pub fn diagnostics(&self, id: DocumentId) -> Result<Vec<Diagnostic>, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => Ok(document.diagnostics()),
            Document::Skill(document) => Ok(document.diagnostics()),
        }
    }

    fn session(&self, id: DocumentId) -> Result<&Session, WorkspaceError> {
        self.sessions
            .get(&id)
            .ok_or(WorkspaceError::UnknownDocument(id))
    }

    fn session_mut(&mut self, id: DocumentId) -> Result<&mut Session, WorkspaceError> {
        self.sessions
            .get_mut(&id)
            .ok_or(WorkspaceError::UnknownDocument(id))
    }

    fn allocate_document(&mut self) -> DocumentId {
        let id = DocumentId(self.next_document);
        self.next_document += 1;
        id
    }

    fn allocate_state(&mut self) -> StateId {
        let id = StateId(self.next_state);
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
