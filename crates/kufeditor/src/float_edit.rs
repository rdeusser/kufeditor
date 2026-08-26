#![allow(
    dead_code,
    reason = "float drafts are consumed by the STG scalar controls built on this foundation"
)]

use kufeditor_workspace::STGFloatValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatCommand {
    Insert(char),
    Backspace,
    Commit,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatOutcome {
    Continue,
    Commit(STGFloatValue),
    Cancel,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct FloatEdit {
    source: STGFloatValue,
    draft: String,
    replace_on_input: bool,
    invalid: bool,
}

impl FloatEdit {
    pub fn new(source: STGFloatValue) -> Self {
        let draft = source
            .finite_value()
            .map_or_else(String::new, finite_float_text);
        Self {
            source,
            replace_on_input: !draft.is_empty(),
            draft,
            invalid: false,
        }
    }

    pub fn replacement(source: STGFloatValue) -> Self {
        Self {
            source,
            draft: String::new(),
            replace_on_input: false,
            invalid: false,
        }
    }

    pub const fn source(&self) -> STGFloatValue {
        self.source
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    pub fn is_valid(&self) -> bool {
        parse_finite(&self.draft).is_some()
    }

    pub fn apply(&mut self, command: FloatCommand) -> FloatOutcome {
        match command {
            FloatCommand::Insert(character) => self.insert(character),
            FloatCommand::Backspace => {
                if self.replace_on_input {
                    self.draft.clear();
                    self.replace_on_input = false;
                } else {
                    self.draft.pop();
                }
                self.invalid = false;
                FloatOutcome::Continue
            }
            FloatCommand::Commit => parse_finite(&self.draft).map_or_else(
                || {
                    self.invalid = true;
                    FloatOutcome::Invalid
                },
                FloatOutcome::Commit,
            ),
            FloatCommand::Cancel => FloatOutcome::Cancel,
        }
    }

    fn insert(&mut self, character: char) -> FloatOutcome {
        if !matches!(character, '0'..='9' | '+' | '-' | '.' | 'e' | 'E') {
            self.invalid = true;
            return FloatOutcome::Invalid;
        }

        if self.replace_on_input {
            self.draft.clear();
            self.replace_on_input = false;
        }
        self.draft.push(character);
        self.invalid = false;
        FloatOutcome::Continue
    }
}

fn parse_finite(value: &str) -> Option<STGFloatValue> {
    value
        .parse::<f32>()
        .ok()
        .and_then(STGFloatValue::from_finite)
}

fn finite_float_text(value: f32) -> String {
    if value == 0.0 {
        if value.is_sign_negative() {
            "-0.0".to_owned()
        } else {
            "0.0".to_owned()
        }
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use kufeditor_workspace::STGFloatValue;

    use super::{FloatCommand, FloatEdit, FloatOutcome};

    #[test]
    fn float_edit_seeds_finite_values_without_changing_source_bits() {
        let source = STGFloatValue::from_bits(12.5_f32.to_bits());
        let edit = FloatEdit::new(source);

        assert_eq!(edit.source(), source);
        assert_eq!(edit.draft(), "12.5");
        assert!(edit.is_valid());
        assert!(!edit.invalid());
    }

    #[test]
    fn float_edit_replacement_starts_empty_and_keeps_nonfinite_source_bits() {
        let source = STGFloatValue::from_bits(0x7fc0_1234);
        let edit = FloatEdit::replacement(source);

        assert_eq!(edit.source(), source);
        assert_eq!(edit.draft(), "");
        assert!(!edit.is_valid());
    }

    #[test]
    fn float_edit_accepts_sign_decimal_and_exponent_input() {
        let source = STGFloatValue::from_bits(42.0_f32.to_bits());
        let mut edit = FloatEdit::new(source);

        for character in "-1.25e+2".chars() {
            assert_eq!(
                edit.apply(FloatCommand::Insert(character)),
                FloatOutcome::Continue
            );
        }

        assert_eq!(edit.draft(), "-1.25e+2");
        assert_eq!(
            edit.apply(FloatCommand::Commit),
            FloatOutcome::Commit(STGFloatValue::from_bits((-125.0_f32).to_bits()))
        );
        assert_eq!(edit.source(), source);
    }

    #[test]
    fn float_edit_backspace_clears_a_selected_seed_then_deletes_one_character() {
        let source = STGFloatValue::from_bits(42.0_f32.to_bits());
        let mut edit = FloatEdit::new(source);

        assert_eq!(edit.apply(FloatCommand::Backspace), FloatOutcome::Continue);
        assert_eq!(edit.draft(), "");
        edit.apply(FloatCommand::Insert('1'));
        edit.apply(FloatCommand::Insert('2'));
        assert_eq!(edit.apply(FloatCommand::Backspace), FloatOutcome::Continue);
        assert_eq!(edit.draft(), "1");
    }

    #[test]
    fn float_edit_cancel_never_replaces_the_source_value() {
        let source = STGFloatValue::from_bits(7.25_f32.to_bits());
        let mut edit = FloatEdit::new(source);
        edit.apply(FloatCommand::Insert('9'));

        assert_eq!(edit.apply(FloatCommand::Cancel), FloatOutcome::Cancel);
        assert_eq!(edit.source(), source);
    }

    #[test]
    fn float_edit_rejects_invalid_nonfinite_and_overflowing_text() {
        for text in [".", "--1", "NaN", "inf", "-infinity", "1e9999"] {
            let source = STGFloatValue::from_bits(1.0_f32.to_bits());
            let mut edit = FloatEdit::replacement(source);
            for character in text.chars() {
                let _ = edit.apply(FloatCommand::Insert(character));
            }

            assert_eq!(edit.apply(FloatCommand::Commit), FloatOutcome::Invalid);
            assert!(edit.invalid(), "{text}");
            assert_eq!(edit.source(), source);
        }
    }

    #[test]
    fn float_edit_rejects_characters_outside_decimal_float_syntax() {
        let source = STGFloatValue::from_bits(1.0_f32.to_bits());
        let mut edit = FloatEdit::replacement(source);

        assert_eq!(edit.apply(FloatCommand::Insert('x')), FloatOutcome::Invalid);
        assert!(edit.invalid());
        assert_eq!(edit.draft(), "");
    }

    #[test]
    fn float_edit_commits_negative_zero_with_its_sign_bit() {
        let source = STGFloatValue::from_bits((-0.0_f32).to_bits());
        let mut seeded = FloatEdit::new(source);
        assert_eq!(seeded.draft(), "-0.0");
        assert_eq!(
            seeded.apply(FloatCommand::Commit),
            FloatOutcome::Commit(source)
        );

        let mut replacement = FloatEdit::replacement(STGFloatValue::from_bits(0));
        for character in "-0.0".chars() {
            replacement.apply(FloatCommand::Insert(character));
        }
        assert_eq!(
            replacement.apply(FloatCommand::Commit),
            FloatOutcome::Commit(source)
        );
    }

    #[test]
    fn float_edit_finite_seed_text_round_trips_representative_bit_patterns() {
        for bits in [
            0_u32,
            1,
            0x007f_ffff,
            0x0080_0000,
            0x3f80_0001,
            0x7f7f_ffff,
            0x8000_0000,
            0xbf80_0001,
            0xff7f_ffff,
        ] {
            let source = STGFloatValue::from_bits(bits);
            let mut edit = FloatEdit::new(source);
            assert_eq!(edit.source(), source);
            assert_eq!(
                edit.apply(FloatCommand::Commit),
                FloatOutcome::Commit(source),
                "0x{bits:08x} was normalized"
            );
        }
    }
}
