use crate::error::{SaveEncodeError, SaveParseError, SaveRegion};

pub(super) const CONTEXT_SIZE: usize = 0x438;
pub(super) const CANONICAL_CONTEXT_OFFSET: usize = 8;

const SIZE_PREFIX_SIZE: usize = size_of::<u32>();
const MAGIC: u32 = 0x6e;
const MAGIC_SIZE: usize = size_of::<u32>();
const CANONICAL_BODY_OFFSET: usize = CANONICAL_CONTEXT_OFFSET + CONTEXT_SIZE;
const PADDED_SIZE: usize = 0x8000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SaveEnvelope {
    pub(super) has_size_prefix: bool,
    pub(super) has_context: bool,
}

#[derive(Debug)]
pub(super) struct NormalizedSave {
    pub(super) bytes: Vec<u8>,
    pub(super) envelope: SaveEnvelope,
    pub(super) source_growth: usize,
}

pub(super) fn normalize(source: &[u8]) -> Result<NormalizedSave, SaveParseError> {
    normalize_with_reserve(source, |bytes, requested| {
        bytes.try_reserve_exact(requested).map_err(|_| ())
    })
}

pub(super) fn retained_tail_length(source_length: usize, parsed_tail_length: usize) -> usize {
    source_length
        .checked_sub(PADDED_SIZE)
        .map_or(parsed_tail_length, |length| length.min(parsed_tail_length))
}

pub(super) fn restore(
    canonical: &[u8],
    envelope: SaveEnvelope,
    retained_tail_length: usize,
) -> Result<Vec<u8>, SaveEncodeError> {
    restore_with_reserve(
        canonical,
        envelope,
        retained_tail_length,
        |bytes, requested| bytes.try_reserve_exact(requested).map_err(|_| ()),
    )
}

fn restore_with_reserve<F>(
    canonical: &[u8],
    envelope: SaveEnvelope,
    retained_tail_length: usize,
    reserve: F,
) -> Result<Vec<u8>, SaveEncodeError>
where
    F: FnOnce(&mut Vec<u8>, usize) -> Result<(), ()>,
{
    let magic_end = SIZE_PREFIX_SIZE
        .checked_add(MAGIC_SIZE)
        .ok_or(SaveEncodeError::LengthOverflow { length: usize::MAX })?;
    let magic = canonical.get(SIZE_PREFIX_SIZE..magic_end).ok_or(
        SaveEncodeError::InvalidCanonicalShape {
            length: canonical.len(),
            minimum: CANONICAL_BODY_OFFSET,
        },
    )?;
    let context = canonical
        .get(CANONICAL_CONTEXT_OFFSET..CANONICAL_BODY_OFFSET)
        .ok_or(SaveEncodeError::InvalidCanonicalShape {
            length: canonical.len(),
            minimum: CANONICAL_BODY_OFFSET,
        })?;
    let body =
        canonical
            .get(CANONICAL_BODY_OFFSET..)
            .ok_or(SaveEncodeError::InvalidCanonicalShape {
                length: canonical.len(),
                minimum: CANONICAL_BODY_OFFSET,
            })?;
    let minimum_length = CANONICAL_BODY_OFFSET.saturating_add(retained_tail_length);
    let main_body_length = body.len().checked_sub(retained_tail_length).ok_or(
        SaveEncodeError::InvalidCanonicalShape {
            length: canonical.len(),
            minimum: minimum_length,
        },
    )?;
    let main_body = body
        .get(..main_body_length)
        .ok_or(SaveEncodeError::InvalidCanonicalShape {
            length: canonical.len(),
            minimum: minimum_length,
        })?;
    let tail = body
        .get(main_body_length..)
        .ok_or(SaveEncodeError::InvalidCanonicalShape {
            length: canonical.len(),
            minimum: minimum_length,
        })?;

    let prefix_length = if envelope.has_size_prefix {
        SIZE_PREFIX_SIZE
    } else {
        0
    };
    let context_length = if envelope.has_context {
        CONTEXT_SIZE
    } else {
        0
    };
    let unpadded_main_length = prefix_length
        .checked_add(MAGIC_SIZE)
        .and_then(|length| length.checked_add(context_length))
        .and_then(|length| length.checked_add(main_body.len()))
        .ok_or(SaveEncodeError::LengthOverflow { length: usize::MAX })?;
    let padded_main_length = unpadded_main_length.max(PADDED_SIZE);
    let final_length = padded_main_length
        .checked_add(tail.len())
        .ok_or(SaveEncodeError::LengthOverflow { length: usize::MAX })?;
    let prefix_value = if envelope.has_size_prefix {
        Some(
            u32::try_from(final_length).map_err(|_| SaveEncodeError::LengthOverflow {
                length: final_length,
            })?,
        )
    } else {
        None
    };

    let mut output = Vec::new();
    reserve(&mut output, final_length).map_err(|()| SaveEncodeError::Allocation {
        requested: final_length,
    })?;
    if envelope.has_size_prefix {
        output.extend_from_slice(&[0; SIZE_PREFIX_SIZE]);
    }
    output.extend_from_slice(magic);
    if envelope.has_context {
        output.extend_from_slice(context);
    }
    output.extend_from_slice(main_body);
    output.resize(padded_main_length, 0);
    output.extend_from_slice(tail);

    if let Some(prefix_value) = prefix_value {
        let output_length = output.len();
        let prefix =
            output
                .get_mut(..SIZE_PREFIX_SIZE)
                .ok_or(SaveEncodeError::InvalidCanonicalShape {
                    length: output_length,
                    minimum: SIZE_PREFIX_SIZE,
                })?;
        prefix.copy_from_slice(&prefix_value.to_le_bytes());
    }

    Ok(output)
}

