use std::{io, path::PathBuf, str::Utf8Error};

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

#[derive(Debug, Error)]
pub enum CatalogFileError {
    #[error("failed to read the catalog file: {source}")]
    Read {
        #[source]
        source: io::Error,
    },

    #[error("failed to parse or access the catalog format: {source}")]
    Format {
        #[source]
        source: FormatError,
    },

    #[error("weapon file is not valid UTF-8: {source}")]
    InvalidWeaponUtf8 {
        #[source]
        source: Utf8Error,
    },

    #[error("invalid weapon syntax on line {line}: {reason}")]
    WeaponSyntax { line: usize, reason: &'static str },

    #[error("invalid field encoding in {role} record {record} field {field}")]
    InvalidFieldEncoding {
        role: CatalogRole,
        record: usize,
        field: usize,
    },
}

#[derive(Debug, Error)]
pub enum CatalogLoadError {
    #[error("selected SOX path is not a directory: {}", path.display())]
    InvalidSoxDirectory { path: PathBuf },

    #[error("no usable core catalogs were loaded")]
    NoUsableCatalogs { issues: Vec<CatalogIssue> },
}
