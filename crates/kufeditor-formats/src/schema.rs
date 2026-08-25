use std::fmt::{self, Display, Formatter};

use crate::{
    error::{FormatError, GeneratedSOXError},
    generated::{
        sox_ability_by_job, sox_ability_info, sox_char_info, sox_custom_random_table,
        sox_item_att_info, sox_item_type_info, sox_job_info, sox_leader_generation,
        sox_library_info, sox_resist_info, sox_skill_info, sox_skill_point_table,
        sox_special_names, sox_troop_info, sox_unit_uv_info, sox_unit_uvid, sox_worldmap_char_info,
        sox_worldmap_troop_info,
    },
    sox::SOXSource,
};

const HEADER_BYTES: usize = 8;
const CUSTOM_RANDOM_TABLE_BYTES: usize = 1_272;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SOXSchema {
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
    UnitUVInfo,
    UnitUVID,
    WorldmapCharInfo,
    WorldmapTroopInfo,
}

impl SOXSchema {
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
        Self::UnitUVInfo,
        Self::UnitUVID,
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
            Self::UnitUVInfo => "UnitUVInfo",
            Self::UnitUVID => "UnitUVID",
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
            Self::JobInfo | Self::LeaderGeneration | Self::UnitUVID => Some((72, 64)),
            Self::LibraryInfo => Some((6, 64)),
            Self::SkillInfo => Some((16, 64)),
            Self::SkillPointTable => Some((8, 64)),
            Self::SpecialNames => Some((4, 64)),
            Self::TroopInfo => Some((148, 64)),
            Self::UnitUVInfo => Some((36, 64)),
            Self::WorldmapCharInfo | Self::WorldmapTroopInfo => Some((28, 64)),
        }
    }
}

impl Display for SOXSchema {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.file_stem())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpecialNameRef<'a> {
    pub key: &'a [u8],
    pub value: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct SchemaDocument {
    schema: SOXSchema,
    source: SOXSource,
    model: SchemaModel,
    trailing: Vec<u8>,
}

