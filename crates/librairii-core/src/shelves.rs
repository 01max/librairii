//! Approved UUID membership and explicit curation decisions, independent of live queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ShelfError {
    #[error("shelf.invalid_name")]
    InvalidName,
    #[error("shelf.duplicate_name")]
    DuplicateName,
    #[error("shelf.not_found")]
    NotFound,
    #[error("shelf.invalid_input")]
    InvalidInput,
    #[error("shelf.stale_review")]
    StaleReview,
}
impl ShelfError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidName => "shelf.invalid_name",
            Self::DuplicateName => "shelf.duplicate_name",
            Self::NotFound => "shelf.not_found",
            Self::InvalidInput => "shelf.invalid_input",
            Self::StaleReview => "shelf.stale_review",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shelf {
    pub id: crate::ShelfId,
    pub name: String,
    pub position: i64,
    pub revision: i64,
    pub count: i64,
    pub broken_count: i64,
    pub unavailable_count: i64,
    pub source_count: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub id: i64,
    pub collection_id: Option<crate::CollectionId>,
    pub name: String,
    pub active: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    /// Lossless stored spelling, independently of its parsed domain identity.
    pub uuid: String,
    pub position: i64,
    /// Historical protection is lossless internal retention data, never a setting.
    pub manually_retained: bool,
    pub cached_title: String,
    pub origins: Vec<Source>,
}
pub fn normalize_name(name: &str) -> Result<(String, String), ShelfError> {
    crate::collections::normalize_name(name).map_err(|_| ShelfError::InvalidName)
}
/// Canonical UUID spelling is the persisted authority; numeric story IDs are not.
pub fn unique_uuids(values: &[String]) -> Result<Vec<String>, ShelfError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for value in values {
        let parsed: crate::StoryUuid = value.parse().map_err(|_| ShelfError::InvalidInput)?;
        if parsed.to_string() != *value {
            return Err(ShelfError::InvalidInput);
        }
        if seen.insert(value) {
            out.push(value.clone());
        }
    }
    Ok(out)
}
mod warnings;
pub use warnings::{WarningAcknowledgements, WarningKey};

impl Member {
    pub fn identity(&self) -> Result<crate::StoryUuid, ShelfError> {
        self.uuid.parse().map_err(|_| ShelfError::InvalidInput)
    }
    pub fn reference(&self) -> Result<StoryReference, ShelfError> {
        StoryReference::from_stored(&self.uuid)
    }
}
/// Admission for text-based/native inputs, before constructing typed commands.
pub fn parse_uuids(values: &[String]) -> Result<Vec<crate::StoryUuid>, ShelfError> {
    unique_uuids(values)?
        .iter()
        .map(|u| u.parse().map_err(|_| ShelfError::InvalidInput))
        .collect()
}

/// A parsed UUID plus its exact stored spelling. Existing decisions may use
/// uppercase hex; canonicalizing a command must never retarget another SQL row.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct StoryReference {
    identity: crate::StoryUuid,
    stored: String,
}
impl StoryReference {
    pub fn from_stored(stored: &str) -> Result<Self, ShelfError> {
        if stored.len() != 36 {
            return Err(ShelfError::InvalidInput);
        }
        Ok(Self {
            identity: stored.parse().map_err(|_| ShelfError::InvalidInput)?,
            stored: stored.to_owned(),
        })
    }
    pub fn identity(&self) -> crate::StoryUuid {
        self.identity
    }
    pub fn as_stored(&self) -> &str {
        &self.stored
    }
}
impl From<crate::StoryUuid> for StoryReference {
    fn from(identity: crate::StoryUuid) -> Self {
        Self {
            identity,
            stored: identity.to_string(),
        }
    }
}
