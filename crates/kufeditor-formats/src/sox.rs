use crate::{FormatError, SkillDocument, TextSOXDocument, TroopDocument};

#[derive(Clone, Debug)]
pub enum SOXDocument {
    Troop(TroopDocument),
    Skill(SkillDocument),
    Text(TextSOXDocument),
}

pub fn parse_sox(bytes: Vec<u8>) -> Result<SOXDocument, FormatError> {
    let source = SOXSource::parse(bytes)?;
    if let Ok(document) = TroopDocument::from_source(source.clone()) {
        return Ok(SOXDocument::Troop(document));
    }
    if let Ok(document) = SkillDocument::from_source(source.clone()) {
        return Ok(SOXDocument::Skill(document));
    }
    if let Ok(document) = TextSOXDocument::from_source(source) {
        return Ok(SOXDocument::Text(document));
    }
    Err(FormatError::UnsupportedSOX)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SOXEnvelope {
    Raw,
    ASCIIHex,
}

#[derive(Clone, Debug)]
pub(crate) struct SOXSource {
    original: Vec<u8>,
    decoded: Vec<u8>,
    envelope: SOXEnvelope,
}

impl SOXSource {
    pub(crate) fn parse(original: Vec<u8>) -> Result<Self, FormatError> {
        Self::parse_with_marker(original, 100)
    }

    pub(crate) fn parse_with_marker(
        original: Vec<u8>,
        expected_marker: u32,
    ) -> Result<Self, FormatError> {
        let envelope = if is_ascii_hex_candidate(&original, expected_marker) {
            SOXEnvelope::ASCIIHex
        } else {
            SOXEnvelope::Raw
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
            SOXEnvelope::Raw => decoded.to_vec(),
            SOXEnvelope::ASCIIHex => encode_ascii_hex(decoded),
        }
    }

    pub(crate) fn rebase(&mut self, saved: &Self, original: Vec<u8>) -> Result<(), FormatError> {
        if is_ascii_hex_candidate(&original, 100) != matches!(saved.envelope, SOXEnvelope::ASCIIHex)
        {
            return Err(FormatError::InconsistentSOXRebase);
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

fn decode_with_envelope(bytes: &[u8], envelope: SOXEnvelope) -> Result<Vec<u8>, FormatError> {
    match envelope {
        SOXEnvelope::Raw => Ok(bytes.to_vec()),
        SOXEnvelope::ASCIIHex => decode_ascii_hex(bytes),
    }
}

fn is_ascii_hex_candidate(bytes: &[u8], expected_marker: u32) -> bool {
    let Some(prefix) = bytes.get(..16) else {
        return false;
    };

    prefix.iter().all(|byte| hex_value(*byte).is_some())
        && prefix
            .as_chunks::<2>()
            .0
            .iter()
            .zip(expected_marker.to_le_bytes())
            .all(|(pair, expected)| decode_hex_pair(pair[0], pair[1]) == Some(expected))
}

fn decode_ascii_hex(encoded: &[u8]) -> Result<Vec<u8>, FormatError> {
    let (pairs, remainder) = encoded.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(FormatError::OddASCIIHexLength {
            length: encoded.len(),
        });
    }

    let mut decoded = Vec::with_capacity(pairs.len());
    for (pair_index, &[high_byte, low_byte]) in pairs.iter().enumerate() {
        let index = pair_index * 2;
        let high = hex_value(high_byte).ok_or(FormatError::InvalidASCIIHexByte { index })?;
        let low =
            hex_value(low_byte).ok_or(FormatError::InvalidASCIIHexByte { index: index + 1 })?;
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

#[cfg(test)]
mod tests {
    use super::{FormatError, SOXSource};

    #[test]
    fn parses_raw_source_for_expected_marker_two() {
        let original = vec![2, 0, 0, 0, 9];

        let source = SOXSource::parse_with_marker(original.clone(), 2).unwrap();

        assert_eq!(source.decoded(), original);
    }

    #[test]
    fn decodes_mixed_case_ascii_hex_for_expected_marker_two() {
        let source = SOXSource::parse_with_marker(b"0200000000000000aB".to_vec(), 2).unwrap();

        assert_eq!(source.decoded(), [2, 0, 0, 0, 0, 0, 0, 0, 0xab]);
    }

    #[test]
    fn expected_marker_ascii_hex_keeps_odd_length_error() {
        let error = SOXSource::parse_with_marker(b"0200000000000000A".to_vec(), 2).unwrap_err();

        assert!(matches!(
            error,
            FormatError::OddASCIIHexLength { length: 17 }
        ));
    }

    #[test]
    fn expected_marker_ascii_hex_keeps_invalid_byte_error() {
        let error = SOXSource::parse_with_marker(b"0200000000000000Z0".to_vec(), 2).unwrap_err();

        assert!(matches!(
            error,
            FormatError::InvalidASCIIHexByte { index: 16 }
        ));
    }

    #[test]
    fn different_marker_prefix_stays_raw() {
        let original = b"6400000000000000".to_vec();

        let source = SOXSource::parse_with_marker(original.clone(), 2).unwrap();

        assert_eq!(source.decoded(), original);
    }

    #[test]
    fn partial_marker_match_stays_raw_for_automatic_detection() {
        let original = b"6400010000000000".to_vec();

        let source = SOXSource::parse(original.clone()).unwrap();

        assert_eq!(source.decoded(), original);
        assert_eq!(source.original_bytes(), original);
    }

    #[test]
    fn non_hex_byte_inside_marker_prefix_stays_raw() {
        let original = b"64G0000000000000".to_vec();

        let source = SOXSource::parse(original.clone()).unwrap();

        assert_eq!(source.decoded(), original);
        assert_eq!(source.original_bytes(), original);
    }
}
