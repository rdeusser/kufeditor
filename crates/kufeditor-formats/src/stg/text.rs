use std::{borrow::Cow, mem::size_of};

use encoding_rs::{EUC_KR, EncoderResult};

use crate::error::{FormatError, STGTextEncoding, STGTextError};

use super::STGTextTarget;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGText<'a> {
    Decoded(Cow<'a, str>),
    Raw(&'a [u8]),
}

impl<'a> STGText<'a> {
    pub fn decoded(&self) -> Option<&str> {
        match self {
            Self::Decoded(value) => Some(value.as_ref()),
            Self::Raw(_) => None,
        }
    }

    pub const fn raw(&self) -> Option<&'a [u8]> {
        match self {
            Self::Decoded(_) => None,
            Self::Raw(value) => Some(*value),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct STGTextImage {
    target: STGTextTarget,
    value: STGTextImageValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct STGTextPreview {
    changed: bool,
    current_retained_bytes: usize,
    replacement_retained_bytes: usize,
}

#[derive(Debug)]
pub struct STGTextRestoreFailure {
    error: Box<FormatError>,
    image: STGTextImage,
}

impl STGTextRestoreFailure {
    pub(super) fn new(error: FormatError, image: STGTextImage) -> Self {
        Self {
            error: Box::new(error),
            image,
        }
    }

    pub fn into_parts(self) -> (FormatError, STGTextImage) {
        (*self.error, self.image)
    }
}

impl STGTextPreview {
    pub(super) const fn new(
        changed: bool,
        current_retained_bytes: usize,
        replacement_retained_bytes: usize,
    ) -> Self {
        Self {
            changed,
            current_retained_bytes,
            replacement_retained_bytes,
        }
    }

    pub const fn is_changed(self) -> bool {
        self.changed
    }

    pub const fn current_retained_bytes(self) -> usize {
        self.current_retained_bytes
    }

    pub const fn replacement_retained_bytes(self) -> usize {
        self.replacement_retained_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum STGTextImageValue {
    Fixed32([u8; 32]),
    Fixed64([u8; 64]),
    Dynamic(Vec<u8>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum STGTextImageKind {
    Fixed32,
    Fixed64,
    Dynamic,
}

impl STGTextImage {
    pub(super) const fn fixed32(target: STGTextTarget, value: [u8; 32]) -> Self {
        Self {
            target,
            value: STGTextImageValue::Fixed32(value),
        }
    }

    pub(super) const fn fixed64(target: STGTextTarget, value: [u8; 64]) -> Self {
        Self {
            target,
            value: STGTextImageValue::Fixed64(value),
        }
    }

    pub(super) fn dynamic(target: STGTextTarget, value: Vec<u8>) -> Self {
        Self {
            target,
            value: STGTextImageValue::Dynamic(exact_vec(value)),
        }
    }

    pub(super) const fn target(&self) -> STGTextTarget {
        self.target
    }

    pub(super) const fn kind(&self) -> STGTextImageKind {
        match &self.value {
            STGTextImageValue::Fixed32(_) => STGTextImageKind::Fixed32,
            STGTextImageValue::Fixed64(_) => STGTextImageKind::Fixed64,
            STGTextImageValue::Dynamic(_) => STGTextImageKind::Dynamic,
        }
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        match &self.value {
            STGTextImageValue::Fixed32(value) => value,
            STGTextImageValue::Fixed64(value) => value,
            STGTextImageValue::Dynamic(value) => value,
        }
    }

    pub(super) fn into_fixed32(self) -> Option<[u8; 32]> {
        match self.value {
            STGTextImageValue::Fixed32(value) => Some(value),
            STGTextImageValue::Fixed64(_) | STGTextImageValue::Dynamic(_) => None,
        }
    }

    pub(super) fn into_fixed64(self) -> Option<[u8; 64]> {
        match self.value {
            STGTextImageValue::Fixed64(value) => Some(value),
            STGTextImageValue::Fixed32(_) | STGTextImageValue::Dynamic(_) => None,
        }
    }

    pub(super) fn into_dynamic(self) -> Option<Vec<u8>> {
        match self.value {
            STGTextImageValue::Dynamic(value) => Some(value),
            STGTextImageValue::Fixed32(_) | STGTextImageValue::Fixed64(_) => None,
        }
    }

    pub fn retained_bytes(&self) -> usize {
        let dynamic = match &self.value {
            STGTextImageValue::Dynamic(value) => value.capacity(),
            STGTextImageValue::Fixed32(_) | STGTextImageValue::Fixed64(_) => 0,
        };
        size_of::<Self>().saturating_add(dynamic)
    }

    pub(super) const fn fixed_retained_bytes() -> usize {
        size_of::<Self>()
    }

    pub(super) const fn dynamic_retained_bytes(length: usize) -> usize {
        size_of::<Self>().saturating_add(length)
    }
}

pub(super) fn decode_fixed(bytes: &[u8], encoding: STGTextEncoding) -> STGText<'_> {
    let visible_end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let Some(visible) = bytes.get(..visible_end) else {
        unreachable!("STG fixed-text terminator exceeds its source image");
    };
    decode(visible, encoding)
}

pub(super) fn decode(bytes: &[u8], encoding: STGTextEncoding) -> STGText<'_> {
    let decoded = match encoding {
        STGTextEncoding::UTF8 => std::str::from_utf8(bytes).ok().map(Cow::Borrowed),
        STGTextEncoding::CP949 => EUC_KR.decode_without_bom_handling_and_without_replacement(bytes),
    };
    decoded.map_or(STGText::Raw(bytes), STGText::Decoded)
}

pub(super) fn encode_fixed<const N: usize>(
    value: String,
    encoding: STGTextEncoding,
) -> Result<[u8; N], STGTextError> {
    let length = fixed_encoded_len::<N>(&value, encoding)?;
    let encoded = encode_exact(value, encoding, length);
    let mut image = [0_u8; N];
    let Some(destination) = image.get_mut(..encoded.len()) else {
        unreachable!("checked STG fixed text does not fit its image");
    };
    destination.copy_from_slice(&encoded);
    Ok(image)
}

pub(super) fn fixed_encoded_len<const N: usize>(
    value: &str,
    encoding: STGTextEncoding,
) -> Result<usize, STGTextError> {
    let maximum = N.saturating_sub(1);
    let length = encoded_len(value, encoding)?;
    if length > maximum {
        return Err(STGTextError::TooLong { length, maximum });
    }
    Ok(length)
}

pub(super) fn dynamic_encoded_len(value: &str, maximum: u32) -> Result<usize, STGTextError> {
    let maximum_usize =
        usize::try_from(maximum).map_err(|_| STGTextError::DynamicLengthOverflow {
            length: value.len(),
            maximum,
        })?;
    let length = encoded_len(value, STGTextEncoding::CP949)?;
    if length > maximum_usize || u32::try_from(length).is_err() {
        return Err(STGTextError::DynamicLengthOverflow { length, maximum });
    }
    Ok(length)
}

pub(super) fn encode_dynamic(value: String, length: usize) -> Vec<u8> {
    encode_exact(value, STGTextEncoding::CP949, length)
}

pub(super) fn encoded_len(value: &str, encoding: STGTextEncoding) -> Result<usize, STGTextError> {
    if let Some(index) = value.as_bytes().iter().position(|byte| *byte == 0) {
        return Err(STGTextError::ContainsZero { index });
    }

    match encoding {
        STGTextEncoding::UTF8 => Ok(value.len()),
        STGTextEncoding::CP949 => cp949_encoded_len(value),
    }
}

fn cp949_encoded_len(value: &str) -> Result<usize, STGTextError> {
    let mut encoder = EUC_KR.new_encoder();
    let mut source = value;
    let mut length = 0_usize;
    let mut scratch = [0_u8; 4_096];
    loop {
        let (result, read, written) =
            encoder.encode_from_utf8_without_replacement(source, &mut scratch, true);
        length = length.saturating_add(written);
        let Some(remaining) = source.get(read..) else {
            unreachable!("CP949 encoder consumed beyond its UTF8 input");
        };
        source = remaining;
        match result {
            EncoderResult::InputEmpty => return Ok(length),
            EncoderResult::OutputFull => {
                if read == 0 && written == 0 {
                    unreachable!("CP949 encoder made no progress with a nonempty scratch buffer");
                }
            }
            EncoderResult::Unmappable(_) => {
                return Err(STGTextError::Unencodable {
                    encoding: STGTextEncoding::CP949,
                });
            }
        }
    }
}

fn encode_exact(value: String, encoding: STGTextEncoding, length: usize) -> Vec<u8> {
    match encoding {
        STGTextEncoding::UTF8 => exact_vec(value.into_bytes()),
        STGTextEncoding::CP949 => encode_cp949_exact(&value, length),
    }
}

fn encode_cp949_exact(value: &str, length: usize) -> Vec<u8> {
    let mut output = vec![0_u8; length];
    let mut encoder = EUC_KR.new_encoder();
    let mut read = 0_usize;
    let mut written = 0_usize;
    loop {
        let Some(source) = value.get(read..) else {
            unreachable!("CP949 encoder consumed beyond its measured UTF8 input");
        };
        let Some(destination) = output.get_mut(written..) else {
            unreachable!("CP949 encoder wrote beyond its measured output");
        };
        let (result, next_read, next_written) =
            encoder.encode_from_utf8_without_replacement(source, destination, true);
        read = read
            .checked_add(next_read)
            .unwrap_or_else(|| unreachable!("CP949 encoder input position overflowed"));
        written = written
            .checked_add(next_written)
            .unwrap_or_else(|| unreachable!("CP949 encoder output position overflowed"));
        match result {
            EncoderResult::InputEmpty => {
                if read != value.len() || written != length {
                    unreachable!("CP949 encoder disagreed with its measured output length");
                }
                return exact_vec(output);
            }
            EncoderResult::OutputFull => {
                if next_read == 0 && next_written == 0 {
                    unreachable!("CP949 encoder made no progress in measured output storage");
                }
            }
            EncoderResult::Unmappable(_) => {
                unreachable!("measured CP949 text became unencodable");
            }
        }
    }
}

fn exact_vec<T>(values: Vec<T>) -> Vec<T> {
    values.into_boxed_slice().into_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp949_length_preflight_is_exact_and_rejects_unmappable_text() {
        assert_eq!(encoded_len("ASCII", STGTextEncoding::CP949), Ok(5));
        assert_eq!(encoded_len("기사", STGTextEncoding::CP949), Ok(4));
        assert_eq!(
            encoded_len("🙂", STGTextEncoding::CP949),
            Err(STGTextError::Unencodable {
                encoding: STGTextEncoding::CP949,
            })
        );
    }

    #[test]
    fn cp949_length_preflight_resumes_after_filling_its_scratch_buffer() {
        let value = format!("{}기사", "a".repeat(4_095));
        let length = dynamic_encoded_len(&value, u32::MAX).unwrap();
        assert_eq!(length, 4_099);

        let encoded = encode_dynamic(value.clone(), length);
        let (expected, _, had_errors) = EUC_KR.encode(&value);
        assert!(!had_errors);
        assert_eq!(encoded.as_slice(), expected.as_ref());
    }
}