fn normalize_with_reserve<F>(source: &[u8], reserve: F) -> Result<NormalizedSave, SaveParseError>
where
    F: FnOnce(&mut Vec<u8>, usize) -> Result<(), ()>,
{
    let envelope = detect(source)?;
    let prefix_growth = if envelope.has_size_prefix {
        0
    } else {
        SIZE_PREFIX_SIZE
    };
    let context_growth = if envelope.has_context {
        0
    } else {
        CONTEXT_SIZE
    };
    let source_growth = prefix_growth
        .checked_add(context_growth)
        .ok_or(SaveParseError::CanonicalLengthOverflow)?;
    let canonical_length = source
        .len()
        .checked_add(source_growth)
        .ok_or(SaveParseError::CanonicalLengthOverflow)?;
    let canonical_length_u32 =
        u32::try_from(canonical_length).map_err(|_| SaveParseError::CanonicalLengthOverflow)?;

    let mut bytes = Vec::new();
    reserve(&mut bytes, canonical_length).map_err(|()| SaveParseError::Allocation {
        requested: canonical_length,
    })?;

    bytes.extend_from_slice(&canonical_length_u32.to_le_bytes());
    bytes.extend_from_slice(&MAGIC.to_le_bytes());

    let source_magic_offset = if envelope.has_size_prefix {
        SIZE_PREFIX_SIZE
    } else {
        0
    };
    let source_body_offset = source_magic_offset
        .checked_add(size_of::<u32>())
        .ok_or(SaveParseError::CanonicalLengthOverflow)?;
    let remaining_offset = if envelope.has_context {
        let context_end = source_body_offset
            .checked_add(CONTEXT_SIZE)
            .ok_or(SaveParseError::CanonicalLengthOverflow)?;
        let context = source.get(source_body_offset..context_end).ok_or_else(|| {
            truncated(
                source,
                SaveRegion::Envelope,
                source_body_offset,
                CONTEXT_SIZE,
            )
        })?;
        bytes.extend_from_slice(context);
        context_end
    } else {
        let context_end = bytes
            .len()
            .checked_add(CONTEXT_SIZE)
            .ok_or(SaveParseError::CanonicalLengthOverflow)?;
        bytes.resize(context_end, 0);
        source_body_offset
    };

    let remaining = source
        .get(remaining_offset..)
        .ok_or(SaveParseError::InvalidEnvelope)?;
    bytes.extend_from_slice(remaining);
    debug_assert_eq!(bytes.len(), canonical_length);

    Ok(NormalizedSave {
        bytes,
        envelope,
        source_growth,
    })
}

