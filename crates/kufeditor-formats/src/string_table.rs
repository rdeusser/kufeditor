use std::fmt::{self, Display, Formatter};

use crate::{FormatError, StringTableEncodeError, StringTableParseError, sox::SoxSource};

const HEADER_SIZE: usize = 8;
const MARKER: u32 = 100;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SoxStringTableLayout {
    Sequential,
    Indexed,
    IndexedPair,
    IndexedTriple,
}

impl SoxStringTableLayout {
    const fn field_count(self) -> usize {
        match self {
            Self::Sequential | Self::Indexed => 1,
            Self::IndexedPair => 2,
            Self::IndexedTriple => 3,
        }
    }

    const fn minimum_record_size(self) -> usize {
        match self {
            Self::Sequential => 2,
            Self::Indexed => 6,
            Self::IndexedPair => 8,
            Self::IndexedTriple => 10,
        }
    }
}

impl Display for SoxStringTableLayout {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Sequential => "Sequential",
            Self::Indexed => "Indexed",
            Self::IndexedPair => "IndexedPair",
            Self::IndexedTriple => "IndexedTriple",
        })
    }
}

#[derive(Clone, Debug)]
struct StringTableRecord {
    id: Option<u32>,
    fields: Vec<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct SoxStringTableDocument {
    layout: SoxStringTableLayout,
    source: SoxSource,
    records: Vec<StringTableRecord>,
    trailing: Vec<u8>,
}

impl SoxStringTableDocument {
    pub fn parse(layout: SoxStringTableLayout, bytes: Vec<u8>) -> Result<Self, FormatError> {
        let source = SoxSource::parse(bytes)?;
        let (records, trailing) = parse_records(layout, source.decoded())?;
        Ok(Self {
            layout,
            source,
            records,
            trailing,
        })
    }

