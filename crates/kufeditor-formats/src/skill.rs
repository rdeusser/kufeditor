use crate::{
    diagnostic::{Diagnostic, DiagnosticField, Severity},
    error::FormatError,
    generated::sox_skill_info::{self, File, SkillInfoRecord},
    sox::SOXSource,
};

const SOX_HEADER_SIZE: usize = 8;
const SOX_FOOTER_SIZE: usize = 64;
const MIN_SKILL_RECORD_SIZE: usize = 4 + 2 + 2 + 4 + 4;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SkillField {
    SkillID,
    LocalizationKey,
    IconPath,
    SkillType,
    MaxLevel,
}

impl SkillField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SkillID => "Skill ID",
            Self::LocalizationKey => "Localization Key",
            Self::IconPath => "Icon Path",
            Self::SkillType => "Skill Type",
            Self::MaxLevel => "Maximum Level",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SkillTextField {
    LocalizationKey,
    IconPath,
}

impl SkillTextField {
    pub const fn label(self) -> &'static str {
        self.as_field().label()
    }

    const fn as_field(self) -> SkillField {
        match self {
            Self::LocalizationKey => SkillField::LocalizationKey,
            Self::IconPath => SkillField::IconPath,
        }
    }

    fn read(self, record: &SkillInfoRecord) -> &[u8] {
        match self {
            Self::LocalizationKey => &record.loc_key.value,
            Self::IconPath => &record.icon.value,
        }
    }

    fn write(self, record: &mut SkillInfoRecord, value: Vec<u8>) {
        match self {
            Self::LocalizationKey => record.loc_key.value = value,
            Self::IconPath => record.icon.value = value,
        }
    }
}

impl std::fmt::Display for SkillTextField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Clone, Debug)]
pub struct SkillDocument {
    source: SOXSource,
    source_file: File,
    file: File,
    trailing_bytes: Vec<u8>,
}

impl SkillDocument {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, FormatError> {
        Self::from_source(SOXSource::parse(bytes)?)
    }

    pub(crate) fn from_source(source: SOXSource) -> Result<Self, FormatError> {
        let decoded = source.decoded();
        preflight_record_count(decoded)?;
        let mut offset = 0;
        let file = File::parse(decoded, &mut offset).map_err(|source| FormatError::SkillParse {
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

    pub fn skill_id(&self, record: usize) -> Result<i32, FormatError> {
        self.record(record, SkillField::SkillID)
            .map(|record| record.skill_id)
    }

    pub fn set_skill_id(&mut self, record: usize, value: i32) -> Result<i32, FormatError> {
        self.record_mut(record, SkillField::SkillID)
            .map(|record| std::mem::replace(&mut record.skill_id, value))
    }

    pub fn skill_type(&self, record: usize) -> Result<u32, FormatError> {
        self.record(record, SkillField::SkillType)
            .map(|record| record.skill_type)
    }

    pub fn set_skill_type(&mut self, record: usize, value: u32) -> Result<u32, FormatError> {
        self.record_mut(record, SkillField::SkillType)
            .map(|record| std::mem::replace(&mut record.skill_type, value))
    }

    pub fn max_level(&self, record: usize) -> Result<u32, FormatError> {
        self.record(record, SkillField::MaxLevel)
            .map(|record| record.max_level)
    }

    pub fn set_max_level(&mut self, record: usize, value: u32) -> Result<u32, FormatError> {
        self.record_mut(record, SkillField::MaxLevel)
            .map(|record| std::mem::replace(&mut record.max_level, value))
    }

    pub fn text(&self, record: usize, field: SkillTextField) -> Result<&str, FormatError> {
        let value = field.read(self.record(record, field.as_field())?);
        std::str::from_utf8(value).map_err(|source| FormatError::SkillUTF8 {
            record,
            field,
            source,
        })
    }

    pub fn set_text(
        &mut self,
        record: usize,
        field: SkillTextField,
        value: String,
    ) -> Result<String, FormatError> {
        let record_value = self.record_mut(record, field.as_field())?;
        let previous = std::str::from_utf8(field.read(record_value))
            .map_err(|source| FormatError::SkillUTF8 {
                record,
                field,
                source,
            })?
            .to_owned();
        field.write(record_value, value.into_bytes());
        Ok(previous)
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (record_index, record) in self.file.records.iter().enumerate() {
            if record.skill_type != 1 && record.skill_type != 2 {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    record: record_index,
                    field: DiagnosticField::Skill(SkillField::SkillType),
                    message: "Skill type should be 1 (Combat) or 2 (Magic)",
                });
            }

            if record.max_level == 0 || record.max_level > 65_535 {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    record: record_index,
                    field: DiagnosticField::Skill(SkillField::MaxLevel),
                    message: "Max level is 0 or exceeds 65535",
                });
            }

            Self::text_diagnostic(
                &mut diagnostics,
                record_index,
                SkillTextField::LocalizationKey,
                &record.loc_key.value,
            );
            Self::text_diagnostic(
                &mut diagnostics,
                record_index,
                SkillTextField::IconPath,
                &record.icon.value,
            );
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
            .map_err(|source| FormatError::SkillEncode(source.into()))?;
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

    fn text_diagnostic(
        diagnostics: &mut Vec<Diagnostic>,
        record: usize,
        field: SkillTextField,
        value: &[u8],
    ) {
        if value.is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                record,
                field: DiagnosticField::Skill(field.as_field()),
                message: match field {
                    SkillTextField::LocalizationKey => "Localization key is empty",
                    SkillTextField::IconPath => "Icon path is empty",
                },
            });
        } else if std::str::from_utf8(value).is_err() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                record,
                field: DiagnosticField::Skill(field.as_field()),
                message: match field {
                    SkillTextField::LocalizationKey => "Localization key is not valid UTF-8",
                    SkillTextField::IconPath => "Icon path is not valid UTF-8",
                },
            });
        }
    }

    fn record(&self, index: usize, field: SkillField) -> Result<&SkillInfoRecord, FormatError> {
        self.file
            .records
            .get(index)
            .ok_or(FormatError::RecordOutOfRange {
                record: index,
                record_count: self.file.records.len(),
                field: DiagnosticField::Skill(field),
            })
    }

    fn record_mut(
        &mut self,
        index: usize,
        field: SkillField,
    ) -> Result<&mut SkillInfoRecord, FormatError> {
        let record_count = self.file.records.len();
        self.file
            .records
            .get_mut(index)
            .ok_or(FormatError::RecordOutOfRange {
                record: index,
                record_count,
                field: DiagnosticField::Skill(field),
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
        / MIN_SKILL_RECORD_SIZE;

    if u128::from(record_count) <= maximum_count as u128 {
        return Ok(());
    }

    Err(FormatError::SkillParse {
        offset: SOX_HEADER_SIZE,
        source: sox_skill_info::Error::InvalidLength {
            field: "records",
            value: i128::from(record_count),
        }
        .into(),
    })
}
