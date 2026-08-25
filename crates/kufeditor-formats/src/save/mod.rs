mod envelope;
mod fields;
mod number;
mod text;

use crate::{
    error::{FormatError, SaveCleaveError, SaveParseError, SaveRegion},
    generated::kuf_save,
};
use envelope::{CANONICAL_CONTEXT_OFFSET, CONTEXT_SIZE, SaveEnvelope, normalize, restore};
use text::extract_context_text;

pub use fields::{
    SaveEquipmentField, SaveEquipmentGroup, SaveEquipmentSlot, SaveMainField, SaveNumberTarget,
    SaveRosterField, SaveTextField, SaveUnitField, SaveUnitGroup,
};

const COUNT_SIZE: usize = size_of::<u32>();
const UNIT_SIZE: usize = 483;
const ROSTER_SIZE: usize = 8;
const SECOND_ARRAY_VALUE_SIZE: usize = size_of::<u32>();
const SELECTED_REFERENCE_SIZE: usize = size_of::<u32>();
const MISSION_BLOCK_SIZE: usize = 20 * size_of::<u32>() + size_of::<u32>();
const AFTER_UNITS_MINIMUM: usize =
    SELECTED_REFERENCE_SIZE + COUNT_SIZE + COUNT_SIZE + MISSION_BLOCK_SIZE;
const AFTER_ROSTER_MINIMUM: usize = COUNT_SIZE + MISSION_BLOCK_SIZE;
const CANONICAL_UNIT_COUNT_OFFSET: usize =
    COUNT_SIZE + COUNT_SIZE + CONTEXT_SIZE + COUNT_SIZE + 0x154;

#[derive(Clone, Debug)]
pub struct SaveDocument {
    source: Vec<u8>,
    source_file: kuf_save::File,
    file: kuf_save::File,
    envelope: SaveEnvelope,
    context_text: Vec<String>,
}