fn detect(source: &[u8]) -> Result<SaveEnvelope, SaveParseError> {
    let first = read_u32(source, 0)?;
    let source_length_matches = u32::try_from(source.len()).is_ok_and(|length| length == first);
    let (has_size_prefix, magic_offset) = if source_length_matches {
        let prefixed_magic = read_u32(source, SIZE_PREFIX_SIZE)?;
        if prefixed_magic == MAGIC {
            (true, SIZE_PREFIX_SIZE)
        } else if first == MAGIC {
            (false, 0)
        } else {
            return Err(SaveParseError::InvalidMagic {
                offset: SIZE_PREFIX_SIZE,
                actual: prefixed_magic,
            });
        }
    } else if first == MAGIC {
        (false, 0)
    } else {
        return Err(SaveParseError::InvalidMagic {
            offset: 0,
            actual: first,
        });
    };

    let campaign_offset = magic_offset
        .checked_add(size_of::<u32>())
        .ok_or(SaveParseError::CanonicalLengthOverflow)?;
    if is_campaign(read_i32(source, campaign_offset)?) {
        return Ok(SaveEnvelope {
            has_size_prefix,
            has_context: false,
        });
    }

    let context_campaign_offset = campaign_offset
        .checked_add(CONTEXT_SIZE)
        .ok_or(SaveParseError::CanonicalLengthOverflow)?;
    if is_campaign(read_i32(source, context_campaign_offset)?) {
        return Ok(SaveEnvelope {
            has_size_prefix,
            has_context: true,
        });
    }

    Err(SaveParseError::InvalidEnvelope)
}

fn read_u32(source: &[u8], offset: usize) -> Result<u32, SaveParseError> {
    let bytes = read_array(source, offset)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i32(source: &[u8], offset: usize) -> Result<i32, SaveParseError> {
    let bytes = read_array(source, offset)?;
    Ok(i32::from_le_bytes(bytes))
}

fn read_array(source: &[u8], offset: usize) -> Result<[u8; 4], SaveParseError> {
    let end = offset
        .checked_add(size_of::<u32>())
        .ok_or(SaveParseError::CanonicalLengthOverflow)?;
    let bytes = source
        .get(offset..end)
        .ok_or_else(|| truncated(source, SaveRegion::Envelope, offset, size_of::<u32>()))?;
    <[u8; 4]>::try_from(bytes)
        .map_err(|_| truncated(source, SaveRegion::Envelope, offset, size_of::<u32>()))
}

fn truncated(source: &[u8], region: SaveRegion, offset: usize, needed: usize) -> SaveParseError {
    SaveParseError::Truncated {
        region,
        offset,
        needed,
        remaining: source.len().saturating_sub(offset),
    }
}

const fn is_campaign(value: i32) -> bool {
    value >= 0 && value <= 3
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{CONTEXT_SIZE, MAGIC, SIZE_PREFIX_SIZE, normalize_with_reserve};
    use crate::error::SaveParseError;

    #[test]
    fn save_allocation_failure_is_typed() {
        let source = [MAGIC.to_le_bytes(), 0_u32.to_le_bytes()].concat();
        let observed_request = Cell::new(0);

        let result = normalize_with_reserve(&source, |_, requested| {
            observed_request.set(requested);
            Err(())
        });

        let requested = source.len() + SIZE_PREFIX_SIZE + CONTEXT_SIZE;
        assert_eq!(observed_request.get(), requested);
        assert!(matches!(
            result,
            Err(SaveParseError::Allocation {
                requested: actual,
            }) if actual == requested
        ));
    }
}
