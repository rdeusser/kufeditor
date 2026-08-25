use kufeditor_formats::SaveRegion;

const CONTEXT_SIZE: usize = 0x438;
const MAIN_SIZE: usize = 0x154;
const PADDED_SIZE: usize = 0x8000;
const UNIT_COUNT_OFFSET: usize = 4 + 4 + CONTEXT_SIZE + 4 + MAIN_SIZE;

#[derive(Clone, Debug)]
pub struct SaveFixtureOptions {
    pub size_prefix: bool,
    pub context: bool,
    pub pad_to_32_kib: bool,
    pub tail: Vec<u8>,
}

impl Default for SaveFixtureOptions {
    fn default() -> Self {
        Self {
            size_prefix: true,
            context: true,
            pad_to_32_kib: true,
            tail: Vec::new(),
        }
    }
}

pub fn save_fixture(options: SaveFixtureOptions) -> Vec<u8> {
    let SaveFixtureOptions {
        size_prefix,
        context,
        pad_to_32_kib,
        tail,
    } = options;
    let mut source = Vec::new();

    if size_prefix {
        append_u32(&mut source, 0);
    }
    append_u32(&mut source, 0x6e);

    if context {
        append_u32(&mut source, u32::MAX);
        source.resize(source.len() + CONTEXT_SIZE - size_of::<u32>(), 0);
    }

    append_u32(&mut source, 0);
    source.resize(source.len() + MAIN_SIZE, 0);
    append_u32(&mut source, 0);
    append_i32(&mut source, -1);
    append_u32(&mut source, 0);
    append_u32(&mut source, 0);
    for _ in 0..20 {
        append_u32(&mut source, 0);
    }
    append_u32(&mut source, 0);
    source.extend(tail);

    if pad_to_32_kib && source.len() < PADDED_SIZE {
        source.resize(PADDED_SIZE, 0);
    }

    if size_prefix {
        let Ok(length) = u32::try_from(source.len()) else {
            panic!("save fixture length does not fit u32");
        };
        patch_u32(&mut source, 0, length);
    }

    source
}

pub fn fixture_with_count(region: SaveRegion, count: u32) -> Vec<u8> {
    let mut source = save_fixture(SaveFixtureOptions::default());
    let offset = match region {
        SaveRegion::Units => UNIT_COUNT_OFFSET,
        SaveRegion::Roster => UNIT_COUNT_OFFSET + 8,
        SaveRegion::SecondArray => UNIT_COUNT_OFFSET + 12,
        SaveRegion::Envelope | SaveRegion::Missions => {
            panic!("{region:?} does not have a dynamic count")
        }
    };
    patch_u32(&mut source, offset, count);
    source
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn patch_u32(bytes: &mut [u8], offset: usize, value: u32) {
    let Some(field) = bytes.get_mut(offset..offset + size_of::<u32>()) else {
        panic!("fixture field is out of bounds");
    };
    field.copy_from_slice(&value.to_le_bytes());
}
