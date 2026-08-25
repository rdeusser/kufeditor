use std::fmt::{self, Display, Formatter};

use crate::{
    error::{FormatError, GeneratedSoxError},
    generated::{
        sox_ability_by_job, sox_ability_info, sox_char_info, sox_custom_random_table,
        sox_item_att_info, sox_item_type_info, sox_job_info, sox_leader_generation,
        sox_library_info, sox_resist_info, sox_skill_info, sox_skill_point_table,
        sox_special_names, sox_troop_info, sox_unit_uv_info, sox_unit_uvid, sox_worldmap_char_info,
        sox_worldmap_troop_info,
    },
    sox::SoxSource,
};

const HEADER_BYTES: usize = 8;
const CUSTOM_RANDOM_TABLE_BYTES: usize = 1_272;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SoxSchema {
    AbilityByJob,
    AbilityInfo,
    CharInfo,
    CustomRandomTable,
    ItemAttInfo,
    ItemTypeInfo,
    JobInfo,
    LeaderGeneration,
    LibraryInfo,
    ResistInfo,
    SkillInfo,
    SkillPointTable,
    SpecialNames,
    TroopInfo,
    UnitUvInfo,
    UnitUvid,
    WorldmapCharInfo,
    WorldmapTroopInfo,
}

impl SoxSchema {
    pub const ALL: [Self; 18] = [
        Self::AbilityByJob,
        Self::AbilityInfo,
        Self::CharInfo,
        Self::CustomRandomTable,
        Self::ItemAttInfo,
        Self::ItemTypeInfo,
        Self::JobInfo,
        Self::LeaderGeneration,
        Self::LibraryInfo,
        Self::ResistInfo,
        Self::SkillInfo,
        Self::SkillPointTable,
        Self::SpecialNames,
        Self::TroopInfo,
        Self::UnitUvInfo,
        Self::UnitUvid,
        Self::WorldmapCharInfo,
        Self::WorldmapTroopInfo,
    ];

    pub const fn file_stem(self) -> &'static str {
        match self {
            Self::AbilityByJob => "AbilityByJob",
            Self::AbilityInfo => "AbilityInfo",
            Self::CharInfo => "CharInfo",
            Self::CustomRandomTable => "KUF2CustomRandomTable",
            Self::ItemAttInfo => "ItemAttInfo",
            Self::ItemTypeInfo => "ItemTypeInfo",
            Self::JobInfo => "JobInfo",
            Self::LeaderGeneration => "LeaderGeneration",
            Self::LibraryInfo => "LibraryInfo",
            Self::ResistInfo => "ResistInfo",
            Self::SkillInfo => "SkillInfo",
            Self::SkillPointTable => "SkillPointTable",
            Self::SpecialNames => "SpecialNames",
            Self::TroopInfo => "TroopInfo",
            Self::UnitUvInfo => "UnitUVInfo",
            Self::UnitUvid => "UnitUVID",
            Self::WorldmapCharInfo => "WorldMap_CharInfo",
            Self::WorldmapTroopInfo => "WorldMap_TroopInfo",
        }
    }

    pub const fn marker(self) -> u32 {
        match self {
            Self::ItemTypeInfo => 2,
            _ => 100,
        }
    }

    const fn record_layout(self) -> Option<(usize, usize)> {
        match self {
            Self::AbilityByJob => Some((24, 64)),
            Self::AbilityInfo => Some((64, 64)),
            Self::CharInfo => Some((136, 64)),
            Self::CustomRandomTable => None,
            Self::ItemAttInfo | Self::ResistInfo => Some((12, 64)),
            Self::ItemTypeInfo => Some((178, 0)),
            Self::JobInfo | Self::LeaderGeneration | Self::UnitUvid => Some((72, 64)),
            Self::LibraryInfo => Some((6, 64)),
            Self::SkillInfo => Some((16, 64)),
            Self::SkillPointTable => Some((8, 64)),
            Self::SpecialNames => Some((4, 64)),
            Self::TroopInfo => Some((148, 64)),
            Self::UnitUvInfo => Some((36, 64)),
            Self::WorldmapCharInfo | Self::WorldmapTroopInfo => Some((28, 64)),
        }
    }
}

impl Display for SoxSchema {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.file_stem())
    }
}

#[derive(Clone, Debug)]
pub struct SchemaDocument {
    schema: SoxSchema,
    source: SoxSource,
    model: SchemaModel,
    trailing: Vec<u8>,
}

impl SchemaDocument {
    pub fn parse(schema: SoxSchema, bytes: Vec<u8>) -> Result<Self, FormatError> {
        let source = SoxSource::parse_with_marker(bytes, schema.marker())?;
        preflight(schema, source.decoded())?;

        let mut offset = 0;
        let model =
            SchemaModel::parse(schema, source.decoded(), &mut offset).map_err(|source| {
                FormatError::SchemaParse {
                    schema,
                    offset,
                    source,
                }
            })?;
        let trailing = source.decoded().get(offset..).unwrap_or_default().to_vec();

        Ok(Self {
            schema,
            source,
            model,
            trailing,
        })
    }

    pub const fn schema(&self) -> SoxSchema {
        self.schema
    }

    pub fn record_count(&self) -> usize {
        self.model.record_count()
    }

    pub fn encode(&self) -> Vec<u8> {
        self.source.original_bytes()
    }

