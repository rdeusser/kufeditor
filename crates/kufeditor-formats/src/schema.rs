use std::fmt::{self, Display, Formatter};

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
}

impl Display for SoxSchema {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.file_stem())
    }
}
