use crate::{FormatError, SkillDocument, TroopDocument};

#[derive(Clone, Debug)]
pub enum SoxDocument {
    Troop(TroopDocument),
    Skill(SkillDocument),
}

pub fn parse_sox(bytes: Vec<u8>) -> Result<SoxDocument, FormatError> {
    let source = SoxSource::parse(bytes)?;
    if let Ok(document) = TroopDocument::from_source(source.clone()) {
        return Ok(SoxDocument::Troop(document));
    }
    if let Ok(document) = SkillDocument::from_source(source) {
        return Ok(SoxDocument::Skill(document));
    }
    Err(FormatError::UnsupportedSox)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SoxEnvelope {
    Raw,
    AsciiHex,
}

#[derive(Clone, Debug)]
pub(crate) struct SoxSource {
    original: Vec<u8>,
    decoded: Vec<u8>,
    envelope: SoxEnvelope,
}

impl SoxSource {
    pub(crate) fn parse(original: Vec<u8>) -> Result<Self, FormatError> {
        let envelope = if is_ascii_hex_candidate(&original) {
            SoxEnvelope::AsciiHex
        } else {
            SoxEnvelope::Raw
        };
        let decoded = decode_with_envelope(&original, envelope)?;

        Ok(Self {
            original,
            decoded,
            envelope,
        })
    }

    pub(crate) fn decoded(&self) -> &[u8] {
        &self.decoded
    }

    pub(crate) fn original_bytes(&self) -> Vec<u8> {
        self.original.clone()
    }

    pub(crate) fn apply_envelope(&self, decoded: &[u8]) -> Vec<u8> {
        match self.envelope {
            SoxEnvelope::Raw => decoded.to_vec(),
            SoxEnvelope::AsciiHex => encode_ascii_hex(decoded),
        }
    }

    pub(crate) fn rebase(&mut self, saved: &Self, original: Vec<u8>) -> Result<(), FormatError> {
        if is_ascii_hex_candidate(&original) != matches!(saved.envelope, SoxEnvelope::AsciiHex) {
            return Err(FormatError::InconsistentSoxRebase);
        }

        let decoded = decode_with_envelope(&original, saved.envelope)?;
        *self = Self {
            original,
            decoded,
            envelope: saved.envelope,
        };
        Ok(())
    }
}

fn decode_with_envelope(bytes: &[u8], envelope: SoxEnvelope) -> Result<Vec<u8>, FormatError> {
    match envelope {
        SoxEnvelope::Raw => Ok(bytes.to_vec()),
        SoxEnvelope::AsciiHex => decode_ascii_hex(bytes),
    }
}

fn is_ascii_hex_candidate(bytes: &[u8]) -> bool {
    let Some(prefix) = bytes.get(..16) else {
        return false;
    };
    let &[first, second, third, fourth, ..] = prefix else {
        return false;
    };

    prefix.iter().all(|byte| hex_value(*byte).is_some())
        && matches!(
            (
                decode_hex_pair(first, second),
                decode_hex_pair(third, fourth)
            ),
            (Some(0x64), Some(0))
        )
}

fn decode_ascii_hex(encoded: &[u8]) -> Result<Vec<u8>, FormatError> {
    let (pairs, remainder) = encoded.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(FormatError::OddAsciiHexLength {
            length: encoded.len(),
        });
    }

    let mut decoded = Vec::with_capacity(pairs.len());
    for (pair_index, &[high_byte, low_byte]) in pairs.iter().enumerate() {
        let index = pair_index * 2;
        let high = hex_value(high_byte).ok_or(FormatError::InvalidAsciiHexByte { index })?;
        let low =
            hex_value(low_byte).ok_or(FormatError::InvalidAsciiHexByte { index: index + 1 })?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn encode_ascii_hex(decoded: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(decoded.len() * 2);
    for &byte in decoded {
        encoded.push(encode_hex_nibble(byte >> 4));
        encoded.push(encode_hex_nibble(byte & 0x0f));
    }
    encoded
}

fn decode_hex_pair(high: u8, low: u8) -> Option<u8> {
    Some((hex_value(high)? << 4) | hex_value(low)?)
}

const fn encode_hex_nibble(nibble: u8) -> u8 {
    if nibble < 10 {
        b'0' + nibble
    } else {
        b'A' + (nibble - 10)
    }
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
