use std::fmt;

use crate::{SkillField, TextSOXField, TroopField};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub record: usize,
    pub field: DiagnosticField,
    pub message: &'static str,
}
