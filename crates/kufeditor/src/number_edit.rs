#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberCommand {
    Insert(char),
    Backspace,
    Increment,
    Decrement,
    Commit,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberOutcome {
    Continue,
    Commit(i64),
    Cancel,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct NumberEdit {
    draft: String,
    minimum: i64,
    maximum: i64,
    replace_on_input: bool,
    invalid: bool,
}

impl NumberEdit {
    pub fn new(value: i64, minimum: i64, maximum: i64) -> Self {
        assert!(minimum <= maximum, "number edit bounds must be ordered");

        Self {
            draft: value.to_string(),
            minimum,
            maximum,
            replace_on_input: true,
            invalid: false,
        }
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub const fn invalid(&self) -> bool {
        self.invalid
    }

    pub fn apply(&mut self, command: NumberCommand) -> NumberOutcome {
        match command {
            NumberCommand::Insert(character) => self.insert(character),
            NumberCommand::Backspace => {
                if self.replace_on_input {
                    self.draft.clear();
                    self.replace_on_input = false;
                } else {
                    self.draft.pop();
                }
                self.invalid = false;
                NumberOutcome::Continue
            }
            NumberCommand::Increment => self.step(1),
            NumberCommand::Decrement => self.step(-1),
            NumberCommand::Commit => {
                if let Ok(value) = self.draft.parse::<i64>()
                    && self.minimum <= value
                    && value <= self.maximum
                {
                    NumberOutcome::Commit(value)
                } else {
                    self.invalid = true;
                    NumberOutcome::Invalid
                }
            }
            NumberCommand::Cancel => NumberOutcome::Cancel,
        }
    }

    fn insert(&mut self, character: char) -> NumberOutcome {
        let replacing = self.replace_on_input;
        let accepts = character.is_ascii_digit()
            || (character == '-' && (replacing || self.draft.is_empty()));
        if !accepts {
            self.invalid = true;
            return NumberOutcome::Invalid;
        }

        if replacing {
            self.draft.clear();
            self.replace_on_input = false;
        }
        self.draft.push(character);
        self.invalid = false;
        NumberOutcome::Continue
    }

    fn step(&mut self, amount: i64) -> NumberOutcome {
        let Ok(value) = self.draft.parse::<i64>() else {
            self.invalid = true;
            return NumberOutcome::Invalid;
        };
        let value = if amount > 0 {
            value.saturating_add(1)
        } else {
            value.saturating_sub(1)
        }
        .clamp(self.minimum, self.maximum);
        self.draft = value.to_string();
        self.replace_on_input = false;
        self.invalid = false;
        NumberOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::{NumberCommand, NumberEdit, NumberOutcome};

    #[test]
    fn first_typed_character_replaces_the_selected_value() {
        let mut edit = NumberEdit::new(130, i64::from(i32::MIN), i64::from(i32::MAX));
        assert_eq!(
            edit.apply(NumberCommand::Insert('2')),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), "2");
        assert_eq!(
            edit.apply(NumberCommand::Insert('5')),
            NumberOutcome::Continue
        );
        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Commit(25));
    }

    #[test]
    fn troop_i32_extremes_remain_representable() {
        let minimum = i64::from(i32::MIN);
        let maximum = i64::from(i32::MAX);

        let mut edit = NumberEdit::new(minimum, minimum, maximum);
        assert_eq!(edit.draft(), i32::MIN.to_string());
        assert_eq!(
            edit.apply(NumberCommand::Commit),
            NumberOutcome::Commit(minimum)
        );

        let mut edit = NumberEdit::new(maximum, minimum, maximum);
        assert_eq!(edit.draft(), i32::MAX.to_string());
        assert_eq!(
            edit.apply(NumberCommand::Commit),
            NumberOutcome::Commit(maximum)
        );
    }

    #[test]
    fn u32_value_above_i32_maximum_remains_representable() {
        let value = i64::from(u32::MAX);
        let mut edit = NumberEdit::new(value, 0, value);

        assert_eq!(edit.draft(), u32::MAX.to_string());
        assert_eq!(
            edit.apply(NumberCommand::Commit),
            NumberOutcome::Commit(value)
        );
    }

    #[test]
    fn arrows_saturate_at_the_configured_bounds() {
        let mut edit = NumberEdit::new(65_535, 1, 65_535);
        assert_eq!(
            edit.apply(NumberCommand::Increment),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), "65535");

        let mut edit = NumberEdit::new(1, 1, 65_535);
        assert_eq!(
            edit.apply(NumberCommand::Decrement),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), "1");
    }

    #[test]
    fn escape_cancels_without_an_edit() {
        let mut edit = NumberEdit::new(130, i64::from(i32::MIN), i64::from(i32::MAX));
        edit.apply(NumberCommand::Insert('9'));
        assert_eq!(edit.apply(NumberCommand::Cancel), NumberOutcome::Cancel);
    }

    #[test]
    fn backspace_clears_a_still_selected_value() {
        let mut edit = NumberEdit::new(130, i64::from(i32::MIN), i64::from(i32::MAX));

        assert_eq!(
            edit.apply(NumberCommand::Backspace),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), "");
        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Invalid);
        assert!(edit.invalid());
    }

    #[test]
    fn commit_rejects_a_valid_integer_outside_the_configured_bounds() {
        let mut edit = NumberEdit::new(0, 0, 10);
        for character in "11".chars() {
            edit.apply(NumberCommand::Insert(character));
        }

        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Invalid);
        assert!(edit.invalid());
    }

    #[test]
    fn skill_maximum_level_accepts_its_inclusive_bounds() {
        let mut edit = NumberEdit::new(1, 1, 65_535);
        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Commit(1));

        let mut edit = NumberEdit::new(65_535, 1, 65_535);
        assert_eq!(
            edit.apply(NumberCommand::Commit),
            NumberOutcome::Commit(65_535)
        );
    }

    #[test]
    fn skill_maximum_level_rejects_values_outside_its_bounds() {
        let mut edit = NumberEdit::new(1, 1, 65_535);
        edit.apply(NumberCommand::Insert('0'));
        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Invalid);
        assert!(edit.invalid());

        let mut edit = NumberEdit::new(1, 1, 65_535);
        for character in "65536".chars() {
            edit.apply(NumberCommand::Insert(character));
        }
        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Invalid);
        assert!(edit.invalid());
    }
}
