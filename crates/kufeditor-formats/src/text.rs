use std::collections::HashMap;

use crate::{
    Diagnostic, DiagnosticField, FormatError, Severity, TextSOXParseError, sox::SOXSource,
};

const HEADER_SIZE: usize = 8;
const RECORD_HEADER_SIZE: usize = 6;
const MIN_RECORD_SIZE: usize = RECORD_HEADER_SIZE + 1;
const MARKER: u32 = 100;
const MAX_RECORD_COUNT: u32 = 10_000;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextSOXField {
    Index,
    Text,
}

impl TextSOXField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Index => "Index",
            Self::Text => "Text",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextSOXRecord {
    index: u32,
    text: String,
}

struct ParsedRecords {
    records: Vec<TextSOXRecord>,
    initial_budgets: Vec<u16>,
    source_tail: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct TextSOXDocument {
    source: SOXSource,
    source_records: Vec<TextSOXRecord>,
    records: Vec<TextSOXRecord>,
    source_tail: Vec<u8>,
    initial_budgets: Vec<u16>,
}

impl TextSOXDocument {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, FormatError> {
        Self::from_source(SOXSource::parse(bytes)?)
    }

    pub(crate) fn from_source(source: SOXSource) -> Result<Self, FormatError> {
        let ParsedRecords {
            records,
            initial_budgets,
            source_tail,
        } = parse_records(source.decoded())?;
        Ok(Self {
            source,
            source_records: records.clone(),
            records,
            source_tail,
            initial_budgets,
        })
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn record_index(&self, record: usize) -> Result<u32, FormatError> {
        self.record(record, TextSOXField::Index)
            .map(|record| record.index)
    }

    pub fn max_length(&self, record: usize) -> Result<u16, FormatError> {
        self.initial_budgets
            .get(record)
            .copied()
            .ok_or_else(|| self.record_error(record, TextSOXField::Text))
    }

    pub fn text(&self, record: usize) -> Result<&str, FormatError> {
        self.record(record, TextSOXField::Text)
            .map(|record| record.text.as_str())
    }

    pub fn set_text(&mut self, record: usize, value: String) -> Result<String, FormatError> {
        let maximum = self.max_length(record)?;
        validate_text(record, &value, maximum)?;
        self.record_mut(record, TextSOXField::Text)
            .map(|record| std::mem::replace(&mut record.text, value))
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut counts = HashMap::new();
        for record in &self.records {
            *counts.entry(record.index).or_insert(0_usize) += 1;
        }

        self.records
            .iter()
            .enumerate()
            .filter(|(_, record)| counts.get(&record.index).is_some_and(|count| *count > 1))
            .map(|(record, _)| Diagnostic {
                severity: Severity::Warning,
                record,
                field: DiagnosticField::TextSOX(TextSOXField::Index),
                message: "Stored index is duplicated",
            })
            .collect()
    }

    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        if self.records == self.source_records {
            return Ok(self.source.original_bytes());
        }

        let mut encoded = Vec::with_capacity(self.source.decoded().len());
        encoded.extend_from_slice(&MARKER.to_le_bytes());
        let count = u32::try_from(self.records.len()).map_err(|_| FormatError::TextSOXTooLong {
            record: self.records.len(),
            length: self.records.len(),
            maximum: u16::MAX,
        })?;
        encoded.extend_from_slice(&count.to_le_bytes());
        for (record_index, record) in self.records.iter().enumerate() {
            let length =
                u16::try_from(record.text.len()).map_err(|_| FormatError::TextSOXTooLong {
                    record: record_index,
                    length: record.text.len(),
                    maximum: u16::MAX,
                })?;
            encoded.extend_from_slice(&record.index.to_le_bytes());
            encoded.extend_from_slice(&length.to_le_bytes());
            encoded.extend_from_slice(record.text.as_bytes());
        }
        encoded.extend_from_slice(&self.source_tail);
        Ok(self.source.apply_envelope(&encoded))
    }

