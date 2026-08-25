mod fields;

pub use fields::{
    SaveEquipmentField, SaveEquipmentGroup, SaveEquipmentSlot, SaveMainField, SaveNumberTarget,
    SaveRosterField, SaveTextField, SaveUnitField, SaveUnitGroup,
};

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