    pub fn canonical_encode(&self) -> Result<Vec<u8>, FormatError> {
        let mut decoded = self
            .model
            .to_bytes()
            .map_err(|source| FormatError::SchemaEncode {
                schema: self.schema,
                source,
            })?;
        decoded.extend_from_slice(&self.trailing);
        Ok(self.source.apply_envelope(&decoded))
    }
}

fn preflight(schema: SoxSchema, decoded: &[u8]) -> Result<(), FormatError> {
    let marker = read_u32(schema, decoded, 0)?;
    if marker != schema.marker() {
        return Err(schema_parse_error(
            schema,
            4,
            GeneratedSoxError::Validation {
                id: "marker.check",
                message: marker_message(schema),
                field: "marker",
            },
        ));
    }

    if schema == SoxSchema::CustomRandomTable {
        if decoded.len() < CUSTOM_RANDOM_TABLE_BYTES {
            return Err(schema_parse_error(
                schema,
                decoded.len(),
                GeneratedSoxError::UnexpectedEof {
                    offset: decoded.len(),
                    needed: CUSTOM_RANDOM_TABLE_BYTES - decoded.len(),
                    remaining: 0,
                },
            ));
        }
        return Ok(());
    }

    let count = read_u32(schema, decoded, 4)?;
    let Some((minimum_record_bytes, footer_bytes)) = schema.record_layout() else {
        return Ok(());
    };
    let required = usize::try_from(count)
        .ok()
        .and_then(|count| minimum_record_bytes.checked_mul(count))
        .and_then(|record_bytes| HEADER_BYTES.checked_add(record_bytes))
        .and_then(|size| size.checked_add(footer_bytes));

    if required.is_none_or(|required| required > decoded.len()) {
        return Err(schema_parse_error(
            schema,
            HEADER_BYTES,
            GeneratedSoxError::InvalidLength {
                field: "records",
                value: i128::from(count),
            },
        ));
    }

    Ok(())
}

fn read_u32(schema: SoxSchema, bytes: &[u8], offset: usize) -> Result<u32, FormatError> {
    let remaining = bytes.len().saturating_sub(offset);
    let Some(end) = offset.checked_add(4) else {
        return Err(schema_parse_error(
            schema,
            offset,
            GeneratedSoxError::LengthOverflow {
                field: "offset",
                value: 4_usize.to_string(),
                target: "usize",
            },
        ));
    };
    let Some(value) = bytes.get(offset..end) else {
        return Err(schema_parse_error(
            schema,
            offset,
            GeneratedSoxError::UnexpectedEof {
                offset,
                needed: 4,
                remaining,
            },
        ));
    };
    let value = <[u8; 4]>::try_from(value).map_err(|_| {
        schema_parse_error(
            schema,
            offset,
            GeneratedSoxError::FixedSize {
                field: "primitive",
                expected: 4,
                actual: value.len(),
            },
        )
    })?;
    Ok(u32::from_le_bytes(value))
}

const fn marker_message(schema: SoxSchema) -> &'static str {
    match schema {
        SoxSchema::ItemTypeInfo => "ItemTypeInfo marker must be 2",
        _ => "SOX marker must be 100",
    }
}

const fn schema_parse_error(
    schema: SoxSchema,
    offset: usize,
    source: GeneratedSoxError,
) -> FormatError {
    FormatError::SchemaParse {
        schema,
        offset,
        source,
    }
}

macro_rules! define_schema_models {
    ($($variant:ident => $module:ident;)+) => {
        #[derive(Clone, Debug)]
        enum SchemaModel {
            $($variant(Box<$module::File>),)+
        }

        impl SchemaModel {
            fn parse(
                schema: SoxSchema,
                bytes: &[u8],
                offset: &mut usize,
            ) -> Result<Self, GeneratedSoxError> {
                match schema {
                    $(
                        SoxSchema::$variant => $module::File::parse(bytes, offset)
                            .map(Box::new)
                            .map(Self::$variant)
                            .map_err(GeneratedSoxError::from),
                    )+
                }
            }

            fn record_count(&self) -> usize {
                match self {
                    $(Self::$variant(file) => file.records.len(),)+
                }
            }

            fn to_bytes(&self) -> Result<Vec<u8>, GeneratedSoxError> {
                match self {
                    $(
                        Self::$variant(file) => file
                            .to_bytes()
                            .map_err(GeneratedSoxError::from),
                    )+
                }
            }
        }
    };
}

define_schema_models!(
    AbilityByJob => sox_ability_by_job;
    AbilityInfo => sox_ability_info;
    CharInfo => sox_char_info;
    CustomRandomTable => sox_custom_random_table;
    ItemAttInfo => sox_item_att_info;
    ItemTypeInfo => sox_item_type_info;
    JobInfo => sox_job_info;
    LeaderGeneration => sox_leader_generation;
    LibraryInfo => sox_library_info;
    ResistInfo => sox_resist_info;
    SkillInfo => sox_skill_info;
    SkillPointTable => sox_skill_point_table;
    SpecialNames => sox_special_names;
    TroopInfo => sox_troop_info;
    UnitUvInfo => sox_unit_uv_info;
    UnitUvid => sox_unit_uvid;
    WorldmapCharInfo => sox_worldmap_char_info;
    WorldmapTroopInfo => sox_worldmap_troop_info;
);
