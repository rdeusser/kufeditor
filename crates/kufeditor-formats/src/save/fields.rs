#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveMainField {
    Field00,
    Field04,
    Field08,
    Field0C,
    Field10,
    Field14,
    Field18,
}

impl SaveMainField {
    pub const ALL: [Self; 7] = [
        Self::Field00,
        Self::Field04,
        Self::Field08,
        Self::Field0C,
        Self::Field10,
        Self::Field14,
        Self::Field18,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Field00 => "Field 0x00",
            Self::Field04 => "Field 0x04",
            Self::Field08 => "Field 0x08",
            Self::Field0C => "Field 0x0C",
            Self::Field10 => "Field 0x10",
            Self::Field14 => "Field 0x14",
            Self::Field18 => "Field 0x18",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveUnitField {
    LeaderNameIndex,
    TroopInfoIndex,
    JobType,
    ModelID,
    STGField34,
    STGField38,
    STGField3C,
    STGField40,
    CharacterID,
    TroopInfoIndex2,
    UCD,
    FormationType,
    GridConfig,
    SkillLevel,
    Byte58,
    HeroFlag,
    Byte5A,
    Field60,
    Field64,
    Field68,
    Field504,
}

impl SaveUnitField {
    pub const ALL: [Self; 21] = [
        Self::LeaderNameIndex,
        Self::TroopInfoIndex,
        Self::JobType,
        Self::ModelID,
        Self::STGField34,
        Self::STGField38,
        Self::STGField3C,
        Self::STGField40,
        Self::CharacterID,
        Self::TroopInfoIndex2,
        Self::UCD,
        Self::FormationType,
        Self::GridConfig,
        Self::SkillLevel,
        Self::Byte58,
        Self::HeroFlag,
        Self::Byte5A,
        Self::Field60,
        Self::Field64,
        Self::Field68,
        Self::Field504,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::LeaderNameIndex => "Leader Name Index",
            Self::TroopInfoIndex => "Troop Info Index",
            Self::JobType => "Job Type",
            Self::ModelID => "Model ID",
            Self::STGField34 => "STG Field 0x34",
            Self::STGField38 => "STG Field 0x38",
            Self::STGField3C => "STG Field 0x3C",
            Self::STGField40 => "STG Field 0x40",
            Self::CharacterID => "Character ID",
            Self::TroopInfoIndex2 => "Troop Info Index 2",
            Self::UCD => "UCD",
            Self::FormationType => "Formation Type",
            Self::GridConfig => "Grid Config",
            Self::SkillLevel => "Skill Level",
            Self::Byte58 => "Byte 0x58",
            Self::HeroFlag => "Hero Flag",
            Self::Byte5A => "Byte 0x5A",
            Self::Field60 => "Field 0x60",
            Self::Field64 => "Field 0x64",
            Self::Field68 => "Field 0x68",
            Self::Field504 => "Field 0x504",
        }
    }

    pub const fn group(self) -> SaveUnitGroup {
        match self {
            Self::TroopInfoIndex
            | Self::JobType
            | Self::ModelID
            | Self::CharacterID
            | Self::TroopInfoIndex2
            | Self::UCD
            | Self::SkillLevel
            | Self::Byte58
            | Self::HeroFlag
            | Self::Byte5A => SaveUnitGroup::Core,
            Self::FormationType | Self::GridConfig => SaveUnitGroup::Formation,
            Self::LeaderNameIndex
            | Self::STGField34
            | Self::STGField38
            | Self::STGField3C
            | Self::STGField40
            | Self::Field60
            | Self::Field64
            | Self::Field68
            | Self::Field504 => SaveUnitGroup::Advanced,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveUnitGroup {
    Core,
    Formation,
    Advanced,
}

impl SaveUnitGroup {
    pub const ALL: [Self; 3] = [Self::Core, Self::Formation, Self::Advanced];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Formation => "Formation",
            Self::Advanced => "Advanced",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveEquipmentSlot {
    LeaderWeapon,
    LeaderAccessory,
    LeaderArmor,
    TroopWeapon,
    TroopAccessory,
    TroopArmor,
}

impl SaveEquipmentSlot {
    pub const ALL: [Self; 6] = [
        Self::LeaderWeapon,
        Self::LeaderAccessory,
        Self::LeaderArmor,
        Self::TroopWeapon,
        Self::TroopAccessory,
        Self::TroopArmor,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::LeaderWeapon => "Leader Weapon",
            Self::LeaderAccessory => "Leader Accessory",
            Self::LeaderArmor => "Leader Armor",
            Self::TroopWeapon => "Troop Weapon",
            Self::TroopAccessory => "Troop Accessory",
            Self::TroopArmor => "Troop Armor",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveEquipmentField {
    AutoID,
    ItemTypeID,
    Level,
    EnhancementTier,
    VariantIndex,
    ItemPower,
    EquippedFlag,
    Reserved,
    Attribute1Index,
    Attribute2Index,
    SkillType1,
    SkillBonus1,
    SkillType2,
    SkillBonus2,
    ResistType1,
    ResistBonus1,
    ResistType2,
    ResistBonus2,
    SlotCategory,
}

impl SaveEquipmentField {
    pub const ALL: [Self; 19] = [
        Self::AutoID,
        Self::ItemTypeID,
        Self::Level,
        Self::EnhancementTier,
        Self::VariantIndex,
        Self::ItemPower,
        Self::EquippedFlag,
        Self::Reserved,
        Self::Attribute1Index,
        Self::Attribute2Index,
        Self::SkillType1,
        Self::SkillBonus1,
        Self::SkillType2,
        Self::SkillBonus2,
        Self::ResistType1,
        Self::ResistBonus1,
        Self::ResistType2,
        Self::ResistBonus2,
        Self::SlotCategory,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::AutoID => "Auto ID",
            Self::ItemTypeID => "Item Type ID",
            Self::Level => "Level",
            Self::EnhancementTier => "Enhancement Tier",
            Self::VariantIndex => "Variant Index",
            Self::ItemPower => "Item Power",
            Self::EquippedFlag => "Equipped Flag",
            Self::Reserved => "Reserved",
            Self::Attribute1Index => "Attribute 1 Index",
            Self::Attribute2Index => "Attribute 2 Index",
            Self::SkillType1 => "Skill Type 1",
            Self::SkillBonus1 => "Skill Bonus 1",
            Self::SkillType2 => "Skill Type 2",
            Self::SkillBonus2 => "Skill Bonus 2",
            Self::ResistType1 => "Resist Type 1",
            Self::ResistBonus1 => "Resist Bonus 1",
            Self::ResistType2 => "Resist Type 2",
            Self::ResistBonus2 => "Resist Bonus 2",
            Self::SlotCategory => "Slot Category",
        }
    }

    pub const fn group(self) -> SaveEquipmentGroup {
        match self {
            Self::ItemTypeID
            | Self::Level
            | Self::EnhancementTier
            | Self::VariantIndex
            | Self::Attribute1Index
            | Self::Attribute2Index => SaveEquipmentGroup::Core,
            Self::SkillType1 | Self::SkillBonus1 | Self::SkillType2 | Self::SkillBonus2 => {
                SaveEquipmentGroup::Skills
            }
            Self::ResistType1 | Self::ResistBonus1 | Self::ResistType2 | Self::ResistBonus2 => {
                SaveEquipmentGroup::Resistances
            }
            Self::AutoID
            | Self::ItemPower
            | Self::EquippedFlag
            | Self::Reserved
            | Self::SlotCategory => SaveEquipmentGroup::Advanced,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveEquipmentGroup {
    Core,
    Skills,
    Resistances,
    Advanced,
}

impl SaveEquipmentGroup {
    pub const ALL: [Self; 4] = [Self::Core, Self::Skills, Self::Resistances, Self::Advanced];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Skills => "Skills",
            Self::Resistances => "Resistances",
            Self::Advanced => "Advanced",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveRosterField {
    Byte60,
    Byte61,
    Byte62,
    Byte63,
    Value64,
}

impl SaveRosterField {
    pub const ALL: [Self; 5] = [
        Self::Byte60,
        Self::Byte61,
        Self::Byte62,
        Self::Byte63,
        Self::Value64,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Byte60 => "Byte 60",
            Self::Byte61 => "Byte 61",
            Self::Byte62 => "Byte 62",
            Self::Byte63 => "Byte 63",
            Self::Value64 => "Value 64",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveTextField {
    MapName,
    SetFile,
    SkyEffects,
}

impl SaveTextField {
    pub const ALL: [Self; 3] = [Self::MapName, Self::SetFile, Self::SkyEffects];

    pub const fn label(self) -> &'static str {
        match self {
            Self::MapName => "Map Name",
            Self::SetFile => "Set File",
            Self::SkyEffects => "Sky Effects",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SaveNumberTarget {
    CampaignIndex,
    Main(SaveMainField),
    SelectedUnit,
    Unit {
        unit: usize,
        field: SaveUnitField,
    },
    Equipment {
        unit: usize,
        slot: SaveEquipmentSlot,
        field: SaveEquipmentField,
    },
    Roster {
        record: usize,
        field: SaveRosterField,
    },
    MissionCompletion {
        slot: usize,
    },
    CurrentMissionIndex,
    SecondArray {
        record: usize,
    },
}

impl SaveNumberTarget {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CampaignIndex => "Campaign",
            Self::Main(field) => field.label(),
            Self::SelectedUnit => "Selected Unit Reference",
            Self::Unit { field, .. } => field.label(),
            Self::Equipment { field, .. } => field.label(),
            Self::Roster { field, .. } => field.label(),
            Self::MissionCompletion { .. } => "Mission Completion",
            Self::CurrentMissionIndex => "Current Mission Index",
            Self::SecondArray { .. } => "Second Array Value",
        }
    }
}
