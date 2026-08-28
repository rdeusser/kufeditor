use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use kufeditor_game::Game;

use crate::{
    FireRatePresetID, FireRateValues, PatchError, PatchID, fire_rate_presets, patch_definitions,
};

pub const MAX_EXECUTABLE_BYTES: u64 = 128 * 1024 * 1024;

const MINIMUM_EXECUTABLE_BYTES: u64 = FACTOR_OFFSET + 4;
pub(crate) const BASE_DELAY_OFFSET: u64 = 0x0007_191A;
pub(crate) const MULTIPLIER_OFFSET: u64 = 0x0007_47D5;
pub(crate) const FACTOR_OFFSET: u64 = 0x002C_0CB4;

const FIRE_RATE_CONTEXTS: [(u64, &[u8]); 3] = [
    (0x0007_1914, &[0xC7, 0x86, 0xD0, 0x0A, 0x00, 0x00]),
    (0x0007_47CF, &[0x8B, 0x87, 0xDC, 0x0A, 0x00, 0x00]),
    (0x0007_47D8, &[0x89, 0x87, 0xD4, 0x0A, 0x00, 0x00]),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchStatus {
    Applied,
    NotApplied,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupStatus {
    Missing,
    Present,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FireRateStatus {
    Preset(FireRatePresetID),
    Custom(FireRateValues),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchState {
    id: PatchID,
    status: PatchStatus,
}

impl PatchState {
    pub const fn id(self) -> PatchID {
        self.id
    }

    pub const fn status(self) -> PatchStatus {
        self.status
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableInspection {
    path: PathBuf,
    backup_path: PathBuf,
    backup_status: BackupStatus,
    patches: [PatchState; 2],
    fire_rate: FireRateStatus,
}

impl ExecutableInspection {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    pub const fn backup_status(&self) -> BackupStatus {
        self.backup_status
    }

    pub const fn patches(&self) -> &[PatchState; 2] {
        &self.patches
    }

    pub fn patch_status(&self, id: PatchID) -> PatchStatus {
        self.patches
            .iter()
            .find(|state| state.id == id)
            .map_or(PatchStatus::Unknown, |state| state.status)
    }

    pub const fn fire_rate(&self) -> FireRateStatus {
        self.fire_rate
    }
}

pub fn inspect(game: Game, root: &Path) -> Result<ExecutableInspection, PatchError> {
    let path = executable_path(game, root)?;
    let metadata = executable_metadata(&path)?;
    validate_executable_metadata(&path, &metadata)?;
    let bytes = fs::read(&path).map_err(|source| PatchError::ExecutableRead {
        path: path.clone(),
        source,
    })?;
    validate_executable_length(&path, bytes.len() as u64)?;

    let backup_path = backup_path(&path);
    let backup_status = inspect_backup(&backup_path, bytes.len() as u64)?;
    let [debug_menu, terrain_bounds] = patch_definitions();
    let patches = [
        PatchState {
            id: debug_menu.id(),
            status: inspect_patch(&bytes, debug_menu),
        },
        PatchState {
            id: terrain_bounds.id(),
            status: inspect_patch(&bytes, terrain_bounds),
        },
    ];

    Ok(ExecutableInspection {
        path,
        backup_path,
        backup_status,
        patches,
        fire_rate: inspect_fire_rate(&bytes),
    })
}

pub(crate) fn executable_path(game: Game, root: &Path) -> Result<PathBuf, PatchError> {
    match game {
        Game::Crusaders => Ok(root.join("Kuf2Main.exe")),
        Game::Heroes => Err(PatchError::UnsupportedGame { game }),
    }
}

pub(crate) fn backup_path(executable: &Path) -> PathBuf {
    let mut path = OsString::from(executable.as_os_str());
    path.push(".bak");
    PathBuf::from(path)
}

pub(crate) fn validate_executable_length(path: &Path, length: u64) -> Result<(), PatchError> {
    if length < MINIMUM_EXECUTABLE_BYTES {
        return Err(PatchError::ExecutableTooShort {
            path: path.to_path_buf(),
            actual: length,
            minimum: MINIMUM_EXECUTABLE_BYTES,
        });
    }
    if length > MAX_EXECUTABLE_BYTES {
        return Err(PatchError::ExecutableTooLarge {
            path: path.to_path_buf(),
            actual: length,
            maximum: MAX_EXECUTABLE_BYTES,
        });
    }
    Ok(())
}

fn executable_metadata(path: &Path) -> Result<fs::Metadata, PatchError> {
    fs::symlink_metadata(path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            PatchError::ExecutableMissing {
                path: path.to_path_buf(),
            }
        } else {
            PatchError::ExecutableMetadata {
                path: path.to_path_buf(),
                source,
            }
        }
    })
}

fn validate_executable_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), PatchError> {
    if metadata.file_type().is_symlink() {
        return Err(PatchError::ExecutableSymbolicLink {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_file() {
        return Err(PatchError::ExecutableNotRegular {
            path: path.to_path_buf(),
        });
    }
    validate_executable_length(path, metadata.len())
}

pub(crate) fn inspect_backup(
    path: &Path,
    expected_length: u64,
) -> Result<BackupStatus, PatchError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BackupStatus::Missing),
        Err(source) => {
            return Err(PatchError::BackupMetadata {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(PatchError::BackupSymbolicLink {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_file() {
        return Err(PatchError::BackupNotRegular {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() != expected_length {
        return Err(PatchError::BackupLength {
            path: path.to_path_buf(),
            actual: metadata.len(),
            expected: expected_length,
        });
    }
    Ok(BackupStatus::Present)
}

pub(crate) fn inspect_patch(bytes: &[u8], definition: &crate::PatchDefinition) -> PatchStatus {
    if definition.contexts().iter().any(|context| {
        let Some(actual) = source_range(bytes, context.offset(), context.original().len()) else {
            return true;
        };
        actual != context.original() && actual != context.patched()
    }) {
        return PatchStatus::Unknown;
    }

    let mut all_original = true;
    let mut all_patched = true;
    for edit in definition.edits() {
        let Some(actual) = source_range(bytes, edit.offset(), edit.original().len()) else {
            return PatchStatus::Unknown;
        };
        all_original &= actual == edit.original();
        all_patched &= actual == edit.patched();
    }
    if all_patched {
        PatchStatus::Applied
    } else if all_original {
        PatchStatus::NotApplied
    } else {
        PatchStatus::Unknown
    }
}

pub(crate) fn inspect_fire_rate(bytes: &[u8]) -> FireRateStatus {
    if FIRE_RATE_CONTEXTS
        .iter()
        .any(|(offset, expected)| source_range(bytes, *offset, expected.len()) != Some(expected))
    {
        return FireRateStatus::Unknown;
    }

    let Some(base_delay) = read_i32(bytes, BASE_DELAY_OFFSET) else {
        return FireRateStatus::Unknown;
    };
    let Some(multiplier) = source_range(bytes, MULTIPLIER_OFFSET, 3).and_then(decode_multiplier)
    else {
        return FireRateStatus::Unknown;
    };
    let Some(distance_factor_bits) = read_u32(bytes, FACTOR_OFFSET) else {
        return FireRateStatus::Unknown;
    };
    let values = FireRateValues::new(base_delay, multiplier, f32::from_bits(distance_factor_bits));
    fire_rate_presets()
        .iter()
        .find(|preset| preset.values() == values)
        .map_or(FireRateStatus::Custom(values), |preset| {
            FireRateStatus::Preset(preset.id())
        })
}

pub(crate) fn source_range(bytes: &[u8], offset: u64, length: usize) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?;
    let end = start.checked_add(length)?;
    bytes.get(start..end)
}

pub(crate) fn decode_multiplier(bytes: &[u8]) -> Option<i32> {
    match bytes {
        [0x8D, 0x04, 0x40] => Some(3),
        [0x8D, 0x04, 0x00] => Some(2),
        [0x89, 0xC0, 0x90] => Some(1),
        _ => None,
    }
}

fn read_i32(bytes: &[u8], offset: u64) -> Option<i32> {
    let array: [u8; 4] = source_range(bytes, offset, 4)?.try_into().ok()?;
    Some(i32::from_le_bytes(array))
}

fn read_u32(bytes: &[u8], offset: u64) -> Option<u32> {
    let array: [u8; 4] = source_range(bytes, offset, 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(array))
}
