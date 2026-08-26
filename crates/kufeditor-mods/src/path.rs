use std::{
    ffi::OsString,
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use kufeditor_game::Game;
use sha2::{Digest, Sha256};

use crate::{GameRootErrorKind, ModError, ModLimits, RelativeGamePathErrorKind};

const DIGEST_BYTES: usize = 32;
const DIGEST_HEX_BYTES: usize = DIGEST_BYTES * 2;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativeGamePath {
    value: String,
    portable_key: String,
    component_count: usize,
}

impl RelativeGamePath {
    pub fn parse(value: &str, limits: &ModLimits) -> Result<Self, ModError> {
        validate_relative_path(value, limits)?;
        Ok(Self {
            value: value.to_owned(),
            portable_key: value.to_lowercase(),
            component_count: value.split('/').count(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn portable_key(&self) -> &str {
        &self.portable_key
    }

    pub const fn component_count(&self) -> usize {
        self.component_count
    }
}

impl fmt::Display for RelativeGamePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

impl AsRef<Path> for RelativeGamePath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.value)
    }
}

fn validate_relative_path(value: &str, limits: &ModLimits) -> Result<(), ModError> {
    let invalid = |kind| ModError::InvalidRelativeGamePath {
        value: value.to_owned(),
        kind,
    };

    if value.is_empty() {
        return Err(invalid(RelativeGamePathErrorKind::Empty));
    }
    if value.len() > limits.max_relative_path_bytes {
        return Err(invalid(RelativeGamePathErrorKind::TooLong));
    }
    if value.starts_with('/') || Path::new(value).is_absolute() {
        return Err(invalid(RelativeGamePathErrorKind::Absolute));
    }
    if value.contains('\\') {
        return Err(invalid(RelativeGamePathErrorKind::Backslash));
    }
    if value.as_bytes().contains(&0) {
        return Err(invalid(RelativeGamePathErrorKind::NUL));
    }
    if value.contains(':') {
        return Err(invalid(RelativeGamePathErrorKind::Colon));
    }

    let components = value.split('/').collect::<Vec<_>>();
    if components.len() > limits.max_relative_path_components {
        return Err(invalid(RelativeGamePathErrorKind::TooManyComponents));
    }
    for component in components {
        if component.is_empty() {
            return Err(invalid(RelativeGamePathErrorKind::EmptyComponent));
        }
        if component == "." {
            return Err(invalid(RelativeGamePathErrorKind::CurrentComponent));
        }
        if component == ".." {
            return Err(invalid(RelativeGamePathErrorKind::ParentComponent));
        }
        if component.ends_with([' ', '.']) {
            return Err(invalid(RelativeGamePathErrorKind::TerminalSpaceOrPeriod));
        }
        if is_windows_device_name(component) {
            return Err(invalid(RelativeGamePathErrorKind::WindowsDeviceName));
        }
    }
    Ok(())
}

fn is_windows_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || upper.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModStorePaths {
    application_data: PathBuf,
    root: PathBuf,
}

impl ModStorePaths {
    pub fn new(application_data: PathBuf) -> Self {
        let root = application_data.join("mods");
        Self {
            application_data,
            root,
        }
    }

    pub fn application_data(&self) -> &Path {
        &self.application_data
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn packages(&self) -> PathBuf {
        self.root.join("packages")
    }

    pub fn installation_registry(&self) -> PathBuf {
        self.root.join("installations-v1.json")
    }

    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn operations(&self) -> PathBuf {
        self.root.join("operations")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameRoot {
    game: Game,
    configured_path: PathBuf,
    canonical_path: PathBuf,
    key: GameRootKey,
}

impl GameRoot {
    pub fn inspect(
        game: Game,
        configured_path: PathBuf,
        stores: &ModStorePaths,
    ) -> Result<Self, ModError> {
        let metadata = match fs::symlink_metadata(&configured_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(invalid_game_root(
                    game,
                    configured_path,
                    GameRootErrorKind::Missing,
                ));
            }
            Err(error) => {
                return Err(ModError::io("inspect game root", configured_path, error));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(invalid_game_root(
                game,
                configured_path,
                GameRootErrorKind::SymbolicLink,
            ));
        }
        if !metadata.is_dir() {
            return Err(invalid_game_root(
                game,
                configured_path,
                GameRootErrorKind::NotDirectory,
            ));
        }
        if configured_path.to_str().is_none() {
            return Err(invalid_game_root(
                game,
                configured_path,
                GameRootErrorKind::NonUnicode,
            ));
        }

        let canonical_path = fs::canonicalize(&configured_path)
            .map_err(|error| ModError::io("canonicalize game root", &configured_path, error))?;
        if canonical_path.to_str().is_none() {
            return Err(invalid_game_root(
                game,
                configured_path,
                GameRootErrorKind::NonUnicode,
            ));
        }
        let canonical_store = canonicalize_missing_path(stores.root())?;
        if canonical_store.starts_with(&canonical_path)
            || canonical_path.starts_with(&canonical_store)
        {
            return Err(invalid_game_root(
                game,
                configured_path,
                GameRootErrorKind::StoreOverlapsGameRoot,
            ));
        }

        let key = GameRootKey::for_root(game, &canonical_path);
        Ok(Self {
            game,
            configured_path,
            canonical_path,
            key,
        })
    }

    pub const fn game(&self) -> Game {
        self.game
    }

    pub fn configured_path(&self) -> &Path {
        &self.configured_path
    }

    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub const fn key(&self) -> GameRootKey {
        self.key
    }
}

fn invalid_game_root(game: Game, path: PathBuf, kind: GameRootErrorKind) -> ModError {
    ModError::InvalidGameRoot { game, path, kind }
}

fn canonicalize_missing_path(path: &Path) -> Result<PathBuf, ModError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| ModError::io("read current directory", path, error))?
            .join(path)
    };
    let mut existing = absolute.as_path();
    let mut missing = Vec::<OsString>::new();
    loop {
        match fs::canonicalize(existing) {
            Ok(mut canonical) => {
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Ok(normalize_path(&canonical));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(ModError::io("canonicalize owned mod store", path, error));
                };
                missing.push(name.to_os_string());
                let Some(parent) = existing.parent() else {
                    return Err(ModError::io("canonicalize owned mod store", path, error));
                };
                existing = parent;
            }
            Err(error) => {
                return Err(ModError::io("canonicalize owned mod store", path, error));
            }
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

macro_rules! digest_id {
    ($name:ident, $kind:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; DIGEST_BYTES]);

        impl $name {
            pub const fn from_bytes(bytes: [u8; DIGEST_BYTES]) -> Self {
                Self(bytes)
            }

            pub fn parse(value: &str) -> Result<Self, ModError> {
                parse_digest(value)
                    .map(Self)
                    .ok_or_else(|| ModError::InvalidID {
                        kind: $kind,
                        value: value.to_owned(),
                    })
            }

            pub const fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&digest_hex(&self.0))
            }
        }
    };
}

