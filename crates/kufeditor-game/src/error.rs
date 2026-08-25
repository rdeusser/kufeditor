use std::{
    error::Error as StdError,
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
    str::Utf8Error,
};

use kufeditor_formats::FormatError;
use thiserror::Error;

use crate::CatalogRole;

#[derive(Debug, Error)]
#[error("failed to load {role} from {}: {error}", path.display())]
pub struct CatalogIssue {
    pub role: CatalogRole,
    pub path: PathBuf,
    #[source]
    pub error: CatalogFileError,
}

#[derive(Debug)]
pub enum CatalogFileError {
    Read {
        source: io::Error,
    },

    Format {
        source: Box<FormatError>,
    },

    InvalidWeaponUTF8 {
        source: Utf8Error,
    },

    WeaponSyntax {
        line: usize,
        reason: &'static str,
    },

    InvalidFieldEncoding {
        role: CatalogRole,
        record: usize,
        field: usize,
    },
}

impl Display for CatalogFileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { source } => {
                write!(formatter, "failed to read the catalog file: {source}")
            }
            Self::Format { source } => {
                write!(
                    formatter,
                    "failed to parse or access the catalog format: {source}"
                )
            }
            Self::InvalidWeaponUTF8 { source } => {
                write!(formatter, "weapon file is not valid UTF-8: {source}")
            }
            Self::WeaponSyntax { line, reason } => {
                write!(formatter, "invalid weapon syntax on line {line}: {reason}")
            }
            Self::InvalidFieldEncoding {
                role,
                record,
                field,
            } => write!(
                formatter,
                "invalid field encoding in {role} record {record} field {field}"
            ),
        }
    }
}

impl StdError for CatalogFileError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Read { source } => Some(source),
            Self::Format { source } => Some(source.as_ref()),
            Self::InvalidWeaponUTF8 { source } => Some(source),
            Self::WeaponSyntax { .. } | Self::InvalidFieldEncoding { .. } => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum CatalogLoadError {
    #[error("selected SOX path is not a directory: {}", path.display())]
    InvalidSOXDirectory { path: PathBuf },

    #[error("no usable core catalogs were loaded")]
    NoUsableCatalogs { issues: Vec<CatalogIssue> },
}