impl SaveDocument {
    pub fn parse(source: Vec<u8>) -> Result<Self, FormatError> {
        let normalized = normalize(&source).map_err(FormatError::SaveParse)?;
        preflight(&normalized.bytes).map_err(|error| {
            FormatError::SaveParse(preflight_source_error(error, normalized.source_growth))
        })?;

        let context_text = if normalized.envelope.has_context {
            let context_end = CANONICAL_CONTEXT_OFFSET.checked_add(CONTEXT_SIZE).ok_or(
                FormatError::SaveParse(SaveParseError::CanonicalLengthOverflow),
            )?;
            let context = normalized
                .bytes
                .get(CANONICAL_CONTEXT_OFFSET..context_end)
                .ok_or_else(|| {
                    FormatError::SaveParse(SaveParseError::Truncated {
                        region: SaveRegion::Envelope,
                        offset: CANONICAL_CONTEXT_OFFSET,
                        needed: CONTEXT_SIZE,
                        remaining: normalized
                            .bytes
                            .len()
                            .saturating_sub(CANONICAL_CONTEXT_OFFSET),
                    })
                })?;
            extract_context_text(context)
        } else {
            Vec::new()
        };

        let mut offset = 0;
        let file = kuf_save::File::parse(&normalized.bytes, &mut offset).map_err(|source| {
            FormatError::SaveParse(SaveParseError::Cleave {
                offset,
                source: SaveCleaveError::from(source),
            })
        })?;

        Ok(Self {
            source,
            source_file: file.clone(),
            file,
            envelope: normalized.envelope,
            context_text,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        if self.file == self.source_file {
            return Ok(self.source.clone());
        }

        let canonical = self
            .file
            .to_bytes()
            .map_err(SaveCleaveError::from)
            .map_err(crate::error::SaveEncodeError::Cleave)
            .map_err(FormatError::SaveEncode)?;
        restore(&canonical, self.envelope, self.file.tail_data.len())
            .map_err(FormatError::SaveEncode)
    }

    pub const fn has_size_prefix(&self) -> bool {
        self.envelope.has_size_prefix
    }

    pub const fn has_context(&self) -> bool {
        self.envelope.has_context
    }

    pub fn context_text(&self) -> &[String] {
        &self.context_text
    }

    pub fn unit_count(&self) -> usize {
        self.file.units.len()
    }

    pub fn roster_count(&self) -> usize {
        self.file.roster_entries.len()
    }

    pub fn second_array_count(&self) -> usize {
        self.file.second_array.len()
    }
}

fn preflight(bytes: &[u8]) -> Result<(), SaveParseError> {
    let unit_count = read_count(bytes, CANONICAL_UNIT_COUNT_OFFSET, SaveRegion::Units)?;
    let units_offset = checked_add(CANONICAL_UNIT_COUNT_OFFSET, COUNT_SIZE)?;
    let units_end = preflight_items(
        bytes,
        units_offset,
        CANONICAL_UNIT_COUNT_OFFSET,
        unit_count,
        UNIT_SIZE,
        AFTER_UNITS_MINIMUM,
        SaveRegion::Units,
    )?;

    let roster_count_offset =
        require_region(bytes, units_end, SELECTED_REFERENCE_SIZE, SaveRegion::Units)?;
    let roster_count = read_count(bytes, roster_count_offset, SaveRegion::Roster)?;
    let roster_offset = checked_add(roster_count_offset, COUNT_SIZE)?;
    let roster_end = preflight_items(
        bytes,
        roster_offset,
        roster_count_offset,
        roster_count,
        ROSTER_SIZE,
        AFTER_ROSTER_MINIMUM,
        SaveRegion::Roster,
    )?;

    let second_array_count = read_count(bytes, roster_end, SaveRegion::SecondArray)?;
    let second_array_offset = checked_add(roster_end, COUNT_SIZE)?;
    let second_array_end = preflight_items(
        bytes,
        second_array_offset,
        roster_end,
        second_array_count,
        SECOND_ARRAY_VALUE_SIZE,
        MISSION_BLOCK_SIZE,
        SaveRegion::SecondArray,
    )?;
    require_region(
        bytes,
        second_array_end,
        MISSION_BLOCK_SIZE,
        SaveRegion::Missions,
    )?;

    Ok(())
}

fn preflight_source_error(error: SaveParseError, source_growth: usize) -> SaveParseError {
    match error {
        SaveParseError::Truncated {
            region,
            offset,
            needed,
            remaining,
        } => {
            let Some(offset) = offset.checked_sub(source_growth) else {
                return SaveParseError::CanonicalLengthOverflow;
            };
            SaveParseError::Truncated {
                region,
                offset,
                needed,
                remaining,
            }
        }
        SaveParseError::ImpossibleCount {
            region,
            offset,
            count,
            item_size,
            remaining,
        } => {
            let Some(offset) = offset.checked_sub(source_growth) else {
                return SaveParseError::CanonicalLengthOverflow;
            };
            SaveParseError::ImpossibleCount {
                region,
                offset,
                count,
                item_size,
                remaining,
            }
        }
        error => error,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "preflight keeps each wire-layout bound explicit at the call site"
)]
fn preflight_items(
    bytes: &[u8],
    items_offset: usize,
    count_offset: usize,
    count: u32,
    item_size: usize,
    mandatory_after: usize,
    region: SaveRegion,
) -> Result<usize, SaveParseError> {
    let remaining = bytes.len().saturating_sub(items_offset);
    let count_usize = usize::try_from(count)
        .map_err(|_| impossible_count(region, count_offset, count, item_size, remaining))?;

    let available_for_items = remaining.saturating_sub(mandatory_after);
    if count_usize > available_for_items / item_size {
        return Err(impossible_count(
            region,
            count_offset,
            count,
            item_size,
            remaining,
        ));
    }

    let item_bytes = count_usize
        .checked_mul(item_size)
        .ok_or_else(|| impossible_count(region, count_offset, count, item_size, remaining))?;
    items_offset
        .checked_add(item_bytes)
        .ok_or_else(|| impossible_count(region, count_offset, count, item_size, remaining))
}

fn read_count(bytes: &[u8], offset: usize, region: SaveRegion) -> Result<u32, SaveParseError> {
    let end = require_region(bytes, offset, COUNT_SIZE, region)?;
    let value = bytes
        .get(offset..end)
        .ok_or_else(|| truncated(bytes, region, offset, COUNT_SIZE))?;
    let value =
        <[u8; 4]>::try_from(value).map_err(|_| truncated(bytes, region, offset, COUNT_SIZE))?;
    Ok(u32::from_le_bytes(value))
}

fn require_region(
    bytes: &[u8],
    offset: usize,
    needed: usize,
    region: SaveRegion,
) -> Result<usize, SaveParseError> {
    let end = checked_add(offset, needed)?;
    if bytes.get(offset..end).is_none() {
        return Err(truncated(bytes, region, offset, needed));
    }
    Ok(end)
}

fn checked_add(left: usize, right: usize) -> Result<usize, SaveParseError> {
    left.checked_add(right)
        .ok_or(SaveParseError::CanonicalLengthOverflow)
}

const fn impossible_count(
    region: SaveRegion,
    offset: usize,
    count: u32,
    item_size: usize,
    remaining: usize,
) -> SaveParseError {
    SaveParseError::ImpossibleCount {
        region,
        offset,
        count,
        item_size,
        remaining,
    }
}

fn truncated(bytes: &[u8], region: SaveRegion, offset: usize, needed: usize) -> SaveParseError {
    SaveParseError::Truncated {
        region,
        offset,
        needed,
        remaining: bytes.len().saturating_sub(offset),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaveChoice {
    pub value: i64,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveEditor {
    Number { minimum: i64, maximum: i64 },
    Choice { choices: &'static [SaveChoice] },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveMutation<T> {
    Unchanged,
    Changed { previous: T },
}

impl SaveEditor {
    pub const CAMPAIGN: Self = Self::Choice {
        choices: &CAMPAIGN_CHOICES,
    };
    pub const UCD: Self = Self::Choice {
        choices: &UCD_CHOICES,
    };
    pub const HERO: Self = Self::Choice {
        choices: &HERO_CHOICES,
    };
    pub const SKILL: Self = Self::Choice {
        choices: &SKILL_CHOICES,
    };
    pub const RESISTANCE: Self = Self::Choice {
        choices: &RESISTANCE_CHOICES,
    };
}

static CAMPAIGN_CHOICES: [SaveChoice; 4] = [
    SaveChoice {
        value: 0,
        label: "Hironeiden (Gerald)",
    },
    SaveChoice {
        value: 1,
        label: "Vellond (Lucretia)",
    },
    SaveChoice {
        value: 2,
        label: "Ecclesia (Kendal)",
    },
    SaveChoice {
        value: 3,
        label: "Dark Legion (Regnier)",
    },
];

static UCD_CHOICES: [SaveChoice; 4] = [
    SaveChoice {
        value: 0,
        label: "Leader",
    },
    SaveChoice {
        value: 1,
        label: "Officer 1",
    },
    SaveChoice {
        value: 2,
        label: "Officer 2",
    },
    SaveChoice {
        value: 3,
        label: "Troop",
    },
];

static HERO_CHOICES: [SaveChoice; 2] = [
    SaveChoice {
        value: 0,
        label: "Hero",
    },
    SaveChoice {
        value: 1,
        label: "Troop",
    },
];

static SKILL_CHOICES: [SaveChoice; 16] = [
    SaveChoice {
        value: -1,
        label: "None",
    },
    SaveChoice {
        value: 0,
        label: "Melee",
    },
    SaveChoice {
        value: 1,
        label: "Range",
    },
    SaveChoice {
        value: 2,
        label: "Frontal",
    },
    SaveChoice {
        value: 3,
        label: "Riding",
    },
    SaveChoice {
        value: 4,
        label: "Teamwork",
    },
    SaveChoice {
        value: 5,
        label: "Scout",
    },
    SaveChoice {
        value: 6,
        label: "Gunpowder",
    },
    SaveChoice {
        value: 7,
        label: "Taming",
    },
    SaveChoice {
        value: 8,
        label: "Fire",
    },
    SaveChoice {
        value: 9,
        label: "Lightning",
    },
    SaveChoice {
        value: 10,
        label: "Ice",
    },
    SaveChoice {
        value: 11,
        label: "Holy",
    },
    SaveChoice {
        value: 12,
        label: "Earth",
    },
    SaveChoice {
        value: 13,
        label: "Curse",
    },
    SaveChoice {
        value: 14,
        label: "Elemental",
    },
];

static RESISTANCE_CHOICES: [SaveChoice; 11] = [
    SaveChoice {
        value: -1,
        label: "None",
    },
    SaveChoice {
        value: 0,
        label: "Melee",
    },
    SaveChoice {
        value: 1,
        label: "Ranged",
    },
    SaveChoice {
        value: 2,
        label: "Explosion",
    },
    SaveChoice {
        value: 3,
        label: "Frontal",
    },
    SaveChoice {
        value: 4,
        label: "Fire",
    },
    SaveChoice {
        value: 5,
        label: "Lightning",
    },
    SaveChoice {
        value: 6,
        label: "Ice",
    },
    SaveChoice {
        value: 7,
        label: "Holy",
    },
    SaveChoice {
        value: 8,
        label: "Poison",
    },
    SaveChoice {
        value: 9,
        label: "Curse",
    },
];
