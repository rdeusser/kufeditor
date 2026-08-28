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
    BackupStatus, FireRatePresetID, FireRateStatus, PatchChange, PatchError, PatchID, PatchStatus,
    PatchTarget, inspect, set_fire_rate, set_patch,
};
use tempfile::TempDir;

#[test]
fn every_patch_applies_reapplies_and_reverts_with_one_stable_backup() {
    for id in [PatchID::DebugMenu, PatchID::TerrainBounds] {
        let fixture = ExecutableFixture::original();
        let source = fs::read(fixture.path()).expect("read source");

        let applied = set_patch(Game::Crusaders, fixture.root(), id, PatchTarget::Applied)
            .expect("apply patch");
        assert_eq!(applied.change(), PatchChange::Changed);
        assert!(applied.backup_created());
        assert_eq!(applied.backup_status(), BackupStatus::Present);
        assert_eq!(
            fs::read(fixture.backup_path()).expect("read backup"),
            source
        );
        assert_eq!(
            inspect(Game::Crusaders, fixture.root())
                .expect("inspect applied")
                .patch_status(id),
            PatchStatus::Applied,
        );

        let backup = fs::read(fixture.backup_path()).expect("read stable backup");
        let repeated = set_patch(Game::Crusaders, fixture.root(), id, PatchTarget::Applied)
            .expect("repeat patch");
        assert_eq!(repeated.change(), PatchChange::Unchanged);
        assert!(!repeated.backup_created());
        assert_eq!(
            fs::read(fixture.backup_path()).expect("read backup"),
            backup
        );

        let reverted = set_patch(Game::Crusaders, fixture.root(), id, PatchTarget::NotApplied)
            .expect("revert patch");
        assert_eq!(reverted.change(), PatchChange::Changed);
        assert!(!reverted.backup_created());
        assert_eq!(
            fs::read(fixture.backup_path()).expect("read backup"),
            source
        );
        assert_eq!(fs::read(fixture.path()).expect("read reverted"), source);
    }
}

#[test]
fn every_fire_rate_preset_is_idempotent_and_restores_original_bytes() {
    let fixture = ExecutableFixture::original();
    let source = fs::read(fixture.path()).expect("read source");

    for preset in [
        FireRatePresetID::Fast,
        FireRatePresetID::Rapid,
        FireRatePresetID::Turbo,
        FireRatePresetID::Original,
    ] {
        let changed = set_fire_rate(Game::Crusaders, fixture.root(), preset)
            .expect("select fire-rate preset");
        assert_eq!(changed.change(), PatchChange::Changed);
        assert_eq!(
            inspect(Game::Crusaders, fixture.root())
                .expect("inspect preset")
                .fire_rate(),
            FireRateStatus::Preset(preset),
        );

        let repeated = set_fire_rate(Game::Crusaders, fixture.root(), preset)
            .expect("repeat fire-rate preset");
        assert_eq!(repeated.change(), PatchChange::Unchanged);
    }

    assert_eq!(
        fs::read(fixture.path()).expect("read original preset"),
        source
    );
    assert_eq!(
        fs::read(fixture.backup_path()).expect("read backup"),
        source
    );
}

#[test]
fn custom_fire_rate_can_move_to_a_preset_but_unknown_states_never_write() {
    let custom = ExecutableFixture::original();
    write_fire_rate(&custom, 4, 2, -0.008);
    let selected = set_fire_rate(Game::Crusaders, custom.root(), FireRatePresetID::Fast)
        .expect("replace custom fire rate");
    assert_eq!(selected.change(), PatchChange::Changed);

    let unknown_patch = ExecutableFixture::original();
    unknown_patch.write(0x000D_76EE, &[0xCC]);
    let before = fs::read(unknown_patch.path()).expect("read unknown patch");
    assert!(matches!(
        set_patch(
            Game::Crusaders,
            unknown_patch.root(),
            PatchID::DebugMenu,
            PatchTarget::Applied,
        ),
        Err(PatchError::UnrecognizedPatch {
            id: PatchID::DebugMenu
        })
    ));
    assert_eq!(
        fs::read(unknown_patch.path()).expect("read unchanged"),
        before
    );
    assert!(!unknown_patch.backup_path().exists());

    let unknown_fire_rate = ExecutableFixture::original();
    unknown_fire_rate.write(0x0007_47D5, &[0xCC, 0xCC, 0xCC]);
    let before = fs::read(unknown_fire_rate.path()).expect("read unknown fire rate");
    assert!(matches!(
        set_fire_rate(
            Game::Crusaders,
            unknown_fire_rate.root(),
            FireRatePresetID::Turbo,
        ),
        Err(PatchError::UnrecognizedFireRate)
    ));
    assert_eq!(
        fs::read(unknown_fire_rate.path()).expect("read unchanged"),
        before,
    );
    assert!(!unknown_fire_rate.backup_path().exists());
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
        file.set_len(0x002C_0CB8).expect("extend executable");
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
