use crate::{
    diagnostic::{Diagnostic, Severity},
    error::FormatError,
    generated::sox_troop_info::{File, TroopInfoRecord},
};

macro_rules! troop_fields {
    ($($variant:ident => ($member:ident, $label:literal, $group:ident)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum TroopField {
            $($variant),+
        }

        impl TroopField {
            pub const ALL: [Self; 37] = [$(Self::$variant),+];

            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }

            pub const fn group(self) -> TroopGroup {
                match self {
                    $(Self::$variant => TroopGroup::$group),+
                }
            }

            fn read(self, record: &TroopInfoRecord) -> i32 {
                match self {
                    $(Self::$variant => record.$member),+
                }
            }

            fn write(self, record: &mut TroopInfoRecord, value: i32) -> i32 {
                match self {
                    $(Self::$variant => std::mem::replace(&mut record.$member, value)),+
                }
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TroopGroup {
    Identity,
    Movement,
    Combat,
    Resistances,
    Formation,
    Leveling,
}

impl TroopGroup {
    pub const ALL: [Self; 6] = [
        Self::Identity,
        Self::Movement,
        Self::Combat,
        Self::Resistances,
        Self::Formation,
        Self::Leveling,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Movement => "Movement",
            Self::Combat => "Combat",
            Self::Resistances => "Resistances",
            Self::Formation => "Formation",
            Self::Leveling => "Leveling",
        }
    }
}

troop_fields! {
    Job => (job, "Job", Identity),
    TypeId => (type_id, "Type ID", Identity),
    MoveSpeed => (move_speed, "Move Speed", Movement),
    RotateRate => (rotate_rate, "Rotate Rate", Movement),
    MoveAcceleration => (move_acceleration, "Acceleration", Movement),
    MoveDeceleration => (move_deceleration, "Deceleration", Movement),
    SightRange => (sight_range, "Sight Range", Combat),
    AttackRangeMax => (attack_range_max, "Maximum Attack Range", Combat),
    AttackRangeMin => (attack_range_min, "Minimum Attack Range", Combat),
    AttackFrontRange => (attack_front_range, "Frontal Attack Range", Combat),
    DirectAttack => (direct_attack, "Direct Attack", Combat),
    IndirectAttack => (indirect_attack, "Indirect Attack", Combat),
    Defense => (defense, "Defense", Combat),
    BaseWidth => (base_width, "Base Width", Combat),
    ResistMelee => (resist_melee, "Melee", Resistances),
    ResistRanged => (resist_ranged, "Ranged", Resistances),
    ResistFrontal => (resist_frontal, "Frontal", Resistances),
    ResistExplosion => (resist_explosion, "Explosion", Resistances),
    ResistFire => (resist_fire, "Fire", Resistances),
    ResistIce => (resist_ice, "Ice", Resistances),
    ResistLightning => (resist_lightning, "Lightning", Resistances),
    ResistHoly => (resist_holy, "Holy", Resistances),
    ResistCurse => (resist_curse, "Curse", Resistances),
    ResistEarth => (resist_earth, "Earth", Resistances),
    MaxUnitSpeedMultiplier => (
        max_unit_speed_multiplier,
        "Maximum Unit Speed Multiplier",
        Movement
    ),
    DefaultUnitHp => (default_unit_hp, "Default Unit HP", Formation),
    FormationRandom => (formation_random, "Formation Randomness", Formation),
    DefaultUnitNumX => (default_unit_num_x, "Units Wide", Formation),
    DefaultUnitNumY => (default_unit_num_y, "Units Deep", Formation),
    UnitHpLevelUp => (unit_hp_level_up, "HP per Level", Leveling),
    LevelUp0SkillId => (level_up_0_skill_id, "Level Skill 1", Leveling),
    LevelUp0Bonus => (level_up_0_bonus, "Level Skill 1 Bonus", Leveling),
    LevelUp1SkillId => (level_up_1_skill_id, "Level Skill 2", Leveling),
    LevelUp1Bonus => (level_up_1_bonus, "Level Skill 2 Bonus", Leveling),
    LevelUp2SkillId => (level_up_2_skill_id, "Level Skill 3", Leveling),
    LevelUp2Bonus => (level_up_2_bonus, "Level Skill 3 Bonus", Leveling),
    DamageDistribution => (damage_distribution, "Damage Distribution", Combat),
}

const RESISTANCE_FIELDS: [TroopField; 10] = [
    TroopField::ResistMelee,
    TroopField::ResistRanged,
    TroopField::ResistFrontal,
    TroopField::ResistExplosion,
    TroopField::ResistFire,
    TroopField::ResistIce,
    TroopField::ResistLightning,
    TroopField::ResistHoly,
    TroopField::ResistCurse,
    TroopField::ResistEarth,
];

#[derive(Clone, Debug)]
pub struct TroopDocument {
    source_bytes: Vec<u8>,
    source_file: File,
    file: File,
    trailing_bytes: Vec<u8>,
}

impl TroopDocument {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, FormatError> {
        let mut offset = 0;
        let file = File::parse(&bytes, &mut offset).map_err(|source| FormatError::TroopParse {
            offset,
            source: source.into(),
        })?;
        let trailing_bytes = bytes.get(offset..).map_or_else(Vec::new, ToOwned::to_owned);

        Ok(Self {
            source_bytes: bytes,
            source_file: file.clone(),
            file,
            trailing_bytes,
        })
    }

    pub fn record_count(&self) -> usize {
        self.file.records.len()
    }

    pub fn value(&self, record: usize, field: TroopField) -> Result<i32, FormatError> {
        self.record(record).map(|record| field.read(record))
    }

    pub fn set_value(
        &mut self,
        record: usize,
        field: TroopField,
        value: i32,
    ) -> Result<i32, FormatError> {
        self.record_mut(record)
            .map(|record| field.write(record, value))
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (record_index, record) in self.file.records.iter().enumerate() {
            for field in RESISTANCE_FIELDS {
                let value = field.read(record);
                if value < 0 || (value > 500 && value < 1_000_000) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        record: record_index,
                        field,
                        message: "Resistance is outside the expected 0 to 500 percent range",
                    });
                }
            }

            if record.default_unit_hp <= 0 {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    record: record_index,
                    field: TroopField::DefaultUnitHp,
                    message: "Default unit HP must be greater than zero",
                });
            }
        }

        diagnostics
    }

    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        if self.file == self.source_file {
            return Ok(self.source_bytes.clone());
        }

        let mut bytes = self
            .file
            .to_bytes()
            .map_err(|source| FormatError::TroopEncode(source.into()))?;
        bytes.extend_from_slice(&self.trailing_bytes);
        Ok(bytes)
    }

    pub fn rebase_source(&mut self, saved: &Self, bytes: Vec<u8>) {
        self.source_bytes = bytes;
        self.source_file = saved.file.clone();
        self.trailing_bytes.clone_from(&saved.trailing_bytes);
    }

    fn record(&self, index: usize) -> Result<&TroopInfoRecord, FormatError> {
        self.file
            .records
            .get(index)
            .ok_or(FormatError::RecordOutOfRange {
                record: index,
                record_count: self.file.records.len(),
            })
    }

    fn record_mut(&mut self, index: usize) -> Result<&mut TroopInfoRecord, FormatError> {
        let record_count = self.file.records.len();
        self.file
            .records
            .get_mut(index)
            .ok_or(FormatError::RecordOutOfRange {
                record: index,
                record_count,
            })
    }
}