impl SchemaDocument {
    pub fn parse(schema: SOXSchema, bytes: Vec<u8>) -> Result<Self, FormatError> {
        let source = SOXSource::parse_with_marker(bytes, schema.marker())?;
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

    pub const fn schema(&self) -> SOXSchema {
        self.schema
    }

    pub fn record_count(&self) -> usize {
        self.model.record_count()
    }

    pub fn special_name(&self, record: usize) -> Option<SpecialNameRef<'_>> {
        let SchemaModel::SpecialNames(file) = &self.model else {
            return None;
        };
        let record = file.records.get(record)?;

        Some(SpecialNameRef {
            key: &record.key.value,
            value: &record.value.value,
        })
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

fn preflight(schema: SOXSchema, decoded: &[u8]) -> Result<(), FormatError> {
    let marker = read_u32(schema, decoded, 0)?;
    if marker != schema.marker() {
        return Err(schema_parse_error(
            schema,
            4,
            GeneratedSOXError::Validation {
                id: "marker.check",
                message: marker_message(schema),
                field: "marker",
            },
        ));
    }

    if schema == SOXSchema::CustomRandomTable {
        if decoded.len() < CUSTOM_RANDOM_TABLE_BYTES {
            return Err(schema_parse_error(
                schema,
                decoded.len(),
                GeneratedSOXError::UnexpectedEOF {
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
            GeneratedSOXError::InvalidLength {
                field: "records",
                value: i128::from(count),
            },
        ));
    }

    Ok(())
}

fn read_u32(schema: SOXSchema, bytes: &[u8], offset: usize) -> Result<u32, FormatError> {
    let remaining = bytes.len().saturating_sub(offset);
    let Some(end) = offset.checked_add(4) else {
        return Err(schema_parse_error(
            schema,
            offset,
            GeneratedSOXError::LengthOverflow {
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
            GeneratedSOXError::UnexpectedEOF {
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
            GeneratedSOXError::FixedSize {
                field: "primitive",
                expected: 4,
                actual: value.len(),
            },
        )
    })?;
    Ok(u32::from_le_bytes(value))
}

const fn marker_message(schema: SOXSchema) -> &'static str {
    match schema {
        SOXSchema::ItemTypeInfo => "ItemTypeInfo marker must be 2",
        _ => "SOX marker must be 100",
    }
}

const fn schema_parse_error(
    schema: SOXSchema,
    offset: usize,
    source: GeneratedSOXError,
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
                schema: SOXSchema,
                bytes: &[u8],
                offset: &mut usize,
            ) -> Result<Self, GeneratedSOXError> {
                match schema {
                    $(
                        SOXSchema::$variant => $module::File::parse(bytes, offset)
                            .map(Box::new)
                            .map(Self::$variant)
                            .map_err(GeneratedSOXError::from),
                    )+
                }
            }

            fn record_count(&self) -> usize {
                match self {
                    $(Self::$variant(file) => file.records.len(),)+
                }
            }

            fn to_bytes(&self) -> Result<Vec<u8>, GeneratedSOXError> {
                match self {
                    $(
                        Self::$variant(file) => file
                            .to_bytes()
                            .map_err(GeneratedSOXError::from),
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
    UnitUVInfo => sox_unit_uv_info;
    UnitUVID => sox_unit_uvid;
    WorldmapCharInfo => sox_worldmap_char_info;
    WorldmapTroopInfo => sox_worldmap_troop_info;
);

const _: fn(SchemaModel) = |model| match model {
    SchemaModel::AbilityByJob(file) => {
        let _: Box<sox_ability_by_job::File> = file;
    }
    SchemaModel::AbilityInfo(file) => {
        let _: Box<sox_ability_info::File> = file;
    }
    SchemaModel::CharInfo(file) => {
        let _: Box<sox_char_info::File> = file;
    }
    SchemaModel::CustomRandomTable(file) => {
        let _: Box<sox_custom_random_table::File> = file;
    }
    SchemaModel::ItemAttInfo(file) => {
        let _: Box<sox_item_att_info::File> = file;
    }
    SchemaModel::ItemTypeInfo(file) => {
        let _: Box<sox_item_type_info::File> = file;
    }
    SchemaModel::JobInfo(file) => {
        let _: Box<sox_job_info::File> = file;
    }
    SchemaModel::LeaderGeneration(file) => {
        let _: Box<sox_leader_generation::File> = file;
    }
    SchemaModel::LibraryInfo(file) => {
        let _: Box<sox_library_info::File> = file;
    }
    SchemaModel::ResistInfo(file) => {
        let _: Box<sox_resist_info::File> = file;
    }
    SchemaModel::SkillInfo(file) => {
        let _: Box<sox_skill_info::File> = file;
    }
    SchemaModel::SkillPointTable(file) => {
        let _: Box<sox_skill_point_table::File> = file;
    }
    SchemaModel::SpecialNames(file) => {
        let _: Box<sox_special_names::File> = file;
    }
    SchemaModel::TroopInfo(file) => {
        let _: Box<sox_troop_info::File> = file;
    }
    SchemaModel::UnitUVInfo(file) => {
        let _: Box<sox_unit_uv_info::File> = file;
    }
    SchemaModel::UnitUVID(file) => {
        let _: Box<sox_unit_uvid::File> = file;
    }
    SchemaModel::WorldmapCharInfo(file) => {
        let _: Box<sox_worldmap_char_info::File> = file;
    }
    SchemaModel::WorldmapTroopInfo(file) => {
        let _: Box<sox_worldmap_troop_info::File> = file;
    }
};

#[cfg(test)]
mod tests {
    use super::{CUSTOM_RANDOM_TABLE_BYTES, SOXSchema};

    #[test]
    fn schema_record_layouts_match_generated_minimums() {
        let cases = [
            (SOXSchema::AbilityByJob, Some((24, 64))),
            (SOXSchema::AbilityInfo, Some((64, 64))),
            (SOXSchema::CharInfo, Some((136, 64))),
            (SOXSchema::CustomRandomTable, None),
            (SOXSchema::ItemAttInfo, Some((12, 64))),
            (SOXSchema::ItemTypeInfo, Some((178, 0))),
            (SOXSchema::JobInfo, Some((72, 64))),
            (SOXSchema::LeaderGeneration, Some((72, 64))),
            (SOXSchema::LibraryInfo, Some((6, 64))),
            (SOXSchema::ResistInfo, Some((12, 64))),
            (SOXSchema::SkillInfo, Some((16, 64))),
            (SOXSchema::SkillPointTable, Some((8, 64))),
            (SOXSchema::SpecialNames, Some((4, 64))),
            (SOXSchema::TroopInfo, Some((148, 64))),
            (SOXSchema::UnitUVInfo, Some((36, 64))),
            (SOXSchema::UnitUVID, Some((72, 64))),
            (SOXSchema::WorldmapCharInfo, Some((28, 64))),
            (SOXSchema::WorldmapTroopInfo, Some((28, 64))),
        ];

        for (schema, expected) in cases {
            assert_eq!(schema.record_layout(), expected, "{schema}");
        }
        assert_eq!(CUSTOM_RANDOM_TABLE_BYTES, 1_272);
    }
}
