const I32_BOUNDS: (i64, i64) = (i32::MIN as i64, i32::MAX as i64);
const U32_BOUNDS: (i64, i64) = (0, u32::MAX as i64);
const U8_BOUNDS: (i64, i64) = (0, u8::MAX as i64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGFieldAccess {
    Editable,
    ReadOnly,
}

macro_rules! stg_metadata {
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

macro_rules! grouped_stg_metadata {
    (
        $name:ident[$count:expr] -> $group:ident {
            $($variant:ident => ($label:literal, $group_variant:ident)),+ $(,)?
        }
    ) => {
        stg_metadata! {
            $name[$count] {
                $($variant => $label),+
            }
        }

        impl $name {
            pub const fn group(self) -> $group {
                match self {
                    $(Self::$variant => $group::$group_variant),+
                }
            }
        }
    };
}

stg_metadata! {
    STGHeaderTextField[8] {
        MapFilename => "Map Filename",
        BitmapFilename => "Bitmap Filename",
        DefaultCamera => "Default Camera",
        UserCamera => "User Camera",
        SettingsFile => "Settings File",
        SkyEffects => "Sky Effects",
        AIScript => "AI Script",
        CubemapTexture => "Cubemap Texture",
    }
}

stg_metadata! {
    STGUnitGroup[5] {
        Core => "Core",
        Leader => "Leader",
        Officers => "Officers",
        Formation => "Formation",
        Advanced => "Advanced",
    }
}

grouped_stg_metadata! {
    STGUnitField[28] -> STGUnitGroup {
        UniqueID => ("Unique ID", Core),
        UCD => ("UCD", Core),
        HeroFlag => ("Hero", Core),
        EnabledFlag => ("Enabled", Core),
        Reserved27 => ("Reserved 0x27", Advanced),
        FacingDirection => ("Facing Direction", Formation),
        ExtraFlags1 => ("Extra Flags 1", Advanced),
        ExtraFlags2 => ("Extra Flags 2", Advanced),
        Category => ("Category", Advanced),
        Reserved50 => ("Reserved 0x50", Advanced),
        LeaderJobType => ("Leader Job Type", Leader),
        LeaderModelID => ("Leader Model ID", Leader),
        LeaderWorldmapID => ("Leader Worldmap ID", Leader),
        LeaderLevel => ("Leader Level", Leader),
        OfficerCount => ("Officer Count", Officers),
        Officer1JobType => ("Officer 1 Job Type", Officers),
        Officer1ModelID => ("Officer 1 Model ID", Officers),
        Officer1WorldmapID => ("Officer 1 Worldmap ID", Officers),
        Officer1Level => ("Officer 1 Level", Officers),
        Officer2JobType => ("Officer 2 Job Type", Officers),
        Officer2ModelID => ("Officer 2 Model ID", Officers),
        Officer2WorldmapID => ("Officer 2 Worldmap ID", Officers),
        Officer2Level => ("Officer 2 Level", Officers),
        AnimationConfig => ("Animation Config", Formation),
        GridX => ("Grid X", Formation),
        GridY => ("Grid Y", Formation),
        TroopInfoIndex => ("Troop Info Index", Formation),
        FormationType => ("Formation Type", Formation),
    }
}

stg_metadata! {
    STGUnitFloatField[5] {
        LeaderHPOverride => "Leader HP Override",
        UnitHPOverride => "Unit HP Override",
        Unknown30 => "Unknown 0x30",
        PositionX => "Position X",
        PositionY => "Position Y",
    }
}

stg_metadata! {
    STGSkillOwner[3] {
        Leader => "Leader",
        Officer1 => "Officer 1",
        Officer2 => "Officer 2",
    }
}

stg_metadata! {
    STGAbilityOwner[3] {
        Leader => "Leader",
        Officer1 => "Officer 1",
        Officer2 => "Officer 2",
    }
}

stg_metadata! {
    STGSkillField[2] {
        ID => "Skill ID",
        Level => "Skill Level",
    }
}

stg_metadata! {
    STGAreaField[3] {
        Unknown20 => "Unknown 0x20",
        Unknown24 => "Unknown 0x24",
        AreaID => "Area ID",
    }
}

stg_metadata! {
    STGAreaFloatField[4] {
        BoundX1 => "Bound X1",
        BoundY1 => "Bound Y1",
        BoundX2 => "Bound X2",
        BoundY2 => "Bound Y2",
    }
}

stg_metadata! {
    STGFooterField[2] {
        SlotData1 => "Slot Data 1",
        SlotData2 => "Slot Data 2",
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGScriptKind {
    Condition,
    Action,
}

impl STGScriptKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Condition => "Condition",
            Self::Action => "Action",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGScriptTarget {
    pub block: usize,
    pub event: usize,
    pub kind: STGScriptKind,
    pub script: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGParameterTarget {
    pub script: STGScriptTarget,
    pub parameter: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGValueTarget {
    VariableInitial { variable: usize },
    ScriptParameter(STGParameterTarget),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGNumberTarget {
    Unit {
        unit: usize,
        field: STGUnitField,
    },
    Skill {
        unit: usize,
        owner: STGSkillOwner,
        slot: usize,
        field: STGSkillField,
    },
    Ability {
        unit: usize,
        owner: STGAbilityOwner,
        slot: usize,
    },
    Area {
        area: usize,
        field: STGAreaField,
    },
    VariableID {
        variable: usize,
    },
    EventBlockHeader {
        block: usize,
    },
    EventID {
        block: usize,
        event: usize,
    },
    ParameterInteger {
        value: STGValueTarget,
    },
    Footer {
        entry: usize,
        field: STGFooterField,
    },
}

impl STGNumberTarget {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unit { field, .. } => field.label(),
            Self::Skill { field, .. } => field.label(),
            Self::Ability { .. } => "Ability ID",
            Self::Area { field, .. } => field.label(),
            Self::VariableID { .. } => "Variable ID",
            Self::EventBlockHeader { .. } => "Event Block Header",
            Self::EventID { .. } => "Event ID",
            Self::ParameterInteger { .. } => "Parameter Value",
            Self::Footer { field, .. } => field.label(),
        }
    }

    pub const fn storage_bounds(self) -> (i64, i64) {
        match self {
            Self::Unit { field, .. } => field.storage_bounds(),
            Self::Skill { .. } => U8_BOUNDS,
            Self::Ability { .. } | Self::ParameterInteger { .. } => I32_BOUNDS,
            Self::Area { .. }
            | Self::VariableID { .. }
            | Self::EventBlockHeader { .. }
            | Self::EventID { .. }
            | Self::Footer { .. } => U32_BOUNDS,
        }
    }

    pub const fn access(self) -> STGFieldAccess {
        match self {
            Self::Unit { field, .. } => field.access(),
            Self::Area { field, .. } => field.access(),
            Self::Skill { .. }
            | Self::Ability { .. }
            | Self::VariableID { .. }
            | Self::EventBlockHeader { .. }
            | Self::EventID { .. }
            | Self::ParameterInteger { .. }
            | Self::Footer { .. } => STGFieldAccess::Editable,
        }
    }

    pub const fn editor(self) -> Option<STGEditor> {
        if matches!(self.access(), STGFieldAccess::ReadOnly) {
            return None;
        }

        match self {
            Self::Unit { field, .. } => field.editor(),
            Self::Skill {
                field: STGSkillField::ID,
                ..
            } => Some(STGEditor::Number {
                minimum: 0,
                maximum: 254,
            }),
            _ => Some(STGEditor::number(self.storage_bounds())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGFloatTarget {
    Unit {
        unit: usize,
        field: STGUnitFloatField,
    },
    StatOverride {
        unit: usize,
        slot: usize,
    },
    Area {
        area: usize,
        field: STGAreaFloatField,
    },
    Parameter {
        value: STGValueTarget,
    },
}

impl STGFloatTarget {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unit { field, .. } => field.label(),
            Self::StatOverride { .. } => "Stat Override",
            Self::Area { field, .. } => field.label(),
            Self::Parameter { .. } => "Parameter Value",
        }
    }

    pub const fn access(self) -> STGFieldAccess {
        match self {
            Self::Unit { field, .. } => field.access(),
            Self::StatOverride { .. } | Self::Area { .. } | Self::Parameter { .. } => {
                STGFieldAccess::Editable
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGTextTarget {
    Header(STGHeaderTextField),
    UnitName { unit: usize },
    AreaDescription { area: usize },
    VariableName { variable: usize },
    EventDescription { block: usize, event: usize },
    ParameterString { value: STGValueTarget },
}

impl STGTextTarget {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Header(field) => field.label(),
            Self::UnitName { .. } => "Unit Name",
            Self::AreaDescription { .. } => "Area Description",
            Self::VariableName { .. } => "Variable Name",
            Self::EventDescription { .. } => "Event Description",
            Self::ParameterString { .. } => "Parameter Value",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGChoice {
    pub value: i64,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum STGEditor {
    Number { minimum: i64, maximum: i64 },
    Choice { choices: &'static [STGChoice] },
}

impl STGEditor {
    pub const fn number_bounds(self) -> Option<(i64, i64)> {
        match self {
            Self::Number { minimum, maximum } => Some((minimum, maximum)),
            Self::Choice { .. } => None,
        }
    }

    const fn number((minimum, maximum): (i64, i64)) -> Self {
        Self::Number { minimum, maximum }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct STGFloatValue(u32);

impl STGFloatValue {
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> u32 {
        self.0
    }

    pub fn from_finite(value: f32) -> Option<Self> {
        value.is_finite().then(|| Self(value.to_bits()))
    }

    pub fn finite_value(self) -> Option<f32> {
        let value = f32::from_bits(self.0);
        value.is_finite().then_some(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum STGMutation<T> {
    Unchanged,
    Changed { previous: T },
}

impl<T> STGMutation<T> {
    pub const fn previous(&self) -> Option<&T> {
        match self {
            Self::Unchanged => None,
            Self::Changed { previous } => Some(previous),
        }
    }
}

impl STGUnitField {
    pub const fn access(self) -> STGFieldAccess {
        match self {
            Self::Reserved27
            | Self::ExtraFlags1
            | Self::ExtraFlags2
            | Self::Category
            | Self::Reserved50 => STGFieldAccess::ReadOnly,
            Self::UniqueID
            | Self::UCD
            | Self::HeroFlag
            | Self::EnabledFlag
            | Self::FacingDirection
            | Self::LeaderJobType
            | Self::LeaderModelID
            | Self::LeaderWorldmapID
            | Self::LeaderLevel
            | Self::OfficerCount
            | Self::Officer1JobType
            | Self::Officer1ModelID
            | Self::Officer1WorldmapID
            | Self::Officer1Level
            | Self::Officer2JobType
            | Self::Officer2ModelID
            | Self::Officer2WorldmapID
            | Self::Officer2Level
            | Self::AnimationConfig
            | Self::GridX
            | Self::GridY
            | Self::TroopInfoIndex
            | Self::FormationType => STGFieldAccess::Editable,
        }
    }

    pub const fn storage_bounds(self) -> (i64, i64) {
        match self {
            Self::Reserved50 | Self::TroopInfoIndex => I32_BOUNDS,
            Self::UniqueID
            | Self::OfficerCount
            | Self::AnimationConfig
            | Self::GridX
            | Self::GridY
            | Self::FormationType => U32_BOUNDS,
            Self::UCD
            | Self::HeroFlag
            | Self::EnabledFlag
            | Self::Reserved27
            | Self::FacingDirection
            | Self::ExtraFlags1
            | Self::ExtraFlags2
            | Self::Category
            | Self::LeaderJobType
            | Self::LeaderModelID
            | Self::LeaderWorldmapID
            | Self::LeaderLevel
            | Self::Officer1JobType
            | Self::Officer1ModelID
            | Self::Officer1WorldmapID
            | Self::Officer1Level
            | Self::Officer2JobType
            | Self::Officer2ModelID
            | Self::Officer2WorldmapID
            | Self::Officer2Level => U8_BOUNDS,
        }
    }

    pub const fn editor(self) -> Option<STGEditor> {
        if matches!(self.access(), STGFieldAccess::ReadOnly) {
            return None;
        }

        match self {
            Self::UCD => Some(STGEditor::Choice {
                choices: &UCD_CHOICES,
            }),
            Self::HeroFlag => Some(STGEditor::Choice {
                choices: &BOOLEAN_CHOICES,
            }),
            Self::EnabledFlag => Some(STGEditor::Choice {
                choices: &ENABLED_CHOICES,
            }),
            Self::FacingDirection => Some(STGEditor::Choice {
                choices: &FACING_CHOICES,
            }),
            Self::LeaderLevel | Self::Officer1Level | Self::Officer2Level => {
                Some(STGEditor::Number {
                    minimum: 1,
                    maximum: 99,
                })
            }
            Self::OfficerCount => Some(STGEditor::Number {
                minimum: 0,
                maximum: 2,
            }),
            Self::GridX | Self::GridY => Some(STGEditor::Number {
                minimum: 1,
                maximum: U32_BOUNDS.1,
            }),
            _ => Some(STGEditor::number(self.storage_bounds())),
        }
    }
}

impl STGUnitFloatField {
    pub const fn access(self) -> STGFieldAccess {
        match self {
            Self::Unknown30 => STGFieldAccess::ReadOnly,
            Self::LeaderHPOverride | Self::UnitHPOverride | Self::PositionX | Self::PositionY => {
                STGFieldAccess::Editable
            }
        }
    }
}

impl STGAreaField {
    pub const fn access(self) -> STGFieldAccess {
        match self {
            Self::Unknown20 | Self::Unknown24 => STGFieldAccess::ReadOnly,
            Self::AreaID => STGFieldAccess::Editable,
        }
    }
}

static UCD_CHOICES: [STGChoice; 4] = [
    STGChoice {
        value: 0,
        label: "Player",
    },
    STGChoice {
        value: 1,
        label: "Enemy",
    },
    STGChoice {
        value: 2,
        label: "Ally",
    },
    STGChoice {
        value: 3,
        label: "Neutral",
    },
];

static BOOLEAN_CHOICES: [STGChoice; 2] = [
    STGChoice {
        value: 0,
        label: "No",
    },
    STGChoice {
        value: 1,
        label: "Yes",
    },
];

static ENABLED_CHOICES: [STGChoice; 2] = [
    STGChoice {
        value: 0,
        label: "Disabled",
    },
    STGChoice {
        value: 1,
        label: "Enabled",
    },
];

static FACING_CHOICES: [STGChoice; 8] = [
    STGChoice {
        value: 0,
        label: "East",
    },
    STGChoice {
        value: 1,
        label: "Northeast",
    },
    STGChoice {
        value: 2,
        label: "North",
    },
    STGChoice {
        value: 3,
        label: "Northwest",
    },
    STGChoice {
        value: 4,
        label: "West",
    },
    STGChoice {
        value: 5,
        label: "Southwest",
    },
    STGChoice {
        value: 6,
        label: "South",
    },
    STGChoice {
        value: 7,
        label: "Southeast",
    },
];
