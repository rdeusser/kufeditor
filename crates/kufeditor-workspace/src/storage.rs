use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use kufeditor_formats::{SOXDocument, SaveDocument, parse_sox};

use crate::{Document, DocumentID, DocumentKind, StateID, WorkspaceError};

#[derive(Debug)]
pub struct LoadedDocument {
    path: PathBuf,
    document: Document,
}

impl LoadedDocument {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn into_parts(self) -> (PathBuf, Document) {
        (self.path, self.document)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaveToken(pub(crate) u64);

#[derive(Debug)]
pub struct SaveRequest {
    pub(crate) document_id: DocumentID,
    pub(crate) token: SaveToken,
    pub(crate) path: PathBuf,
    pub(crate) state: StateID,
    pub(crate) snapshot: Document,
}

impl SaveRequest {
    pub fn document_id(&self) -> DocumentID {
        self.document_id
    }

    pub fn token(&self) -> SaveToken {
        self.token
    }

    pub fn run(self) -> Result<SavedDocument, WorkspaceError> {
        let bytes = self
            .snapshot
            .encode()
            .map_err(|source| WorkspaceError::Encode {
                path: self.path.clone(),
                source,
            })?;
        let parent = save_parent(&self.path)?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| WorkspaceError::Write {
                path: self.path.clone(),
                source,
            })?;
        temporary
            .write_all(&bytes)
            .map_err(|source| WorkspaceError::Write {
                path: self.path.clone(),
                source,
            })?;
        temporary
            .as_file_mut()
            .sync_all()
            .map_err(|source| WorkspaceError::Write {
                path: self.path.clone(),
                source,
            })?;
        temporary
            .persist(&self.path)
            .map_err(|error| WorkspaceError::Write {
                path: self.path.clone(),
                source: error.error,
            })?;

        Ok(SavedDocument {
            document_id: self.document_id,
            token: self.token,
            path: self.path,
            state: self.state,
            snapshot: self.snapshot,
            bytes,
        })
    }
}

#[derive(Debug)]
pub struct SavedDocument {
    pub(crate) document_id: DocumentID,
    pub(crate) token: SaveToken,
    pub(crate) path: PathBuf,
    pub(crate) state: StateID,
    pub(crate) snapshot: Document,
    pub(crate) bytes: Vec<u8>,
}

pub fn load_path(path: PathBuf) -> Result<LoadedDocument, WorkspaceError> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    let is_sox = extension.is_some_and(|extension| extension.eq_ignore_ascii_case("sox"));
    let is_save = extension.is_some_and(|extension| extension.eq_ignore_ascii_case("sav"));
    if !is_sox && !is_save {
        return Err(WorkspaceError::UnsupportedFile { path });
    }

    let bytes = fs::read(&path).map_err(|source| WorkspaceError::Read {
        path: path.clone(),
        source,
    })?;
    let document = if is_save {
        SaveDocument::parse(bytes)
            .map(Document::Save)
            .map_err(|source| WorkspaceError::Parse {
                path: path.clone(),
                source,
            })?
    } else {
        match parse_sox(bytes).map_err(|source| WorkspaceError::Parse {
            path: path.clone(),
            source,
        })? {
            SOXDocument::Troop(document) => Document::Troop(document),
            SOXDocument::Skill(document) => Document::Skill(document),
            SOXDocument::Text(document) => Document::TextSOX(document),
        }
    };
    Ok(LoadedDocument { path, document })
}

pub(crate) fn normalize_save_target(
    mut path: PathBuf,
    kind: DocumentKind,
) -> Result<PathBuf, WorkspaceError> {
    let expected = match kind {
        DocumentKind::TroopInfo | DocumentKind::SkillInfo | DocumentKind::TextSOX => "sox",
        DocumentKind::CrusadersSave => "sav",
    };
    let Some(actual) = path.extension() else {
        path.set_extension(expected);
        return Ok(path);
    };
    if actual
        .to_str()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
    {
        return Ok(path);
    }

    let actual = actual.to_string_lossy().into_owned();
    Err(WorkspaceError::WrongExtension {
        path,
        expected,
        actual,
    })
}

fn save_parent(path: &Path) -> Result<&Path, WorkspaceError> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => Ok(Path::new(".")),
        Some(parent) => Ok(parent),
        None => Err(WorkspaceError::Write {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
        }),
    }
}
