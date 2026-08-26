#![allow(
    clippy::unwrap_used,
    reason = "controlled save fixture dimensions and offsets make failures fatal"
)]

use std::{
    cell::RefCell,
    collections::VecDeque,
    mem::size_of,
    path::{Path, PathBuf},
};

use gpui::{Context, PathPromptOptions, Task};

use crate::{
    frame::{
        AppFrame,
        mods::{ModPathPromptResult, ModPathsPromptResult, ModPromptLauncher},
    },
    mod_status::ModPromptKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModPathsPromptRequest {
    pub(crate) kind: ModPromptKind,
    pub(crate) initial_directory: Option<PathBuf>,
    pub(crate) files: bool,
    pub(crate) directories: bool,
    pub(crate) multiple: bool,
    pub(crate) prompt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModExportPromptRequest {
    pub(crate) directory: PathBuf,
    pub(crate) suggested_name: Option<String>,
}

#[derive(Default)]
pub(crate) struct ControlledModPromptLauncher {
    paths_results: RefCell<VecDeque<ModPathsPromptResult>>,
    export_results: RefCell<VecDeque<ModPathPromptResult>>,
    paths_requests: RefCell<Vec<ModPathsPromptRequest>>,
    export_requests: RefCell<Vec<ModExportPromptRequest>>,
}

impl ControlledModPromptLauncher {
    pub(crate) fn queue_paths(&self, result: ModPathsPromptResult) {
        self.paths_results.borrow_mut().push_back(result);
    }

    pub(crate) fn queue_export(&self, result: ModPathPromptResult) {
        self.export_results.borrow_mut().push_back(result);
    }

    pub(crate) fn paths_requests(&self) -> Vec<ModPathsPromptRequest> {
        self.paths_requests.borrow().clone()
    }

    pub(crate) fn export_requests(&self) -> Vec<ModExportPromptRequest> {
        self.export_requests.borrow().clone()
    }
}

impl ModPromptLauncher for ControlledModPromptLauncher {
    fn launch_paths(
        &self,
        kind: ModPromptKind,
        initial_directory: Option<PathBuf>,
        options: PathPromptOptions,
        _: &mut Context<AppFrame>,
    ) -> Task<ModPathsPromptResult> {
        self.paths_requests
            .borrow_mut()
            .push(ModPathsPromptRequest {
                kind,
                initial_directory,
                files: options.files,
                directories: options.directories,
                multiple: options.multiple,
                prompt: options.prompt.map(|prompt| prompt.to_string()),
            });
        Task::ready(
            self.paths_results
                .borrow_mut()
                .pop_front()
                .unwrap_or(ModPathsPromptResult::Canceled),
        )
    }

    fn launch_export(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
        _: &mut Context<AppFrame>,
    ) -> Task<ModPathPromptResult> {
        self.export_requests
            .borrow_mut()
            .push(ModExportPromptRequest {
                directory: directory.to_path_buf(),
                suggested_name: suggested_name.map(ToOwned::to_owned),
            });
        Task::ready(
            self.export_results
                .borrow_mut()
                .pop_front()
                .unwrap_or(ModPathPromptResult::Canceled),
        )
    }
}

const CONTEXT_SIZE: usize = 0x438;
const MAIN_SIZE: usize = 0x154;
const UNIT_SIZE: usize = 483;
const EQUIPMENT_SIZE: usize = 64;
const MINIMUM_FILE_SIZE: usize = 0x8000;

pub(crate) struct SaveFixture {
    unit_count: usize,
    roster_count: usize,
    second_array_count: usize,
    unit_roles: Vec<u32>,
    invalid_map_name_byte: Option<u8>,
    save_file_name: Option<Vec<u8>>,
}

impl SaveFixture {
    pub(crate) const fn new(
        unit_count: usize,
        roster_count: usize,
        second_array_count: usize,
    ) -> Self {
        Self {
            unit_count,
            roster_count,
            second_array_count,
            unit_roles: Vec::new(),
            invalid_map_name_byte: None,
            save_file_name: None,
        }
    }

    pub(crate) fn with_unit_roles(mut self, roles: impl IntoIterator<Item = u32>) -> Self {
        self.unit_roles = roles.into_iter().collect();
        assert!(self.unit_roles.len() <= self.unit_count);
        self
    }

    pub(crate) const fn with_invalid_map_name_byte(mut self, byte: u8) -> Self {
        self.invalid_map_name_byte = Some(byte);
        self
    }

    pub(crate) fn with_save_file_name(mut self, name: impl Into<Vec<u8>>) -> Self {
        self.save_file_name = Some(name.into());
        self
    }

    pub(crate) fn build(self) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_u32(&mut bytes, 0);
        append_u32(&mut bytes, 0x6e);
        append_u32(&mut bytes, u32::MAX);
        bytes.resize(bytes.len() + CONTEXT_SIZE - size_of::<u32>(), 0);
        append_u32(&mut bytes, 0);
        bytes.resize(bytes.len() + MAIN_SIZE, 0);

        append_u32(&mut bytes, u32::try_from(self.unit_count).unwrap());
        if self.unit_count > 0 {
            append_complete_unit(&mut bytes);
            append_zero_records(&mut bytes, self.unit_count - 1, UNIT_SIZE);
        }

        append_i32(&mut bytes, -1);
        append_u32(&mut bytes, u32::try_from(self.roster_count).unwrap());
        append_zero_records(&mut bytes, self.roster_count, 8);

        append_u32(&mut bytes, u32::try_from(self.second_array_count).unwrap());
        for value in 0..self.second_array_count {
            append_u32(&mut bytes, u32::try_from(value).unwrap());
        }
        for slot in 0_i32..20 {
            append_i32(&mut bytes, slot - 1);
        }
        append_i32(&mut bytes, -2);

        if bytes.len() < MINIMUM_FILE_SIZE {
            bytes.resize(MINIMUM_FILE_SIZE, 0);
        }
        let length = u32::try_from(bytes.len()).unwrap();
        bytes
            .get_mut(..size_of::<u32>())
            .unwrap()
            .copy_from_slice(&length.to_le_bytes());

        for (unit, role) in self.unit_roles.into_iter().enumerate() {
            let ucd_offset = unit_offset(unit) + 10 * size_of::<u32>();
            bytes
                .get_mut(ucd_offset..ucd_offset + size_of::<u32>())
                .unwrap()
                .copy_from_slice(&role.to_le_bytes());
        }
        let main_offset = main_offset();
        if let Some(byte) = self.invalid_map_name_byte {
            *bytes.get_mut(main_offset + 0x20).unwrap() = byte;
        }
        if let Some(name) = self.save_file_name {
            assert!(name.len() <= 32);
            bytes
                .get_mut(main_offset + 0x60..main_offset + 0x60 + name.len())
                .unwrap()
                .copy_from_slice(&name);
        }
        bytes
    }
}

fn main_offset() -> usize {
    2 * size_of::<u32>() + CONTEXT_SIZE + size_of::<u32>()
}

fn unit_offset(unit: usize) -> usize {
    main_offset() + MAIN_SIZE + size_of::<u32>() + unit * UNIT_SIZE
}

fn append_complete_unit(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    append_i32(bytes, -1);
    for value in [2_u32, 2, 4, 0x34, 0x38, 0x3c, 0x40] {
        append_u32(bytes, value);
    }
    append_i32(bytes, -1);
    for value in [5_u32, 99, 6, 7, 8] {
        append_u32(bytes, value);
    }
    bytes.extend_from_slice(&[1, 0, 1]);
    for value in [60_u32, 64, 68] {
        append_u32(bytes, value);
    }
    bytes.extend(0xa0_u8..=0xb7);
    append_named_equipment(bytes);
    append_zero_records(bytes, 5, EQUIPMENT_SIZE);
    append_u32(bytes, 504);
    assert_eq!(bytes.len() - start, UNIT_SIZE);
}

fn append_named_equipment(bytes: &mut Vec<u8>) {
    append_u32(bytes, 1_000);
    append_i32(bytes, 0);
    append_u16(bytes, 5);
    append_i16(bytes, -1);
    append_u16(bytes, 0);
    append_i16(bytes, 12);
    append_u16(bytes, 1);
    append_u16(bytes, 0);
    append_i32(bytes, 91);
    append_i32(bytes, -1);
    append_i32(bytes, -1);
    append_i32(bytes, 3);
    append_i32(bytes, 9);
    append_i32(bytes, 4);
    append_i32(bytes, -1);
    append_i32(bytes, 5);
    append_i32(bytes, 4);
    append_i32(bytes, 6);
    append_i32(bytes, 0);
}

fn append_zero_records(bytes: &mut Vec<u8>, count: usize, record_size: usize) {
    bytes.resize(bytes.len() + count * record_size, 0);
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
