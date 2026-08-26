use std::fmt;

use crate::{
    STGFloatTarget, STGNumberTarget, STGRegion, STGScriptTarget, STGTextTarget, SaveNumberTarget,
    SkillField, TextSOXField, TroopField,
};

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
    STGDocument,
    Record {
        record: usize,
        field: DiagnosticField,
    },
    Save(SaveNumberTarget),
    STGNumber(STGNumberTarget),
    STGFloat(STGFloatTarget),
    STGText(STGTextTarget),
    STGScript(STGScriptTarget),
    STGTail {
        region: STGRegion,
        offset: usize,
    },
}

impl DiagnosticLocation {
    pub const fn record(self) -> Option<usize> {
        match self {
            Self::Record { record, .. }
            | Self::Save(
                SaveNumberTarget::Roster { record, .. }
                | SaveNumberTarget::SecondArray { record }
                | SaveNumberTarget::Unit { unit: record, .. }
                | SaveNumberTarget::Equipment { unit: record, .. }
                | SaveNumberTarget::MissionCompletion { slot: record },
            )
            | Self::STGNumber(
                STGNumberTarget::Unit { unit: record, .. }
                | STGNumberTarget::Skill { unit: record, .. }
                | STGNumberTarget::Ability { unit: record, .. }
                | STGNumberTarget::Area { area: record, .. }
                | STGNumberTarget::VariableID { variable: record },
            )
            | Self::STGFloat(
                STGFloatTarget::Unit { unit: record, .. }
                | STGFloatTarget::StatOverride { unit: record, .. }
                | STGFloatTarget::Area { area: record, .. },
            )
            | Self::STGText(
                STGTextTarget::UnitName { unit: record }
                | STGTextTarget::AreaDescription { area: record }
                | STGTextTarget::VariableName { variable: record },
            ) => Some(record),
            Self::STGDocument
            | Self::Save(
                SaveNumberTarget::CampaignIndex
                | SaveNumberTarget::Main(_)
                | SaveNumberTarget::SelectedUnit
                | SaveNumberTarget::CurrentMissionIndex,
            )
            | Self::STGNumber(
                STGNumberTarget::EventBlockHeader { .. }
                | STGNumberTarget::EventID { .. }
                | STGNumberTarget::ParameterInteger { .. }
                | STGNumberTarget::Footer { .. },
            )
            | Self::STGFloat(STGFloatTarget::Parameter { .. })
            | Self::STGText(
                STGTextTarget::Header(_)
                | STGTextTarget::EventDescription { .. }
                | STGTextTarget::ParameterString { .. },
            )
            | Self::STGScript(_)
            | Self::STGTail { .. } => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::STGDocument => "STG Document",
            Self::Record { field, .. } => field.label(),
            Self::Save(target) => target.label(),
            Self::STGNumber(target) => target.label(),
            Self::STGFloat(target) => target.label(),
            Self::STGText(target) => target.label(),
            Self::STGScript(target) => target.kind.label(),
            Self::STGTail { .. } => "Raw STG Tail",
        }
    }

    pub const fn stg_tail(self) -> Option<(STGRegion, usize)> {
        match self {
            Self::STGTail { region, offset } => Some((region, offset)),
            Self::STGDocument
            | Self::Record { .. }
            | Self::Save(_)
            | Self::STGNumber(_)
            | Self::STGFloat(_)
            | Self::STGText(_)
            | Self::STGScript(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: DiagnosticLocation,
    pub message: &'static str,
}
