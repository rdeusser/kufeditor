use std::{fs, io, path::Path};

const MAGIC: u32 = 1_001;
const HEADER_SIZE: usize = 620;
const UNIT_SIZE: usize = 544;
const AREA_SIZE: usize = 84;

pub const SYNTHETIC_PARSED_STG_PATH: &str = "nested/Alpha.STG";
pub const SYNTHETIC_EMPTY_STG_PATH: &str = "nested/deeper/beta.stg";
pub const SYNTHETIC_RAW_STG_PATH: &str = "raw/Gamma.StG";

#[derive(Clone, Debug)]
pub struct STGFixture {
    pub bytes: Vec<u8>,
    pub offsets: STGFixtureOffsets,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGFixtureOffsets {
    pub header_map: usize,
    pub unit_name: usize,
    pub unit_leader_hp: usize,
    pub unit_stat_override: usize,
    pub tail_start: usize,
    pub area_count: usize,
    pub area_description: usize,
    pub area_bound_x1: usize,
    pub variable_count: usize,
    pub variable_name: usize,
    pub variable_integer_type: usize,
    pub variable_float_type: usize,
    pub variable_string_type: usize,
    pub variable_string_length: usize,
    pub variable_enum_type: usize,
    pub event_block_count: usize,
    pub event_count: usize,
    pub event_description: usize,
    pub condition_count: usize,
    pub condition_parameter_count: usize,
    pub condition_integer_type: usize,
    pub condition_float_type: usize,
    pub action_count: usize,
    pub action_parameter_count: usize,
    pub action_string_type: usize,
    pub action_string_length: usize,
    pub action_enum_type: usize,
    pub footer_count: usize,
    pub suffix: usize,
}

pub fn complete_stg_fixture() -> STGFixture {
    let mut bytes = stg_prefix_fixture(1);
    let header_map = 4 + 68;
    let unit_name = 4 + HEADER_SIZE + size_of::<u32>();
    let unit_leader_hp = unit_name + 40;
    let unit_stat_override = unit_name + UNIT_SIZE - 22 * size_of::<f32>();
    let tail_start = bytes.len();

    let area_count = bytes.len();
    push_u32(&mut bytes, 1);
    let area_description = bytes.len();
    let area_bound_x1 = area_description + 68;
    bytes.resize(bytes.len() + AREA_SIZE, 0);

    let variable_count = bytes.len();
    push_u32(&mut bytes, 4);
    let variable_name = bytes.len();
    let variable_integer_type = append_variable(&mut bytes, 100, Parameter::Integer(-12));
    let variable_float_type = append_variable(&mut bytes, 101, Parameter::Float(17.25));
    let variable_string_type = append_variable(&mut bytes, 102, Parameter::String(b"variable"));
    let variable_string_length = variable_string_type + size_of::<u32>();
    let variable_enum_type = append_variable(&mut bytes, 103, Parameter::Enum(7));

    let event_block_count = bytes.len();
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 0x0102_0304);
    let event_count = bytes.len();
    push_u32(&mut bytes, 2);

    let event_description = bytes.len();
    append_fixed_text::<64>(&mut bytes, b"Primary Event");
    push_u32(&mut bytes, 500);
    let condition_count = bytes.len();
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 19);
    let condition_parameter_count = bytes.len();
    push_u32(&mut bytes, 2);
    let condition_integer_type = append_parameter(&mut bytes, Parameter::Integer(23));
    let condition_float_type = append_parameter(&mut bytes, Parameter::Float(-0.0));
    let action_count = bytes.len();
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 55);
    let action_parameter_count = bytes.len();
    push_u32(&mut bytes, 2);
    let action_string_type = append_parameter(&mut bytes, Parameter::String(b"action"));
    let action_string_length = action_string_type + size_of::<u32>();
    let action_enum_type = append_parameter(&mut bytes, Parameter::Enum(-3));

    append_fixed_text::<64>(&mut bytes, b"Empty Event");
    push_u32(&mut bytes, 501);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, 0x0506_0708);
    push_u32(&mut bytes, 0);

    let footer_count = bytes.len();
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 700);
    push_u32(&mut bytes, 701);
    push_u32(&mut bytes, 702);
    push_u32(&mut bytes, 703);

    let suffix = bytes.len();
    bytes.extend_from_slice(&[0xf0, 0x0d, 0xca, 0xfe]);

    STGFixture {
        bytes,
        offsets: STGFixtureOffsets {
            header_map,
            unit_name,
            unit_leader_hp,
            unit_stat_override,
            tail_start,
            area_count,
            area_description,
            area_bound_x1,
            variable_count,
            variable_name,
            variable_integer_type,
            variable_float_type,
            variable_string_type,
            variable_string_length,
            variable_enum_type,
            event_block_count,
            event_count,
            event_description,
            condition_count,
            condition_parameter_count,
            condition_integer_type,
            condition_float_type,
            action_count,
            action_parameter_count,
            action_string_type,
            action_string_length,
            action_enum_type,
            footer_count,
            suffix,
        },
    }
}

