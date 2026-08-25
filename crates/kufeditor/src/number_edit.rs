use kufeditor_workspace::{DocumentId, TroopField};

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
    Commit(i32),
    Cancel,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct NumberEdit {
    document: DocumentId,
    record: usize,
    field: TroopField,
    draft: String,
    replace_on_input: bool,
    invalid: bool,
}

impl NumberEdit {
    pub fn new(document: DocumentId, record: usize, field: TroopField, value: i32) -> Self {
        Self {
            document,
            record,
            field,
            draft: value.to_string(),
            replace_on_input: true,
            invalid: false,
        }
    }

    pub const fn document(&self) -> DocumentId {
        self.document
    }

    pub const fn record(&self) -> usize {
        self.record
    }

    pub const fn field(&self) -> TroopField {
        self.field
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
                if let Ok(value) = self.draft.parse::<i32>() {
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

    fn step(&mut self, amount: i32) -> NumberOutcome {
        let Ok(value) = self.draft.parse::<i32>() else {
            self.invalid = true;
            return NumberOutcome::Invalid;
        };
        let value = if amount > 0 {
            value.saturating_add(1)
        } else {
            value.saturating_sub(1)
        };
        self.draft = value.to_string();
        self.replace_on_input = false;
        self.invalid = false;
        NumberOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "synthetic fixtures use known fixed-size byte ranges"
    )]

    use std::path::PathBuf;

    use kufeditor_workspace::{Document, DocumentId, TroopDocument, TroopField, Workspace};

    use super::{NumberCommand, NumberEdit, NumberOutcome};

    fn document_id() -> DocumentId {
        let mut bytes = vec![0_u8; 8 + 148 + 64];
        bytes
            .get_mut(0..4)
            .unwrap()
            .copy_from_slice(&100_u32.to_le_bytes());
        bytes
            .get_mut(4..8)
            .unwrap()
            .copy_from_slice(&1_u32.to_le_bytes());
        bytes
            .get_mut(108..112)
            .unwrap()
            .copy_from_slice(&800_i32.to_le_bytes());
        let document = TroopDocument::parse(bytes).unwrap();
        let mut workspace = Workspace::new();
        workspace.open_loaded(PathBuf::from("TroopInfo.sox"), Document::Troop(document))
    }

    #[test]
    fn first_typed_character_replaces_the_selected_value() {
        let mut edit = NumberEdit::new(document_id(), 0, TroopField::MoveSpeed, 130);
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
    fn arrows_change_the_whole_value_without_overflow() {
        let mut edit = NumberEdit::new(document_id(), 0, TroopField::MoveSpeed, i32::MAX);
        assert_eq!(
            edit.apply(NumberCommand::Increment),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), i32::MAX.to_string());
        assert_eq!(
            edit.apply(NumberCommand::Decrement),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), (i32::MAX - 1).to_string());
    }

    #[test]
    fn escape_cancels_without_an_edit() {
        let mut edit = NumberEdit::new(document_id(), 0, TroopField::MoveSpeed, 130);
        edit.apply(NumberCommand::Insert('9'));
        assert_eq!(edit.apply(NumberCommand::Cancel), NumberOutcome::Cancel);
    }

    #[test]
    fn backspace_clears_a_still_selected_value() {
        let mut edit = NumberEdit::new(document_id(), 0, TroopField::MoveSpeed, 130);

        assert_eq!(
            edit.apply(NumberCommand::Backspace),
            NumberOutcome::Continue
        );
        assert_eq!(edit.draft(), "");
        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Invalid);
        assert!(edit.invalid());
    }

    #[test]
    fn commit_rejects_an_out_of_range_integer() {
        let mut edit = NumberEdit::new(document_id(), 0, TroopField::MoveSpeed, 0);
        for character in "2147483648".chars() {
            edit.apply(NumberCommand::Insert(character));
        }

        assert_eq!(edit.apply(NumberCommand::Commit), NumberOutcome::Invalid);
        assert!(edit.invalid());
    }
}
