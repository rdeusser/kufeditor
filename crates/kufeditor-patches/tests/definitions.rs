use kufeditor_patches::{FireRatePresetID, PatchID, fire_rate_presets, patch_definitions};

#[test]
fn patch_definitions_match_every_retained_offset_and_byte() {
    let [debug, terrain] = patch_definitions();
    assert_eq!(debug.id(), PatchID::DebugMenu);
    assert_eq!(debug.name(), "Debug Menu");
    assert!(!debug.experimental());
    assert_eq!(
        debug
            .edits()
            .iter()
            .map(|edit| (edit.offset(), edit.original(), edit.patched()))
            .collect::<Vec<_>>(),
        vec![
            (0x000D_76EE, &[0xB0][..], &[0xAC][..]),
            (0x000D_7712, &[0xB0][..], &[0xAC][..]),
        ],
    );
    assert_eq!(
        debug
            .contexts()
            .iter()
            .map(|context| (context.offset(), context.original(), context.patched()))
            .collect::<Vec<_>>(),
        vec![
            (
                0x000D_76EC,
                &[0x8B, 0x35, 0xB0, 0x3C, 0x74, 0x00][..],
                &[0x8B, 0x35, 0xAC, 0x3C, 0x74, 0x00][..],
            ),
            (
                0x000D_7710,
                &[0x8B, 0x0D, 0xB0, 0x3C, 0x74, 0x00][..],
                &[0x8B, 0x0D, 0xAC, 0x3C, 0x74, 0x00][..],
            ),
        ],
    );

    assert_eq!(terrain.id(), PatchID::TerrainBounds);
    assert_eq!(terrain.name(), "Terrain Bounds Check");
    assert!(terrain.experimental());
    assert!(terrain.contexts().is_empty());
    let [terrain_call, terrain_wrapper] = terrain.edits() else {
        panic!("terrain patch must have two edits");
    };
    assert_eq!(terrain_call.offset(), 0x0022_D991);
    assert_eq!(terrain_call.original(), &[0xE8, 0x8A, 0x95, 0x01, 0x00]);
    assert_eq!(terrain_call.patched(), &[0xE8, 0x88, 0xBB, 0x08, 0x00]);
    assert_eq!(terrain_wrapper.offset(), 0x002B_951E);
    assert_eq!(terrain_wrapper.original(), &[0; 87]);
    assert_eq!(terrain_wrapper.patched(), TERRAIN_BOUNDS_WRAPPER);
}

#[test]
fn fire_rate_presets_match_the_retained_values_and_order() {
    let presets = fire_rate_presets();
    assert_eq!(
        presets
            .iter()
            .map(|preset| (
                preset.id(),
                preset.name(),
                preset.values().base_delay(),
                preset.values().multiplier(),
                preset.values().distance_factor_bits(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                FireRatePresetID::Original,
                "Original",
                5,
                3,
                (-0.009_f32).to_bits()
            ),
            (FireRatePresetID::Fast, "Fast", 2, 1, (-0.009_f32).to_bits()),
            (
                FireRatePresetID::Rapid,
                "Rapid",
                1,
                1,
                (-0.0045_f32).to_bits()
            ),
            (
                FireRatePresetID::Turbo,
                "Turbo",
                1,
                1,
                (-0.00225_f32).to_bits()
            ),
        ],
    );
}

const TERRAIN_BOUNDS_WRAPPER: &[u8] = &[
    0xF3, 0x0F, 0x10, 0x44, 0x24, 0x04, 0x0F, 0x57, 0xC9, 0x0F, 0x2F, 0xC1, 0x76, 0x46, 0xF3, 0x0F,
    0x10, 0x44, 0x24, 0x08, 0x0F, 0x2F, 0xC1, 0x76, 0x3B, 0xF3, 0x0F, 0x2A, 0x81, 0x10, 0x01, 0x00,
    0x00, 0xF3, 0x0F, 0x59, 0x05, 0x1C, 0xD5, 0x6B, 0x00, 0xF3, 0x0F, 0x10, 0x4C, 0x24, 0x04, 0x0F,
    0x2F, 0xC1, 0x76, 0x20, 0xF3, 0x0F, 0x2A, 0x81, 0x14, 0x01, 0x00, 0x00, 0xF3, 0x0F, 0x59, 0x05,
    0x1C, 0xD5, 0x6B, 0x00, 0xF3, 0x0F, 0x10, 0x4C, 0x24, 0x08, 0x0F, 0x2F, 0xC1, 0x76, 0x05, 0xE9,
    0xAE, 0xD9, 0xF8, 0xFF, 0xD9, 0xEE, 0xC3,
];