pub fn empty_stg_fixture() -> Vec<u8> {
    let mut bytes = stg_prefix_fixture(0);
    for _ in 0..5 {
        push_u32(&mut bytes, 0);
    }
    bytes
}

pub fn stg_prefix_fixture(unit_count: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, MAGIC);
    bytes.resize(bytes.len() + HEADER_SIZE, 0);
    let Ok(unit_count_wire) = u32::try_from(unit_count) else {
        panic!("STG fixture unit count does not fit u32");
    };
    push_u32(&mut bytes, unit_count_wire);
    let Some(unit_bytes) = unit_count.checked_mul(UNIT_SIZE) else {
        panic!("STG fixture unit bytes overflow usize");
    };
    bytes.resize(bytes.len() + unit_bytes, 0);
    bytes
}

pub fn synthetic_raw_stg_fixture() -> Vec<u8> {
    let mut bytes = stg_prefix_fixture(1);
    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc]);
    bytes
}

pub fn synthetic_stg_corpus_files() -> [(&'static str, Vec<u8>); 3] {
    [
        (SYNTHETIC_PARSED_STG_PATH, complete_stg_fixture().bytes),
        (SYNTHETIC_EMPTY_STG_PATH, empty_stg_fixture()),
        (SYNTHETIC_RAW_STG_PATH, synthetic_raw_stg_fixture()),
    ]
}

pub fn write_synthetic_stg_corpus(root: &Path) -> io::Result<()> {
    for (relative_path, bytes) in synthetic_stg_corpus_files() {
        let path = root.join(relative_path);
        let Some(parent) = path.parent() else {
            return Err(io::Error::other("synthetic STG path has no parent"));
        };
        fs::create_dir_all(parent)?;
        fs::write(path, bytes)?;
    }
    fs::write(root.join("ignored.txt"), b"not an STG file")
}

#[derive(Clone, Copy, Debug)]
enum Parameter<'a> {
    Integer(i32),
    Float(f32),
    String(&'a [u8]),
    Enum(i32),
}

fn append_variable(bytes: &mut Vec<u8>, id: u32, parameter: Parameter<'_>) -> usize {
    append_fixed_text::<64>(bytes, format!("Variable {id}").as_bytes());
    push_u32(bytes, id);
    append_parameter(bytes, parameter)
}

fn append_parameter(bytes: &mut Vec<u8>, parameter: Parameter<'_>) -> usize {
    let type_offset = bytes.len();
    match parameter {
        Parameter::Integer(value) => {
            push_u32(bytes, 0);
            push_i32(bytes, value);
        }
        Parameter::Float(value) => {
            push_u32(bytes, 1);
            push_u32(bytes, value.to_bits());
        }
        Parameter::String(value) => {
            push_u32(bytes, 2);
            let Ok(length) = u32::try_from(value.len()) else {
                panic!("STG fixture string length does not fit u32");
            };
            push_u32(bytes, length);
            bytes.extend_from_slice(value);
        }
        Parameter::Enum(value) => {
            push_u32(bytes, 3);
            push_i32(bytes, value);
        }
    }
    type_offset
}

fn append_fixed_text<const N: usize>(bytes: &mut Vec<u8>, value: &[u8]) {
    assert!(value.len() < N, "fixture text must leave a terminator");
    bytes.extend_from_slice(value);
    bytes.resize(bytes.len() + N - value.len(), 0);
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
