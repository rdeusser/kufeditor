#![allow(
    clippy::expect_used,
    reason = "tests use controlled temporary executables and fixed byte fixtures"
)]

use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use kufeditor_game::Game;
use kufeditor_patches::{
    BackupStatus, FireRatePresetID, FireRateStatus, FireRateValues, MAX_EXECUTABLE_BYTES,
    PatchError, PatchID, PatchStatus, inspect,
};
use tempfile::TempDir;

#[test]
fn original_and_applied_images_inspect_every_patch_and_fire_rate() {
    let fixture = ExecutableFixture::original();

    let original = inspect(Game::Crusaders, fixture.root()).expect("inspect original executable");
    assert_eq!(original.path(), fixture.path());
    assert_eq!(original.backup_status(), BackupStatus::Missing);
    assert_eq!(
        original.patch_status(PatchID::DebugMenu),
        PatchStatus::NotApplied
    );
    assert_eq!(
        original.patch_status(PatchID::TerrainBounds),
        PatchStatus::NotApplied
    );
    assert_eq!(
        original.fire_rate(),
        FireRateStatus::Preset(FireRatePresetID::Original),
    );

    fixture.write(0x000D_76EC, &[0x8B, 0x35, 0xAC, 0x3C, 0x74, 0x00]);
    fixture.write(0x000D_7710, &[0x8B, 0x0D, 0xAC, 0x3C, 0x74, 0x00]);
    fixture.write(0x0022_D991, &[0xE8, 0x88, 0xBB, 0x08, 0x00]);
    fixture.write(0x002B_951E, TERRAIN_BOUNDS_WRAPPER);
    write_fire_rate(&fixture, 1, 1, -0.0045);

    let applied = inspect(Game::Crusaders, fixture.root()).expect("inspect patched executable");
    assert_eq!(
        applied.patch_status(PatchID::DebugMenu),
        PatchStatus::Applied
    );
    assert_eq!(
        applied.patch_status(PatchID::TerrainBounds),
        PatchStatus::Applied
    );
    assert_eq!(
        applied.fire_rate(),
        FireRateStatus::Preset(FireRatePresetID::Rapid),
    );
}

#[test]
fn mixed_and_unrecognized_bytes_are_unknown_without_hiding_other_state() {
    let fixture = ExecutableFixture::original();
    fixture.write(0x000D_76EC, &[0x8B, 0x35, 0xAC, 0x3C, 0x74, 0x00]);
    fixture.write(0x0022_D991, &[0x90, 0x90, 0x90, 0x90, 0x90]);

    let mixed = inspect(Game::Crusaders, fixture.root()).expect("inspect mixed executable");
    assert_eq!(mixed.patch_status(PatchID::DebugMenu), PatchStatus::Unknown);
    assert_eq!(
        mixed.patch_status(PatchID::TerrainBounds),
        PatchStatus::Unknown
    );
    assert_eq!(
        mixed.fire_rate(),
        FireRateStatus::Preset(FireRatePresetID::Original),
    );
}

#[test]
fn fire_rate_distinguishes_custom_values_from_invalid_context_and_instructions() {
    let fixture = ExecutableFixture::original();
    write_fire_rate(&fixture, 4, 2, -0.008);

    let custom = inspect(Game::Crusaders, fixture.root()).expect("inspect custom fire rate");
    assert_eq!(
        custom.fire_rate(),
        FireRateStatus::Custom(FireRateValues::new(4, 2, -0.008)),
    );

    fixture.write(0x0007_47D5, &[0xCC, 0xCC, 0xCC]);
    let invalid_instruction =
        inspect(Game::Crusaders, fixture.root()).expect("inspect invalid multiplier instruction");
    assert_eq!(invalid_instruction.fire_rate(), FireRateStatus::Unknown);

    fixture.write(0x0007_47D5, &[0x8D, 0x04, 0x00]);
    fixture.write(0x0007_1914, &[0; 6]);
    let invalid_context =
        inspect(Game::Crusaders, fixture.root()).expect("inspect invalid fire-rate context");
    assert_eq!(invalid_context.fire_rate(), FireRateStatus::Unknown);
}

#[test]
fn backup_status_requires_a_complete_regular_non_link_file() {
    let fixture = ExecutableFixture::original();
    fs::copy(fixture.path(), fixture.backup_path()).expect("copy complete backup");
    let inspected = inspect(Game::Crusaders, fixture.root()).expect("inspect backup");
    assert_eq!(inspected.backup_status(), BackupStatus::Present);

    fs::write(fixture.backup_path(), b"incomplete").expect("truncate backup");
    assert!(matches!(
        inspect(Game::Crusaders, fixture.root()),
        Err(PatchError::BackupLength {
            actual: 10,
            expected: MINIMUM_EXECUTABLE_BYTES,
            ..
        })
    ));
}

#[test]
fn unsupported_missing_short_and_oversized_executables_are_typed() {
    let missing = TempDir::new().expect("temp root");
    assert!(matches!(
        inspect(Game::Crusaders, missing.path()),
        Err(PatchError::ExecutableMissing { .. })
    ));
    assert!(matches!(
        inspect(Game::Heroes, missing.path()),
        Err(PatchError::UnsupportedGame { game: Game::Heroes })
    ));

    let short = TempDir::new().expect("temp root");
    fs::write(short.path().join("Kuf2Main.exe"), [0; 16]).expect("write short executable");
    assert!(matches!(
        inspect(Game::Crusaders, short.path()),
        Err(PatchError::ExecutableTooShort { .. })
    ));

    let oversized = TempDir::new().expect("temp root");
    let path = oversized.path().join("Kuf2Main.exe");
    let file = File::create(&path).expect("create oversized executable");
    file.set_len(MAX_EXECUTABLE_BYTES + 1)
        .expect("extend oversized executable");
    assert!(matches!(
        inspect(Game::Crusaders, oversized.path()),
        Err(PatchError::ExecutableTooLarge { .. })
    ));
}

