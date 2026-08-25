use kufeditor_formats::{Diagnostic, SaveDocument, SaveEditor, SaveNumberTarget, SaveTextField};

use crate::{Document, DocumentID, Workspace, WorkspaceError};

impl Workspace {
    pub fn save_has_size_prefix(&self, id: DocumentID) -> Result<bool, WorkspaceError> {
        Ok(self.save_document(id)?.has_size_prefix())
    }

    pub fn save_has_context(&self, id: DocumentID) -> Result<bool, WorkspaceError> {
        Ok(self.save_document(id)?.has_context())
    }

    pub fn save_context_text(&self, id: DocumentID) -> Result<&[String], WorkspaceError> {
        Ok(self.save_document(id)?.context_text())
    }

    pub fn save_unit_count(&self, id: DocumentID) -> Result<usize, WorkspaceError> {
        Ok(self.save_document(id)?.unit_count())
    }

    pub fn save_roster_count(&self, id: DocumentID) -> Result<usize, WorkspaceError> {
        Ok(self.save_document(id)?.roster_count())
    }

    pub fn save_second_array_count(&self, id: DocumentID) -> Result<usize, WorkspaceError> {
        Ok(self.save_document(id)?.second_array_count())
    }

    pub fn save_number(
        &self,
        id: DocumentID,
        target: SaveNumberTarget,
    ) -> Result<i64, WorkspaceError> {
        self.save_document(id)?.number(target).map_err(Into::into)
    }

    pub fn save_number_storage_bounds(
        &self,
        id: DocumentID,
        target: SaveNumberTarget,
    ) -> Result<(i64, i64), WorkspaceError> {
        self.save_document(id)?
            .number_storage_bounds(target)
            .map_err(Into::into)
    }

    pub fn save_number_editor(
        &self,
        id: DocumentID,
        target: SaveNumberTarget,
    ) -> Result<SaveEditor, WorkspaceError> {
        self.save_document(id)?
            .number_editor(target)
            .map_err(Into::into)
    }

    pub fn save_text(
        &self,
        id: DocumentID,
        field: SaveTextField,
    ) -> Result<String, WorkspaceError> {
        self.save_document(id)?.text(field).map_err(Into::into)
    }

    pub fn save_unit_skill_data(
        &self,
        id: DocumentID,
        unit: usize,
    ) -> Result<[u8; 24], WorkspaceError> {
        self.save_document(id)?
            .unit_skill_data(unit)
            .map_err(Into::into)
    }

    pub fn save_diagnostics(&self, id: DocumentID) -> Result<Vec<Diagnostic>, WorkspaceError> {
        Ok(self.save_document(id)?.diagnostics())
    }

    fn save_document(&self, id: DocumentID) -> Result<&SaveDocument, WorkspaceError> {
        let session = self.session(id)?;
        match &session.document {
            Document::Save(document) => Ok(document),
            _ => Err(WorkspaceError::NotSave(id)),
        }
    }
}