    pub const fn layout(&self) -> SoxStringTableLayout {
        self.layout
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn record_id(&self, record: usize) -> Result<Option<u32>, FormatError> {
        self.record(record).map(|record| record.id)
    }

    pub fn field(&self, record: usize, field: usize) -> Result<&[u8], FormatError> {
        let record_value = self.record(record)?;
        record_value.fields.get(field).map(Vec::as_slice).ok_or(
            FormatError::StringTableFieldOutOfRange {
                layout: self.layout,
                record,
                field,
                field_count: record_value.fields.len(),
            },
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        self.source.original_bytes()
    }

    pub fn canonical_encode(&self) -> Result<Vec<u8>, FormatError> {
        let mut decoded = Vec::with_capacity(self.source.decoded().len());
        decoded.extend_from_slice(&MARKER.to_le_bytes());
        let count = u32::try_from(self.records.len()).map_err(|_| {
            self.encode_error(StringTableEncodeError::RecordCountOverflow {
                count: self.records.len(),
                maximum: u32::MAX,
            })
        })?;
        decoded.extend_from_slice(&count.to_le_bytes());

        for (record_index, record) in self.records.iter().enumerate() {
            match self.layout {
                SoxStringTableLayout::Sequential => {}
                SoxStringTableLayout::Indexed
                | SoxStringTableLayout::IndexedPair
                | SoxStringTableLayout::IndexedTriple => append_stored_id(&mut decoded, record),
            }

            for (field, value) in record.fields.iter().enumerate() {
                let length = u16::try_from(value.len()).map_err(|_| {
                    self.encode_error(StringTableEncodeError::FieldLengthOverflow {
                        record: record_index,
                        field,
                        length: value.len(),
                        maximum: u16::MAX,
                    })
                })?;
                decoded.extend_from_slice(&length.to_le_bytes());
                decoded.extend_from_slice(value);
            }
        }
        decoded.extend_from_slice(&self.trailing);
        Ok(self.source.apply_envelope(&decoded))
    }

    fn record(&self, record: usize) -> Result<&StringTableRecord, FormatError> {
        self.records
            .get(record)
            .ok_or(FormatError::StringTableRecordOutOfRange {
                layout: self.layout,
                record,
                record_count: self.records.len(),
            })
    }

    const fn encode_error(&self, source: StringTableEncodeError) -> FormatError {
        FormatError::StringTableEncode {
            layout: self.layout,
            source,
        }
    }
}

fn parse_records(
    layout: SoxStringTableLayout,
    decoded: &[u8],
) -> Result<(Vec<StringTableRecord>, Vec<u8>), FormatError> {
    let count = parse_header(layout, decoded)?;
    let capacity = preflight_record_count(layout, decoded, count)?;
    let mut records = Vec::with_capacity(capacity);
    let mut offset = HEADER_SIZE;

    for record in 0..capacity {
        let id = match layout {
            SoxStringTableLayout::Sequential => None,
            SoxStringTableLayout::Indexed
            | SoxStringTableLayout::IndexedPair
            | SoxStringTableLayout::IndexedTriple => {
                Some(parse_stored_id(layout, decoded, record, &mut offset)?)
            }
        };
        let mut fields = Vec::with_capacity(layout.field_count());
        for field in 0..layout.field_count() {
            fields.push(parse_field(layout, decoded, record, field, &mut offset)?);
        }
        records.push(StringTableRecord { id, fields });
    }

    let trailing = decoded
        .get(offset..)
        .map_or_else(Vec::new, ToOwned::to_owned);
    Ok((records, trailing))
}

fn parse_header(layout: SoxStringTableLayout, decoded: &[u8]) -> Result<u32, FormatError> {
    let header = decoded.get(..HEADER_SIZE).ok_or_else(|| {
        parse_error(
            layout,
            decoded.len(),
            StringTableParseError::TruncatedHeader {
                actual: decoded.len(),
            },
        )
    })?;
    let marker = header.get(..4).and_then(read_u32).ok_or_else(|| {
        parse_error(
            layout,
            decoded.len(),
            StringTableParseError::TruncatedHeader {
                actual: decoded.len(),
            },
        )
    })?;
    if marker != MARKER {
        return Err(parse_error(
            layout,
            0,
            StringTableParseError::InvalidMarker { marker },
        ));
    }
    header.get(4..8).and_then(read_u32).ok_or_else(|| {
        parse_error(
            layout,
            decoded.len(),
            StringTableParseError::TruncatedHeader {
                actual: decoded.len(),
            },
        )
    })
}

fn preflight_record_count(
    layout: SoxStringTableLayout,
    decoded: &[u8],
    count: u32,
) -> Result<usize, FormatError> {
    let remaining = decoded.len().saturating_sub(HEADER_SIZE);
    let minimum_record_size = layout.minimum_record_size();
    let capacity = usize::try_from(count).ok();
    let required = capacity.and_then(|count| count.checked_mul(minimum_record_size));
    if required.is_none_or(|required| required > remaining) {
        return Err(parse_error(
            layout,
            HEADER_SIZE,
            StringTableParseError::ImpossibleRecordCount {
                count,
                minimum_record_size,
                remaining,
            },
        ));
    }

    capacity.ok_or_else(|| {
        parse_error(
            layout,
            HEADER_SIZE,
            StringTableParseError::ImpossibleRecordCount {
                count,
                minimum_record_size,
                remaining,
            },
        )
    })
}

fn parse_stored_id(
    layout: SoxStringTableLayout,
    decoded: &[u8],
    record: usize,
    offset: &mut usize,
) -> Result<u32, FormatError> {
    let remaining = decoded.len().saturating_sub(*offset);
    let end = offset.checked_add(4).ok_or_else(|| {
        parse_error(
            layout,
            *offset,
            StringTableParseError::TruncatedStoredId { record, remaining },
        )
    })?;
    let id = decoded
        .get(*offset..end)
        .and_then(read_u32)
        .ok_or_else(|| {
            parse_error(
                layout,
                *offset,
                StringTableParseError::TruncatedStoredId { record, remaining },
            )
        })?;
    *offset = end;
    Ok(id)
}

fn parse_field(
    layout: SoxStringTableLayout,
    decoded: &[u8],
    record: usize,
    field: usize,
    offset: &mut usize,
) -> Result<Vec<u8>, FormatError> {
    let length_remaining = decoded.len().saturating_sub(*offset);
    let length_end = offset.checked_add(2).ok_or_else(|| {
        parse_error(
            layout,
            *offset,
            StringTableParseError::TruncatedFieldLength {
                record,
                field,
                remaining: length_remaining,
            },
        )
    })?;
    let length = decoded
        .get(*offset..length_end)
        .and_then(read_u16)
        .ok_or_else(|| {
            parse_error(
                layout,
                *offset,
                StringTableParseError::TruncatedFieldLength {
                    record,
                    field,
                    remaining: length_remaining,
                },
            )
        })?;
    let field_offset = length_end;
    let remaining = decoded.len().saturating_sub(field_offset);
    let field_end = field_offset
        .checked_add(usize::from(length))
        .ok_or_else(|| {
            parse_error(
                layout,
                field_offset,
                StringTableParseError::TruncatedFieldPayload {
                    record,
                    field,
                    length,
                    remaining,
                },
            )
        })?;
    let value = decoded.get(field_offset..field_end).ok_or_else(|| {
        parse_error(
            layout,
            field_offset,
            StringTableParseError::TruncatedFieldPayload {
                record,
                field,
                length,
                remaining,
            },
        )
    })?;
    *offset = field_end;
    Ok(value.to_vec())
}

fn append_stored_id(decoded: &mut Vec<u8>, record: &StringTableRecord) {
    if let Some(id) = record.id {
        decoded.extend_from_slice(&id.to_le_bytes());
    }
}

fn read_u16(bytes: &[u8]) -> Option<u16> {
    <[u8; 2]>::try_from(bytes).ok().map(u16::from_le_bytes)
}

fn read_u32(bytes: &[u8]) -> Option<u32> {
    <[u8; 4]>::try_from(bytes).ok().map(u32::from_le_bytes)
}

const fn parse_error(
    layout: SoxStringTableLayout,
    offset: usize,
    source: StringTableParseError,
) -> FormatError {
    FormatError::StringTableParse {
        layout,
        offset,
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::SoxStringTableLayout;

    #[test]
    fn layout_minimum_record_sizes_match_the_wire_table() {
        let cases = [
            (SoxStringTableLayout::Sequential, 2),
            (SoxStringTableLayout::Indexed, 6),
            (SoxStringTableLayout::IndexedPair, 8),
            (SoxStringTableLayout::IndexedTriple, 10),
        ];

        for (layout, expected) in cases {
            assert_eq!(layout.minimum_record_size(), expected);
        }
    }
}
