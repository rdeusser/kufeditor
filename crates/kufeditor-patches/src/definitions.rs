#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PatchID {
    DebugMenu,
    TerrainBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteEdit {
    offset: u64,
    original: &'static [u8],
    patched: &'static [u8],
}

impl ByteEdit {
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn original(&self) -> &'static [u8] {
        self.original
    }

    pub const fn patched(&self) -> &'static [u8] {
        self.patched
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextImage {
    offset: u64,
    original: &'static [u8],
    patched: &'static [u8],
}

impl ContextImage {
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn original(&self) -> &'static [u8] {
        self.original
    }

    pub const fn patched(&self) -> &'static [u8] {
        self.patched
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatchDefinition {
    id: PatchID,
    name: &'static str,
    description: &'static str,
    experimental: bool,
    edits: &'static [ByteEdit],
    contexts: &'static [ContextImage],
}

impl PatchDefinition {
    pub const fn id(&self) -> PatchID {
        self.id
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn description(&self) -> &'static str {
        self.description
    }

    pub const fn experimental(&self) -> bool {
        self.experimental
    }

    pub const fn edits(&self) -> &'static [ByteEdit] {
        self.edits
    }

    pub const fn contexts(&self) -> &'static [ContextImage] {
        self.contexts
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FireRatePresetID {
    Original,
    Fast,
    Rapid,
    Turbo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FireRateValues {
    base_delay: i32,
    multiplier: i32,
    distance_factor_bits: u32,
}

impl FireRateValues {
    pub const fn new(base_delay: i32, multiplier: i32, distance_factor: f32) -> Self {
        Self {
            base_delay,
            multiplier,
            distance_factor_bits: distance_factor.to_bits(),
        }
    }

    pub const fn base_delay(self) -> i32 {
        self.base_delay
    }

    pub const fn multiplier(self) -> i32 {
        self.multiplier
    }

    pub const fn distance_factor_bits(self) -> u32 {
        self.distance_factor_bits
    }

    pub const fn distance_factor(self) -> f32 {
        f32::from_bits(self.distance_factor_bits)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FireRatePreset {
    id: FireRatePresetID,
    name: &'static str,
    description: &'static str,
    values: FireRateValues,
}

impl FireRatePreset {
    pub const fn id(&self) -> FireRatePresetID {
        self.id
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn description(&self) -> &'static str {
        self.description
    }

    pub const fn values(&self) -> FireRateValues {
        self.values
    }
}

pub const fn patch_definitions() -> &'static [PatchDefinition; 2] {
    &PATCH_DEFINITIONS
}

pub const fn fire_rate_presets() -> &'static [FireRatePreset] {
    &FIRE_RATE_PRESETS
}

const DEBUG_MENU_EDITS: [ByteEdit; 2] = [
    ByteEdit {
        offset: 0x000D_76EE,
        original: &[0xB0],
        patched: &[0xAC],
    },
    ByteEdit {
        offset: 0x000D_7712,
        original: &[0xB0],
        patched: &[0xAC],
    },
];

const DEBUG_MENU_CONTEXTS: [ContextImage; 2] = [
    ContextImage {
        offset: 0x000D_76EC,
        original: &[0x8B, 0x35, 0xB0, 0x3C, 0x74, 0x00],
        patched: &[0x8B, 0x35, 0xAC, 0x3C, 0x74, 0x00],
    },
    ContextImage {
        offset: 0x000D_7710,
        original: &[0x8B, 0x0D, 0xB0, 0x3C, 0x74, 0x00],
        patched: &[0x8B, 0x0D, 0xAC, 0x3C, 0x74, 0x00],
    },
];

const TERRAIN_ORIGINAL_WRAPPER: [u8; 87] = [0; 87];
const TERRAIN_BOUNDS_WRAPPER: [u8; 87] = [
    0xF3, 0x0F, 0x10, 0x44, 0x24, 0x04, 0x0F, 0x57, 0xC9, 0x0F, 0x2F, 0xC1, 0x76, 0x46, 0xF3, 0x0F,
    0x10, 0x44, 0x24, 0x08, 0x0F, 0x2F, 0xC1, 0x76, 0x3B, 0xF3, 0x0F, 0x2A, 0x81, 0x10, 0x01, 0x00,
    0x00, 0xF3, 0x0F, 0x59, 0x05, 0x1C, 0xD5, 0x6B, 0x00, 0xF3, 0x0F, 0x10, 0x4C, 0x24, 0x04, 0x0F,
    0x2F, 0xC1, 0x76, 0x20, 0xF3, 0x0F, 0x2A, 0x81, 0x14, 0x01, 0x00, 0x00, 0xF3, 0x0F, 0x59, 0x05,
    0x1C, 0xD5, 0x6B, 0x00, 0xF3, 0x0F, 0x10, 0x4C, 0x24, 0x08, 0x0F, 0x2F, 0xC1, 0x76, 0x05, 0xE9,
    0xAE, 0xD9, 0xF8, 0xFF, 0xD9, 0xEE, 0xC3,
];

const TERRAIN_BOUNDS_EDITS: [ByteEdit; 2] = [
    ByteEdit {
        offset: 0x0022_D991,
        original: &[0xE8, 0x8A, 0x95, 0x01, 0x00],
        patched: &[0xE8, 0x88, 0xBB, 0x08, 0x00],
    },
    ByteEdit {
        offset: 0x002B_951E,
        original: &TERRAIN_ORIGINAL_WRAPPER,
        patched: &TERRAIN_BOUNDS_WRAPPER,
    },
];

const PATCH_DEFINITIONS: [PatchDefinition; 2] = [
    PatchDefinition {
        id: PatchID::DebugMenu,
        name: "Debug Menu",
        description: "Make the tilde key open the developer debug menu (CTestMenu) instead of PC Key/Mouse Settings.",
        experimental: false,
        edits: &DEBUG_MENU_EDITS,
        contexts: &DEBUG_MENU_CONTEXTS,
    },
    PatchDefinition {
        id: PatchID::TerrainBounds,
        name: "Terrain Bounds Check",
        description: "Prevent crashes from terrain height checks near map edges or with large sight ranges.",
        experimental: true,
        edits: &TERRAIN_BOUNDS_EDITS,
        contexts: &[],
    },
];

const FIRE_RATE_PRESETS: [FireRatePreset; 4] = [
    FireRatePreset {
        id: FireRatePresetID::Original,
        name: "Original",
        description: "Use the original fire rate.",
        values: FireRateValues::new(5, 3, -0.009),
    },
    FireRatePreset {
        id: FireRatePresetID::Fast,
        name: "Fast",
        description: "Use about twice the original fire rate.",
        values: FireRateValues::new(2, 1, -0.009),
    },
    FireRatePreset {
        id: FireRatePresetID::Rapid,
        name: "Rapid",
        description: "Use about four times the original fire rate.",
        values: FireRateValues::new(1, 1, -0.0045),
    },
    FireRatePreset {
        id: FireRatePresetID::Turbo,
        name: "Turbo",
        description: "Use nearly continuous fire with the shortest delays.",
        values: FireRateValues::new(1, 1, -0.00225),
    },
];
