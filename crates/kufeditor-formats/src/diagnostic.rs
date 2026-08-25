use std::fmt;

use crate::{SaveNumberTarget, SkillField, TextSOXField, TroopField};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticField {
    Troop(TroopField),
    Skill(SkillField),
    TextSOX(TextSOXField),
}

impl DiagnosticField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Troop(field) => field.label(),
            Self::Skill(field) => field.label(),
            Self::TextSOX(field) => field.label(),
        }
    }
}

impl fmt::Display for DiagnosticField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticLocation {
    Record {
        record: usize,
        field: DiagnosticField,
    },
    Save(SaveNumberTarget),
}

impl DiagnosticLocation {
    pub const fn record(self) -> Option<usize> {
        match self {
            Self::Record { record, .. }
            | Self::Save(
                SaveNumberTarget::Roster { record, .. } | SaveNumberTarget::SecondArray { record },
            ) => Some(record),
            Self::Save(
                SaveNumberTarget::Unit { unit, .. } | SaveNumberTarget::Equipment { unit, .. },
            ) => Some(unit),
            Self::Save(SaveNumberTarget::MissionCompletion { slot }) => Some(slot),
            Self::Save(
                SaveNumberTarget::CampaignIndex
                | SaveNumberTarget::Main(_)
                | SaveNumberTarget::SelectedUnit
                | SaveNumberTarget::CurrentMissionIndex,
            ) => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Record { field, .. } => field.label(),
            Self::Save(target) => target.label(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: DiagnosticLocation,
    pub message: &'static str,
}
