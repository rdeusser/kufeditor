use std::collections::HashSet;

use kufeditor_game::Game;
use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::{ManifestErrorKind, ModError, ModLimits, RelativeGamePath};

const MANIFEST_VERSION: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModTimestamp(String);

impl ModTimestamp {
    pub fn parse(value: &str) -> Result<Self, ModError> {
        let parsed = OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|_| ModError::manifest(ManifestErrorKind::InvalidTimestamp))?;
        let canonical = parsed
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| ModError::manifest(ManifestErrorKind::InvalidTimestamp))?;
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModMetadata {
    name: String,
    version: String,
    author: Option<String>,
    description: Option<String>,
    created: Option<ModTimestamp>,
}

impl ModMetadata {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        author: Option<String>,
        description: Option<String>,
        created: Option<ModTimestamp>,
    ) -> Result<Self, ModError> {
        let name = name.into();
        let version = version.into();
        validate_required(&name, ManifestErrorKind::EmptyName)?;
        validate_required(&version, ManifestErrorKind::EmptyVersion)?;
        validate_optional(author.as_deref(), ManifestErrorKind::EmptyAuthor)?;
        validate_optional(description.as_deref(), ManifestErrorKind::EmptyDescription)?;
        Ok(Self {
            name,
            version,
            author,
            description,
            created,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub const fn created(&self) -> Option<&ModTimestamp> {
        self.created.as_ref()
    }
}

fn validate_required(value: &str, kind: ManifestErrorKind) -> Result<(), ModError> {
    if value.trim().is_empty() {
        Err(ModError::manifest(kind))
    } else {
        Ok(())
    }
}

fn validate_optional(value: Option<&str>, kind: ManifestErrorKind) -> Result<(), ModError> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        Err(ModError::manifest(kind))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModManifest {
    metadata: ModMetadata,
    game: Game,
    files: Vec<RelativeGamePath>,
}

impl ModManifest {
    pub fn new(
        metadata: ModMetadata,
        game: Game,
        mut files: Vec<RelativeGamePath>,
    ) -> Result<Self, ModError> {
        if files.is_empty() {
            return Err(ModError::manifest(ManifestErrorKind::EmptyFiles));
        }
        files.sort_by(|left, right| left.portable_key().cmp(right.portable_key()));
        if files.windows(2).any(
            |pair| matches!(pair, [left, right] if left.portable_key() == right.portable_key()),
        ) {
            return Err(ModError::manifest(ManifestErrorKind::DuplicatePath));
        }
        Ok(Self {
            metadata,
            game,
            files,
        })
    }

    pub fn from_json(source: &[u8], limits: &ModLimits) -> Result<Self, ModError> {
        if u64::try_from(source.len()).unwrap_or(u64::MAX) > limits.max_manifest_bytes {
            return Err(ModError::manifest(ManifestErrorKind::TooLarge));
        }
        let image: ManifestImage = serde_json::from_slice(source)
            .map_err(|_| ModError::manifest(ManifestErrorKind::InvalidJSON))?;
        if image.format_version.unwrap_or(MANIFEST_VERSION) != MANIFEST_VERSION {
            return Err(ModError::manifest(ManifestErrorKind::UnsupportedVersion));
        }
        if u64::try_from(image.files.len()).unwrap_or(u64::MAX) > limits.max_package_files {
            return Err(ModError::manifest(ManifestErrorKind::TooManyFiles));
        }
        let game = parse_game(&image.game)?;
        let created = image
            .created
            .as_deref()
            .map(ModTimestamp::parse)
            .transpose()?;
        let metadata = ModMetadata::new(
            image.name,
            image.version,
            image.author,
            image.description,
            created,
        )?;
        let mut portable_keys = HashSet::with_capacity(image.files.len());
        let mut files = Vec::with_capacity(image.files.len());
        for value in image.files {
            let path = RelativeGamePath::parse(&value, limits)?;
            if !portable_keys.insert(path.portable_key().to_owned()) {
                return Err(ModError::manifest(ManifestErrorKind::DuplicatePath));
            }
            files.push(path);
        }
        Self::new(metadata, game, files)
    }

    pub fn to_json(&self) -> Result<Vec<u8>, ModError> {
        let image = ManifestImageRef {
            format_version: MANIFEST_VERSION,
            name: self.metadata.name(),
            version: self.metadata.version(),
            author: self.metadata.author(),
            description: self.metadata.description(),
            game: game_name(self.game),
            created: self.metadata.created().map(ModTimestamp::as_str),
            files: self.files.iter().map(RelativeGamePath::as_str).collect(),
        };
        let mut encoded = serde_json::to_vec_pretty(&image)
            .map_err(|_| ModError::manifest(ManifestErrorKind::Serialization))?;
        encoded.push(b'\n');
        Ok(encoded)
    }

    pub const fn metadata(&self) -> &ModMetadata {
        &self.metadata
    }

    pub const fn game(&self) -> Game {
        self.game
    }

    pub fn files(&self) -> &[RelativeGamePath] {
        &self.files
    }
}

fn parse_game(value: &str) -> Result<Game, ModError> {
    if value.eq_ignore_ascii_case("crusaders") {
        Ok(Game::Crusaders)
    } else if value.eq_ignore_ascii_case("heroes") {
        Ok(Game::Heroes)
    } else {
        Err(ModError::manifest(ManifestErrorKind::UnknownGame))
    }
}

const fn game_name(game: Game) -> &'static str {
    match game {
        Game::Crusaders => "crusaders",
        Game::Heroes => "heroes",
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestImage {
    #[serde(default, deserialize_with = "deserialize_manifest_version")]
    format_version: Option<u64>,
    name: String,
    version: String,
    author: Option<String>,
    description: Option<String>,
    game: String,
    created: Option<String>,
    files: Vec<String>,
}

fn deserialize_manifest_version<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Some)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestImageRef<'a> {
    format_version: u64,
    name: &'a str,
    version: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    game: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<&'a str>,
    files: Vec<&'a str>,
}
