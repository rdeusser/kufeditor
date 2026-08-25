//! Supported games, installation discovery, paths, and data catalogs.

mod catalog;
mod error;
mod static_translations;

use std::fmt;

pub use catalog::CatalogRole;
pub use error::{CatalogFileError, CatalogIssue, CatalogLoadError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Game {
    #[default]
    Crusaders,
    Heroes,
}

impl Game {
    pub const ALL: [Self; 2] = [Self::Crusaders, Self::Heroes];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Crusaders => "Crusaders",
            Self::Heroes => "Heroes",
        }
    }
}

impl fmt::Display for Game {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::Game;

    #[test]
    fn supported_games_have_stable_labels() {
        assert_eq!(Game::ALL, [Game::Crusaders, Game::Heroes]);
        assert_eq!(Game::Crusaders.label(), "Crusaders");
        assert_eq!(Game::Heroes.label(), "Heroes");
        assert_eq!(Game::default(), Game::Crusaders);
    }
}
