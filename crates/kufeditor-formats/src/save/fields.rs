macro_rules! save_metadata {
    ($name:ident[$count:expr] { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: [Self; $count] = [$(Self::$variant),+];

            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

macro_rules! grouped_save_metadata {
    (
        $name:ident[$count:expr] -> $group:ident {
            $($variant:ident => ($label:literal, $group_variant:ident)),+ $(,)?
        }
    ) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: [Self; $count] = [$(Self::$variant),+];

            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }

            pub const fn group(self) -> $group {
                match self {
                    $(Self::$variant => $group::$group_variant),+
                }
            }
        }
    };
}

save_metadata! {
    SaveMainField[7] {
        Field00 => "Field 0x00",
        Field04 => "Field 0x04",
        Field08 => "Field 0x08",
        Field0C => "Field 0x0C",
        Field10 => "Field 0x10",
        Field14 => "Field 0x14",
        Field18 => "Field 0x18",
    }
}

grouped_save_metadata! {
    SaveUnitField[21] -> SaveUnitGroup {
        LeaderNameIndex => ("Leader Name Index", Advanced),
        TroopInfoIndex => ("Troop Info Index", Core),
        JobType => ("Job Type", Core),
        ModelID => ("Model ID", Core),
        STGField34 => ("STG Field 0x34", Advanced),
        STGField38 => ("STG Field 0x38", Advanced),
        STGField3C => ("STG Field 0x3C", Advanced),
        STGField40 => ("STG Field 0x40", Advanced),
        CharacterID => ("Character ID", Core),
        TroopInfoIndex2 => ("Troop Info Index 2", Core),
        UCD => ("UCD", Core),
        FormationType => ("Formation Type", Formation),
        GridConfig => ("Grid Config", Formation),
        SkillLevel => ("Skill Level", Core),
        Byte58 => ("Byte 0x58", Core),
        HeroFlag => ("Hero Flag", Core),
        Byte5A => ("Byte 0x5A", Core),
        Field60 => ("Field 0x60", Advanced),
        Field64 => ("Field 0x64", Advanced),
        Field68 => ("Field 0x68", Advanced),
        Field504 => ("Field 0x504", Advanced),
    }
}

save_metadata! {
    SaveUnitGroup[3] {
        Core => "Core",
        Formation => "Formation",
        Advanced => "Advanced",
    }
}

save_metadata! {
    SaveEquipmentSlot[6] {
        LeaderWeapon => "Leader Weapon",
        LeaderAccessory => "Leader Accessory",
        LeaderArmor => "Leader Armor",
        TroopWeapon => "Troop Weapon",
        TroopAccessory => "Troop Accessory",
        TroopArmor => "Troop Armor",
    }
}

grouped_save_metadata! {
    SaveEquipmentField[19] -> SaveEquipmentGroup {
        AutoID => ("Auto ID", Advanced),
        ItemTypeID => ("Item Type ID", Core),
        Level => ("Level", Core),
        EnhancementTier => ("Enhancement Tier", Core),
        VariantIndex => ("Variant Index", Core),
        ItemPower => ("Item Power", Advanced),
        EquippedFlag => ("Equipped Flag", Advanced),
        Reserved => ("Reserved", Advanced),
        Attribute1Index => ("Attribute 1 Index", Core),
        Attribute2Index => ("Attribute 2 Index", Core),
        SkillType1 => ("Skill Type 1", Skills),
        SkillBonus1 => ("Skill Bonus 1", Skills),
        SkillType2 => ("Skill Type 2", Skills),
        SkillBonus2 => ("Skill Bonus 2", Skills),
        ResistType1 => ("Resist Type 1", Resistances),
        ResistBonus1 => ("Resist Bonus 1", Resistances),
        ResistType2 => ("Resist Type 2", Resistances),
        ResistBonus2 => ("Resist Bonus 2", Resistances),
        SlotCategory => ("Slot Category", Advanced),
    }
}

save_metadata! {
    SaveEquipmentGroup[4] {
        Core => "Core",
        Skills => "Skills",
        Resistances => "Resistances",
        Advanced => "Advanced",
    }
}

save_metadata! {
    SaveRosterField[5] {
        Byte60 => "Byte 60",
        Byte61 => "Byte 61",
        Byte62 => "Byte 62",
        Byte63 => "Byte 63",
        Value64 => "Value 64",
    }
}

save_metadata! {
    SaveTextField[3] {
        MapName => "Map Name",
        SetFile => "Set File",
        SkyEffects => "Sky Effects",
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
