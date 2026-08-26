use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use kufeditor_formats::{
    FormatError, MAX_STG_SOURCE_BYTES, SOXDocument, STGCommittedImage, STGDocument, STGParseError,
    SaveDocument, parse_sox,
};

use crate::{Document, DocumentID, DocumentKind, StateID, WorkspaceError};

const SOX_EXTENSION: &str = "sox";
const SAV_EXTENSION: &str = "sav";
const STG_EXTENSION: &str = "stg";

pub const SUPPORTED_OPEN_EXTENSIONS: [&str; 3] = [SOX_EXTENSION, SAV_EXTENSION, STG_EXTENSION];

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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn run(self) -> Result<SavedDocument, WorkspaceError> {
        let SaveRequest {
            document_id,
            token,
            path,
            state,
            snapshot,
        } = self;
        let committed = match snapshot {
            Document::STG(document) => {
                let image = document
                    .prepare_commit()
                    .map_err(|source| WorkspaceError::Encode {
                        path: path.clone(),
                        source,
                    })?;
                drop(document);
                CommittedDocumentImage::STG(image)
            }
            snapshot => {
                let bytes = snapshot.encode().map_err(|source| WorkspaceError::Encode {
                    path: path.clone(),
                    source,
                })?;
                CommittedDocumentImage::Standard {
                    snapshot: Box::new(snapshot),
                    bytes,
                }
            }
        };
        write_atomic(&path, committed.bytes())?;

        Ok(SavedDocument {
            document_id,
            token,
            path,
            state,
            committed,
        })
    }
}

#[derive(Debug)]
pub(crate) enum CommittedDocumentImage {
    Standard {
        snapshot: Box<Document>,
        bytes: Vec<u8>,
    },
    STG(STGCommittedImage),
}

impl CommittedDocumentImage {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Standard { bytes, .. } => bytes,
            Self::STG(image) => image.bytes(),
        }
    }
}

#[derive(Debug)]
pub struct SavedDocument {
    pub(crate) document_id: DocumentID,
    pub(crate) token: SaveToken,
    pub(crate) path: PathBuf,
    pub(crate) state: StateID,
    pub(crate) committed: CommittedDocumentImage,
}

#[derive(Debug)]
enum STGReadError {
    Read(io::Error),
    TooLarge { length: usize, maximum: usize },
    Allocation { requested: usize },
}

pub fn load_path(path: PathBuf) -> Result<LoadedDocument, WorkspaceError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| {
            SUPPORTED_OPEN_EXTENSIONS
                .iter()
                .copied()
                .find(|supported| extension.eq_ignore_ascii_case(supported))
        });
    let Some(extension) = extension else {
        return Err(WorkspaceError::UnsupportedFile { path });
    };

    let bytes = if extension == STG_EXTENSION {
        read_stg_path(&path, MAX_STG_SOURCE_BYTES)?
    } else {
        fs::read(&path).map_err(|source| WorkspaceError::Read {
            path: path.clone(),
            source,
        })?
    };
    let document = match extension {
        SAV_EXTENSION => SaveDocument::parse(bytes).map(Document::Save),
        STG_EXTENSION => STGDocument::parse(bytes).map(Document::STG),
        _ => parse_sox(bytes).map(|document| match document {
            SOXDocument::Troop(document) => Document::Troop(document),
            SOXDocument::Skill(document) => Document::Skill(document),
            SOXDocument::Text(document) => Document::TextSOX(document),
        }),
    };
    let document = document.map_err(|source| WorkspaceError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok(LoadedDocument { path, document })
}

