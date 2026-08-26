use kufeditor_formats::SaveRegion;

const CONTEXT_SIZE: usize = 0x438;
const MAIN_SIZE: usize = 0x154;
const MAP_NAME_OFFSET: usize = 0x20;
const SAVE_TEXT_SIZE: usize = 32;
const PADDED_SIZE: usize = 0x8000;
const UNIT_SIZE: usize = 483;
const ROSTER_SIZE: usize = 8;
const SECOND_ARRAY_VALUE_SIZE: usize = 4;
const UNIT_COUNT_OFFSET: usize = 4 + 4 + CONTEXT_SIZE + 4 + MAIN_SIZE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteSaveOffsets {
    pub magic: usize,
    pub context: Option<usize>,
    pub campaign: usize,
    pub main: usize,
    pub unit: usize,
    pub selected_unit: usize,
    pub roster: usize,
    pub second_array: usize,
    pub mission_completion: usize,
    pub current_mission: usize,
    pub tail: usize,
}

#[derive(Clone, Debug)]
pub struct SaveFixtureOptions {
    pub size_prefix: bool,
    pub context: bool,
    pub pad_to_32_kib: bool,
    pub tail: Vec<u8>,
    pub post_padding_tail: Vec<u8>,
}

impl Default for SaveFixtureOptions {
    fn default() -> Self {
        Self {
            size_prefix: true,
            context: true,
            pad_to_32_kib: true,
            tail: Vec::new(),
            post_padding_tail: Vec::new(),
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
        post_padding_tail,
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
    source.extend(post_padding_tail);

    if size_prefix {
        patch_size_prefix(&mut source);
    }

    source
}

pub fn complete_save_fixture(options: SaveFixtureOptions) -> Vec<u8> {
    let SaveFixtureOptions {
        size_prefix,
        context,
        pad_to_32_kib,
        tail,
        post_padding_tail,
    } = options;
    let mut source = Vec::new();

    if size_prefix {
        append_u32(&mut source, 0);
    }
    append_u32(&mut source, 0x6e);

    if context {
        append_u32(&mut source, u32::MAX);
        let context_end = source.len() + CONTEXT_SIZE - size_of::<u32>();
        source.resize(context_end, 0);
    }

    append_u32(&mut source, 0);
    append_main_block(&mut source);
    append_u32(&mut source, 1);
    append_complete_unit(&mut source);
    append_i32(&mut source, -1);
    append_u32(&mut source, 1);
    append_roster_record(&mut source);
    append_u32(&mut source, 1);
    append_u32(&mut source, 0x0203_0405);
    for slot in 0_i32..20 {
        append_i32(&mut source, slot - 1);
    }
    append_i32(&mut source, -2);
    source.extend(tail);
    if pad_to_32_kib && source.len() < PADDED_SIZE {
        source.resize(PADDED_SIZE, 0);
    }
    source.extend(post_padding_tail);

    if size_prefix {
        patch_size_prefix(&mut source);
    }

    source
}

pub fn save_with_noncanonical_map_field() -> Vec<u8> {
    let mut source = complete_save_fixture(SaveFixtureOptions::default());
    let offsets = complete_save_offsets(true, true);
    patch_noncanonical_map_field(&mut source, offsets.main);
    source
}

pub fn patch_noncanonical_map_field(source: &mut [u8], main_offset: usize) {
    let start = main_offset + MAP_NAME_OFFSET;
    let end = start + SAVE_TEXT_SIZE;
    let Some(field) = source.get_mut(start..end) else {
        panic!("fixture map-name field is out of bounds");
    };

    field.fill(0);
    let Some(visible) = field.get_mut(..5) else {
        panic!("fixture map-name visible text is out of bounds");
    };
    visible.copy_from_slice(b"MapA\0");
    let Some(pattern) = field.get_mut(5..31) else {
        panic!("fixture map-name post-zero pattern is out of bounds");
    };
    pattern.copy_from_slice(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ");
}

pub fn fixture_with_unknown_choices(ucd: u32, skill: i32, resistance: i32) -> Vec<u8> {
    let mut source = complete_save_fixture(SaveFixtureOptions::default());
    let offsets = complete_save_offsets(true, true);
    patch_u32(&mut source, offsets.unit + 40, ucd);
    patch_i32(&mut source, offsets.unit + 95 + 28, skill);
    patch_i32(&mut source, offsets.unit + 95 + 44, resistance);
    source
}

pub const fn complete_save_offsets(size_prefix: bool, context: bool) -> CompleteSaveOffsets {
    let magic = if size_prefix { 4 } else { 0 };
    let context_start = magic + size_of::<u32>();
    let campaign = context_start + if context { CONTEXT_SIZE } else { 0 };
    let main = campaign + size_of::<u32>();
    let unit = main + MAIN_SIZE + size_of::<u32>();
    let selected_unit = unit + UNIT_SIZE;
    let roster = selected_unit + size_of::<u32>() + size_of::<u32>();
    let second_array = roster + ROSTER_SIZE + size_of::<u32>();
    let mission_completion = second_array + SECOND_ARRAY_VALUE_SIZE;
    let current_mission = mission_completion + 20 * size_of::<u32>();
    let tail = current_mission + size_of::<u32>();

    CompleteSaveOffsets {
        magic,
        context: if context { Some(context_start) } else { None },
        campaign,
        main,
        unit,
        selected_unit,
        roster,
        second_array,
        mission_completion,
        current_mission,
        tail,
    }
}

pub fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let Some(field) = bytes.get(offset..offset + size_of::<u32>()) else {
        panic!("fixture field is out of bounds");
    };
    let Ok(field) = <[u8; 4]>::try_from(field) else {
        panic!("fixture field has the wrong size");
    };
    u32::from_le_bytes(field)
}

pub fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    let Some(field) = bytes.get(offset..offset + size_of::<i32>()) else {
        panic!("fixture field is out of bounds");
    };
    let Ok(field) = <[u8; 4]>::try_from(field) else {
        panic!("fixture field has the wrong size");
    };
    i32::from_le_bytes(field)
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

fn append_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn append_main_block(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    for value in [0x100_u32, 0x104] {
        append_u32(bytes, value);
    }
    append_i32(bytes, -8);
    for value in [0x10c_u32, 0x110, 0x114, 0x118] {
        append_u32(bytes, value);
    }
    bytes.resize(start + MAIN_SIZE, 0);
}

fn append_complete_unit(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    append_i32(bytes, -1);
    append_u32(bytes, 2);
    append_u32(bytes, 3);
    append_u32(bytes, 4);
    for value in [0x34_u32, 0x38, 0x3c, 0x40] {
        append_u32(bytes, value);
    }
    append_i32(bytes, -1);
    append_u32(bytes, 5);
    append_u32(bytes, 0);
    append_u32(bytes, 6);
    append_u32(bytes, 7);
    append_u32(bytes, 8);
    bytes.extend_from_slice(&[1, 0, 1]);
    for value in [60_u32, 64, 68] {
        append_u32(bytes, value);
    }
    bytes.extend(0xa0_u8..=0xb7);
    for slot in 0_u16..6 {
        append_equipment_slot(bytes, slot);
    }
    append_u32(bytes, 504);
    assert_eq!(bytes.len() - start, UNIT_SIZE);
}

fn append_equipment_slot(bytes: &mut Vec<u8>, slot: u16) {
    let slot_signed = i32::from(slot);
    let Ok(short_slot) = i16::try_from(slot) else {
        panic!("fixture equipment slot does not fit i16");
    };

    append_u32(bytes, 1_000 + u32::from(slot));
    append_i32(bytes, -100 - slot_signed);
    append_u16(bytes, 200 + slot);
    append_i16(bytes, -200 - short_slot);
    append_u16(bytes, 300 + slot);
    append_i16(bytes, -300 - short_slot);
    append_u16(bytes, 400 + slot);
    append_u16(bytes, 500 + slot);
    append_i32(bytes, -400 - slot_signed);
    append_i32(bytes, 600 + slot_signed);
    append_i32(bytes, slot_signed);
    append_i32(bytes, -500 - slot_signed);
    append_i32(bytes, 9 + slot_signed);
    append_i32(bytes, 700 + slot_signed);
    append_i32(bytes, slot_signed);
    append_i32(bytes, -600 - slot_signed);
    append_i32(bytes, 4 + slot_signed);
    append_i32(bytes, 800 + slot_signed);
    append_i32(bytes, -700 - slot_signed);
}

fn append_roster_record(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&[61, 60, 62, 63]);
    append_u32(bytes, 6_400);
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

pub fn patch_i32(bytes: &mut [u8], offset: usize, value: i32) {
    let Some(field) = bytes.get_mut(offset..offset + size_of::<i32>()) else {
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