digest_id!(ModPackageID, "mod package");
digest_id!(InstallationID, "installation");
digest_id!(FileSHA256, "file SHA256");
digest_id!(BackupID, "backup");
digest_id!(OperationID, "operation");
digest_id!(GameRootKey, "game root");

impl InstallationID {
    pub(crate) fn for_installation(
        root: GameRootKey,
        package: ModPackageID,
        operation: OperationID,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"kufeditor-installation-v1\0");
        hasher.update(root.as_bytes());
        hasher.update(package.as_bytes());
        hasher.update(operation.as_bytes());
        Self(hasher.finalize().into())
    }
}

impl GameRootKey {
    pub(crate) fn for_root(game: Game, root: &Path) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(match game {
            Game::Crusaders => b"crusaders".as_slice(),
            Game::Heroes => b"heroes".as_slice(),
        });
        hasher.update([0]);
        hasher.update(root.to_string_lossy().as_bytes());
        Self(hasher.finalize().into())
    }
}

fn parse_digest(value: &str) -> Option<[u8; DIGEST_BYTES]> {
    if value.len() != DIGEST_HEX_BYTES
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return None;
    }

    let mut bytes = [0; DIGEST_BYTES];
    let mut digits = value.bytes();
    for byte in &mut bytes {
        let high = hex_value(digits.next()?)?;
        let low = hex_value(digits.next()?)?;
        *byte = (high << 4) | low;
    }
    Some(bytes)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn digest_hex(bytes: &[u8; DIGEST_BYTES]) -> String {
    let mut text = String::with_capacity(DIGEST_HEX_BYTES);
    for byte in bytes {
        text.push(hex_character(byte >> 4));
        text.push(hex_character(byte & 0x0f));
    }
    text
}

fn hex_character(value: u8) -> char {
    char::from(if value < 10 {
        b'0' + value
    } else {
        b'a' + (value - 10)
    })
}
