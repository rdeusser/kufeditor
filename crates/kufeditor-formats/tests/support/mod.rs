use kufeditor_formats::SaveRegion;

const CONTEXT_SIZE: usize = 0x438;
const MAIN_SIZE: usize = 0x154;
const PADDED_SIZE: usize = 0x8000;
const UNIT_SIZE: usize = 483;
const ROSTER_SIZE: usize = 8;
const SECOND_ARRAY_VALUE_SIZE: usize = 4;
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

#[derive(Clone, Copy, Debug, Default)]
pub struct SaveFixtureArrays {
    pub unit_count: u32,
    pub unit_records: usize,
    pub roster_count: u32,
    pub roster_records: usize,
    pub second_array_count: u32,
    pub second_array_values: usize,
}

pub fn save_fixture(options: SaveFixtureOptions) -> Vec<u8> {
    save_fixture_with_arrays(options, &SaveFixtureArrays::default())
}

pub fn save_fixture_with_arrays(
    options: SaveFixtureOptions,
    arrays: &SaveFixtureArrays,
) -> Vec<u8> {
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
    append_u32(&mut source, arrays.unit_count);
    append_zero_records(&mut source, arrays.unit_records, UNIT_SIZE);
    append_i32(&mut source, -1);
    append_u32(&mut source, arrays.roster_count);
    append_zero_records(&mut source, arrays.roster_records, ROSTER_SIZE);
    append_u32(&mut source, arrays.second_array_count);
    append_zero_records(
        &mut source,
        arrays.second_array_values,
        SECOND_ARRAY_VALUE_SIZE,
    );
    for _ in 0..20 {
        append_u32(&mut source, 0);
    }
    append_u32(&mut source, 0);
    source.extend(tail);

    if pad_to_32_kib && source.len() < PADDED_SIZE {
        source.resize(PADDED_SIZE, 0);
    }

    if size_prefix {
        patch_size_prefix(&mut source);
    }

    source
}

pub fn truncate_save(source: &mut Vec<u8>, length: usize, has_size_prefix: bool) {
    source.truncate(length);
    if has_size_prefix {
        patch_size_prefix(source);
    }
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

fn append_zero_records(bytes: &mut Vec<u8>, count: usize, item_size: usize) {
    let Some(additional) = count.checked_mul(item_size) else {
        panic!("fixture array size overflows usize");
    };
    let Some(length) = bytes.len().checked_add(additional) else {
        panic!("fixture length overflows usize");
    };
    bytes.resize(length, 0);
}

pub fn patch_u32(bytes: &mut [u8], offset: usize, value: u32) {
    let Some(field) = bytes.get_mut(offset..offset + size_of::<u32>()) else {
        panic!("fixture field is out of bounds");
    };
    field.copy_from_slice(&value.to_le_bytes());
}

fn patch_size_prefix(source: &mut [u8]) {
    let Ok(length) = u32::try_from(source.len()) else {
        panic!("save fixture length does not fit u32");
    };
    patch_u32(source, 0, length);
}