#[cfg(unix)]
#[test]
fn executable_and_backup_symbolic_links_are_rejected() {
    use std::os::unix::fs::symlink;

    let executable_link_root = TempDir::new().expect("temp root");
    let outside = executable_link_root.path().join("outside.exe");
    File::create(&outside)
        .expect("create outside executable")
        .set_len(MINIMUM_EXECUTABLE_BYTES)
        .expect("extend outside executable");
    symlink(&outside, executable_link_root.path().join("Kuf2Main.exe")).expect("link executable");
    assert!(matches!(
        inspect(Game::Crusaders, executable_link_root.path()),
        Err(PatchError::ExecutableSymbolicLink { .. })
    ));

    let backup_link_fixture = ExecutableFixture::original();
    let outside_backup = backup_link_fixture.root().join("outside.bak");
    fs::write(&outside_backup, b"backup").expect("write outside backup");
    symlink(&outside_backup, backup_link_fixture.backup_path()).expect("link backup");
    assert!(matches!(
        inspect(Game::Crusaders, backup_link_fixture.root()),
        Err(PatchError::BackupSymbolicLink { .. })
    ));
}

struct ExecutableFixture {
    directory: TempDir,
    path: PathBuf,
}

impl ExecutableFixture {
    fn original() -> Self {
        let directory = TempDir::new().expect("temp root");
        let path = directory.path().join("Kuf2Main.exe");
        let file = File::create(&path).expect("create executable");
        file.set_len(MINIMUM_EXECUTABLE_BYTES)
            .expect("extend executable");
        let fixture = Self { directory, path };
        fixture.write(0x000D_76EC, &[0x8B, 0x35, 0xB0, 0x3C, 0x74, 0x00]);
        fixture.write(0x000D_7710, &[0x8B, 0x0D, 0xB0, 0x3C, 0x74, 0x00]);
        fixture.write(0x0022_D991, &[0xE8, 0x8A, 0x95, 0x01, 0x00]);
        fixture.write(0x002B_951E, &[0; 87]);
        fixture.write(0x0007_1914, &[0xC7, 0x86, 0xD0, 0x0A, 0x00, 0x00]);
        fixture.write(0x0007_47CF, &[0x8B, 0x87, 0xDC, 0x0A, 0x00, 0x00]);
        fixture.write(0x0007_47D8, &[0x89, 0x87, 0xD4, 0x0A, 0x00, 0x00]);
        write_fire_rate(&fixture, 5, 3, -0.009);
        fixture
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn backup_path(&self) -> PathBuf {
        self.directory.path().join("Kuf2Main.exe.bak")
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        let mut file = File::options()
            .write(true)
            .open(&self.path)
            .expect("open executable");
        file.seek(SeekFrom::Start(offset)).expect("seek executable");
        file.write_all(bytes).expect("write executable");
    }
}

fn write_fire_rate(fixture: &ExecutableFixture, base_delay: i32, multiplier: i32, factor: f32) {
    fixture.write(0x0007_191A, &base_delay.to_le_bytes());
    fixture.write(
        0x0007_47D5,
        match multiplier {
            3 => &[0x8D, 0x04, 0x40],
            2 => &[0x8D, 0x04, 0x00],
            1 => &[0x89, 0xC0, 0x90],
            _ => panic!("unsupported fixture multiplier"),
        },
    );
    fixture.write(0x002C_0CB4, &factor.to_bits().to_le_bytes());
}

const MINIMUM_EXECUTABLE_BYTES: u64 = 0x002C_0CB8;
const TERRAIN_BOUNDS_WRAPPER: &[u8] = &[
    0xF3, 0x0F, 0x10, 0x44, 0x24, 0x04, 0x0F, 0x57, 0xC9, 0x0F, 0x2F, 0xC1, 0x76, 0x46, 0xF3, 0x0F,
    0x10, 0x44, 0x24, 0x08, 0x0F, 0x2F, 0xC1, 0x76, 0x3B, 0xF3, 0x0F, 0x2A, 0x81, 0x10, 0x01, 0x00,
    0x00, 0xF3, 0x0F, 0x59, 0x05, 0x1C, 0xD5, 0x6B, 0x00, 0xF3, 0x0F, 0x10, 0x4C, 0x24, 0x04, 0x0F,
    0x2F, 0xC1, 0x76, 0x20, 0xF3, 0x0F, 0x2A, 0x81, 0x14, 0x01, 0x00, 0x00, 0xF3, 0x0F, 0x59, 0x05,
    0x1C, 0xD5, 0x6B, 0x00, 0xF3, 0x0F, 0x10, 0x4C, 0x24, 0x08, 0x0F, 0x2F, 0xC1, 0x76, 0x05, 0xE9,
    0xAE, 0xD9, 0xF8, 0xFF, 0xD9, 0xEE, 0xC3,
];
