use crate::{
    diagnostic::{Diagnostic, DiagnosticField, DiagnosticLocation, Severity},
    error::FormatError,
    generated::sox_troop_info::{self, File, TroopInfoRecord},
    sox::SOXSource,
};

const SOX_HEADER_SIZE: usize = 8;
const SOX_FOOTER_SIZE: usize = 64;
const TROOP_RECORD_SIZE: usize = 37 * 4;

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
    TypeID => (type_id, "Type ID", Identity),
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
    DefaultUnitHP => (default_unit_hp, "Default Unit HP", Formation),
    FormationRandom => (formation_random, "Formation Randomness", Formation),
    DefaultUnitNumX => (default_unit_num_x, "Units Wide", Formation),
    DefaultUnitNumY => (default_unit_num_y, "Units Deep", Formation),
    UnitHPLevelUp => (unit_hp_level_up, "HP per Level", Leveling),
    LevelUp0SkillID => (level_up_0_skill_id, "Level Skill 1", Leveling),
    LevelUp0Bonus => (level_up_0_bonus, "Level Skill 1 Bonus", Leveling),
    LevelUp1SkillID => (level_up_1_skill_id, "Level Skill 2", Leveling),
    LevelUp1Bonus => (level_up_1_bonus, "Level Skill 2 Bonus", Leveling),
    LevelUp2SkillID => (level_up_2_skill_id, "Level Skill 3", Leveling),
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
    source: SOXSource,
    source_file: File,
    file: File,
    trailing_bytes: Vec<u8>,
}

impl TroopDocument {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, FormatError> {
        Self::from_source(SOXSource::parse(bytes)?)
    }

    pub(crate) fn from_source(source: SOXSource) -> Result<Self, FormatError> {
        let decoded = source.decoded();
        preflight_record_count(decoded)?;
        let mut offset = 0;
        let file = File::parse(decoded, &mut offset).map_err(|source| FormatError::TroopParse {
            offset,
            source: source.into(),
        })?;
        let trailing_bytes = decoded
            .get(offset..)
            .map_or_else(Vec::new, ToOwned::to_owned);

        Ok(Self {
            source,
            source_file: file.clone(),
            file,
            trailing_bytes,
        })
    }

    pub fn record_count(&self) -> usize {
        self.file.records.len()
    }

    pub fn value(&self, record: usize, field: TroopField) -> Result<i32, FormatError> {
        self.record(record, field).map(|record| field.read(record))
    }

    pub fn set_value(
        &mut self,
        record: usize,
        field: TroopField,
        value: i32,
    ) -> Result<i32, FormatError> {
        self.record_mut(record, field)
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
                        location: DiagnosticLocation::Record {
                            record: record_index,
                            field: DiagnosticField::Troop(field),
                        },
                        message: "Resistance is outside the expected 0 to 500 percent range",
                    });
                }
            }

            if record.default_unit_hp <= 0 {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    location: DiagnosticLocation::Record {
                        record: record_index,
                        field: DiagnosticField::Troop(TroopField::DefaultUnitHP),
                    },
                    message: "Default unit HP must be greater than zero",
                });
            }
        }

        diagnostics
    }

    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        if self.file == self.source_file {
            return Ok(self.source.original_bytes());
        }

        let mut bytes = self
            .file
            .to_bytes()
            .map_err(|source| FormatError::TroopEncode(source.into()))?;
        bytes.extend_from_slice(&self.trailing_bytes);
        Ok(self.source.apply_envelope(&bytes))
    }

    pub fn rebase_source(&mut self, saved: &Self, bytes: Vec<u8>) -> Result<(), FormatError> {
        if bytes != saved.encode()? {
            return Err(FormatError::InconsistentSOXRebase);
        }
        self.source.rebase(&saved.source, bytes)?;
        self.source_file = saved.file.clone();
        self.trailing_bytes.clone_from(&saved.trailing_bytes);
        Ok(())
    }

    fn record(&self, index: usize, field: TroopField) -> Result<&TroopInfoRecord, FormatError> {
        self.file
            .records
            .get(index)
            .ok_or(FormatError::RecordOutOfRange {
                record: index,
                record_count: self.file.records.len(),
                field: DiagnosticField::Troop(field),
            })
    }

    fn record_mut(
        &mut self,
        index: usize,
        field: TroopField,
    ) -> Result<&mut TroopInfoRecord, FormatError> {
        let record_count = self.file.records.len();
        self.file
            .records
            .get_mut(index)
            .ok_or(FormatError::RecordOutOfRange {
                record: index,
                record_count,
                field: DiagnosticField::Troop(field),
            })
    }
}

fn preflight_record_count(bytes: &[u8]) -> Result<(), FormatError> {
    let Some(count_bytes) = bytes.get(4..SOX_HEADER_SIZE) else {
        return Ok(());
    };
    let &[first, second, third, fourth] = count_bytes else {
        return Ok(());
    };
    let record_count = u32::from_le_bytes([first, second, third, fourth]);
    let maximum_count = bytes
        .len()
        .saturating_sub(SOX_HEADER_SIZE + SOX_FOOTER_SIZE)
        / TROOP_RECORD_SIZE;

    if u128::from(record_count) <= maximum_count as u128 {
        return Ok(());
    }

    Err(FormatError::TroopParse {
        offset: SOX_HEADER_SIZE,
        source: sox_troop_info::Error::InvalidLength {
            field: "records",
            value: i128::from(record_count),
        }
        .into(),
    })
}
