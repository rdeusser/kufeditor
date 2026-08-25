//! GPUI-free document sessions, history, validation, and save coordination.

mod document;
mod history;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub use document::{Document, DocumentEdit, DocumentId, StateId};
pub use kufeditor_formats::{Diagnostic, Severity, TroopDocument, TroopField, TroopGroup};
use thiserror::Error;

use crate::history::HistoryEntry;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("document {0:?} is not open")]
    UnknownDocument(DocumentId),

    #[error("document {0:?} is not a TroopInfo document")]
    NotTroop(DocumentId),

    #[error(transparent)]
    Format(#[from] kufeditor_formats::FormatError),
}

#[derive(Debug)]
struct Session {
    path: PathBuf,
    document: Document,
    current_state: StateId,
    saved_state: StateId,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

#[derive(Debug)]
pub struct Workspace {
    sessions: HashMap<DocumentId, Session>,
    next_document: u64,
    next_state: u64,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_document: 1,
            next_state: 1,
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
            },
        );
        id
    }

    pub fn apply(&mut self, id: DocumentId, edit: DocumentEdit) -> Result<(), WorkspaceError> {
        let (before, inverse) = {
            let session = self.session_mut(id)?;
            let before = session.current_state;
            let inverse = session.document.apply(edit)?;
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

        if let Err(error) = session.document.apply(entry.inverse) {
            session.undo.push(entry);
            return Err(error.into());
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

        if let Err(error) = session.document.apply(entry.forward) {
            session.redo.push(entry);
            return Err(error.into());
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

    pub fn record_count(&self, id: DocumentId) -> Result<usize, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => Ok(document.record_count()),
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
        }
    }

    pub fn diagnostics(&self, id: DocumentId) -> Result<Vec<Diagnostic>, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Troop(document) => Ok(document.diagnostics()),
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
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}