    pub fn rebase_source(&mut self, saved: &Self, bytes: Vec<u8>) -> Result<(), FormatError> {
        if bytes != saved.encode()? {
            return Err(FormatError::InconsistentSOXRebase);
        }

        self.source.rebase(&saved.source, bytes)?;
        self.source_records.clone_from(&saved.records);
        self.source_tail.clone_from(&saved.source_tail);
        Ok(())
    }

    fn record(&self, record: usize, field: TextSOXField) -> Result<&TextSOXRecord, FormatError> {
        self.records
            .get(record)
            .ok_or_else(|| self.record_error(record, field))
    }

    fn record_mut(
        &mut self,
        record: usize,
        field: TextSOXField,
    ) -> Result<&mut TextSOXRecord, FormatError> {
        let record_count = self.records.len();
        self.records
            .get_mut(record)
            .ok_or(FormatError::RecordOutOfRange {
                record,
                record_count,
                field: DiagnosticField::TextSOX(field),
            })
    }

    fn record_error(&self, record: usize, field: TextSOXField) -> FormatError {
        FormatError::RecordOutOfRange {
            record,
            record_count: self.records.len(),
            field: DiagnosticField::TextSOX(field),
        }
    }
}

fn parse_records(decoded: &[u8]) -> Result<ParsedRecords, FormatError> {
    let count = parse_record_count(decoded)?;
    let capacity = preflight_record_count(decoded, count)?;
    let mut records = Vec::with_capacity(capacity);
    let mut initial_budgets = Vec::with_capacity(capacity);
    let mut offset = HEADER_SIZE;

    for record in 0..capacity {
        let (parsed, budget) = parse_record(decoded, record, &mut offset)?;
        records.push(parsed);
        initial_budgets.push(budget);
    }

    let source_tail = decoded
        .get(offset..)
        .map_or_else(Vec::new, ToOwned::to_owned);
    Ok(ParsedRecords {
        records,
        initial_budgets,
        source_tail,
    })
}

fn parse_record_count(decoded: &[u8]) -> Result<u32, FormatError> {
    let header = decoded.get(..HEADER_SIZE).ok_or_else(|| {
        parse_error(
            decoded.len(),
            TextSOXParseError::TruncatedHeader {
                actual: decoded.len(),
            },
        )
    })?;
    let marker = read_u32(header.get(..4).ok_or_else(|| {
        parse_error(
            0,
            TextSOXParseError::TruncatedHeader {
                actual: header.len(),
            },
        )
    })?);
    if marker != MARKER {
        return Err(parse_error(0, TextSOXParseError::InvalidMarker { marker }));
    }

    let count = read_u32(header.get(4..).ok_or_else(|| {
        parse_error(
            4,
            TextSOXParseError::TruncatedHeader {
                actual: header.len(),
            },
        )
    })?);
    if !(1..=MAX_RECORD_COUNT).contains(&count) {
        return Err(parse_error(
            4,
            TextSOXParseError::InvalidRecordCount { count },
        ));
    }
    Ok(count)
}

fn preflight_record_count(decoded: &[u8], count: u32) -> Result<usize, FormatError> {
    let remaining = decoded.len().saturating_sub(HEADER_SIZE);
    if count == 1 {
        let header = decoded
            .get(HEADER_SIZE..HEADER_SIZE + RECORD_HEADER_SIZE)
            .ok_or_else(|| {
                parse_error(
                    HEADER_SIZE,
                    TextSOXParseError::TruncatedRecordHeader {
                        record: 0,
                        remaining,
                    },
                )
            })?;
        if read_u16(header.get(4..).ok_or_else(|| {
            parse_error(
                HEADER_SIZE,
                TextSOXParseError::TruncatedRecordHeader {
                    record: 0,
                    remaining,
                },
            )
        })?) == 0
        {
            return Err(parse_error(
                HEADER_SIZE + RECORD_HEADER_SIZE,
                TextSOXParseError::EmptyText { record: 0 },
            ));
        }
    }

    let maximum = remaining / MIN_RECORD_SIZE;
    if u128::from(count) > maximum as u128 {
        return Err(parse_error(
            HEADER_SIZE,
            TextSOXParseError::ImpossibleRecordCount { count, maximum },
        ));
    }
    Ok(usize::try_from(count).unwrap_or(maximum))
}

fn parse_record(
    decoded: &[u8],
    record: usize,
    offset: &mut usize,
) -> Result<(TextSOXRecord, u16), FormatError> {
    let record_bytes = decoded.get(*offset..).unwrap_or_default();
    let header = record_bytes.get(..RECORD_HEADER_SIZE).ok_or_else(|| {
        parse_error(
            *offset,
            TextSOXParseError::TruncatedRecordHeader {
                record,
                remaining: record_bytes.len(),
            },
        )
    })?;
    let index = read_u32(header.get(..4).ok_or_else(|| {
        parse_error(
            *offset,
            TextSOXParseError::TruncatedRecordHeader {
                record,
                remaining: record_bytes.len(),
            },
        )
    })?);
    let length = read_u16(header.get(4..).ok_or_else(|| {
        parse_error(
            *offset,
            TextSOXParseError::TruncatedRecordHeader {
                record,
                remaining: record_bytes.len(),
            },
        )
    })?);
    *offset += RECORD_HEADER_SIZE;
    if length == 0 {
        return Err(parse_error(
            *offset,
            TextSOXParseError::EmptyText { record },
        ));
    }

    let text = parse_text(decoded, record, length, offset)?;
    Ok((TextSOXRecord { index, text }, length))
}

fn parse_text(
    decoded: &[u8],
    record: usize,
    length: u16,
    offset: &mut usize,
) -> Result<String, FormatError> {
    let remaining = decoded.get(*offset..).unwrap_or_default();
    let text_length = usize::from(length);
    let text_bytes = remaining.get(..text_length).ok_or_else(|| {
        parse_error(
            *offset,
            TextSOXParseError::TruncatedText {
                record,
                length,
                remaining: remaining.len(),
            },
        )
    })?;
    for (index, &byte) in text_bytes.iter().enumerate() {
        if !is_allowed_text_byte(byte) {
            return Err(parse_error(
                *offset + index,
                TextSOXParseError::InvalidTextByte {
                    record,
                    index,
                    byte,
                },
            ));
        }
    }
    let text = std::str::from_utf8(text_bytes).map_err(|error| {
        let index = error.valid_up_to();
        parse_error(
            *offset + index,
            TextSOXParseError::InvalidTextByte {
                record,
                index,
                byte: text_bytes.get(index).copied().unwrap_or_default(),
            },
        )
    })?;
    *offset += text_length;
    Ok(text.to_owned())
}

fn parse_error(offset: usize, source: TextSOXParseError) -> FormatError {
    FormatError::TextSOXParse { offset, source }
}

fn read_u16(bytes: &[u8]) -> u16 {
    let [first, second, ..] = bytes else {
        return 0;
    };
    u16::from_le_bytes([*first, *second])
}

fn read_u32(bytes: &[u8]) -> u32 {
    let [first, second, third, fourth, ..] = bytes else {
        return 0;
    };
    u32::from_le_bytes([*first, *second, *third, *fourth])
}

fn validate_text(record: usize, value: &str, maximum: u16) -> Result<(), FormatError> {
    if value.is_empty() {
        return Err(FormatError::TextSOXEmptyText { record });
    }
    if value.len() > usize::from(maximum) {
        return Err(FormatError::TextSOXTooLong {
            record,
            length: value.len(),
            maximum,
        });
    }
    for (index, &byte) in value.as_bytes().iter().enumerate() {
        if !is_allowed_text_byte(byte) {
            return Err(FormatError::TextSOXInvalidTextByte {
                record,
                index,
                byte,
            });
        }
    }
    Ok(())
}

const fn is_allowed_text_byte(byte: u8) -> bool {
    matches!(byte, b' '..=b'~' | b'\t' | b'\n' | b'\r')
}
