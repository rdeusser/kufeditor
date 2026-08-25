use std::collections::HashSet;

use super::{SaveDocument, SaveMutation, SaveTextField};
use crate::error::FormatError;

const COLOR_MARKERS: [&[u8]; 2] = [b"@(color=", b"(color="];
const SAVE_TEXT_SIZE: usize = 32;
const MAXIMUM_SAVE_TEXT_LENGTH: usize = SAVE_TEXT_SIZE - 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveTextImage([u8; SAVE_TEXT_SIZE]);

impl SaveTextImage {
    fn into_bytes(self) -> [u8; SAVE_TEXT_SIZE] {
        self.0
    }
}

impl SaveDocument {
    pub fn text(&self, field: SaveTextField) -> Result<String, FormatError> {
        visible_text(field, text_field_bytes(&self.file.main_save_block, field))
    }

    pub fn text_image(&self, field: SaveTextField) -> SaveTextImage {
        SaveTextImage(*text_field_bytes(&self.file.main_save_block, field))
    }

    pub fn set_text(
        &mut self,
        field: SaveTextField,
        value: String,
    ) -> Result<SaveMutation<SaveTextImage>, FormatError> {
        let previous = self.text_image(field);
        let current = visible_text(field, &previous.0)?;
        if current == value {
            return Ok(SaveMutation::Unchanged);
        }

        let value = value.into_bytes();
        let replacement = checked_text_image(field, &value)?;
        *text_field_bytes_mut(&mut self.file.main_save_block, field) = replacement;
        Ok(SaveMutation::Changed { previous })
    }

    pub fn restore_text(
        &mut self,
        field: SaveTextField,
        value: SaveTextImage,
    ) -> SaveMutation<SaveTextImage> {
        let previous = self.text_image(field);
        if previous == value {
            return SaveMutation::Unchanged;
        }

        *text_field_bytes_mut(&mut self.file.main_save_block, field) = value.into_bytes();
        SaveMutation::Changed { previous }
    }
}

fn visible_text(field: SaveTextField, image: &[u8; SAVE_TEXT_SIZE]) -> Result<String, FormatError> {
    let mut text = String::with_capacity(SAVE_TEXT_SIZE);
    for (index, byte) in image
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .enumerate()
    {
        if !byte.is_ascii() {
            return Err(FormatError::SaveInvalidStoredText { field, index, byte });
        }
        text.push(char::from(byte));
    }
    Ok(text)
}

fn checked_text_image(
    field: SaveTextField,
    value: &[u8],
) -> Result<[u8; SAVE_TEXT_SIZE], FormatError> {
    for (index, byte) in value.iter().copied().enumerate() {
        if !byte.is_ascii() {
            return Err(FormatError::SaveInvalidTextByte { field, index, byte });
        }
        if byte == 0 {
            return Err(FormatError::SaveTextContainsZero { field, index });
        }
    }

    if value.len() > MAXIMUM_SAVE_TEXT_LENGTH {
        return Err(FormatError::SaveTextTooLong {
            field,
            length: value.len(),
            maximum: MAXIMUM_SAVE_TEXT_LENGTH,
        });
    }

    let mut image = [0; SAVE_TEXT_SIZE];
    let Some(destination) = image.get_mut(..value.len()) else {
        unreachable!("validated save text length must fit its image");
    };
    destination.copy_from_slice(value);
    Ok(image)
}

fn text_field_bytes(block: &[u8; 340], field: SaveTextField) -> &[u8; SAVE_TEXT_SIZE] {
    let offset = text_field_offset(field);
    let Some(bytes) = block.get(offset..offset + SAVE_TEXT_SIZE) else {
        unreachable!("save text field must fit the main block");
    };
    let Ok(bytes) = <&[u8; SAVE_TEXT_SIZE]>::try_from(bytes) else {
        unreachable!("save text field must have a fixed size");
    };
    bytes
}

fn text_field_bytes_mut(block: &mut [u8; 340], field: SaveTextField) -> &mut [u8; SAVE_TEXT_SIZE] {
    let offset = text_field_offset(field);
    let Some(bytes) = block.get_mut(offset..offset + SAVE_TEXT_SIZE) else {
        unreachable!("save text field must fit the main block");
    };
    let Ok(bytes) = <&mut [u8; SAVE_TEXT_SIZE]>::try_from(bytes) else {
        unreachable!("save text field must have a fixed size");
    };
    bytes
}

const fn text_field_offset(field: SaveTextField) -> usize {
    match field {
        SaveTextField::MapName => 0x20,
        SaveTextField::SetFile => 0x60,
        SaveTextField::SkyEffects => 0xa0,
    }
}

pub(super) fn extract_context_text(context: &[u8]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut text = Vec::new();

    for segment in context.split(|byte| !is_context_byte(*byte)) {
        if segment.len() < 4 {
            continue;
        }

        let cleaned = strip_color_codes(segment);
        for line in cleaned.split('\n') {
            let line = line.trim_matches(|character: char| character.is_ascii_whitespace());
            if line.len() < 4 {
                continue;
            }

            let line = line.to_owned();
            if seen.insert(line.clone()) {
                text.push(line);
            }
        }
    }

    text
}

const fn is_context_byte(byte: u8) -> bool {
    byte == b'\r' || byte == b'\n' || (byte >= 0x20 && byte <= 0x7e)
}

fn strip_color_codes(segment: &[u8]) -> String {
    let mut cleaned = String::with_capacity(segment.len());
    let mut remaining = segment;

    while let Some((&byte, rest)) = remaining.split_first() {
        let is_color_code = COLOR_MARKERS
            .iter()
            .any(|marker| remaining.starts_with(marker));
        if is_color_code {
            let Some(closing_offset) = remaining.iter().position(|candidate| *candidate == b')')
            else {
                break;
            };
            let next_offset = closing_offset.saturating_add(1);
            remaining = remaining.get(next_offset..).unwrap_or_default();
            continue;
        }

        cleaned.push(char::from(byte));
        remaining = rest;
    }

    if let Some((fragment, remainder)) = cleaned.split_once(')')
        && !fragment.is_empty()
        && fragment.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return remainder.to_owned();
    }

    cleaned
}