fn read_stg_path(path: &Path, maximum: usize) -> Result<Vec<u8>, WorkspaceError> {
    let mut file = fs::File::open(path).map_err(|source| WorkspaceError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    read_stg_bytes_limited(&mut file, maximum).map_err(|error| match error {
        STGReadError::Read(source) => WorkspaceError::Read {
            path: path.to_path_buf(),
            source,
        },
        STGReadError::TooLarge { length, maximum } => WorkspaceError::Parse {
            path: path.to_path_buf(),
            source: FormatError::STGParse(STGParseError::SourceTooLarge { length, maximum }),
        },
        STGReadError::Allocation { requested } => WorkspaceError::Read {
            path: path.to_path_buf(),
            source: io::Error::other(format!(
                "failed to allocate {requested} bytes while reading STG source"
            )),
        },
    })
}

fn read_stg_bytes_limited(
    reader: &mut (impl Read + ?Sized),
    maximum: usize,
) -> Result<Vec<u8>, STGReadError> {
    let scratch_length = maximum
        .checked_add(1)
        .ok_or(STGReadError::Allocation { requested: maximum })?;
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(scratch_length)
        .map_err(|_| STGReadError::Allocation {
            requested: scratch_length,
        })?;
    scratch.resize(scratch_length, 0);
    let mut scratch = scratch.into_boxed_slice();

    let mut length = 0;
    while length < scratch_length {
        let Some(remaining) = scratch.get_mut(length..) else {
            return Err(STGReadError::TooLarge { length, maximum });
        };
        match reader.read(remaining) {
            Ok(0) => break,
            Ok(count) => {
                length = length.checked_add(count).ok_or(STGReadError::TooLarge {
                    length: usize::MAX,
                    maximum,
                })?;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(STGReadError::Read(error)),
        }
    }
    if length > maximum {
        return Err(STGReadError::TooLarge { length, maximum });
    }

    let mut accepted = Vec::new();
    accepted
        .try_reserve_exact(length)
        .map_err(|_| STGReadError::Allocation { requested: length })?;
    let Some(source) = scratch.get(..length) else {
        return Err(STGReadError::TooLarge { length, maximum });
    };
    accepted.extend_from_slice(source);
    Ok(accepted.into_boxed_slice().into_vec())
}

pub(crate) fn normalize_save_target(
    mut path: PathBuf,
    kind: DocumentKind,
) -> Result<PathBuf, WorkspaceError> {
    let expected = match kind {
        DocumentKind::TroopInfo | DocumentKind::SkillInfo | DocumentKind::TextSOX => SOX_EXTENSION,
        DocumentKind::CrusadersSave => SAV_EXTENSION,
        DocumentKind::CrusadersSTG => STG_EXTENSION,
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

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), WorkspaceError> {
    let parent = save_parent(path)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|source| WorkspaceError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .write_all(bytes)
        .map_err(|source| WorkspaceError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|source| WorkspaceError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| WorkspaceError::Write {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use super::{STGReadError, read_stg_bytes_limited};

    struct ChunkedReader {
        bytes: Vec<u8>,
        offset: usize,
        chunk_size: usize,
        largest_buffer: usize,
    }

    impl ChunkedReader {
        fn new(bytes: Vec<u8>, chunk_size: usize) -> Self {
            Self {
                bytes,
                offset: 0,
                chunk_size,
                largest_buffer: 0,
            }
        }
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.largest_buffer = self.largest_buffer.max(buffer.len());
            let remaining = self.bytes.len().saturating_sub(self.offset);
            let count = remaining.min(self.chunk_size).min(buffer.len());
            let end = self.offset + count;
            buffer
                .get_mut(..count)
                .expect("read count is bounded by the destination")
                .copy_from_slice(
                    self.bytes
                        .get(self.offset..end)
                        .expect("read count is bounded by the source"),
                );
            self.offset = end;
            Ok(count)
        }
    }

    struct GrowingReader {
        inner: ChunkedReader,
        growth: Vec<u8>,
        grew: bool,
    }

    impl Read for GrowingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.grew && self.inner.offset > 0 {
                self.inner.bytes.append(&mut self.growth);
                self.grew = true;
            }
            self.inner.read(buffer)
        }
    }

    #[test]
    fn bounded_stg_reader_accepts_the_limit_and_canonicalizes_capacity() {
        let maximum = 8;
        let mut reader = ChunkedReader::new((0_u8..8).collect(), 3);

        let bytes = read_stg_bytes_limited(&mut reader, maximum).unwrap();

        assert_eq!(bytes, (0_u8..8).collect::<Vec<_>>());
        assert_eq!(bytes.capacity(), bytes.len());
        assert!(reader.largest_buffer <= maximum + 1);
    }

    #[test]
    fn bounded_stg_reader_rejects_a_source_larger_than_the_limit() {
        let maximum = 8;
        let mut reader = ChunkedReader::new((0_u8..12).collect(), 3);

        let error = read_stg_bytes_limited(&mut reader, maximum).unwrap_err();

        assert!(matches!(
            error,
            STGReadError::TooLarge {
                length: 9,
                maximum: 8,
            }
        ));
        assert!(reader.largest_buffer <= maximum + 1);
    }

    #[test]
    fn bounded_stg_reader_rejects_a_source_that_grows_during_reading() {
        let maximum = 8;
        let mut reader = GrowingReader {
            inner: ChunkedReader::new((0_u8..8).collect(), 4),
            growth: vec![8, 9, 10, 11],
            grew: false,
        };

        let error = read_stg_bytes_limited(&mut reader, maximum).unwrap_err();

        assert!(reader.grew);
        assert!(matches!(
            error,
            STGReadError::TooLarge {
                length: 9,
                maximum: 8,
            }
        ));
        assert!(reader.inner.largest_buffer <= maximum + 1);
    }
}
